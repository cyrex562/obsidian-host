use crate::error::{AppError, AppResult};
use crate::models::{
    CreateFileRequest, CreateUploadSessionRequest, UpdateFileRequest, UploadSessionResponse,
};
use crate::routes::vaults::AppState;
use crate::services::{FileService, ImageService, WikiLinkResolver};
use actix_multipart::Multipart;
use actix_web::http::header::{ETAG, IF_NONE_MATCH};
use actix_web::{delete, get, post, put, web, HttpRequest, HttpResponse};
use chrono::Utc;
use flate2::write::GzEncoder;
use flate2::Compression;
use futures::{StreamExt, TryStreamExt};
use std::io::{Cursor, Write};
use std::path::Path;
use std::time::UNIX_EPOCH;
use uuid::Uuid;
use walkdir::WalkDir;
use zip::write::SimpleFileOptions;
use zip::ZipWriter;

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
    req: HttpRequest,
    path: web::Path<(String, String)>,
) -> AppResult<HttpResponse> {
    let (vault_id, file_path) = path.into_inner();
    let vault = state.db.get_vault(&vault_id).await?;

    let content = FileService::read_file(&vault.path, &file_path)?;
    let etag = build_file_etag(&content);

    if if_none_match_matches(req.headers().get(IF_NONE_MATCH), &etag) {
        return Ok(HttpResponse::NotModified()
            .insert_header((ETAG, etag))
            .finish());
    }

    Ok(HttpResponse::Ok().insert_header((ETAG, etag)).json(content))
}

fn build_file_etag(content: &crate::models::FileContent) -> String {
    format!("\"{:x}\"", content.modified.timestamp_millis())
}

fn if_none_match_matches(
    if_none_match: Option<&actix_web::http::header::HeaderValue>,
    etag: &str,
) -> bool {
    let Some(header_value) = if_none_match else {
        return false;
    };

    let Ok(header_str) = header_value.to_str() else {
        return false;
    };

    let normalized_current = normalize_etag(etag);

    header_str.split(',').any(|candidate| {
        let candidate = candidate.trim();
        candidate == "*" || normalize_etag(candidate) == normalized_current
    })
}

fn normalize_etag(value: &str) -> String {
    let trimmed = value.trim();
    let without_weak = trimmed
        .strip_prefix("W/")
        .or_else(|| trimmed.strip_prefix("w/"))
        .unwrap_or(trimmed)
        .trim();

    without_weak.trim_matches('"').to_string()
}

#[get("/api/vaults/{vault_id}/raw/{file_path:.*}")]
async fn serve_raw_file(
    state: web::Data<AppState>,
    path: web::Path<(String, String)>,
) -> AppResult<HttpResponse> {
    let (vault_id, file_path) = path.into_inner();
    let vault = state.db.get_vault(&vault_id).await?;

    let raw_content = state.storage.read_raw(&vault.path, &file_path)?;

    // Determine MIME type based on file extension
    let mime_type = get_mime_type(&file_path);

    Ok(HttpResponse::Ok().content_type(mime_type).body(raw_content))
}

#[derive(serde::Deserialize)]
struct ThumbnailQuery {
    width: Option<u32>,
    height: Option<u32>,
}

