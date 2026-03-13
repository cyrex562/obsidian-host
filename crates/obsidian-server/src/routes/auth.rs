use crate::config::AppConfig;
use crate::error::{AppError, AppResult};
use crate::middleware::AuthenticatedUser;
use crate::models::AuthenticatedUserProfile;
use crate::models::ChangePasswordRequest;
use crate::routes::vaults::AppState;
use crate::services::authenticate_username_password;
use actix_web::{get, post, web, HttpMessage, HttpRequest, HttpResponse};
use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, SaltString},
    Argon2, PasswordVerifier,
};
use chrono::Utc;
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Deserialize)]
pub struct RefreshRequest {
    pub refresh_token: String,
}

#[derive(Debug, Serialize)]
pub struct LoginResponse {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_in: u64,
}

#[derive(Debug, Serialize)]
pub struct LogoutResponse {
    pub success: bool,
}

#[derive(Debug, Serialize, Deserialize)]
struct Claims {
    sub: String,
    username: String,
    auth_method: String,
    token_type: String,
    exp: i64,
    iat: i64,
}

#[post("/api/auth/login")]
async fn login(
    state: web::Data<AppState>,
    config: web::Data<AppConfig>,
    req: web::Json<LoginRequest>,
) -> AppResult<HttpResponse> {
    let username = req.username.trim();
    let password = req.password.trim();

    if username.is_empty() || password.is_empty() {
        return Err(AppError::InvalidInput(
            "Username and password are required".to_string(),
        ));
    }

    let principal =
        authenticate_username_password(&state.db, &config.auth, username, password).await?;

    let response = issue_tokens(
        &principal.user_id,
        &principal.username,
        &principal.auth_method,
        &config.auth,
    )?;
    Ok(HttpResponse::Ok().json(response))
}

#[post("/api/auth/refresh")]
async fn refresh_access_token(
    config: web::Data<AppConfig>,
    req: web::Json<RefreshRequest>,
) -> AppResult<HttpResponse> {
    let claims = decode_token(&req.refresh_token, &config.auth.jwt_secret)?;

    if claims.token_type != "refresh" {
        return Err(AppError::Unauthorized("Invalid refresh token".to_string()));
    }

    let response = issue_tokens(
        &claims.sub,
        &claims.username,
        &claims.auth_method,
        &config.auth,
    )?;
    Ok(HttpResponse::Ok().json(response))
}

#[post("/api/auth/logout")]
async fn logout() -> AppResult<HttpResponse> {
    // Short-lived JWT strategy for now (no server-side token revocation table yet).
    Ok(HttpResponse::Ok().json(LogoutResponse { success: true }))
}

#[get("/api/auth/me")]
async fn me(
    state: web::Data<AppState>,
    config: web::Data<AppConfig>,
    req: HttpRequest,
) -> AppResult<HttpResponse> {
    let user = req
        .extensions()
        .get::<AuthenticatedUser>()
        .cloned()
        .ok_or_else(|| AppError::Unauthorized("Authentication required".to_string()))?;

    let groups = state.db.list_groups_for_user(&user.user_id).await?;
    let is_admin = state.db.is_user_admin(&user.user_id).await?;
    let must_change_password = state.db.user_must_change_password(&user.user_id).await?;

    Ok(HttpResponse::Ok().json(AuthenticatedUserProfile {
        id: user.user_id,
        username: user.username,
        is_admin,
        must_change_password,
        groups,
        auth_method: config.auth.provider.clone(),
    }))
}

#[post("/api/auth/change-password")]
async fn change_password(
    state: web::Data<AppState>,
    req: HttpRequest,
    body: web::Json<ChangePasswordRequest>,
) -> AppResult<HttpResponse> {
    let user = req
        .extensions()
        .get::<AuthenticatedUser>()
        .cloned()
        .ok_or_else(|| AppError::Unauthorized("Authentication required".to_string()))?;

    let current_password = body.current_password.trim();
    let new_password = body.new_password.trim();

    if current_password.is_empty() || new_password.is_empty() {
        return Err(AppError::InvalidInput(
            "Current password and new password are required".to_string(),
        ));
    }

    if new_password.len() < 12 {
        return Err(AppError::InvalidInput(
            "New password must be at least 12 characters".to_string(),
        ));
    }

    let auth_row = state
        .db
        .get_user_auth_by_id(&user.user_id)
        .await?
        .ok_or_else(|| AppError::Unauthorized("User not found".to_string()))?;

    let parsed_hash = PasswordHash::new(&auth_row.2)
        .map_err(|_| AppError::Unauthorized("Invalid credentials".to_string()))?;
    Argon2::default()
        .verify_password(current_password.as_bytes(), &parsed_hash)
        .map_err(|_| AppError::Unauthorized("Invalid current password".to_string()))?;

    let salt = SaltString::generate(&mut OsRng);
    let new_hash = Argon2::default()
        .hash_password(new_password.as_bytes(), &salt)
        .map_err(|e| AppError::InternalError(format!("Failed to hash password: {e}")))?
        .to_string();

    state
        .db
        .set_user_password(&user.user_id, &new_hash, false)
        .await?;

    Ok(HttpResponse::Ok().json(serde_json::json!({ "success": true })))
}

fn issue_tokens(
    user_id: &str,
    username: &str,
    auth_method: &str,
    auth_cfg: &crate::config::AuthConfig,
) -> AppResult<LoginResponse> {
    let secret = effective_jwt_secret(auth_cfg);
    let now = Utc::now().timestamp();

    let access_claims = Claims {
        sub: user_id.to_string(),
        username: username.to_string(),
        auth_method: auth_method.to_string(),
        token_type: "access".to_string(),
        iat: now,
        exp: now + auth_cfg.access_token_ttl as i64,
    };

    let refresh_claims = Claims {
        sub: user_id.to_string(),
        username: username.to_string(),
        auth_method: auth_method.to_string(),
        token_type: "refresh".to_string(),
        iat: now,
        exp: now + auth_cfg.refresh_token_ttl as i64,
    };

    let access_token = encode(
        &Header::default(),
        &access_claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .map_err(|e| AppError::InternalError(format!("Failed to issue access token: {e}")))?;

    let refresh_jwt = encode(
        &Header::default(),
        &refresh_claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .map_err(|e| AppError::InternalError(format!("Failed to issue refresh token: {e}")))?;

    Ok(LoginResponse {
        access_token,
        refresh_token: refresh_jwt,
        expires_in: auth_cfg.access_token_ttl,
    })
}

fn decode_token(token: &str, jwt_secret: &str) -> AppResult<Claims> {
    let secret = if jwt_secret.trim().is_empty() {
        crate::config::DEFAULT_DEV_JWT_SECRET.to_string()
    } else {
        jwt_secret.to_string()
    };

    let mut validation = Validation::default();
    validation.validate_exp = true;

    let data = decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &validation,
    )
    .map_err(|_| AppError::Unauthorized("Invalid or expired token".to_string()))?;

    Ok(data.claims)
}

fn effective_jwt_secret(auth_cfg: &crate::config::AuthConfig) -> String {
    if auth_cfg.jwt_secret.trim().is_empty() {
        crate::config::DEFAULT_DEV_JWT_SECRET.to_string()
    } else {
        auth_cfg.jwt_secret.clone()
    }
}

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(login)
        .service(me)
    .service(change_password)
        .service(refresh_access_token)
        .service(logout);
}
