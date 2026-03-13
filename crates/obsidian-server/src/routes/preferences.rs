use crate::config::AppConfig;
use crate::error::{AppError, AppResult};
use crate::middleware::AuthenticatedUser;
use crate::models::UserPreferences;
use crate::routes::vaults::AppState;
use actix_web::{get, post, put, web, HttpMessage, HttpRequest, HttpResponse};

fn preference_scope_user_id(req: &HttpRequest, config: &AppConfig) -> AppResult<Option<String>> {
    if !config.auth.enabled {
        return Ok(None);
    }

    let user = req
        .extensions()
        .get::<AuthenticatedUser>()
        .cloned()
        .ok_or_else(|| AppError::Unauthorized("Authentication required".to_string()))?;

    Ok(Some(user.user_id))
}

#[get("/api/preferences")]
async fn get_preferences(
    state: web::Data<AppState>,
    config: web::Data<AppConfig>,
    req: HttpRequest,
) -> AppResult<HttpResponse> {
    let scope_user_id = preference_scope_user_id(&req, config.get_ref())?;
    let prefs = state
        .db
        .get_preferences_for_user(scope_user_id.as_deref())
        .await?;
    Ok(HttpResponse::Ok().json(prefs))
}

#[put("/api/preferences")]
async fn update_preferences(
    state: web::Data<AppState>,
    config: web::Data<AppConfig>,
    req: HttpRequest,
    prefs: web::Json<UserPreferences>,
) -> AppResult<HttpResponse> {
    let scope_user_id = preference_scope_user_id(&req, config.get_ref())?;
    state
        .db
        .update_preferences_for_user(scope_user_id.as_deref(), &prefs)
        .await?;
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
async fn reset_preferences(
    state: web::Data<AppState>,
    config: web::Data<AppConfig>,
    req: HttpRequest,
) -> AppResult<HttpResponse> {
    let scope_user_id = preference_scope_user_id(&req, config.get_ref())?;
    let default = UserPreferences::default();
    state
        .db
        .update_preferences_for_user(scope_user_id.as_deref(), &default)
        .await?;
    Ok(HttpResponse::Ok().json(default))
}

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(get_preferences)
        .service(update_preferences)
        .service(get_recent_files)
        .service(record_recent_file)
        .service(reset_preferences);
}