#[get("/api/vaults/{vault_id}/thumbnail/{file_path:.*}")]
async fn get_thumbnail(
    state: web::Data<AppState>,
    path: web::Path<(String, String)>,
    query: web::Query<ThumbnailQuery>,
) -> AppResult<HttpResponse> {
    let (vault_id, file_path) = path.into_inner();
    let vault = state.db.get_vault(&vault_id).await?;

    // Resolving path to ensure it's within vault
    let full_path = FileService::resolve_path(&vault.path, &file_path)?;

    // Width/Height defaults to 200 if not specified
    let width = query.width.unwrap_or(200);
    let height = query.height.unwrap_or(200);

    // Generate thumbnail (returns PNG bytes)
    // Note: This operation is CPU intensive and blocking, so we should run it in blocking thread
    // but for simplicity in this file we can run it here or wrap it.
    // Ideally: web::block(move || ImageService::generate_thumbnail(...)).await
    // Since ImageReader does IO, let's just run it. Using web::block is better for actix.

    // For now, running synchronously (in async context) might block the worker thread.
    // Given the constraints and existing code style, I'll allow it or use web::block if easily possible.
    // Let's use web::block to be safe.

    let thumbnail_data =
        web::block(move || ImageService::generate_thumbnail(&full_path, width, height))
            .await
            .map_err(|e| {
                crate::error::AppError::InternalError(format!(
                    "Thumbnail generation task canceled: {}",
                    e
                ))
            })??;

    Ok(HttpResponse::Ok()
        .content_type("image/png")
        // Cache for 1 hour
        .insert_header(("Cache-Control", "public, max-age=3600"))
        .body(thumbnail_data))
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

#[derive(serde::Deserialize)]
struct FileChangesQuery {
    since: Option<i64>,
}

#[derive(serde::Deserialize)]
struct SyncFileRequest {
    path: String,
    client_etag: Option<String>,
    client_mtime: Option<i64>,
}

#[derive(serde::Deserialize)]
struct SyncRequest {
    files: Vec<SyncFileRequest>,
}

#[derive(serde::Serialize)]
struct SyncResponse {
    stale: Vec<String>,
    deleted: Vec<String>,
    server_newer: Vec<String>,
}

#[derive(serde::Serialize)]
struct FileMetadataResponse {
    path: String,
    size: u64,
    mtime: i64,
    etag: String,
    frontmatter_keys: Vec<String>,
}

#[get("/api/vaults/{vault_id}/changes")]
async fn get_file_changes(
    state: web::Data<AppState>,
    vault_id: web::Path<String>,
    query: web::Query<FileChangesQuery>,
) -> AppResult<HttpResponse> {
    let vault_id = vault_id.into_inner();
    state.db.get_vault(&vault_id).await?;

    let since = query.since.unwrap_or(0).max(0);
    let events = state.db.get_file_changes_since(&vault_id, since).await?;

    Ok(HttpResponse::Ok().json(events))
}

#[post("/api/vaults/{vault_id}/sync")]
async fn sync_files(
    state: web::Data<AppState>,
    vault_id: web::Path<String>,
    req: web::Json<SyncRequest>,
) -> AppResult<HttpResponse> {
    let vault_id = vault_id.into_inner();
    let vault = state.db.get_vault(&vault_id).await?;

    let mut stale = Vec::new();
    let mut deleted = Vec::new();
    let mut server_newer = Vec::new();

    for entry in &req.files {
        let full_path = match FileService::resolve_path(&vault.path, &entry.path) {
            Ok(path) => path,
            Err(_) => {
                deleted.push(entry.path.clone());
                continue;
            }
        };

        if !full_path.exists() || !full_path.is_file() {
            deleted.push(entry.path.clone());
            continue;
        }

        let metadata = std::fs::metadata(&full_path)?;
        let server_mtime = metadata
            .modified()
            .ok()
            .and_then(|m| m.duration_since(UNIX_EPOCH).ok())
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0);
        let server_etag = format!("\"{:x}\"", server_mtime);

        if let Some(client_etag) = &entry.client_etag {
            if normalize_etag(client_etag) != normalize_etag(&server_etag) {
                stale.push(entry.path.clone());
            }
            continue;
        }

        if let Some(client_mtime) = entry.client_mtime {
            if server_mtime > client_mtime {
                server_newer.push(entry.path.clone());
            }
        }
    }

    Ok(HttpResponse::Ok().json(SyncResponse {
        stale,
        deleted,
        server_newer,
    }))
}

#[get("/api/vaults/{vault_id}/files/{file_path:.*}/metadata")]
async fn get_file_metadata(
    state: web::Data<AppState>,
    path: web::Path<(String, String)>,
) -> AppResult<HttpResponse> {
    let (vault_id, file_path) = path.into_inner();
    let vault = state.db.get_vault(&vault_id).await?;

    let full_path = FileService::resolve_path(&vault.path, &file_path)?;

    if !full_path.exists() {
        return Err(AppError::NotFound(format!("File not found: {}", file_path)));
    }

    if full_path.is_dir() {
        return Err(AppError::InvalidInput(
            "Cannot get metadata for a directory".to_string(),
        ));
    }

    let metadata = std::fs::metadata(&full_path)?;
    let mtime = metadata
        .modified()
        .ok()
        .and_then(|m| m.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0);
    let etag = format!("\"{:x}\"", mtime);

    let mut frontmatter_keys = if file_path.ends_with(".md") {
        std::fs::read_to_string(&full_path)
            .ok()
            .and_then(|raw| crate::services::frontmatter_service::parse_frontmatter(&raw).ok())
            .and_then(|(frontmatter, _)| frontmatter)
            .and_then(|fm| fm.as_object().cloned())
            .map(|obj| obj.keys().cloned().collect::<Vec<String>>())
            .unwrap_or_default()
    } else {
        Vec::new()
    };

    frontmatter_keys.sort();

    Ok(HttpResponse::Ok().json(FileMetadataResponse {
        path: file_path,
        size: metadata.len(),
        mtime,
        etag,
        frontmatter_keys,
    }))
}

#[post("/api/vaults/{vault_id}/files")]
async fn create_file(
    state: web::Data<AppState>,
    vault_id: web::Path<String>,
    req: web::Json<CreateFileRequest>,
) -> AppResult<HttpResponse> {
    let vault_id = vault_id.into_inner();
    let vault = state.db.get_vault(&vault_id).await?;

    let content = state
        .storage
        .create_file(&vault.path, &req.path, req.content.as_deref())?;
    let etag = build_file_etag(&content);

    state
        .db
        .log_file_change(
            &vault_id,
            &req.path,
            "created",
            Some(etag.as_str()),
            None,
            state.change_log_retention_days,
        )
        .await?;

    // Update search index if it's a markdown file
    if req.path.ends_with(".md") {
        state
            .search_index
            .update_file(&vault_id, &req.path, content.content.clone())?;
    }

    Ok(HttpResponse::Created().json(content))
}

