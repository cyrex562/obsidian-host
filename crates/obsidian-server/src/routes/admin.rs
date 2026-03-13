use crate::error::{AppError, AppResult};
use crate::middleware::AuthenticatedUser;
use crate::models::{CreateUserRequest, CreateUserResponse};
use crate::routes::vaults::AppState;
use actix_web::{get, post, web, HttpMessage, HttpRequest, HttpResponse};
use argon2::{
    password_hash::{rand_core::OsRng, PasswordHasher, SaltString},
    Argon2,
};
use rand::{distr::Alphanumeric, Rng};

fn require_authenticated_user(req: &HttpRequest) -> AppResult<AuthenticatedUser> {
    req.extensions()
        .get::<AuthenticatedUser>()
        .cloned()
        .ok_or_else(|| AppError::Unauthorized("Authentication required".to_string()))
}

async fn require_admin_user(
    state: &web::Data<AppState>,
    req: &HttpRequest,
) -> AppResult<AuthenticatedUser> {
    let user = require_authenticated_user(req)?;
    let is_admin = state.db.is_user_admin(&user.user_id).await?;
    if !is_admin {
        return Err(AppError::Forbidden(
            "Administrator privileges are required".to_string(),
        ));
    }
    Ok(user)
}

fn generate_temporary_password() -> String {
    rand::rng()
        .sample_iter(&Alphanumeric)
        .take(20)
        .map(char::from)
        .collect::<String>()
}

fn hash_password(password: &str) -> AppResult<String> {
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map_err(|e| AppError::InternalError(format!("Failed to hash password: {e}")))
        .map(|h| h.to_string())
}

#[get("/api/admin/users")]
async fn list_users(state: web::Data<AppState>, req: HttpRequest) -> AppResult<HttpResponse> {
    let _admin = require_admin_user(&state, &req).await?;
    let users = state.db.list_users().await?;
    Ok(HttpResponse::Ok().json(users))
}

#[post("/api/admin/users")]
async fn create_user(
    state: web::Data<AppState>,
    req: HttpRequest,
    body: web::Json<CreateUserRequest>,
) -> AppResult<HttpResponse> {
    let _admin = require_admin_user(&state, &req).await?;

    let username = body.username.trim();
    if username.is_empty() {
        return Err(AppError::InvalidInput(
            "Username cannot be empty".to_string(),
        ));
    }

    let temporary_password = body
        .temporary_password
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(str::to_string)
        .unwrap_or_else(generate_temporary_password);

    if temporary_password.len() < 12 {
        return Err(AppError::InvalidInput(
            "Temporary password must be at least 12 characters".to_string(),
        ));
    }

    let password_hash = hash_password(&temporary_password)?;

    let (id, username, is_admin, must_change_password) = state
        .db
        .create_user_with_options(
            username,
            &password_hash,
            body.is_admin.unwrap_or(false),
            true,
        )
        .await?;

    Ok(HttpResponse::Created().json(CreateUserResponse {
        id,
        username,
        temporary_password,
        is_admin,
        must_change_password,
    }))
}

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(list_users).service(create_user);
}
