use crate::error::AppResult;
use crate::services::MarkdownService;
use actix_web::{post, web, HttpResponse};
use serde::Deserialize;

#[derive(Deserialize)]
pub struct RenderRequest {
    content: String,
}

#[post("/api/render")]
pub async fn render_markdown(req: web::Json<RenderRequest>) -> AppResult<HttpResponse> {
    let html = MarkdownService::to_html(&req.content);
    Ok(HttpResponse::Ok().content_type("text/html").body(html))
}

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(render_markdown);
}