#[put("/api/vaults/{vault_id}/files/{file_path:.*}")]
async fn update_file(
    state: web::Data<AppState>,
    path: web::Path<(String, String)>,
    http_req: HttpRequest,
    req: web::Json<UpdateFileRequest>,
) -> AppResult<HttpResponse> {
    let (vault_id, file_path) = path.into_inner();
    let vault = state.db.get_vault(&vault_id).await?;

    // Support If-Match header for ETag-based conflict detection.
    // When present, read the current file and compare ETags before writing.
    if let Some(if_match) = http_req.headers().get(actix_web::http::header::IF_MATCH) {
        if let Ok(if_match_str) = if_match.to_str() {
            if let Ok(current) = crate::services::FileService::read_file(&vault.path, &file_path) {
                let current_etag = build_file_etag(&current);
                let normalized_required = normalize_etag(if_match_str);
                let normalized_current = normalize_etag(&current_etag);
                if normalized_required != "*" && normalized_required != normalized_current {
                    // 412 Precondition Failed — return the server's current content
                    // so the client can display a conflict resolver.
                    return Ok(HttpResponse::PreconditionFailed()
                        .insert_header((actix_web::http::header::ETAG, current_etag))
                        .json(serde_json::json!({
                            "error": "precondition_failed",
                            "message": "ETag mismatch: the file was modified since you last read it",
                            "server_content": current,
                        })));
                }
            }
        }
    }

    let content = state.storage.write_file(
        &vault.path,
        &file_path,
        &req.content,
        req.last_modified,
        req.frontmatter.as_ref(),
    )?;
    let etag = build_file_etag(&content);

    state
        .db
        .log_file_change(
            &vault_id,
            &file_path,
            "modified",
            Some(etag.as_str()),
            None,
            state.change_log_retention_days,
        )
        .await?;

    // Update search index if it's a markdown file
    if file_path.ends_with(".md") {
        state
            .search_index
            .update_file(&vault_id, &file_path, content.content.clone())?;
    }

    Ok(HttpResponse::Ok()
        .insert_header((actix_web::http::header::ETAG, etag))
        .json(content))
}

#[delete("/api/vaults/{vault_id}/files/{file_path:.*}")]
async fn delete_file(
    state: web::Data<AppState>,
    path: web::Path<(String, String)>,
) -> AppResult<HttpResponse> {
    let (vault_id, file_path) = path.into_inner();
    let vault = state.db.get_vault(&vault_id).await?;

    FileService::delete_file(&vault.path, &file_path)?;

    state
        .db
        .log_file_change(
            &vault_id,
            &file_path,
            "deleted",
            None,
            None,
            state.change_log_retention_days,
        )
        .await?;

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

    let from = req["from"]
        .as_str()
        .ok_or(crate::error::AppError::InvalidInput(
            "Missing 'from' field".to_string(),
        ))?;
    let to = req["to"]
        .as_str()
        .ok_or(crate::error::AppError::InvalidInput(
            "Missing 'to' field".to_string(),
        ))?;

    let strategy_str = req["strategy"].as_str().unwrap_or("fail");
    let strategy = match strategy_str {
        "overwrite" => crate::services::RenameStrategy::Overwrite,
        "autorename" => crate::services::RenameStrategy::AutoRename,
        _ => crate::services::RenameStrategy::Fail,
    };

    let new_path = FileService::rename(&vault.path, from, to, strategy)?;

    state
        .db
        .log_file_change(
            &vault_id,
            &new_path,
            "renamed",
            None,
            Some(from),
            state.change_log_retention_days,
        )
        .await?;

    // Update search index if it's a markdown file
    if from.ends_with(".md") {
        state.search_index.remove_file(&vault_id, from)?;
    }
    if new_path.ends_with(".md") {
        if let Ok(content) = FileService::read_file(&vault.path, &new_path) {
            state
                .search_index
                .update_file(&vault_id, &new_path, content.content)?;
        }
    }

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "from": from,
        "to": new_path,
    })))
}

#[post("/api/vaults/{vault_id}/upload")]
async fn upload_files(
    state: web::Data<AppState>,
    vault_id: web::Path<String>,
    mut payload: Multipart,
) -> AppResult<HttpResponse> {
    let vault_id = vault_id.into_inner();
    let vault = state.db.get_vault(&vault_id).await?;

    let mut uploaded_files = Vec::new();
    let max_file_size = 100 * 1024 * 1024; // 100MB limit
    let mut requested_target_path = String::new();

    while let Ok(Some(mut field)) = payload.try_next().await {
        // Get the field name and filename
        let content_disposition = field.content_disposition();

        let filename = content_disposition
            .and_then(|cd| cd.get_filename().map(|s| s.to_string()))
            .ok_or(AppError::InvalidInput("Missing filename".to_string()))?;

        // Get the path from the field name (if provided)
        let field_name = content_disposition
            .and_then(|cd| cd.get_name())
            .unwrap_or("file");

        // If field name is "path", this contains the directory path
        if field_name == "path" {
            let mut path_bytes = Vec::new();
            while let Some(chunk) = field.next().await {
                let data =
                    chunk.map_err(|e| AppError::InternalError(format!("Upload error: {}", e)))?;
                path_bytes.extend_from_slice(&data);
            }
            requested_target_path = String::from_utf8(path_bytes)
                .map_err(|_| AppError::InvalidInput("Invalid path encoding".to_string()))?;
            continue;
        }

        let session_id = Uuid::new_v4().to_string();
        state
            .storage
            .create_upload_session_temp(&vault.path, &session_id)?;

        let mut total_size = 0;

        while let Some(chunk) = field.next().await {
            let data =
                chunk.map_err(|e| AppError::InternalError(format!("Upload error: {}", e)))?;
            total_size += data.len();

            if total_size > max_file_size {
                // Clean up partial upload session temp file
                let _ = state
                    .storage
                    .delete_upload_session_temp(&vault.path, &session_id);
                return Err(AppError::InvalidInput(format!(
                    "File {} exceeds maximum size of 100MB",
                    filename
                )));
            }

            state
                .storage
                .append_upload_chunk(&vault.path, &session_id, &data)?;
        }

        let final_path_str = state.storage.finalize_upload_session(
            &vault.path,
            &session_id,
            &requested_target_path,
            &filename,
        )?;

        // Update search index if it's a markdown file
        if final_path_str.ends_with(".md") {
            if let Ok(content) = FileService::read_file(&vault.path, &final_path_str) {
                state
                    .search_index
                    .update_file(&vault_id, &final_path_str, content.content)?;
            }
        }

        uploaded_files.push(serde_json::json!({
            "path": final_path_str,
            "size": total_size,
            "filename": filename,
        }));
    }

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "uploaded": uploaded_files,
        "count": uploaded_files.len(),
    })))
}

