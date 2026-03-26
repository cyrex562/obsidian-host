use crate::error::{AppError, AppResult};
use crate::middleware::AuthenticatedUser;
use crate::models::{AcceptInviteRequest, CreateInviteRequest};
use crate::routes::vaults::AppState;
use actix_web::{get, post, web, HttpMessage, HttpRequest, HttpResponse};
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

fn generate_invite_token() -> String {
    let mut rng = rand::rng();
    let bytes: [u8; 16] = rng.random();
    hex::encode(bytes)
}

/// Create a new invitation link.
#[post("/api/invitations")]
async fn create_invitation(
    state: web::Data<AppState>,
    config: web::Data<crate::config::AppConfig>,
    req: HttpRequest,
    body: web::Json<CreateInviteRequest>,
) -> AppResult<HttpResponse> {
    let user = require_user(&req)?;
    let _ = config; // available for future policy checks

    let role = body.role.trim().to_ascii_lowercase();
    if role != "editor" && role != "viewer" {
        return Err(AppError::InvalidInput(
            "Invite role must be 'editor' or 'viewer'".to_string(),
        ));
    }

    let id = Uuid::new_v4().to_string();
    let token = generate_invite_token();
    let expires_at =
        Utc::now() + chrono::Duration::hours(body.expires_in_hours.max(1) as i64);

    state
        .db
        .create_invitation(
            &id,
            &token,
            &role,
            body.vault_id.as_deref(),
            &user.user_id,
            expires_at,
        )
        .await?;

    let _ = state
        .db
        .write_audit_log(
            Some(&user.user_id),
            Some(&user.username),
            "invitation_created",
            Some(&format!(
                "Created invite (role={role}, vault={:?}, expires={})",
                body.vault_id, expires_at
            )),
            None,
            true,
        )
        .await;

    Ok(HttpResponse::Created().json(serde_json::json!({
        "id": id,
        "token": token,
        "role": role,
        "vault_id": body.vault_id,
        "expires_at": expires_at.to_rfc3339(),
    })))
}

/// List invitations created by the authenticated user.
#[get("/api/invitations")]
async fn list_invitations(
    state: web::Data<AppState>,
    req: HttpRequest,
) -> AppResult<HttpResponse> {
    let user = require_user(&req)?;
    let invites = state.db.list_invitations_by_creator(&user.user_id).await?;
    Ok(HttpResponse::Ok().json(invites))
}

/// Accept an invitation — creates a new user account and grants vault access.
/// This endpoint does NOT require authentication (it's how new users sign up).
#[post("/api/invitations/accept")]
async fn accept_invitation(
    state: web::Data<AppState>,
    config: web::Data<crate::config::AppConfig>,
    body: web::Json<AcceptInviteRequest>,
) -> AppResult<HttpResponse> {
    let token = body.token.trim();
    if token.is_empty() {
        return Err(AppError::InvalidInput("Token is required".to_string()));
    }

    let row = state
        .db
        .get_invitation_by_token(token)
        .await?
        .ok_or_else(|| AppError::NotFound("Invitation not found".to_string()))?;

    let (invite_id, role, vault_id, _created_by, _created_at, expires_at_str, accepted, _) = row;

    if accepted {
        return Err(AppError::Conflict(
            "This invitation has already been accepted".to_string(),
        ));
    }

    let expires_at = chrono::DateTime::parse_from_rfc3339(&expires_at_str)
        .ok()
        .map(|dt| dt.with_timezone(&Utc));
    if let Some(exp) = expires_at {
        if exp < Utc::now() {
            return Err(AppError::InvalidInput(
                "This invitation has expired".to_string(),
            ));
        }
    }

    // Validate password.
    crate::services::validate_password_policy(&body.password, &config.auth)?;

    // Create the user account.
    let salt = SaltString::generate(&mut OsRng);
    let password_hash = Argon2::default()
        .hash_password(body.password.as_bytes(), &salt)
        .map_err(|e| AppError::InternalError(format!("Failed to hash password: {e}")))?
        .to_string();

    let (user_id, username, _, _) = state
        .db
        .create_user_with_options(body.username.trim(), &password_hash, false, false)
        .await?;

    // Grant vault access if specified.
    if let Some(ref vid) = vault_id {
        let vault_role = match role.as_str() {
            "editor" => crate::models::VaultRole::Editor,
            "viewer" => crate::models::VaultRole::Viewer,
            _ => crate::models::VaultRole::Viewer,
        };
        state
            .db
            .share_vault_with_user(vid, &user_id, &vault_role)
            .await?;
    }

    // Mark invitation as accepted.
    state.db.accept_invitation(&invite_id, &user_id).await?;

    let _ = state
        .db
        .write_audit_log(
            Some(&user_id),
            Some(&username),
            "invitation_accepted",
            Some(&format!("Accepted invite {invite_id} (role={role})")),
            None,
            true,
        )
        .await;

    Ok(HttpResponse::Created().json(serde_json::json!({
        "success": true,
        "user_id": user_id,
        "username": username,
        "role": role,
        "vault_id": vault_id,
    })))
}

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(create_invitation)
        .service(list_invitations)
        .service(accept_invitation);
}
