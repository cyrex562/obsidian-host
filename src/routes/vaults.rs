use crate::db::Database;
use crate::error::{AppError, AppResult};
use crate::models::{CreateVaultRequest, FileChangeEvent};
use crate::services::SearchIndex;
use crate::watcher::FileWatcher;
use actix_web::{delete, get, post, web, HttpResponse};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::fs;
use tokio::sync::{broadcast, Mutex};

pub struct AppState {
    pub db: Database,
    pub search_index: SearchIndex,
    pub watcher: Arc<Mutex<FileWatcher>>,
    pub event_broadcaster: broadcast::Sender<FileChangeEvent>,
}

#[post("/api/vaults")]
async fn create_vault(
    state: web::Data<AppState>,
    req: web::Json<CreateVaultRequest>,
) -> AppResult<HttpResponse> {
    // Validate path exists
    let path = PathBuf::from(&req.path);
    if !path.exists() {
        if let Err(err) = fs::create_dir_all(&path).await {
            return Err(AppError::InvalidInput(format!(
                "Failed to create vault directory {}: {}",
                req.path, err
            )));
        }
    }

    if !path.is_dir() {
        return Err(AppError::InvalidInput(format!(
            "Path is not a directory: {}",
            req.path
        )));
    }

    // Create vault in database
    let vault = state
        .db
        .create_vault(req.name.clone(), req.path.clone())
        .await?;

    // Start watching the vault
    let mut watcher = state.watcher.lock().await;
    watcher.watch_vault(vault.id.clone(), path.clone())?;
    drop(watcher);

    // Index the vault
    let indexed_count = state.search_index.index_vault(&vault.id, &req.path)?;
    tracing::info!("Indexed {} files in vault {}", indexed_count, vault.id);

    Ok(HttpResponse::Created().json(vault))
}

#[get("/api/vaults")]
async fn list_vaults(state: web::Data<AppState>) -> AppResult<HttpResponse> {
    let vaults = state.db.list_vaults().await?;
    Ok(HttpResponse::Ok().json(vaults))
}

#[get("/api/vaults/{id}")]
async fn get_vault(state: web::Data<AppState>, path: web::Path<String>) -> AppResult<HttpResponse> {
    let vault = state.db.get_vault(&path).await?;
    Ok(HttpResponse::Ok().json(vault))
}

#[delete("/api/vaults/{id}")]
async fn delete_vault(
    state: web::Data<AppState>,
    path: web::Path<String>,
) -> AppResult<HttpResponse> {
    let vault_id = path.into_inner();

    // Stop watching
    let mut watcher = state.watcher.lock().await;
    watcher.unwatch_vault(&vault_id)?;
    drop(watcher);

    // Remove from search index
    state.search_index.remove_vault(&vault_id)?;

    // Delete from database
    state.db.delete_vault(&vault_id).await?;

    Ok(HttpResponse::NoContent().finish())
}

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(create_vault)
        .service(list_vaults)
        .service(get_vault)
        .service(delete_vault);
}