#[get("/api/vaults/{vault_id}/download/{file_path:.*}")]
async fn download_file(
    state: web::Data<AppState>,
    path: web::Path<(String, String)>,
) -> AppResult<HttpResponse> {
    let (vault_id, file_path) = path.into_inner();
    let vault = state.db.get_vault(&vault_id).await?;

    let full_path = FileService::resolve_path(&vault.path, &file_path)?;

    if !full_path.exists() {
        return Err(AppError::NotFound(format!("File not found: {}", file_path)));
    }

    if full_path.is_dir() {
        return Err(AppError::InvalidInput(
            "Cannot download directory as single file. Use zip download instead.".to_string(),
        ));
    }

    let content = std::fs::read(&full_path)?;
    let filename = full_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("download");

    Ok(HttpResponse::Ok()
        .content_type("application/octet-stream")
        .insert_header((
            "Content-Disposition",
            format!("attachment; filename=\"{}\"", filename),
        ))
        .body(content))
}

#[post("/api/vaults/{vault_id}/download-zip")]
async fn download_zip(
    state: web::Data<AppState>,
    vault_id: web::Path<String>,
    req: web::Json<serde_json::Value>,
) -> AppResult<HttpResponse> {
    let vault_id = vault_id.into_inner();
    let vault = state.db.get_vault(&vault_id).await?;

    // Get paths array from request
    let paths = req["paths"]
        .as_array()
        .ok_or(AppError::InvalidInput("Missing 'paths' array".to_string()))?;

    if paths.is_empty() {
        return Err(AppError::InvalidInput("No paths provided".to_string()));
    }

    // Create zip file in memory
    let mut buffer = Cursor::new(Vec::new());
    {
        let mut zip = ZipWriter::new(&mut buffer);
        let options = SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated)
            .compression_level(Some(6));

        for path_value in paths {
            let file_path = path_value
                .as_str()
                .ok_or(AppError::InvalidInput("Invalid path in array".to_string()))?;

            let full_path = FileService::resolve_path(&vault.path, file_path)?;

            if !full_path.exists() {
                continue; // Skip non-existent paths
            }

            if full_path.is_file() {
                // Add single file
                let content = std::fs::read(&full_path)?;
                zip.start_file(file_path, options)?;
                zip.write_all(&content)?;
            } else if full_path.is_dir() {
                // Add directory recursively
                for entry in WalkDir::new(&full_path).into_iter().filter_map(|e| e.ok()) {
                    let entry_path = entry.path();
                    if entry_path.is_file() {
                        // Get relative path from vault root
                        let relative_path = entry_path
                            .strip_prefix(&vault.path)
                            .map_err(|_| AppError::InternalError("Path error".to_string()))?;

                        let zip_path = relative_path
                            .to_str()
                            .ok_or(AppError::InternalError("Invalid UTF-8 in path".to_string()))?;

                        let content = std::fs::read(entry_path)?;
                        zip.start_file(zip_path, options)?;
                        zip.write_all(&content)?;
                    }
                }
            }
        }

        zip.finish()?;
    }

    let zip_data = buffer.into_inner();

    // Generate filename
    let zip_filename = if paths.len() == 1 {
        let single_path = paths[0].as_str().unwrap_or("download");
        let name = Path::new(single_path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("download");
        format!("{}.zip", name)
    } else {
        format!("{}_files.zip", paths.len())
    };

    Ok(HttpResponse::Ok()
        .content_type("application/zip")
        .insert_header((
            "Content-Disposition",
            format!("attachment; filename=\"{}\"", zip_filename),
        ))
        .body(zip_data))
}

#[get("/api/vaults/{vault_id}/random")]
async fn get_random_file(
    state: web::Data<AppState>,
    vault_id: web::Path<String>,
) -> AppResult<HttpResponse> {
    let vault_id = vault_id.into_inner();

    // Verify vault exists
    state.db.get_vault(&vault_id).await?;

    let random_file = state.search_index.get_random_file(&vault_id)?;

    if let Some(path) = random_file {
        Ok(HttpResponse::Ok().json(serde_json::json!({
            "path": path,
        })))
    } else {
        Ok(HttpResponse::NotFound().json(serde_json::json!({
            "error": "No markdown files found in vault",
        })))
    }
}

#[derive(serde::Deserialize)]
struct DailyNoteRequest {
    date: String,
}

