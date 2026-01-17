use crate::db::Database;
use crate::error::AppResult;
use crate::models::{CreateFileRequest, UpdateFileRequest};
use crate::routes::vaults::AppState;
use crate::services::FileService;
use actix_web::{delete, get, post, put, web, HttpResponse};
use std::path::Path;

#[get("/api/vaults/{vault_id}/files")]
async fn get_file_tree(
    state: web::Data<AppState>,
    path: web::Path<String>,
) -> AppResult<HttpResponse> {
    let vault_id = path.into_inner();
    let vault = state.db.get_vault(&vault_id).await?;

    let tree = FileService::get_file_tree(&vault.path)?;
    Ok(HttpResponse::Ok().json(tree))
}

#[get("/api/vaults/{vault_id}/files/{file_path:.*}")]
async fn read_file(
    state: web::Data<AppState>,
    path: web::Path<(String, String)>,
) -> AppResult<HttpResponse> {
    let (vault_id, file_path) = path.into_inner();
    let vault = state.db.get_vault(&vault_id).await?;

    let content = FileService::read_file(&vault.path, &file_path)?;
    Ok(HttpResponse::Ok().json(content))
}

#[get("/api/vaults/{vault_id}/raw/{file_path:.*}")]
async fn serve_raw_file(
    state: web::Data<AppState>,
    path: web::Path<(String, String)>,
) -> AppResult<HttpResponse> {
    let (vault_id, file_path) = path.into_inner();
    let vault = state.db.get_vault(&vault_id).await?;

    let raw_content = FileService::read_raw_file(&vault.path, &file_path)?;

    // Determine MIME type based on file extension
    let mime_type = get_mime_type(&file_path);

    Ok(HttpResponse::Ok()
        .content_type(mime_type)
        .body(raw_content))
}

fn get_mime_type(file_path: &str) -> &'static str {
    let path = Path::new(file_path);
    match path.extension().and_then(|s| s.to_str()) {
        Some("png") => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("svg") => "image/svg+xml",
        Some("webp") => "image/webp",
        Some("pdf") => "application/pdf",
        Some("mp3") => "audio/mpeg",
        Some("wav") => "audio/wav",
        Some("ogg") => "audio/ogg",
        Some("mp4") => "video/mp4",
        Some("webm") => "video/webm",
        Some("js") => "text/javascript",
        Some("json") => "application/json",
        Some("css") => "text/css",
        Some("html") => "text/html",
        Some("txt") => "text/plain",
        Some("md") => "text/markdown",
        _ => "application/octet-stream",
    }
}

#[post("/api/vaults/{vault_id}/files")]
async fn create_file(
    state: web::Data<AppState>,
    vault_id: web::Path<String>,
    req: web::Json<CreateFileRequest>,
) -> AppResult<HttpResponse> {
    let vault_id = vault_id.into_inner();
    let vault = state.db.get_vault(&vault_id).await?;

    let content = FileService::create_file(
        &vault.path,
        &req.path,
        req.content.as_deref(),
    )?;

    // Update search index if it's a markdown file
    if req.path.ends_with(".md") {
        state.search_index.update_file(
            &vault_id,
            &req.path,
            content.content.clone(),
        )?;
    }

    Ok(HttpResponse::Created().json(content))
}

#[put("/api/vaults/{vault_id}/files/{file_path:.*}")]
async fn update_file(
    state: web::Data<AppState>,
    path: web::Path<(String, String)>,
    req: web::Json<UpdateFileRequest>,
) -> AppResult<HttpResponse> {
    let (vault_id, file_path) = path.into_inner();
    let vault = state.db.get_vault(&vault_id).await?;

    let content = FileService::write_file(
        &vault.path,
        &file_path,
        &req.content,
        req.last_modified,
    )?;

    // Update search index if it's a markdown file
    if file_path.ends_with(".md") {
        state.search_index.update_file(
            &vault_id,
            &file_path,
            content.content.clone(),
        )?;
    }

    Ok(HttpResponse::Ok().json(content))
}

#[delete("/api/vaults/{vault_id}/files/{file_path:.*}")]
async fn delete_file(
    state: web::Data<AppState>,
    path: web::Path<(String, String)>,
) -> AppResult<HttpResponse> {
    let (vault_id, file_path) = path.into_inner();
    let vault = state.db.get_vault(&vault_id).await?;

    FileService::delete_file(&vault.path, &file_path)?;

    // Remove from search index
    state.search_index.remove_file(&vault_id, &file_path)?;

    Ok(HttpResponse::NoContent().finish())
}

#[post("/api/vaults/{vault_id}/directories")]
async fn create_directory(
    state: web::Data<AppState>,
    vault_id: web::Path<String>,
    req: web::Json<CreateFileRequest>,
) -> AppResult<HttpResponse> {
    let vault_id = vault_id.into_inner();
    let vault = state.db.get_vault(&vault_id).await?;

    FileService::create_directory(&vault.path, &req.path)?;

    Ok(HttpResponse::Created().json(serde_json::json!({
        "path": req.path,
    })))
}

#[post("/api/vaults/{vault_id}/rename")]
async fn rename_file(
    state: web::Data<AppState>,
    vault_id: web::Path<String>,
    req: web::Json<serde_json::Value>,
) -> AppResult<HttpResponse> {
    let vault_id = vault_id.into_inner();
    let vault = state.db.get_vault(&vault_id).await?;

    let from = req["from"].as_str()
        .ok_or(crate::error::AppError::InvalidInput("Missing 'from' field".to_string()))?;
    let to = req["to"].as_str()
        .ok_or(crate::error::AppError::InvalidInput("Missing 'to' field".to_string()))?;

    FileService::rename(&vault.path, from, to)?;

    // Update search index if it's a markdown file
    if from.ends_with(".md") {
        state.search_index.remove_file(&vault_id, from)?;
    }
    if to.ends_with(".md") {
        if let Ok(content) = FileService::read_file(&vault.path, to) {
            state.search_index.update_file(&vault_id, to, content.content)?;
        }
    }

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "from": from,
        "to": to,
    })))
}

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(get_file_tree)
        .service(read_file)
        .service(serve_raw_file)
        .service(create_file)
        .service(update_file)
        .service(delete_file)
        .service(create_directory)
        .service(rename_file);
}
