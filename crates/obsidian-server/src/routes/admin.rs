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
    config: web::Data<crate::config::AppConfig>,
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

    crate::services::validate_password_policy(&temporary_password, &config.auth)?;

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

#[post("/api/admin/users/{user_id}/edit")]
async fn edit_user(
    state: web::Data<AppState>,
    config: web::Data<crate::config::AppConfig>,
    req: HttpRequest,
    path: web::Path<String>,
    body: web::Json<EditUserRequest>,
) -> AppResult<HttpResponse> {
    let admin = require_admin_user(&state, &req).await?;
    let user_id = path.into_inner();

    // Toggle admin status.
    if let Some(is_admin) = body.is_admin {
        if admin.user_id == user_id && !is_admin {
            return Err(AppError::InvalidInput(
                "Cannot revoke your own admin privileges".to_string(),
            ));
        }
        sqlx::query("UPDATE users SET is_admin = ? WHERE id = ?")
            .bind(if is_admin { 1_i64 } else { 0_i64 })
            .bind(&user_id)
            .execute(state.db.pool())
            .await
            .map_err(AppError::from)?;
        state
            .db
            .write_audit_log(
                Some(&admin.user_id),
                Some(&admin.username),
                "user_admin_toggled",
                Some(&format!(
                    "Set is_admin={is_admin} for user {user_id}"
                )),
                None,
                true,
            )
            .await?;
    }

    // Reset password.
    if let Some(ref new_password) = body.reset_password {
        crate::services::validate_password_policy(new_password, &config.auth)?;
        let password_hash = hash_password(new_password)?;
        state
            .db
            .set_user_password(&user_id, &password_hash, true)
            .await?;
        state
            .db
            .write_audit_log(
                Some(&admin.user_id),
                Some(&admin.username),
                "user_password_reset",
                Some(&format!("Admin reset password for user {user_id}")),
                None,
                true,
            )
            .await?;
    }

    Ok(HttpResponse::Ok().json(serde_json::json!({ "success": true })))
}

#[derive(Debug, serde::Deserialize)]
struct EditUserRequest {
    is_admin: Option<bool>,
    reset_password: Option<String>,
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

/// Bulk import users from a JSON array.
#[post("/api/admin/users/bulk-import")]
async fn bulk_import_users(
    state: web::Data<AppState>,
    config: web::Data<crate::config::AppConfig>,
    req: HttpRequest,
    body: web::Json<Vec<crate::models::BulkUserEntry>>,
) -> AppResult<HttpResponse> {
    let admin = require_admin_user(&state, &req).await?;

    let mut created = Vec::new();
    let mut failed = Vec::new();

    for entry in body.into_inner() {
        let username = entry.username.trim().to_string();
        if username.is_empty() {
            failed.push(crate::models::BulkImportError {
                username: username.clone(),
                error: "Username cannot be empty".to_string(),
            });
            continue;
        }

        let password = entry
            .temporary_password
            .as_deref()
            .filter(|p| !p.is_empty())
            .map(str::to_string)
            .unwrap_or_else(generate_temporary_password);

        if let Err(e) = crate::services::validate_password_policy(&password, &config.auth) {
            failed.push(crate::models::BulkImportError {
                username,
                error: e.to_string(),
            });
            continue;
        }

        let password_hash = match hash_password(&password) {
            Ok(h) => h,
            Err(e) => {
                failed.push(crate::models::BulkImportError {
                    username,
                    error: e.to_string(),
                });
                continue;
            }
        };

        match state
            .db
            .create_user_with_options(&username, &password_hash, entry.is_admin, true)
            .await
        {
            Ok(_) => created.push(username),
            Err(e) => {
                failed.push(crate::models::BulkImportError {
                    username,
                    error: e.to_string(),
                });
            }
        }
    }

    let _ = state
        .db
        .write_audit_log(
            Some(&admin.user_id),
            Some(&admin.username),
            "bulk_user_import",
            Some(&format!(
                "Created {} user(s), {} failed",
                created.len(),
                failed.len()
            )),
            None,
            true,
        )
        .await;

    Ok(HttpResponse::Ok().json(crate::models::BulkImportResult { created, failed }))
}

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(list_users)
        .service(create_user)
        .service(edit_user)
        .service(deactivate_user)
        .service(reactivate_user)
        .service(delete_user)
        .service(get_audit_log)
        .service(bulk_import_users);
}