#[post("/api/vaults/{vault_id}/daily")]
async fn get_daily_note(
    state: web::Data<AppState>,
    vault_id: web::Path<String>,
    req: web::Json<DailyNoteRequest>,
) -> AppResult<HttpResponse> {
    let vault_id = vault_id.into_inner();
    let vault = state.db.get_vault(&vault_id).await?;

    let file_path = format!("{}.md", req.date);

    // Try to read the file
    match FileService::read_file(&vault.path, &file_path) {
        Ok(content) => Ok(HttpResponse::Ok().json(content)),
        Err(AppError::NotFound(_)) => {
            // Create the file if it doesn't exist
            let header = format!("# {}\n\n", req.date);
            let content = FileService::create_file(&vault.path, &file_path, Some(&header))?;
            let etag = build_file_etag(&content);

            state
                .db
                .log_file_change(
                    &vault_id,
                    &file_path,
                    "created",
                    Some(etag.as_str()),
                    None,
                    state.change_log_retention_days,
                )
                .await?;

            // Update search index
            state
                .search_index
                .update_file(&vault_id, &file_path, content.content.clone())?;

            Ok(HttpResponse::Created().json(content))
        }
        Err(e) => Err(e),
    }
}

/// Request to resolve a wiki link to a file path
#[derive(serde::Deserialize)]
pub struct ResolveWikiLinkRequest {
    /// The wiki link to resolve (e.g., "Note", "folder/Note", "Note#header")
    pub link: String,
    /// Optional: current file path for relative resolution
    pub current_file: Option<String>,
}

/// Response for wiki link resolution
#[derive(serde::Serialize)]
pub struct ResolveWikiLinkResponse {
    /// The resolved file path relative to vault root
    pub path: String,
    /// Whether the link target exists
    pub exists: bool,
    /// If ambiguous, list of all matching paths
    pub alternatives: Vec<String>,
    /// Whether link resolution was ambiguous
    pub ambiguous: bool,
}

/// Resolve a wiki link to an actual file path
#[post("/api/vaults/{vault_id}/resolve-link")]
async fn resolve_wiki_link(
    state: web::Data<AppState>,
    vault_id: web::Path<String>,
    req: web::Json<ResolveWikiLinkRequest>,
) -> AppResult<HttpResponse> {
    let vault_id = vault_id.into_inner();
    let vault = state.db.get_vault(&vault_id).await?;

    let result = if let Some(current_file) = &req.current_file {
        WikiLinkResolver::resolve_relative(&vault.path, &req.link, current_file)?
    } else {
        WikiLinkResolver::resolve(&vault.path, &req.link)?
    };

    let response = ResolveWikiLinkResponse {
        path: result.path,
        exists: result.exists,
        ambiguous: !result.alternatives.is_empty(),
        alternatives: result.alternatives,
    };

    Ok(HttpResponse::Ok().json(response))
}

/// Batch resolve multiple wiki links at once
#[derive(serde::Deserialize)]
pub struct BatchResolveRequest {
    /// List of wiki links to resolve
    pub links: Vec<String>,
    /// Optional: current file path for relative resolution
    pub current_file: Option<String>,
}

#[derive(serde::Serialize)]
pub struct BatchResolveResponse {
    /// Map of original link to resolved result
    pub resolved: std::collections::HashMap<String, ResolveWikiLinkResponse>,
}

#[post("/api/vaults/{vault_id}/resolve-links")]
async fn batch_resolve_wiki_links(
    state: web::Data<AppState>,
    vault_id: web::Path<String>,
    req: web::Json<BatchResolveRequest>,
) -> AppResult<HttpResponse> {
    let vault_id = vault_id.into_inner();
    let vault = state.db.get_vault(&vault_id).await?;

    let mut resolved = std::collections::HashMap::new();

    for link in &req.links {
        let result = if let Some(current_file) = &req.current_file {
            WikiLinkResolver::resolve_relative(&vault.path, link, current_file)?
        } else {
            WikiLinkResolver::resolve(&vault.path, link)?
        };

        resolved.insert(
            link.clone(),
            ResolveWikiLinkResponse {
                path: result.path,
                exists: result.exists,
                ambiguous: !result.alternatives.is_empty(),
                alternatives: result.alternatives,
            },
        );
    }

    Ok(HttpResponse::Ok().json(BatchResolveResponse { resolved }))
}

#[get("/api/vaults/{vault_id}/files-html")]
async fn get_file_tree_html(
    state: web::Data<AppState>,
    path: web::Path<String>,
) -> AppResult<HttpResponse> {
    let vault_id = path.into_inner();
    let vault = state.db.get_vault(&vault_id).await?;

    let tree = FileService::get_file_tree(&vault.path)?;

    let html = render_file_tree_to_html(&tree);

    Ok(HttpResponse::Ok().content_type("text/html").body(html))
}

