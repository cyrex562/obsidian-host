use crate::db::Database;
use crate::error::{AppError, AppResult};
use crate::models::{CreateFileRequest, UpdateFileRequest};
use crate::routes::vaults::AppState;
use crate::services::FileService;
use actix_multipart::Multipart;
use actix_web::{delete, get, post, put, web, HttpResponse};
use futures::{StreamExt, TryStreamExt};
use std::io::{Write, Cursor};
use std::path::Path;
use zip::write::SimpleFileOptions;
use zip::ZipWriter;
use walkdir::WalkDir;

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
        req.frontmatter.as_ref(),
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

        // Extract directory path if provided (format: "path" field)
        let mut target_path = String::new();

        // If field name is "path", this contains the directory path
        if field_name == "path" {
            let mut path_bytes = Vec::new();
            while let Some(chunk) = field.next().await {
                let data = chunk.map_err(|e| AppError::InternalError(format!("Upload error: {}", e)))?;
                path_bytes.extend_from_slice(&data);
            }
            target_path = String::from_utf8(path_bytes)
                .map_err(|_| AppError::InvalidInput("Invalid path encoding".to_string()))?;
            continue;
        }

        // Construct full file path
        let file_path = if target_path.is_empty() {
            filename.to_string()
        } else {
            format!("{}/{}", target_path.trim_end_matches('/'), filename)
        };

        // Create the full path on disk
        let full_path = FileService::resolve_path(&vault.path, &file_path)
            .map_err(|e| AppError::InvalidInput(format!("Invalid file path: {}", e)))?;

        // Create parent directory if needed
        if let Some(parent) = full_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        // Write file data
        let mut total_size = 0;
        let mut file = std::fs::File::create(&full_path)?;

        while let Some(chunk) = field.next().await {
            let data = chunk.map_err(|e| AppError::InternalError(format!("Upload error: {}", e)))?;
            total_size += data.len();

            if total_size > max_file_size {
                // Clean up partial file
                drop(file);
                let _ = std::fs::remove_file(&full_path);
                return Err(AppError::InvalidInput(format!(
                    "File {} exceeds maximum size of 100MB",
                    filename
                )));
            }

            file.write_all(&data)?;
        }

        drop(file);

        // Update search index if it's a markdown file
        if file_path.ends_with(".md") {
            if let Ok(content) = FileService::read_file(&vault.path, &file_path) {
                state.search_index.update_file(&vault_id, &file_path, content.content)?;
            }
        }

        uploaded_files.push(serde_json::json!({
            "path": file_path,
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
        return Err(AppError::InvalidInput("Cannot download directory as single file. Use zip download instead.".to_string()));
    }

    let content = std::fs::read(&full_path)?;
    let filename = full_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("download");

    Ok(HttpResponse::Ok()
        .content_type("application/octet-stream")
        .insert_header(("Content-Disposition", format!("attachment; filename=\"{}\"", filename)))
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
    let paths = req["paths"].as_array()
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
            let file_path = path_value.as_str()
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
                        let relative_path = entry_path.strip_prefix(&vault.path)
                            .map_err(|_| AppError::InternalError("Path error".to_string()))?;

                        let zip_path = relative_path.to_str()
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
        .insert_header(("Content-Disposition", format!("attachment; filename=\"{}\"", zip_filename)))
        .body(zip_data))
}

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(get_file_tree)
        .service(read_file)
        .service(serve_raw_file)
        .service(create_file)
        .service(update_file)
        .service(delete_file)
        .service(create_directory)
        .service(rename_file)
        .service(upload_files)
        .service(download_file)
        .service(download_zip);
}
