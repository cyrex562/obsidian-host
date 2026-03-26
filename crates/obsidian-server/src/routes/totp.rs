use crate::error::{AppError, AppResult};
use crate::middleware::AuthenticatedUser;
use crate::models::{TotpEnrollResponse, TotpVerifyRequest};
use crate::routes::vaults::AppState;
use actix_web::{get, post, web, HttpMessage, HttpRequest, HttpResponse};
use rand::Rng;
use totp_rs::{Algorithm, Secret, TOTP};

fn require_user(req: &HttpRequest) -> AppResult<AuthenticatedUser> {
    req.extensions()
        .get::<AuthenticatedUser>()
        .cloned()
        .ok_or_else(|| AppError::Unauthorized("Authentication required".to_string()))
}

fn build_totp(secret_base32: &str, username: &str) -> AppResult<TOTP> {
    let secret = Secret::Encoded(secret_base32.to_string())
        .to_bytes()
        .map_err(|e| AppError::InternalError(format!("Invalid TOTP secret: {e}")))?;
    TOTP::new(
        Algorithm::SHA1,
        6,
        1,
        30,
        secret,
        Some("ObsidianHost".to_string()),
        username.to_string(),
    )
    .map_err(|e| AppError::InternalError(format!("Failed to create TOTP: {e}")))
}

fn generate_backup_codes(count: usize) -> Vec<String> {
    let mut rng = rand::rng();
    (0..count)
        .map(|_| {
            let n: u32 = rng.random_range(10_000_000..99_999_999);
            format!("{n:08}")
        })
        .collect()
}

/// Begin TOTP enrollment: generate secret, return otpauth URL and backup codes.
#[post("/api/auth/totp/enroll")]
async fn totp_enroll(
    state: web::Data<AppState>,
    req: HttpRequest,
) -> AppResult<HttpResponse> {
    let user = require_user(&req)?;

    // Check if already enabled.
    let (enabled, _, _) = state.db.get_totp_state(&user.user_id).await?;
    if enabled {
        return Err(AppError::Conflict(
            "TOTP is already enabled. Disable it first to re-enroll.".to_string(),
        ));
    }

    let secret = Secret::generate_secret();
    let secret_base32 = secret.to_encoded().to_string();
    let totp = build_totp(&secret_base32, &user.username)?;
    let otpauth_url = totp.get_url();
    let backup_codes = generate_backup_codes(8);

    // Store secret and backup codes (not yet enabled — must verify first).
    state
        .db
        .set_totp_secret(&user.user_id, &secret_base32, &backup_codes)
        .await?;

    let _ = state
        .db
        .write_audit_log(
            Some(&user.user_id),
            Some(&user.username),
            "totp_enrollment_started",
            None,
            None,
            true,
        )
        .await;

    Ok(HttpResponse::Ok().json(TotpEnrollResponse {
        otpauth_url,
        secret: secret_base32,
        backup_codes,
    }))
}

/// Verify a TOTP code to complete enrollment (enables 2FA).
#[post("/api/auth/totp/verify")]
async fn totp_verify(
    state: web::Data<AppState>,
    req: HttpRequest,
    body: web::Json<TotpVerifyRequest>,
) -> AppResult<HttpResponse> {
    let user = require_user(&req)?;

    let (enabled, secret_opt, _) = state.db.get_totp_state(&user.user_id).await?;
    if enabled {
        return Err(AppError::Conflict("TOTP is already enabled".to_string()));
    }
    let secret = secret_opt.ok_or_else(|| {
        AppError::InvalidInput("No TOTP enrollment in progress. Call /enroll first.".to_string())
    })?;

    let totp = build_totp(&secret, &user.username)?;
    if !totp.check_current(&body.code).unwrap_or(false) {
        return Err(AppError::Unauthorized(
            "Invalid TOTP code. Check your authenticator app.".to_string(),
        ));
    }

    state.db.enable_totp(&user.user_id).await?;

    let _ = state
        .db
        .write_audit_log(
            Some(&user.user_id),
            Some(&user.username),
            "totp_enabled",
            None,
            None,
            true,
        )
        .await;

    Ok(HttpResponse::Ok().json(serde_json::json!({ "success": true, "totp_enabled": true })))
}

/// Disable TOTP 2FA.
#[post("/api/auth/totp/disable")]
async fn totp_disable(
    state: web::Data<AppState>,
    req: HttpRequest,
) -> AppResult<HttpResponse> {
    let user = require_user(&req)?;

    state.db.disable_totp(&user.user_id).await?;

    let _ = state
        .db
        .write_audit_log(
            Some(&user.user_id),
            Some(&user.username),
            "totp_disabled",
            None,
            None,
            true,
        )
        .await;

    Ok(HttpResponse::Ok().json(serde_json::json!({ "success": true, "totp_enabled": false })))
}

/// Get TOTP status for the authenticated user.
#[get("/api/auth/totp/status")]
async fn totp_status(
    state: web::Data<AppState>,
    req: HttpRequest,
) -> AppResult<HttpResponse> {
    let user = require_user(&req)?;
    let (enabled, _, _) = state.db.get_totp_state(&user.user_id).await?;
    Ok(HttpResponse::Ok().json(serde_json::json!({ "totp_enabled": enabled })))
}

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(totp_enroll)
        .service(totp_verify)
        .service(totp_disable)
        .service(totp_status);
}
