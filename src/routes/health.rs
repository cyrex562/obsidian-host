use crate::routes::AppState;
use actix_web::{get, web, HttpResponse};
use chrono::Utc;

/// GET /health - Full health check (verifies database connectivity)
#[get("/health")]
async fn health(state: web::Data<AppState>) -> HttpResponse {
    match state.db.health_check().await {
        Ok(_) => HttpResponse::Ok().json(serde_json::json!({
            "status": "ok",
            "timestamp": Utc::now().to_rfc3339(),
        })),
        Err(_) => HttpResponse::ServiceUnavailable().json(serde_json::json!({
            "status": "unhealthy",
            "error": "database unavailable",
        })),
    }
}

/// GET /health/live - Liveness probe (app is running)
#[get("/health/live")]
async fn liveness() -> HttpResponse {
    HttpResponse::Ok().json(serde_json::json!({"status": "alive"}))
}

/// GET /health/ready - Readiness probe (dependencies are ready)
#[get("/health/ready")]
async fn readiness(state: web::Data<AppState>) -> HttpResponse {
    match state.db.health_check().await {
        Ok(_) => HttpResponse::Ok().json(serde_json::json!({"status": "ready"})),
        Err(_) => HttpResponse::ServiceUnavailable().json(serde_json::json!({"status": "not_ready"})),
    }
}

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(health)
        .service(liveness)
        .service(readiness);
}
