use crate::config::AppConfig;
use crate::db::Database;
use crate::error::{AppError, AppResult};
use crate::middleware::AuthenticatedUser;
use crate::models::{
    CreateVaultRequest, FileChangeEvent, MlUndoReceipt, ShareVaultWithGroupRequest,
    ShareVaultWithUserRequest,
};
use crate::services::SearchIndex;
use crate::watcher::FileWatcher;
use actix_web::{delete, get, post, web, HttpMessage, HttpRequest, HttpResponse};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::fs;
use tokio::sync::{broadcast, Mutex};

pub struct AppState {
    pub db: Database,
    pub search_index: SearchIndex,
    pub storage: Arc<dyn crate::services::StorageBackend>,
    pub watcher: Arc<Mutex<FileWatcher>>,
    pub event_broadcaster: broadcast::Sender<FileChangeEvent>,
    pub change_log_retention_days: u64,
    pub ml_undo_store: Arc<Mutex<HashMap<String, MlUndoReceipt>>>,
}

fn require_authenticated_user(req: &HttpRequest) -> AppResult<AuthenticatedUser> {
    req.extensions()
        .get::<AuthenticatedUser>()
        .cloned()
        .ok_or_else(|| AppError::Unauthorized("Authentication required".to_string()))
}

fn sanitize_vault_name(name: &str) -> String {
    let mut value = name
        .trim()
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>();

    while value.contains("--") {
        value = value.replace("--", "-");
    }

    value.trim_matches('-').to_string()
}

fn resolve_vault_path(config: &AppConfig, body: &CreateVaultRequest) -> AppResult<String> {
    if let Some(path) = body
        .path
        .as_deref()
        .map(str::trim)
        .filter(|p| !p.is_empty())
    {
        return Ok(path.to_string());
    }

    let slug = sanitize_vault_name(&body.name);
    if slug.is_empty() {
        return Err(AppError::InvalidInput(
            "Vault name must contain letters or numbers".to_string(),
        ));
    }

    let base_dir = PathBuf::from(config.vault.base_dir.trim());
    Ok(base_dir.join(slug).to_string_lossy().to_string())
}

#[post("/api/vaults")]
async fn create_vault(
    state: web::Data<AppState>,
    config: web::Data<AppConfig>,
    http_req: HttpRequest,
    body: web::Json<CreateVaultRequest>,
) -> AppResult<HttpResponse> {
    let owner_user = if config.auth.enabled {
        Some(require_authenticated_user(&http_req)?)
    } else {
        None
    };

    let resolved_path = resolve_vault_path(&config, &body)?;

    // Validate path exists
    let path = PathBuf::from(&resolved_path);
    if !path.exists() {
        if let Err(err) = fs::create_dir_all(&path).await {
            return Err(AppError::InvalidInput(format!(
                "Failed to create vault directory {}: {}",
                resolved_path, err
            )));
        }
    }

    if !Path::new(&path).is_dir() {
        return Err(AppError::InvalidInput(format!(
            "Path is not a directory: {}",
            resolved_path
        )));
    }

    // Create vault in database
    let vault = state
        .db
        .create_vault_for_owner(
            body.name.clone(),
            resolved_path.clone(),
            owner_user.as_ref().map(|user| user.user_id.as_str()),
        )
        .await?;

    // Start watching the vault
    let mut watcher = state.watcher.lock().await;
    watcher.watch_vault(vault.id.clone(), path.clone())?;
    drop(watcher);

    // Index the vault
    let indexed_count = state.search_index.index_vault(&vault.id, &resolved_path)?;
    tracing::info!("Indexed {} files in vault {}", indexed_count, vault.id);

    Ok(HttpResponse::Created().json(vault))
}

