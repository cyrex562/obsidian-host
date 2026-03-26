use crate::error::{AppError, AppResult};
use crate::middleware::AuthenticatedUser;
use crate::models::{CreateApiKeyRequest, CreateApiKeyResponse};
use crate::routes::vaults::AppState;
use actix_web::{delete, get, post, web, HttpMessage, HttpRequest, HttpResponse};
use argon2::{
    password_hash::{rand_core::OsRng, PasswordHasher, SaltString},
    Argon2,
};
use chrono::Utc;
use rand::Rng;
use uuid::Uuid;

fn require_user(req: &HttpRequest) -> AppResult<AuthenticatedUser> {
    req.extensions()
        .get::<AuthenticatedUser>()
        .cloned()
        .ok_or_else(|| AppError::Unauthorized("Authentication required".to_string()))
}

/// Generate a random API key string: `obh_` prefix + 48 random hex chars.
fn generate_api_key() -> String {
    let mut rng = rand::rng();
    let bytes: [u8; 24] = rng.random();
    format!("obh_{}", hex::encode(bytes))
}

/// Extract the short prefix from a key for DB lookup (first 12 chars after `obh_`).
fn key_prefix(key: &str) -> String {
    let body = key.strip_prefix("obh_").unwrap_or(key);
    body.chars().take(12).collect()
}

fn hash_key(key: &str) -> AppResult<String> {
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(key.as_bytes(), &salt)
        .map_err(|e| AppError::InternalError(format!("Failed to hash API key: {e}")))
        .map(|h| h.to_string())
}

/// Generate a new API key for the authenticated user.
#[post("/api/auth/api-keys")]
async fn create_api_key(
    state: web::Data<AppState>,
    req: HttpRequest,
    body: web::Json<CreateApiKeyRequest>,
) -> AppResult<HttpResponse> {
    let user = require_user(&req)?;

    let name = body.name.trim();
    if name.is_empty() {
        return Err(AppError::InvalidInput(
            "API key name cannot be empty".to_string(),
        ));
    }

    let raw_key = generate_api_key();
    let prefix = key_prefix(&raw_key);
    let key_hash = hash_key(&raw_key)?;
    let id = Uuid::new_v4().to_string();

    let expires_at = body.expires_in_days.map(|days| {
        Utc::now() + chrono::Duration::days(days as i64)
    });

    state
        .db
        .create_api_key(&id, name, &prefix, &key_hash, &user.user_id, expires_at)
        .await?;

    let _ = state
        .db
        .write_audit_log(
            Some(&user.user_id),
            Some(&user.username),
            "api_key_created",
            Some(&format!("Created API key '{name}' (prefix: {prefix})")),
            None,
            true,
        )
        .await;

    Ok(HttpResponse::Created().json(CreateApiKeyResponse {
        id,
        name: name.to_string(),
        api_key: raw_key,
        prefix,
        expires_at,
    }))
}

/// List all API keys for the authenticated user.
#[get("/api/auth/api-keys")]
async fn list_api_keys(
    state: web::Data<AppState>,
    req: HttpRequest,
) -> AppResult<HttpResponse> {
    let user = require_user(&req)?;
    let keys = state.db.list_api_keys(&user.user_id).await?;
    Ok(HttpResponse::Ok().json(keys))
}

/// Revoke an API key.
#[delete("/api/auth/api-keys/{key_id}")]
async fn revoke_api_key(
    state: web::Data<AppState>,
    req: HttpRequest,
    path: web::Path<String>,
) -> AppResult<HttpResponse> {
    let user = require_user(&req)?;
    let key_id = path.into_inner();

    state.db.revoke_api_key(&key_id, &user.user_id).await?;

    let _ = state
        .db
        .write_audit_log(
            Some(&user.user_id),
            Some(&user.username),
            "api_key_revoked",
            Some(&format!("Revoked API key {key_id}")),
            None,
            true,
        )
        .await;

    Ok(HttpResponse::Ok().json(serde_json::json!({ "success": true })))
}

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(create_api_key)
        .service(list_api_keys)
        .service(revoke_api_key);
}
