use crate::error::AppResult;
use crate::routes::vaults::AppState;
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

/// Render markdown to HTML (no vault context — uses the default parser).
#[post("/api/render")]
pub async fn render_markdown(
    state: web::Data<AppState>,
    req: web::Json<RenderRequest>,
) -> AppResult<HttpResponse> {
    let doc = state.document_parser.render(&req.content);
    Ok(HttpResponse::Ok().content_type("text/html").body(doc.html))
}

/// Render markdown with wiki link resolution for a specific vault.
#[post("/api/vaults/{vault_id}/render")]
pub async fn render_markdown_with_resolution(
    state: web::Data<AppState>,
    vault_id: web::Path<String>,
    req: web::Json<RenderWithResolutionRequest>,
) -> AppResult<HttpResponse> {
    let vault_id = vault_id.into_inner();
    let vault = state.db.get_vault(&vault_id).await?;

    let doc = state.document_parser.render_with_context(
        &req.content,
        Some(&vault.path),
        req.current_file.as_deref(),
    );
    Ok(HttpResponse::Ok().content_type("text/html").body(doc.html))
}

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(render_markdown)
        .service(render_markdown_with_resolution);
}
