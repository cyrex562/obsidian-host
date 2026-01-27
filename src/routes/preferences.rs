use crate::error::AppResult;
use crate::models::UserPreferences;
use crate::routes::vaults::AppState;
use actix_web::{get, post, put, web, HttpResponse};

#[get("/api/preferences")]
async fn get_preferences(state: web::Data<AppState>) -> AppResult<HttpResponse> {
    let prefs = state.db.get_preferences().await?;
    Ok(HttpResponse::Ok().json(prefs))
}

#[put("/api/preferences")]
async fn update_preferences(
    state: web::Data<AppState>,
    prefs: web::Json<UserPreferences>,
) -> AppResult<HttpResponse> {
    state.db.update_preferences(&prefs).await?;
    Ok(HttpResponse::Ok().json(&*prefs))
}

#[get("/api/vaults/{vault_id}/recent")]
async fn get_recent_files(
    state: web::Data<AppState>,
    vault_id: web::Path<String>,
) -> AppResult<HttpResponse> {
    let recent = state.db.get_recent_files(&vault_id, 20).await?;
    Ok(HttpResponse::Ok().json(recent))
}

#[post("/api/vaults/{vault_id}/recent")]
async fn record_recent_file(
    state: web::Data<AppState>,
    vault_id: web::Path<String>,
    req: web::Json<serde_json::Value>,
) -> AppResult<HttpResponse> {
    let path = req["path"]
        .as_str()
        .ok_or(crate::error::AppError::InvalidInput(
            "Missing path field".to_string(),
        ))?;

    state.db.record_recent_file(&vault_id, path).await?;
    Ok(HttpResponse::Ok().finish())
}

#[post("/api/preferences/reset")]
async fn reset_preferences(state: web::Data<AppState>) -> AppResult<HttpResponse> {
    let default = UserPreferences::default();
    state.db.update_preferences(&default).await?;
    Ok(HttpResponse::Ok().json(default))
}

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(get_preferences)
        .service(update_preferences)
        .service(get_recent_files)
        .service(record_recent_file)
        .service(reset_preferences);
}
