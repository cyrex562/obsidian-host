use crate::error::{AppError, AppResult};
use crate::models::{
    CreateFileRequest, CreateUploadSessionRequest, UpdateFileRequest, UploadSessionResponse,
};
use crate::routes::vaults::AppState;
use crate::services::{FileService, ImageService, WikiLinkResolver};
use actix_multipart::Multipart;
use actix_web::{delete, get, post, put, web, HttpResponse};
use futures::{StreamExt, TryStreamExt};
use std::fs::OpenOptions;
use std::io::{Cursor, Write};
use std::path::{Path, PathBuf};
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

#[post("/api/vaults/{vault_id}/files")]
async fn create_file(
    state: web::Data<AppState>,
    vault_id: web::Path<String>,
    req: web::Json<CreateFileRequest>,
) -> AppResult<HttpResponse> {
    let vault_id = vault_id.into_inner();
    let vault = state.db.get_vault(&vault_id).await?;

    let content = FileService::create_file(&vault.path, &req.path, req.content.as_deref())?;

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
        state
            .search_index
            .update_file(&vault_id, &file_path, content.content.clone())?;
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
                let data =
                    chunk.map_err(|e| AppError::InternalError(format!("Upload error: {}", e)))?;
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
            let data =
                chunk.map_err(|e| AppError::InternalError(format!("Upload error: {}", e)))?;
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
                state
                    .search_index
                    .update_file(&vault_id, &file_path, content.content)?;
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

#[derive(serde::Deserialize)]
struct FinishUploadRequest {
    filename: String,
    path: String,
}

#[post("/api/vaults/{vault_id}/upload-sessions")]
async fn create_upload_session(
    state: web::Data<AppState>,
    vault_id: web::Path<String>,
    req: web::Json<CreateUploadSessionRequest>,
) -> AppResult<HttpResponse> {
    let vault_id = vault_id.into_inner();
    let vault = state.db.get_vault(&vault_id).await?;

    // Use .obsidian/uploads/ as temp dir
    let upload_dir = Path::new(&vault.path).join(".obsidian").join("uploads");
    std::fs::create_dir_all(&upload_dir)?;

    let session_id = Uuid::new_v4().to_string();
    let temp_file_path = upload_dir.join(&session_id);

    // Create empty file
    std::fs::File::create(&temp_file_path)?;

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

    let upload_dir = Path::new(&vault.path).join(".obsidian").join("uploads");
    let temp_file_path = upload_dir.join(&session_id);

    if !temp_file_path.exists() {
        return Err(AppError::NotFound("Upload session not found".to_string()));
    }

    // Append chunk
    let mut file = OpenOptions::new().append(true).open(&temp_file_path)?;
    file.write_all(&body)?;

    let metadata = file.metadata()?;

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "uploaded_bytes": metadata.len()
    })))
}

#[get("/api/vaults/{vault_id}/upload-sessions/{session_id}")]
async fn get_upload_status(
    state: web::Data<AppState>,
    path: web::Path<(String, String)>,
) -> AppResult<HttpResponse> {
    let (vault_id, session_id) = path.into_inner();
    let vault = state.db.get_vault(&vault_id).await?;

    let upload_dir = Path::new(&vault.path).join(".obsidian").join("uploads");
    let temp_file_path = upload_dir.join(&session_id);

    if !temp_file_path.exists() {
        return Err(AppError::NotFound("Upload session not found".to_string()));
    }

    let metadata = std::fs::metadata(&temp_file_path)?;

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "session_id": session_id,
        "uploaded_bytes": metadata.len()
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

    let upload_dir = Path::new(&vault.path).join(".obsidian").join("uploads");
    let temp_file_path = upload_dir.join(&session_id);

    if !temp_file_path.exists() {
        return Err(AppError::NotFound("Upload session not found".to_string()));
    }

    // Resolve safe target path
    let safe_target_dir = if req.path.is_empty() {
        PathBuf::from(&vault.path)
    } else {
        FileService::resolve_path(&vault.path, &req.path)?
    };

    if !safe_target_dir.exists() {
        std::fs::create_dir_all(&safe_target_dir)?;
    } else if !safe_target_dir.is_dir() {
        return Err(AppError::InvalidInput(
            "Target path is not a directory".to_string(),
        ));
    }

    let final_path = safe_target_dir.join(&req.filename);

    // Rename/Move
    if let Err(_) = std::fs::rename(&temp_file_path, &final_path) {
        std::fs::copy(&temp_file_path, &final_path)?;
        std::fs::remove_file(&temp_file_path)?;
    }

    let mixed_path = final_path.to_string_lossy().to_string();
    // Re-resolve relative path properly for consistency
    // Simple way:
    let final_path_str = final_path
        .strip_prefix(&vault.path)
        .unwrap_or(&final_path)
        .to_string_lossy()
        .to_string();

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
        "filename": req.filename
    })))
}

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(get_file_tree)
        .service(get_file_tree_html)
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
        .service(download_file)
        .service(download_zip)
        .service(get_random_file)
        .service(get_daily_note)
        .service(resolve_wiki_link)
        .service(batch_resolve_wiki_links);
}