fn render_file_tree_to_html(nodes: &[crate::models::FileNode]) -> String {
    // Handle empty file tree
    if nodes.is_empty() {
        return r#"<p style="padding: 1rem; text-align: center; color: var(--text-muted);">No files found</p>"#.to_string();
    }

    let mut html = String::new();

    // Sort nodes: directories first, then files, then alphabetical
    let mut sorted_nodes = nodes.to_vec();
    sorted_nodes.sort_by(|a, b| {
        if a.is_directory != b.is_directory {
            return b.is_directory.cmp(&a.is_directory);
        }
        a.name.to_lowercase().cmp(&b.name.to_lowercase())
    });

    for node in sorted_nodes {
        let icon = if node.is_directory { "📁" } else { "📄" };
        let type_class = if node.is_directory { "folder" } else { "file" };

        // Escape name for HTML safety (basic)
        let safe_name = node
            .name
            .replace("&", "&amp;")
            .replace("<", "&lt;")
            .replace(">", "&gt;");
        let safe_path = node.path.replace("\"", "&quot;");

        // Add expand/collapse arrow for folders with children
        let has_children =
            node.is_directory && node.children.as_ref().map_or(false, |c| !c.is_empty());
        let arrow = if has_children {
            r#"<span class="folder-arrow">▼</span>"#
        } else {
            ""
        };

        html.push_str(&format!(
            r#"<div class="file-tree-node">
                <div class="file-tree-item {}" data-path="{}" data-type="{}" data-is-directory="{}" draggable="true">
                    {}
                    <span class="file-icon">{}</span>
                    <span class="file-name">{}</span>
                </div>"#,
            type_class, safe_path, type_class, node.is_directory, arrow, icon, safe_name
        ));

        if node.is_directory {
            if let Some(children) = &node.children {
                if !children.is_empty() {
                    html.push_str("<div class=\"file-tree-children\">");
                    html.push_str(&render_file_tree_to_html(children));
                    html.push_str("</div>");
                }
            }
        }

        html.push_str("</div>");
    }

    html
}

/// How to handle an upload that collides with an existing file.
///
/// * `fail` (default) – return a 409 error.
/// * `overwrite` – silently replace the existing file.
/// * `rename_with_timestamp` – keep both; append `_YYYYMMDD_HHmmss[_N]` before the extension.
#[derive(serde::Deserialize, Default, PartialEq, Clone, Copy)]
#[serde(rename_all = "snake_case")]
enum ConflictStrategy {
    #[default]
    Fail,
    Overwrite,
    RenameWithTimestamp,
}

#[derive(serde::Deserialize)]
struct FinishUploadRequest {
    filename: String,
    path: String,
    #[serde(default)]
    conflict: ConflictStrategy,
}

#[derive(serde::Deserialize)]
struct ImportArchiveQuery {
    /// Target folder inside the vault (empty = vault root).
    #[serde(default)]
    path: String,
    /// Archive type: "zip", "tar", or "tar.gz" / "tgz".
    #[serde(default)]
    archive_type: String,
}

/// Derive a conflict-renamed path by appending a timestamp (and optional serial) before the
/// extension: `stem_YYYYMMDD_HHmmss.ext` or `stem_YYYYMMDD_HHmmss_N.ext`.
fn conflict_rename(base: &Path) -> std::path::PathBuf {
    let stamp = Utc::now().format("%Y%m%d_%H%M%S").to_string();
    let stem = base.file_stem().and_then(|s| s.to_str()).unwrap_or("file");
    let ext = base.extension().and_then(|s| s.to_str());
    let parent = base.parent().unwrap_or_else(|| Path::new(""));

    let candidate = if let Some(ext) = ext {
        parent.join(format!("{}_{}.{}", stem, stamp, ext))
    } else {
        parent.join(format!("{}_{}", stem, stamp))
    };

    if !candidate.exists() {
        return candidate;
    }

    // Extremely unlikely but add a serial suffix to break collisions.
    for n in 1u32.. {
        let with_serial = if let Some(ext) = ext {
            parent.join(format!("{}_{}_{}.{}", stem, stamp, n, ext))
        } else {
            parent.join(format!("{}_{}_{}", stem, stamp, n))
        };
        if !with_serial.exists() {
            return with_serial;
        }
    }
    unreachable!()
}

#[post("/api/vaults/{vault_id}/upload-sessions")]
async fn create_upload_session(
    state: web::Data<AppState>,
    vault_id: web::Path<String>,
    req: web::Json<CreateUploadSessionRequest>,
) -> AppResult<HttpResponse> {
    let vault_id = vault_id.into_inner();
    let vault = state.db.get_vault(&vault_id).await?;

    let session_id = Uuid::new_v4().to_string();
    state
        .storage
        .create_upload_session_temp(&vault.path, &session_id)?;

    Ok(HttpResponse::Created().json(UploadSessionResponse {
        session_id,
        uploaded_bytes: 0,
        total_size: req.total_size,
    }))
}

#[put("/api/vaults/{vault_id}/upload-sessions/{session_id}")]
async fn upload_chunk(
    state: web::Data<AppState>,
    path: web::Path<(String, String)>,
    body: web::Bytes,
) -> AppResult<HttpResponse> {
    let (vault_id, session_id) = path.into_inner();
    let vault = state.db.get_vault(&vault_id).await?;
    let uploaded_bytes = state
        .storage
        .append_upload_chunk(&vault.path, &session_id, &body)?;

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "uploaded_bytes": uploaded_bytes
    })))
}

#[get("/api/vaults/{vault_id}/upload-sessions/{session_id}")]
async fn get_upload_status(
    state: web::Data<AppState>,
    path: web::Path<(String, String)>,
) -> AppResult<HttpResponse> {
    let (vault_id, session_id) = path.into_inner();
    let vault = state.db.get_vault(&vault_id).await?;
    let uploaded_bytes = state
        .storage
        .get_upload_session_size(&vault.path, &session_id)?;

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "session_id": session_id,
        "uploaded_bytes": uploaded_bytes
    })))
}

