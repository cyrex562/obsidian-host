use crate::routes::vaults::AppState;
use actix_web::{get, web, HttpResponse};

#[get("/api/health")]
async fn health(state: web::Data<AppState>) -> HttpResponse {
    // Quick DB connectivity check.
    let db_ok = state.db.user_count().await.is_ok();

    if db_ok {
        HttpResponse::Ok().json(serde_json::json!({
            "status": "healthy",
            "database": "connected",
        }))
    } else {
        HttpResponse::ServiceUnavailable().json(serde_json::json!({
            "status": "unhealthy",
            "database": "disconnected",
        }))
    }
}

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(health);
}
