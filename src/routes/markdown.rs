use crate::error::AppResult;
use crate::routes::vaults::AppState;
use crate::services::markdown_service::RenderOptions;
use crate::services::MarkdownService;
use actix_web::{post, web, HttpResponse};
use serde::Deserialize;

#[derive(Deserialize)]
pub struct RenderRequest {
    content: String,
}

#[derive(Deserialize)]
pub struct RenderWithResolutionRequest {
    content: String,
    /// Current file path for relative link resolution
    current_file: Option<String>,
}

#[post("/api/render")]
pub async fn render_markdown(req: web::Json<RenderRequest>) -> AppResult<HttpResponse> {
    let html = MarkdownService::to_html(&req.content);
    Ok(HttpResponse::Ok().content_type("text/html").body(html))
}

/// Render markdown with wiki link resolution for a specific vault
#[post("/api/vaults/{vault_id}/render")]
pub async fn render_markdown_with_resolution(
    state: web::Data<AppState>,
    vault_id: web::Path<String>,
    req: web::Json<RenderWithResolutionRequest>,
) -> AppResult<HttpResponse> {
    let vault_id = vault_id.into_inner();
    let vault = state.db.get_vault(&vault_id).await?;

    let render_opts = RenderOptions {
        vault_path: Some(&vault.path),
        current_file: req.current_file.as_deref(),
        file_index: None,
        enable_highlighting: true,
    };

    let html = MarkdownService::to_html_with_link_resolution(&req.content, &render_opts);
    Ok(HttpResponse::Ok().content_type("text/html").body(html))
}

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(render_markdown)
        .service(render_markdown_with_resolution);
}
