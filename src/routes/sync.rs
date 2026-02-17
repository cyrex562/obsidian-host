use crate::error::{AppError, AppResult};
use crate::middleware::auth::AuthenticatedUser;
use crate::routes::AppState;
use crate::services::vault_service::VaultService;
use actix_multipart::Multipart;
use actix_web::{web, HttpResponse};
use futures::{StreamExt, TryStreamExt};
use std::io::Write;
use tempfile::NamedTempFile;

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::resource("/{vault_id}/upload")
            .route(web::post().to(upload_zip))
    );
}

/// Upload a vault ZIP file
async fn upload_zip(
    path: web::Path<String>, // vault_id
    mut payload: Multipart,
    data: web::Data<AppState>,
    user: AuthenticatedUser,
) -> AppResult<HttpResponse> {
    let vault_id = path.into_inner();

    // Stream the file to a temporary file
    let mut temp_file = NamedTempFile::new().map_err(|e| AppError::InternalError(format!("Failed to create temp file: {}", e)))?;

    while let Ok(Some(mut field)) = payload.try_next().await {
        // field.content_disposition() returns reference (not option?) or Option?
        // Compiler said it is Option. So let's handle it.
        if let Some(content_disposition) = field.content_disposition() {
            if let Some(name) = content_disposition.get_name() {
                 if name == "file" {
                    while let Some(chunk) = field.next().await {
                        let data = chunk.map_err(|e| AppError::InvalidInput(e.to_string()))?;
                        temp_file.write_all(&data).map_err(|e| AppError::InternalError(format!("Failed to write temp file: {}", e)))?;
                    }
                 }
            }
        }
    }
    
    // Seek to start
    let file = temp_file.reopen().map_err(|e| AppError::InternalError(format!("Failed to reopen temp file: {}", e)))?;

    // Determine target directory
    let vault_path = data.config.vault.root_dir.join(&vault_id);
    
    // Extract
    // Run blocking task
    let target_path = vault_path.clone();
    web::block(move || -> AppResult<()> {
        VaultService::extract_zip(file, &target_path)
    })
    .await
    .map_err(|e| AppError::InternalError(format!("Blocking task failed: {}", e)))?
    .map_err(|e| e)?;

    // Re-index the vault
    let search_index = data.search_index.clone();
    let vault_path_str = vault_path.to_string_lossy().to_string();
    let vault_id_clone = vault_id.clone();
    
    web::block(move || -> AppResult<usize> {
        search_index.index_vault(&vault_id_clone, &vault_path_str)
    })
    .await
    .map_err(|e| AppError::InternalError(format!("Indexing failed: {}", e)))?
    .map_err(|e| e)?;


    Ok(HttpResponse::Ok().json(serde_json::json!({
        "status": "success",
        "message": "Vault uploaded and indexed",
        "vault_id": vault_id
    })))
}
