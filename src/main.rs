mod config;
mod db;
mod error;
mod models;
mod routes;
mod services;
mod watcher;

use crate::config::AppConfig;
use crate::db::Database;
use crate::models::FileChangeEvent;
use crate::routes::AppState;
use crate::services::SearchIndex;
use crate::watcher::FileWatcher;
use actix_files as fs;
use actix_web::{middleware, web, App, HttpServer};
use std::sync::Arc;
use tokio::sync::{broadcast, Mutex};
use tracing::{error, info};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // Initialize logging
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,obsidian_host=debug,actix_web=info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Load configuration
    let config = AppConfig::load().unwrap_or_default();
    info!("Starting Obsidian Host server...");
    info!("Server config: {}:{}", config.server.host, config.server.port);

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
                crate::models::FileChangeType::Created | crate::models::FileChangeType::Modified => {
                    if change_event.path.ends_with(".md") {
                        // Read the file and update index
                        if let Ok(vault) = db_clone.get_vault(&change_event.vault_id).await {
                            if let Ok(content) = crate::services::FileService::read_file(&vault.path, &change_event.path) {
                                let _ = search_index_clone.update_file(
                                    &change_event.vault_id,
                                    &change_event.path,
                                    content.content,
                                );
                            }
                        }
                    }
                }
                crate::models::FileChangeType::Deleted => {
                    let _ = search_index_clone.remove_file(&change_event.vault_id, &change_event.path);
                }
                crate::models::FileChangeType::Renamed { from, to } => {
                    let _ = search_index_clone.remove_file(&change_event.vault_id, from);
                    if to.ends_with(".md") {
                        if let Ok(vault) = db_clone.get_vault(&change_event.vault_id).await {
                            if let Ok(content) = crate::services::FileService::read_file(&vault.path, to) {
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
            .wrap(middleware::Logger::default())
            .wrap(middleware::Compress::default())
            .configure(routes::vaults::configure)
            .configure(routes::files::configure)
            .configure(routes::search::configure)
            .configure(routes::ws::configure)
            .service(fs::Files::new("/", "./frontend/public").index_file("index.html"))
    })
    .bind((server_host.as_str(), server_port))?
    .run()
    .await
}
