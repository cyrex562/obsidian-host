use crate::error::{AppError, AppResult};
use crate::middleware::AdminUser;
use crate::models::auth::{AdminUserResponse, AuthUserResponse, UpdateUserRoleRequest, UserRole};
use crate::routes::AppState;
use actix_web::{delete, get, post, put, web, HttpRequest, HttpResponse};
use serde::Deserialize;
use tracing::info;

const SESSION_COOKIE_NAME: &str = "obsidian_session";

// =============================================================================
// Auth flow routes (public - no extractor needed)
// =============================================================================

/// GET /api/auth/login - Redirect to Google OIDC login
#[get("/api/auth/login")]
async fn login(state: web::Data<AppState>) -> AppResult<HttpResponse> {
    let auth = state.auth_service.as_ref().ok_or_else(|| {
        AppError::InternalError("Auth is not enabled".into())
    })?;

    let (auth_url, csrf_token, nonce, pkce_verifier) = auth.generate_auth_url()?;

    state
        .db
        .store_oidc_state(&csrf_token, &nonce, &pkce_verifier)
        .await?;

    Ok(HttpResponse::Found()
        .append_header(("Location", auth_url))
        .finish())
}

#[derive(Deserialize)]
pub struct CallbackQuery {
    pub code: String,
    pub state: String,
}

/// GET /api/auth/callback - Handle OIDC callback from Google
#[get("/api/auth/callback")]
async fn callback(
    state: web::Data<AppState>,
    query: web::Query<CallbackQuery>,
) -> AppResult<HttpResponse> {
    let auth = state.auth_service.as_ref().ok_or_else(|| {
        AppError::InternalError("Auth is not enabled".into())
    })?;

    let (nonce, pkce_verifier) = state
        .db
        .consume_oidc_state(&query.state)
        .await?
        .ok_or_else(|| AppError::Unauthorized("Invalid or expired CSRF state".into()))?;

    let (email, name, picture, subject, issuer) = auth
        .exchange_code(&query.code, &nonce, &pkce_verifier)
        .await?;

    info!("OIDC login successful for {}", email);

    let user = state
        .db
        .upsert_user_from_oidc(&email, &name, picture.as_deref(), &subject, &issuer)
        .await?;

    info!(
        "User {} ({}) logged in with role {:?}",
        user.email, user.id, user.role
    );

    let token = auth.create_session(&state.db, &user.id).await?;
    let cookie = build_session_cookie(&token, auth.session_duration_hours(), auth.external_url());

    let redirect_url = match user.role {
        UserRole::Pending => "/login?status=pending",
        UserRole::Suspended => "/login?status=suspended",
        _ => "/",
    };

    Ok(HttpResponse::Found()
        .append_header(("Location", redirect_url))
        .append_header(("Set-Cookie", cookie))
        .finish())
}

/// POST /api/auth/logout
#[post("/api/auth/logout")]
async fn logout(req: HttpRequest, state: web::Data<AppState>) -> AppResult<HttpResponse> {
    if let Some(cookie) = req.cookie(SESSION_COOKIE_NAME) {
        if let Some(auth) = &state.auth_service {
            let _ = auth.logout(&state.db, cookie.value()).await;
        }
    }

    let clear_cookie = format!(
        "{}=; Path=/; Max-Age=0; HttpOnly; SameSite=Lax",
        SESSION_COOKIE_NAME
    );

    Ok(HttpResponse::Ok()
        .append_header(("Set-Cookie", clear_cookie))
        .json(serde_json::json!({"message": "Logged out"})))
}

/// GET /api/auth/me - Get current user info
#[get("/api/auth/me")]
async fn me(req: HttpRequest, state: web::Data<AppState>) -> AppResult<HttpResponse> {
    let auth = match &state.auth_service {
        Some(a) => a,
        None => {
            return Ok(HttpResponse::Ok().json(serde_json::json!({
                "id": "anonymous",
                "email": "anonymous@localhost",
                "name": "Anonymous",
                "picture": null,
                "role": "admin"
            })));
        }
    };

    let token = req
        .cookie(SESSION_COOKIE_NAME)
        .map(|c| c.value().to_string())
        .ok_or_else(|| AppError::Unauthorized("Not logged in".into()))?;

    let (_session, user) = auth
        .validate_session(&state.db, &token)
        .await?
        .ok_or_else(|| AppError::Unauthorized("Invalid or expired session".into()))?;

    let response: AuthUserResponse = user.into();
    Ok(HttpResponse::Ok().json(response))
}

/// GET /api/auth/status - Check if auth is enabled (public)
#[get("/api/auth/status")]
async fn auth_status(state: web::Data<AppState>) -> HttpResponse {
    let enabled = state.auth_service.is_some();
    HttpResponse::Ok().json(serde_json::json!({
        "auth_enabled": enabled
    }))
}

// =============================================================================
// Admin routes (use AdminUser extractor for automatic auth + admin check)
// =============================================================================

/// GET /api/admin/users - List all users
#[get("/api/admin/users")]
async fn list_users(
    _admin: AdminUser,
    state: web::Data<AppState>,
) -> AppResult<HttpResponse> {
    let users = state.db.list_users().await?;
    let response: Vec<AdminUserResponse> = users.into_iter().map(|u| u.into()).collect();
    Ok(HttpResponse::Ok().json(response))
}

/// PUT /api/admin/users/{id}/role - Update a user's role
#[put("/api/admin/users/{id}/role")]
async fn update_user_role(
    admin: AdminUser,
    state: web::Data<AppState>,
    path: web::Path<String>,
    body: web::Json<UpdateUserRoleRequest>,
) -> AppResult<HttpResponse> {
    let target_id = path.into_inner();

    if admin.0.id == target_id && body.role != UserRole::Admin {
        return Err(AppError::InvalidInput(
            "Cannot change your own role away from admin".into(),
        ));
    }

    let updated = state.db.update_user_role(&target_id, &body.role).await?;
    info!(
        "Admin {} changed user {} role to {:?}",
        admin.0.email, target_id, body.role
    );

    let response: AdminUserResponse = updated.into();
    Ok(HttpResponse::Ok().json(response))
}

/// DELETE /api/admin/users/{id} - Delete a user
#[delete("/api/admin/users/{id}")]
async fn delete_user(
    admin: AdminUser,
    state: web::Data<AppState>,
    path: web::Path<String>,
) -> AppResult<HttpResponse> {
    let target_id = path.into_inner();

    if admin.0.id == target_id {
        return Err(AppError::InvalidInput("Cannot delete yourself".into()));
    }

    state.db.delete_user(&target_id).await?;
    info!("Admin {} deleted user {}", admin.0.email, target_id);

    Ok(HttpResponse::NoContent().finish())
}

// =============================================================================
// Helpers
// =============================================================================

fn build_session_cookie(token: &str, duration_hours: i64, external_url: &str) -> String {
    let max_age = duration_hours * 3600;
    let secure = external_url.starts_with("https://");
    let secure_flag = if secure { "; Secure" } else { "" };

    format!(
        "{}={}; Path=/; Max-Age={}; HttpOnly; SameSite=Lax{}",
        SESSION_COOKIE_NAME, token, max_age, secure_flag
    )
}

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(login)
        .service(callback)
        .service(logout)
        .service(me)
        .service(auth_status)
        .service(list_users)
        .service(update_user_role)
        .service(delete_user);
}
