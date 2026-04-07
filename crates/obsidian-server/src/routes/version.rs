use actix_web::{get, web, HttpResponse};

/// Build information embedded at compile time via build.rs.
const VERSION: &str = env!("CARGO_PKG_VERSION");
const GIT_HASH: &str = env!("GIT_HASH");
const BUILD_DATE: &str = env!("BUILD_DATE");

#[get("/api/version")]
async fn version() -> HttpResponse {
    HttpResponse::Ok().json(serde_json::json!({
        "version": VERSION,
        "git_hash": GIT_HASH,
        "build_date": BUILD_DATE,
    }))
}

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(version);
}
