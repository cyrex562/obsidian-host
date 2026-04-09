pub mod assets;

pub mod config;
pub mod db;
pub mod error;
pub mod middleware;
pub mod models;
pub mod routes;
pub mod services;
pub mod watcher;

use actix_cors::Cors;
use actix_web::{web, App, HttpServer};
use config::AppConfig;
use db::Database;
use routes::AppState;
use services::{
    EntityTypeRegistry, LabelService, MarkdownParser, ReindexService, RelationTypeRegistry,
    SchemaService, SearchIndex,
};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{broadcast, Mutex};
use tracing::{error, info, warn};
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};
use uuid::Uuid;
use watcher::FileWatcher;
use anyhow::Context as _;

// TLS support — only imported when actually needed at runtime
use rustls::ServerConfig as RustlsServerConfig;
use rustls_pemfile::{certs, private_key};

#[cfg(not(debug_assertions))]
use actix_web::{HttpResponse, Result as WebResult};
#[cfg(not(debug_assertions))]
use assets::Assets;
#[cfg(debug_assertions)]
use actix_files as fs;
#[cfg(not(debug_assertions))]
use mime_guess::from_path;

#[cfg(not(debug_assertions))]
async fn serve_embedded_file(path: web::Path<String>) -> WebResult<HttpResponse> {
    let path_str = path.into_inner();
    let file_path = if path_str.is_empty() {
        "index.html"
    } else {
        &path_str
    };
    match Assets::get(file_path) {
        Some(content) => {
            let mime_type = from_path(file_path).first_or_octet_stream();
            Ok(HttpResponse::Ok()
                .content_type(mime_type.as_ref())
                .body(content.data.into_owned()))
        }
        None => Ok(HttpResponse::NotFound().body("404 Not Found")),
    }
}

#[cfg(not(debug_assertions))]
async fn serve_embedded_index() -> WebResult<HttpResponse> {
    serve_embedded_file(web::Path::from("index.html".to_string())).await
}

#[cfg(not(debug_assertions))]
fn configure_static(cfg: &mut web::ServiceConfig) {
    cfg.route("/", web::get().to(serve_embedded_index))
        .route("/{filename:.*}", web::get().to(serve_embedded_file));
}

#[cfg(debug_assertions)]
fn configure_static(cfg: &mut web::ServiceConfig) {
    cfg.service(fs::Files::new("/", "./target/frontend").index_file("index.html"));
}