#[post("/api/vaults/{vault_id}/upload-sessions/{session_id}/finish")]
async fn finish_upload_session(
    state: web::Data<AppState>,
    path: web::Path<(String, String)>,
    req: web::Json<FinishUploadRequest>,
) -> AppResult<HttpResponse> {
    let (vault_id, session_id) = path.into_inner();
    let vault = state.db.get_vault(&vault_id).await?;

    // Resolve where the file would land before finalizing so we can apply conflict logic.
    let safe_target_dir = if req.path.is_empty() {
        std::path::PathBuf::from(&vault.path)
    } else {
        FileService::resolve_path(&vault.path, &req.path)?
    };
    let intended_final = safe_target_dir.join(&req.filename);

    // Apply conflict strategy when the destination already exists.
    let (effective_path, effective_filename) = if intended_final.exists() {
        match req.conflict {
            ConflictStrategy::Fail => {
                return Err(AppError::Conflict(format!(
                    "File already exists: {}/{}",
                    req.path, req.filename
                )));
            }
            ConflictStrategy::Overwrite => (req.path.clone(), req.filename.clone()),
            ConflictStrategy::RenameWithTimestamp => {
                let renamed = conflict_rename(&intended_final);
                let new_filename = renamed
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or(&req.filename)
                    .to_string();
                // Keep same directory; only the filename changes.
                (req.path.clone(), new_filename)
            }
        }
    } else {
        (req.path.clone(), req.filename.clone())
    };

    let final_path_str = state.storage.finalize_upload_session(
        &vault.path,
        &session_id,
        &effective_path,
        &effective_filename,
    )?;

    // Update index if markdown
    if final_path_str.ends_with(".md") {
        if let Ok(content) = FileService::read_file(&vault.path, &final_path_str) {
            state
                .search_index
                .update_file(&vault_id, &final_path_str, content.content)?;
        }
    }

    Ok(HttpResponse::Created().json(serde_json::json!({
        "path": final_path_str,
        "filename": effective_filename,
    })))
}

/// POST /api/vaults/{vault_id}/import-archive
///
/// Accepts the raw binary body of a ZIP or TAR(.GZ) archive and extracts its
/// contents into the vault at the optionally-specified `path`.
/// Query params:
///   - `path` – target subdirectory inside the vault (default: vault root)
///   - `archive_type` – "zip", "tar", "tar.gz", or "tgz"
///   - `conflict` – "fail" | "overwrite" | "rename_with_timestamp" (default: rename_with_timestamp)
#[post("/api/vaults/{vault_id}/import-archive")]
async fn import_archive(
    state: web::Data<AppState>,
    vault_id: web::Path<String>,
    query: web::Query<ImportArchiveQuery>,
    body: web::Bytes,
) -> AppResult<HttpResponse> {
    let vault_id = vault_id.into_inner();
    let vault = state.db.get_vault(&vault_id).await?;

    let target_dir = if query.path.is_empty() {
        std::path::PathBuf::from(&vault.path)
    } else {
        FileService::resolve_path(&vault.path, &query.path)?
    };

    if !target_dir.exists() {
        std::fs::create_dir_all(&target_dir)?;
    }

    // Detect archive type from the query parameter (or sniff the magic bytes).
    let archive_type = query.archive_type.to_ascii_lowercase();
    let is_zip =
        archive_type == "zip" || (archive_type.is_empty() && body.starts_with(b"PK\x03\x04"));
    let is_tar_gz = !is_zip
        && (archive_type == "tar.gz"
            || archive_type == "tgz"
            || (archive_type.is_empty() && body.starts_with(b"\x1f\x8b")));
    let is_tar = !is_zip
        && !is_tar_gz
        && (archive_type == "tar"
            || (archive_type.is_empty() && body.len() >= 265 && &body[257..262] == b"ustar"));

    if !is_zip && !is_tar_gz && !is_tar {
        return Err(AppError::InvalidInput(
            "Could not determine archive type. Set ?archive_type=zip|tar|tar.gz".to_string(),
        ));
    }

    let mut extracted: Vec<String> = Vec::new();

    if is_zip {
        let cursor = Cursor::new(body.as_ref());
        let mut archive = zip::ZipArchive::new(cursor)
            .map_err(|e| AppError::InvalidInput(format!("Invalid zip: {}", e)))?;
        for i in 0..archive.len() {
            let mut zf = archive
                .by_index(i)
                .map_err(|e| AppError::InternalError(format!("Zip read error: {}", e)))?;
            // Skip directories; we create them as needed.
            if zf.is_dir() {
                continue;
            }
            let raw_name = zf.name().to_string();
            // Sanitize: reject absolute paths or path traversal.
            if raw_name.starts_with('/') || raw_name.contains("..") {
                continue;
            }
            let dest = target_dir.join(&raw_name);
            if let Some(parent) = dest.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let final_dest = if dest.exists() {
                conflict_rename(&dest)
            } else {
                dest
            };
            let mut out = std::fs::File::create(&final_dest)?;
            std::io::copy(&mut zf, &mut out)?;
            let relative = final_dest
                .strip_prefix(&vault.path)
                .unwrap_or(&final_dest)
                .to_string_lossy()
                .to_string();
            extracted.push(relative);
        }
    } else {
        // tar or tar.gz – decompress first if gzip-compressed.
        let cursor = Cursor::new(body.as_ref());
        let mut tar = if is_tar_gz {
            tar::Archive::new(
                Box::new(flate2::read::GzDecoder::new(cursor)) as Box<dyn std::io::Read>
            )
        } else {
            tar::Archive::new(Box::new(cursor) as Box<dyn std::io::Read>)
        };
        for entry in tar
            .entries()
            .map_err(|e| AppError::InvalidInput(format!("Invalid tar: {}", e)))?
        {
            let mut entry =
                entry.map_err(|e| AppError::InternalError(format!("Tar read error: {}", e)))?;
            let entry_path = entry
                .path()
                .map_err(|e| AppError::InternalError(format!("Tar path error: {}", e)))?
                .into_owned();
            if entry.header().entry_type().is_dir() {
                continue;
            }
            let raw_name = entry_path.to_string_lossy().to_string();
            if raw_name.starts_with('/') || raw_name.contains("..") {
                continue;
            }
            let dest = target_dir.join(&raw_name);
            if let Some(parent) = dest.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let final_dest = if dest.exists() {
                conflict_rename(&dest)
            } else {
                dest
            };
            entry
                .unpack(&final_dest)
                .map_err(|e| AppError::InternalError(format!("Tar unpack error: {}", e)))?;
            let relative = final_dest
                .strip_prefix(&vault.path)
                .unwrap_or(&final_dest)
                .to_string_lossy()
                .to_string();
            extracted.push(relative);
        }
    }

    // Refresh search index for any markdown files extracted.
    for path in &extracted {
        if path.ends_with(".md") {
            if let Ok(content) = FileService::read_file(&vault.path, path) {
                let _ = state
                    .search_index
                    .update_file(&vault_id, path, content.content);
            }
        }
    }

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "extracted": extracted,
        "count": extracted.len(),
    })))
}

