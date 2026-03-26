use crate::error::{AppError, AppResult};
use crate::middleware::AuthenticatedUser;
use crate::models::{CreateUserRequest, CreateUserResponse};
use crate::routes::vaults::AppState;
use actix_web::{delete, get, post, web, HttpMessage, HttpRequest, HttpResponse};
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

#[post("/api/admin/users/{user_id}/deactivate")]
async fn deactivate_user(
    state: web::Data<AppState>,
    req: HttpRequest,
    path: web::Path<String>,
) -> AppResult<HttpResponse> {
    let admin = require_admin_user(&state, &req).await?;
    let user_id = path.into_inner();

    if admin.user_id == user_id {
        return Err(AppError::InvalidInput(
            "Cannot deactivate your own account".to_string(),
        ));
    }

    state.db.deactivate_user(&user_id).await?;
    state
        .db
        .write_audit_log(
            Some(&admin.user_id),
            Some(&admin.username),
            "user_deactivated",
            Some(&format!("Deactivated user {user_id}")),
            None,
            true,
        )
        .await?;
    Ok(HttpResponse::Ok().json(serde_json::json!({ "success": true })))
}

#[post("/api/admin/users/{user_id}/reactivate")]
async fn reactivate_user(
    state: web::Data<AppState>,
    req: HttpRequest,
    path: web::Path<String>,
) -> AppResult<HttpResponse> {
    let admin = require_admin_user(&state, &req).await?;
    let user_id = path.into_inner();

    state.db.reactivate_user(&user_id).await?;
    state
        .db
        .write_audit_log(
            Some(&admin.user_id),
            Some(&admin.username),
            "user_reactivated",
            Some(&format!("Reactivated user {user_id}")),
            None,
            true,
        )
        .await?;
    Ok(HttpResponse::Ok().json(serde_json::json!({ "success": true })))
}

#[delete("/api/admin/users/{user_id}")]
async fn delete_user(
    state: web::Data<AppState>,
    req: HttpRequest,
    path: web::Path<String>,
) -> AppResult<HttpResponse> {
    let admin = require_admin_user(&state, &req).await?;
    let user_id = path.into_inner();

    if admin.user_id == user_id {
        return Err(AppError::InvalidInput(
            "Cannot delete your own account".to_string(),
        ));
    }

    // Look up username before deletion for the audit log.
    let username = state
        .db
        .get_user_by_id(&user_id)
        .await?
        .map(|(_, u)| u)
        .unwrap_or_else(|| user_id.clone());

    state.db.delete_user(&user_id).await?;
    state
        .db
        .write_audit_log(
            Some(&admin.user_id),
            Some(&admin.username),
            "user_deleted",
            Some(&format!("Deleted user {username} ({user_id})")),
            None,
            true,
        )
        .await?;
    Ok(HttpResponse::Ok().json(serde_json::json!({ "success": true })))
}

#[get("/api/admin/audit-log")]
async fn get_audit_log(
    state: web::Data<AppState>,
    req: HttpRequest,
    query: web::Query<AuditLogQuery>,
) -> AppResult<HttpResponse> {
    let _admin = require_admin_user(&state, &req).await?;
    let entries = state.db.get_audit_log(query.limit).await?;
    Ok(HttpResponse::Ok().json(entries))
}

#[derive(Debug, serde::Deserialize)]
struct AuditLogQuery {
    limit: Option<i64>,
}

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(list_users)
        .service(create_user)
        .service(deactivate_user)
        .service(reactivate_user)
        .service(delete_user)
        .service(get_audit_log);
}