/// Start the Codex HTTP server with the given configuration.
///
/// Sets up logging, initialises the database, file watcher, search index,
/// and plugin registries, then runs the Actix-web server until a shutdown
/// signal is received.
///
/// This function is callable from both the standalone binary (`main.rs`)
/// and from a future Tauri shell (which will run it on a background thread
/// with its own `actix_web::rt::System`).
pub async fn run(config: AppConfig) -> anyhow::Result<()> {
    // --- Logging -----------------------------------------------------------
    let log_dir = std::path::Path::new("./logs");
    std::fs::create_dir_all(log_dir)?;

    let file_appender = tracing_appender::rolling::daily(log_dir, "codex.log");
    let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);

    let use_json = std::env::var("LOG_FORMAT")
        .map(|v| v.to_lowercase() == "json")
        .unwrap_or(false);

    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| {
        "warn,codex=info,actix_web=info,actix_server=info".into()
    });

    // try_init instead of init so the function is safe to call multiple times
    // (e.g. from tests or when Tauri sets up its own subscriber first).
    let _ = if use_json {
        tracing_subscriber::registry()
            .with(env_filter)
            .with(
                fmt::layer()
                    .json()
                    .with_writer(non_blocking)
                    .with_target(true)
                    .with_thread_ids(true)
                    .with_file(true)
                    .with_line_number(true),
            )
            .with(fmt::layer().with_writer(std::io::stdout).with_ansi(true))
            .try_init()
    } else {
        tracing_subscriber::registry()
            .with(env_filter)
            .with(
                fmt::layer()
                    .with_writer(non_blocking)
                    .with_target(true)
                    .with_thread_ids(false)
                    .with_file(false)
                    .with_line_number(false),
            )
            .with(fmt::layer().with_writer(std::io::stdout).with_ansi(true))
            .try_init()
    };

    info!(
        "Logging initialized (format: {})",
        if use_json { "JSON" } else { "text" }
    );

    // --- Config validation -------------------------------------------------
    let mut config = config;

    info!("Starting Codex server...");
    info!(
        "Server config: {}:{}",
        config.server.host, config.server.port
    );

    if config.auth.jwt_secret.trim().is_empty() {
        let generated_secret = format!("{}{}", Uuid::new_v4(), Uuid::new_v4()).replace('-', "");
        config.auth.jwt_secret = generated_secret;
        warn!(
            "auth.jwt_secret is empty; generated an ephemeral runtime secret. \
             Set [auth].jwt_secret in config.toml for persistent tokens across restarts."
        );
    } else if config.auth.jwt_secret.trim() == config::DEFAULT_DEV_JWT_SECRET {
        warn!(
            "Using insecure default JWT secret. \
             Set [auth].jwt_secret in config.toml before production use."
        );
    }

    // --- Database ----------------------------------------------------------
    let db_url = format!("sqlite:{}", config.database.path);
    let db = Database::new(&db_url)
        .await
        .expect("Failed to initialize database");
    info!("Database initialized at {}", config.database.path);

    match db
        .bootstrap_admin_if_empty(
            config.auth.bootstrap_admin_username.as_deref(),
            config.auth.bootstrap_admin_password.as_deref(),
        )
        .await
    {
        Ok(true) => {
            info!(
                "No users were found. Bootstrapped admin user '{}' from config.toml",
                config
                    .auth
                    .bootstrap_admin_username
                    .as_deref()
                    .unwrap_or("<unknown>")
            );
        }
        Ok(false) => {
            info!("User bootstrap skipped (existing users found)");
        }
        Err(e) => {
            warn!(
                "User bootstrap skipped: {}. Configure [auth] bootstrap_admin_username/\
                 bootstrap_admin_password in config.toml to create the first admin.",
                e
            );
        }
    }

    // Seed core labels (idempotent — safe to run on every startup)
    if let Err(e) = LabelService::seed_core_labels(&db).await {
        warn!("Failed to seed core labels: {e}");
    } else {
        info!("Core labels seeded");
    }

    // --- Search & watcher --------------------------------------------------
    let search_index = SearchIndex::new();
    info!("Search index initialized");

    let (watcher, mut change_rx) = FileWatcher::new().expect("Failed to create file watcher");
    let watcher = Arc::new(Mutex::new(watcher));
    info!("File watcher initialized");

    // --- Event loop --------------------------------------------------------
    let (event_tx, _) = broadcast::channel::<models::FileChangeEvent>(100);
    let event_tx_clone = event_tx.clone();
    let (ws_tx, _) = broadcast::channel::<models::WsMessage>(64);

    let search_index_clone = search_index.clone();
    let db_clone = db.clone();
    tokio::spawn(async move {
        while let Some(change_event) = change_rx.recv().await {
            info!("File change detected: {:?}", change_event);

            match &change_event.event_type {
                models::FileChangeType::Created | models::FileChangeType::Modified => {
                    if change_event.path.ends_with(".md") {
                        if let Ok(vault) = db_clone.get_vault(&change_event.vault_id).await {
                            if let Ok(content) = services::FileService::read_file(
                                &vault.path,
                                &change_event.path,
                            ) {
                                let _ = search_index_clone.update_file(
                                    &change_event.vault_id,
                                    &change_event.path,
                                    content.content,
                                );
                            }
                            let abs_path = format!(
                                "{}/{}",
                                vault.path.trim_end_matches('/'),
                                change_event.path
                            );
                            if let Err(e) = ReindexService::index_file(
                                &db_clone,
                                &change_event.vault_id,
                                &change_event.path,
                                &abs_path,
                            )
                            .await
                            {
                                warn!(
                                    "Entity index_file failed for {}: {e}",
                                    change_event.path
                                );
                            }
                        }
                    }
                }
                models::FileChangeType::Deleted => {
                    let _ = search_index_clone
                        .remove_file(&change_event.vault_id, &change_event.path);
                    if let Err(e) = ReindexService::remove_file(
                        &db_clone,
                        &change_event.vault_id,
                        &change_event.path,
                    )
                    .await
                    {
                        warn!(
                            "Entity remove_file failed for {}: {e}",
                            change_event.path
                        );
                    }
                }
                models::FileChangeType::Renamed { from, to } => {
                    let _ = search_index_clone.remove_file(&change_event.vault_id, from);
                    if let Err(e) =
                        ReindexService::remove_file(&db_clone, &change_event.vault_id, from).await
                    {
                        warn!("Entity remove_file (rename from) failed for {from}: {e}");
                    }
                    if to.ends_with(".md") {
                        if let Ok(vault) = db_clone.get_vault(&change_event.vault_id).await {
                            if let Ok(content) =
                                services::FileService::read_file(&vault.path, to)
                            {
                                let _ = search_index_clone.update_file(
                                    &change_event.vault_id,
                                    to,
                                    content.content,
                                );
                            }
                            let abs_path = format!(
                                "{}/{}",
                                vault.path.trim_end_matches('/'),
                                to
                            );
                            if let Err(e) = ReindexService::index_file(
                                &db_clone,
                                &change_event.vault_id,
                                to,
                                &abs_path,
                            )
                            .await
                            {
                                warn!(
                                    "Entity index_file (rename to) failed for {to}: {e}"
                                );
                            }
                        }
                    }
                }
            }

            if let Err(e) = event_tx_clone.send(change_event) {
                error!("Failed to broadcast event: {}", e);
            }
        }
    });

    // --- Vault loading -----------------------------------------------------
    let vaults = db.list_vaults().await.expect("Failed to list vaults");
    for vault in vaults {
        info!("Loading vault: {} at {}", vault.name, vault.path);

        if !std::path::Path::new(&vault.path).exists() {
            warn!(
                "Removing vault {} because path is missing: {}",
                vault.id, vault.path
            );
            if let Err(e) = search_index.remove_vault(&vault.id) {
                error!(
                    "Failed to remove vault {} from search index: {}",
                    vault.id, e
                );
            }
            if let Err(e) = db.delete_vault(&vault.id).await {
                error!("Failed to delete missing vault {} from DB: {}", vault.id, e);
            }
            continue;
        }

        let mut w = watcher.lock().await;
        if let Err(e) = w.watch_vault(vault.id.clone(), vault.path.clone().into()) {
            error!("Failed to watch vault {}: {}", vault.id, e);
        }
        drop(w);

        match search_index.index_vault(&vault.id, &vault.path) {
            Ok(count) => info!("Indexed {} files in vault {}", count, vault.name),
            Err(e) => error!("Failed to index vault {}: {}", vault.id, e),
        }

        let db_reindex = db.clone();
        let vid = vault.id.clone();
        let vpath = vault.path.clone();
        tokio::spawn(async move {
            if let Err(e) = ReindexService::reindex_vault(&db_reindex, &vid, &vpath).await {
                error!("Entity reindex failed for vault {vid}: {e}");
            }
        });
    }

    // --- Plugin schemas ----------------------------------------------------
    let (shutdown_tx, _) = broadcast::channel::<()>(1);

    let entity_type_registry = EntityTypeRegistry::new();
    let relation_type_registry = RelationTypeRegistry::new();
    {
        use services::PluginService;
        let mut plugin_svc = PluginService::new("./plugins");
        match plugin_svc.discover_plugins() {
            Ok(plugins) => {
                if let Err(e) = SchemaService::load_plugin_schemas(
                    &db,
                    &plugins,
                    &entity_type_registry,
                    &relation_type_registry,
                )
                .await
                {
                    warn!("Schema loading error: {e}");
                }
            }
            Err(e) => {
                warn!("Plugin discovery failed during schema load: {e}");
            }
        }
    }
    info!("Plugin schemas loaded");

    // --- HTTP server -------------------------------------------------------
    let app_state = web::Data::new(AppState {
        db,
        search_index,
        watcher,
        event_broadcaster: event_tx,
        ws_broadcaster: ws_tx,
        change_log_retention_days: config.sync.change_log_retention_days,
        ml_undo_store: Arc::new(Mutex::new(HashMap::new())),
        entity_type_registry,
        relation_type_registry,
        plugins_dir: "./plugins".to_string(),
        shutdown_tx: shutdown_tx.clone(),
        document_parser: Arc::new(MarkdownParser),
    });
    let app_config = web::Data::new(config.clone());

    let server_host = config.server.host.clone();
    let server_port = config.server.port;
    let cors_allowed_origins = config.cors.allowed_origins.clone();
    let tls_config = config.tls.clone();

    let http_server = HttpServer::new(move || {
        let mut cors = Cors::default()
            .allow_any_header()
            .allow_any_method()
            .max_age(3600);

        if cors_allowed_origins.is_empty() {
            cors = cors.allow_any_origin();
        } else {
            for origin in &cors_allowed_origins {
                cors = cors.allowed_origin(origin);
            }
        }

        App::new()
            .app_data(app_state.clone())
            .app_data(app_config.clone())
            .wrap(cors)
            .wrap(middleware::RequestLogging)
            .wrap(middleware::RequestIdMiddleware)
            .wrap(middleware::RateLimitMiddleware)
            .wrap(middleware::AuthMiddleware)
            .wrap(actix_web::middleware::Compress::default())
            .configure(routes::health::configure)
            .configure(routes::version::configure)
            .configure(routes::auth::configure)
            .configure(routes::admin::configure)
            .configure(routes::groups::configure)
            .configure(routes::vaults::configure)
            .configure(routes::files::configure)
            .configure(routes::search::configure)
            .configure(routes::ml::configure)
            .configure(routes::ws::configure)
            .configure(routes::markdown::configure)
            .configure(routes::preferences::configure)
            .configure(routes::entities::configure)
            .configure(routes::plugins::configure)
            .configure(configure_static)
            .configure(routes::bookmarks::configure)
            .configure(routes::tags::configure)
            .configure(routes::api_keys::configure)
            .configure(routes::totp::configure)
            .configure(routes::invitations::configure)
            .configure(routes::oidc::configure)
    })
    .shutdown_timeout(10);

    // Bind with TLS if cert_file + key_file are both configured; otherwise plain HTTP.
    let server = match (tls_config.cert_file.as_deref(), tls_config.key_file.as_deref()) {
        (Some(cert_path), Some(key_path)) => {
            info!("TLS enabled — loading certificate from {cert_path}");
            let cert_bytes = std::fs::read(cert_path)
                .with_context(|| format!("Failed to read TLS certificate: {cert_path}"))?;
            let key_bytes = std::fs::read(key_path)
                .with_context(|| format!("Failed to read TLS private key: {key_path}"))?;

            let cert_chain: Vec<rustls::pki_types::CertificateDer<'static>> =
                certs(&mut std::io::BufReader::new(cert_bytes.as_slice()))
                    .collect::<Result<Vec<_>, _>>()
                    .with_context(|| "Failed to parse TLS certificate chain")?;

            let private_key = private_key(&mut std::io::BufReader::new(key_bytes.as_slice()))
                .with_context(|| "Failed to parse TLS private key")?
                .ok_or_else(|| anyhow::anyhow!("No private key found in {key_path}"))?;

            let rustls_cfg = RustlsServerConfig::builder()
                .with_no_client_auth()
                .with_single_cert(cert_chain, private_key)
                .with_context(|| "Failed to build rustls server config")?;

            info!("Starting HTTPS server on {}:{}", server_host, server_port);
            http_server
                .bind_rustls_0_23((server_host.as_str(), server_port), rustls_cfg)?
                .run()
        }
        (None, None) => {
            info!("Starting HTTP server on {}:{}", server_host, server_port);
            http_server.bind((server_host.as_str(), server_port))?.run()
        }
        _ => {
            anyhow::bail!(
                "TLS configuration error: both `tls.cert_file` and `tls.key_file` must be set (or neither)"
            );
        }
    };

    let server_handle = server.handle();

    // Spawn signal listener → graceful shutdown.
    tokio::spawn(async move {
        wait_for_shutdown_signal().await;
        info!("Shutdown signal received — notifying WebSocket clients and draining requests");
        let _ = shutdown_tx.send(());
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        server_handle.stop(true).await;
    });

    server.await?;
    Ok(())
}

/// Waits for SIGTERM (Unix) or Ctrl+C (all platforms).
async fn wait_for_shutdown_signal() {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{signal, SignalKind};
        let mut sigterm = signal(SignalKind::terminate()).expect("failed to install SIGTERM handler");
        tokio::select! {
            _ = sigterm.recv() => {}
            _ = tokio::signal::ctrl_c() => {}
        }
    }
    #[cfg(not(unix))]
    {
        let _ = tokio::signal::ctrl_c().await;
    }
}
