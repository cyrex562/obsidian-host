#[cfg(debug_assertions)]
use actix_files as fs;
use actix_web::{middleware, web, App, HttpServer};
#[cfg(not(debug_assertions))]
use actix_web::{HttpResponse, Result};
#[cfg(not(debug_assertions))]
use mime_guess::from_path;
#[cfg(not(debug_assertions))]
use obsidian_host::assets::Assets;
use obsidian_host::config::AppConfig;
use obsidian_host::db::Database;
use obsidian_host::models::FileChangeEvent;
use obsidian_host::routes::AppState;
use obsidian_host::services::SearchIndex;
use obsidian_host::watcher::FileWatcher;
use std::sync::Arc;
use tokio::sync::{broadcast, Mutex};
use tracing::{error, info, warn};
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

#[cfg(not(debug_assertions))]
async fn serve_embedded_file(path: web::Path<String>) -> Result<HttpResponse> {
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
async fn serve_embedded_index() -> Result<HttpResponse> {
    serve_embedded_file(web::Path::from("index.html".to_string())).await
}

#[cfg(not(debug_assertions))]
fn configure_static(cfg: &mut web::ServiceConfig) {
    cfg.route("/", web::get().to(serve_embedded_index))
        .route("/{filename:.*}", web::get().to(serve_embedded_file));
}

#[cfg(debug_assertions)]
fn configure_static(cfg: &mut web::ServiceConfig) {
    cfg.service(fs::Files::new("/", "./frontend/public").index_file("index.html"));
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // Initialize logging with file rotation
    let log_dir = std::path::Path::new("./logs");
    std::fs::create_dir_all(log_dir)?;

    // Create a file appender with daily rotation
    let file_appender = tracing_appender::rolling::daily(log_dir, "obsidian-host.log");
    let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);

    // Determine log format from environment
    let use_json = std::env::var("LOG_FORMAT")
        .map(|v| v.to_lowercase() == "json")
        .unwrap_or(false);

    // Configure log level from environment or use default
    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| {
        // Default log levels:
        // - info for obsidian_host
        // - debug for obsidian_host modules
        // - info for actix_web
        // - warn for everything else
        "warn,obsidian_host=info,actix_web=info,actix_server=info".into()
    });

    if use_json {
        // JSON format for production/structured logging
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
            .init();
    } else {
        // Human-readable format for development
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
            .init();
    }

    info!(
        "Logging initialized (format: {})",
        if use_json { "JSON" } else { "text" }
    );

    // Load configuration
    let config = AppConfig::load().unwrap_or_default();
    info!("Starting Obsidian Host server...");
    info!(
        "Server config: {}:{}",
        config.server.host, config.server.port
    );

    // Initialize database
    let db_url = format!("sqlite:{}", config.database.path);
    let db = Database::new(&db_url)
        .await
        .expect("Failed to initialize database");
    info!("Database initialized at {}", config.database.path);

    // Initialize search index
    let search_index = SearchIndex::new();
    info!("Search index initialized");

    // Initialize file watcher
    let (watcher, mut change_rx) = FileWatcher::new().expect("Failed to create file watcher");
    let watcher = Arc::new(Mutex::new(watcher));
    info!("File watcher initialized");

    // Create broadcast channel for file change events
    let (event_tx, _) = broadcast::channel::<FileChangeEvent>(100);
    let event_tx_clone = event_tx.clone();

    // Spawn task to forward file changes to broadcast channel
    let search_index_clone = search_index.clone();
    let db_clone = db.clone();
    tokio::spawn(async move {
        while let Some(change_event) = change_rx.recv().await {
            info!("File change detected: {:?}", change_event);

            // Update search index based on change type
            match &change_event.event_type {
                obsidian_host::models::FileChangeType::Created
                | obsidian_host::models::FileChangeType::Modified => {
                    if change_event.path.ends_with(".md") {
                        // Read the file and update index
                        if let Ok(vault) = db_clone.get_vault(&change_event.vault_id).await {
                            if let Ok(content) = obsidian_host::services::FileService::read_file(
                                &vault.path,
                                &change_event.path,
                            ) {
                                let _ = search_index_clone.update_file(
                                    &change_event.vault_id,
                                    &change_event.path,
                                    content.content,
                                );
                            }
                        }
                    }
                }
                obsidian_host::models::FileChangeType::Deleted => {
                    let _ =
                        search_index_clone.remove_file(&change_event.vault_id, &change_event.path);
                }
                obsidian_host::models::FileChangeType::Renamed { from, to } => {
                    let _ = search_index_clone.remove_file(&change_event.vault_id, from);
                    if to.ends_with(".md") {
                        if let Ok(vault) = db_clone.get_vault(&change_event.vault_id).await {
                            if let Ok(content) =
                                obsidian_host::services::FileService::read_file(&vault.path, to)
                            {
                                let _ = search_index_clone.update_file(
                                    &change_event.vault_id,
                                    to,
                                    content.content,
                                );
                            }
                        }
                    }
                }
            }

            // Broadcast to websocket clients
            if let Err(e) = event_tx_clone.send(change_event) {
                error!("Failed to broadcast event: {}", e);
            }
        }
    });

    // Load existing vaults and start watching
    let vaults = db.list_vaults().await.expect("Failed to list vaults");
    for vault in vaults {
        info!("Loading vault: {} at {}", vault.name, vault.path);

        // Remove records for missing vault paths to keep the DB clean
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

        // Start watching
        let mut w = watcher.lock().await;
        if let Err(e) = w.watch_vault(vault.id.clone(), vault.path.clone().into()) {
            error!("Failed to watch vault {}: {}", vault.id, e);
        }
        drop(w);

        // Index vault
        match search_index.index_vault(&vault.id, &vault.path) {
            Ok(count) => info!("Indexed {} files in vault {}", count, vault.name),
            Err(e) => error!("Failed to index vault {}: {}", vault.id, e),
        }
    }

    // Create app state
    let app_state = web::Data::new(AppState {
        db,
        search_index,
        watcher,
        event_broadcaster: event_tx,
    });

    let server_host = config.server.host.clone();
    let server_port = config.server.port;

    // Start HTTP server
    info!("Starting HTTP server on {}:{}", server_host, server_port);

    HttpServer::new(move || {
        App::new()
            .app_data(app_state.clone())
            .wrap(obsidian_host::middleware::RequestLogging)
            .wrap(middleware::Compress::default())
            .configure(obsidian_host::routes::vaults::configure)
            .configure(obsidian_host::routes::files::configure)
            .configure(obsidian_host::routes::search::configure)
            .configure(obsidian_host::routes::ws::configure)
            .configure(obsidian_host::routes::markdown::configure)
            .configure(obsidian_host::routes::preferences::configure)
            .configure(obsidian_host::routes::plugins::configure)
            .configure(configure_static)
    })
    .bind((server_host.as_str(), server_port))?
    .run()
    .await
}