#[get("/api/vaults")]
async fn list_vaults(
    state: web::Data<AppState>,
    config: web::Data<AppConfig>,
    req: HttpRequest,
) -> AppResult<HttpResponse> {
    let vaults = if config.auth.enabled {
        let user = require_authenticated_user(&req)?;
        state.db.list_vaults_for_user(&user.user_id).await?
    } else {
        state.db.list_vaults().await?
    };
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

#[get("/api/vaults/{id}/shares")]
async fn list_vault_shares(
    state: web::Data<AppState>,
    path: web::Path<String>,
) -> AppResult<HttpResponse> {
    let vault_id = path.into_inner();
    let shares = state.db.list_vault_shares(&vault_id).await?;
    Ok(HttpResponse::Ok().json(shares))
}

#[post("/api/vaults/{id}/shares/users")]
async fn share_vault_with_user(
    state: web::Data<AppState>,
    path: web::Path<String>,
    body: web::Json<ShareVaultWithUserRequest>,
) -> AppResult<HttpResponse> {
    let vault_id = path.into_inner();
    let target_user_id = if let Some(user_id) = &body.user_id {
        user_id.clone()
    } else if let Some(username) = &body.username {
        state
            .db
            .get_user_by_username(username)
            .await?
            .map(|(id, _)| id)
            .ok_or_else(|| AppError::NotFound(format!("User {} not found", username)))?
    } else {
        return Err(AppError::InvalidInput(
            "Provide either user_id or username".to_string(),
        ));
    };

    state
        .db
        .share_vault_with_user(&vault_id, &target_user_id, &body.role)
        .await?;
    let shares = state.db.list_vault_shares(&vault_id).await?;
    Ok(HttpResponse::Ok().json(shares))
}

#[post("/api/vaults/{id}/shares/groups")]
async fn share_vault_with_group(
    state: web::Data<AppState>,
    path: web::Path<String>,
    body: web::Json<ShareVaultWithGroupRequest>,
) -> AppResult<HttpResponse> {
    let vault_id = path.into_inner();
    state
        .db
        .share_vault_with_group(&vault_id, &body.group_id, &body.role)
        .await?;
    let shares = state.db.list_vault_shares(&vault_id).await?;
    Ok(HttpResponse::Ok().json(shares))
}

#[delete("/api/vaults/{id}/shares/users/{user_id}")]
async fn revoke_vault_user_share(
    state: web::Data<AppState>,
    path: web::Path<(String, String)>,
) -> AppResult<HttpResponse> {
    let (vault_id, user_id) = path.into_inner();
    state
        .db
        .revoke_vault_user_share(&vault_id, &user_id)
        .await?;
    let shares = state.db.list_vault_shares(&vault_id).await?;
    Ok(HttpResponse::Ok().json(shares))
}

#[delete("/api/vaults/{id}/shares/groups/{group_id}")]
async fn revoke_vault_group_share(
    state: web::Data<AppState>,
    path: web::Path<(String, String)>,
) -> AppResult<HttpResponse> {
    let (vault_id, group_id) = path.into_inner();
    state
        .db
        .revoke_vault_group_share(&vault_id, &group_id)
        .await?;
    let shares = state.db.list_vault_shares(&vault_id).await?;
    Ok(HttpResponse::Ok().json(shares))
}

/// Transfer vault ownership to another user.
#[post("/api/vaults/{vault_id}/transfer-ownership")]
async fn transfer_vault_ownership(
    state: web::Data<AppState>,
    req: actix_web::HttpRequest,
    path: web::Path<String>,
    body: web::Json<TransferOwnershipRequest>,
) -> crate::error::AppResult<HttpResponse> {
    let user = req
        .extensions()
        .get::<crate::middleware::AuthenticatedUser>()
        .cloned()
        .ok_or_else(|| {
            crate::error::AppError::Unauthorized("Authentication required".to_string())
        })?;
    let vault_id = path.into_inner();

    // Verify caller is the current owner.
    let role = state
        .db
        .get_vault_role_for_user(&vault_id, &user.user_id)
        .await?;
    if role != Some(crate::models::VaultRole::Owner) {
        return Err(crate::error::AppError::Forbidden(
            "Only the vault owner can transfer ownership".to_string(),
        ));
    }

    state
        .db
        .transfer_vault_ownership(&vault_id, &body.new_owner_user_id)
        .await?;

    let _ = state
        .db
        .write_audit_log(
            Some(&user.user_id),
            Some(&user.username),
            "vault_ownership_transferred",
            Some(&format!(
                "Transferred vault {vault_id} to user {}",
                body.new_owner_user_id
            )),
            None,
            true,
        )
        .await;

    Ok(HttpResponse::Ok().json(serde_json::json!({ "success": true })))
}

#[derive(Debug, serde::Deserialize)]
struct TransferOwnershipRequest {
    new_owner_user_id: String,
}

/// Toggle vault visibility between 'public' and 'private'.
#[post("/api/vaults/{vault_id}/visibility")]
async fn set_vault_visibility(
    state: web::Data<AppState>,
    req: actix_web::HttpRequest,
    path: web::Path<String>,
    body: web::Json<SetVisibilityRequest>,
) -> crate::error::AppResult<HttpResponse> {
    let user = req
        .extensions()
        .get::<crate::middleware::AuthenticatedUser>()
        .cloned()
        .ok_or_else(|| {
            crate::error::AppError::Unauthorized("Authentication required".to_string())
        })?;
    let vault_id = path.into_inner();

    let visibility = body.visibility.trim().to_ascii_lowercase();
    if visibility != "public" && visibility != "private" {
        return Err(crate::error::AppError::InvalidInput(
            "Visibility must be 'public' or 'private'".to_string(),
        ));
    }

    // Only owner can change visibility.
    let role = state
        .db
        .get_vault_role_for_user(&vault_id, &user.user_id)
        .await?;
    if role != Some(crate::models::VaultRole::Owner) {
        return Err(crate::error::AppError::Forbidden(
            "Only the vault owner can change visibility".to_string(),
        ));
    }

    state
        .db
        .set_vault_visibility(&vault_id, &visibility)
        .await?;

    Ok(HttpResponse::Ok().json(serde_json::json!({ "visibility": visibility })))
}

#[derive(Debug, serde::Deserialize)]
struct SetVisibilityRequest {
    visibility: String,
}

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(create_vault)
        .service(list_vaults)
        .service(get_vault)
        .service(delete_vault)
        .service(list_vault_shares)
        .service(share_vault_with_user)
        .service(share_vault_with_group)
        .service(revoke_vault_user_share)
        .service(revoke_vault_group_share)
        .service(transfer_vault_ownership)
        .service(set_vault_visibility);
}