/// POST /api/vaults/{vault_id}/download-tar
///
/// Same semantics as `download-zip` but produces a `.tar.gz` archive.
#[post("/api/vaults/{vault_id}/download-tar")]
async fn download_tar(
    state: web::Data<AppState>,
    vault_id: web::Path<String>,
    req: web::Json<serde_json::Value>,
) -> AppResult<HttpResponse> {
    let vault_id = vault_id.into_inner();
    let vault = state.db.get_vault(&vault_id).await?;

    let paths = req["paths"]
        .as_array()
        .ok_or(AppError::InvalidInput("Missing 'paths' array".to_string()))?;

    if paths.is_empty() {
        return Err(AppError::InvalidInput("No paths provided".to_string()));
    }

    // Build tar.gz in memory.
    let buf = Vec::new();
    let gz = GzEncoder::new(buf, Compression::default());
    let mut tar_builder = tar::Builder::new(gz);

    for path_value in paths {
        let file_path = path_value
            .as_str()
            .ok_or(AppError::InvalidInput("Invalid path in array".to_string()))?;

        let full_path = FileService::resolve_path(&vault.path, file_path)?;

        if !full_path.exists() {
            continue;
        }

        if full_path.is_file() {
            tar_builder
                .append_path_with_name(&full_path, file_path)
                .map_err(|e| AppError::InternalError(format!("Tar append error: {}", e)))?;
        } else if full_path.is_dir() {
            for entry in WalkDir::new(&full_path).into_iter().filter_map(|e| e.ok()) {
                let entry_path = entry.path();
                if entry_path.is_file() {
                    let relative = entry_path
                        .strip_prefix(&vault.path)
                        .map_err(|_| AppError::InternalError("Path error".to_string()))?;
                    let tar_path = relative
                        .to_str()
                        .ok_or(AppError::InternalError("Invalid UTF-8 in path".to_string()))?;
                    tar_builder
                        .append_path_with_name(entry_path, tar_path)
                        .map_err(|e| AppError::InternalError(format!("Tar append error: {}", e)))?;
                }
            }
        }
    }

    let gz = tar_builder
        .into_inner()
        .map_err(|e| AppError::InternalError(format!("Tar finish error: {}", e)))?;
    let tar_gz_data = gz
        .finish()
        .map_err(|e| AppError::InternalError(format!("Gzip finish error: {}", e)))?;

    let tar_filename = if paths.len() == 1 {
        let single_path = paths[0].as_str().unwrap_or("download");
        let name = Path::new(single_path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("download");
        format!("{}.tar.gz", name)
    } else {
        format!("{}_files.tar.gz", paths.len())
    };

    Ok(HttpResponse::Ok()
        .content_type("application/gzip")
        .insert_header((
            "Content-Disposition",
            format!("attachment; filename=\"{}\"", tar_filename),
        ))
        .body(tar_gz_data))
}

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(get_file_tree)
        .service(get_file_tree_html)
        .service(get_file_changes)
        .service(sync_files)
        .service(get_file_metadata)
        .service(read_file)
        .service(serve_raw_file)
        .service(get_thumbnail)
        .service(create_file)
        .service(update_file)
        .service(delete_file)
        .service(create_directory)
        .service(rename_file)
        .service(upload_files)
        .service(create_upload_session)
        .service(upload_chunk)
        .service(get_upload_status)
        .service(finish_upload_session)
        .service(import_archive)
        .service(download_file)
        .service(download_zip)
        .service(download_tar)
        .service(get_random_file)
        .service(get_daily_note)
        .service(resolve_wiki_link)
        .service(batch_resolve_wiki_links);
}
