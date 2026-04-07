use crate::config::AppConfig;
use crate::error::{AppError, AppResult};
use crate::routes::vaults::AppState;
use crate::services::oidc_provider;
use actix_web::{get, web, HttpResponse};
use argon2::{
    password_hash::{rand_core::OsRng, PasswordHasher, SaltString},
    Argon2,
};
use rand::Rng;
use uuid::Uuid;

/// Generate a random state token for CSRF protection.
fn generate_state_token() -> String {
    let mut rng = rand::rng();
    let bytes: [u8; 16] = rng.random();
    hex::encode(bytes)
}

/// Step 1: Redirect the user to the OIDC provider's authorization page.
/// Returns the URL the client should redirect to.
#[get("/api/auth/oidc/authorize")]
async fn oidc_authorize(config: web::Data<AppConfig>) -> AppResult<HttpResponse> {
    let issuer = config
        .auth
        .oidc_issuer_url
        .as_deref()
        .ok_or_else(|| AppError::InternalError("OIDC is not configured".to_string()))?;

    let discovery = oidc_provider::fetch_discovery(issuer).await?;
    let state_token = generate_state_token();
    let authorize_url =
        oidc_provider::build_authorize_url(&discovery, &config.auth, &state_token)?;

    Ok(HttpResponse::Ok().json(oidc_provider::OidcAuthorizeResponse {
        authorize_url,
        state: state_token,
    }))
}

/// Step 2: OIDC callback after the user authenticates with the provider.
/// Exchanges the code for tokens, fetches user info, creates/finds local user,
/// and issues our own JWT tokens.
#[get("/api/auth/oidc/callback")]
async fn oidc_callback(
    state: web::Data<AppState>,
    config: web::Data<AppConfig>,
    query: web::Query<OidcCallbackQuery>,
) -> AppResult<HttpResponse> {
    let issuer = config
        .auth
        .oidc_issuer_url
        .as_deref()
        .ok_or_else(|| AppError::InternalError("OIDC is not configured".to_string()))?;

    if let Some(ref error) = query.error {
        return Err(AppError::Unauthorized(format!(
            "OIDC provider returned error: {error}"
        )));
    }

    let code = query
        .code
        .as_deref()
        .ok_or_else(|| AppError::InvalidInput("Missing authorization code".to_string()))?;

    // Exchange code for tokens.
    let discovery = oidc_provider::fetch_discovery(issuer).await?;
    let tokens = oidc_provider::exchange_code(&discovery, &config.auth, code).await?;

    // Fetch user info.
    let userinfo = oidc_provider::fetch_userinfo(&discovery, &tokens.access_token).await?;
    let username = oidc_provider::derive_username(&userinfo);

    // Find or create local user.
    let local_user = state.db.get_user_auth_by_username(&username).await?;
    let user_id = match local_user {
        Some((id, _, _)) => id,
        None => {
            // Auto-provision: create user with a random placeholder password.
            let placeholder = format!("oidc-managed-{}", Uuid::new_v4());
            let salt = SaltString::generate(&mut OsRng);
            let hash = Argon2::default()
                .hash_password(placeholder.as_bytes(), &salt)
                .map_err(|e| AppError::InternalError(format!("Hash failed: {e}")))?
                .to_string();

            let (id, _, _, _) = state
                .db
                .create_user_with_options(&username, &hash, false, false)
                .await?;
            let _ = state
                .db
                .write_audit_log(
                    Some(&id),
                    Some(&username),
                    "oidc_user_provisioned",
                    Some(&format!("Auto-provisioned from OIDC (sub={})", userinfo.sub)),
                    None,
                    true,
                )
                .await;
            id
        }
    };

    let _ = state
        .db
        .write_audit_log(
            Some(&user_id),
            Some(&username),
            "login_success",
            Some("Authenticated via OIDC"),
            None,
            true,
        )
        .await;

    // Issue our own JWT tokens.
    let (response, refresh_jti, refresh_exp) = crate::routes::auth::issue_tokens_public(
        &user_id,
        &username,
        "oidc",
        &config.auth,
    )?;

    let _ = state.db.create_session(&refresh_jti, &user_id, refresh_exp).await;

    Ok(HttpResponse::Ok().json(response))
}

#[derive(Debug, serde::Deserialize)]
struct OidcCallbackQuery {
    code: Option<String>,
    state: Option<String>,
    error: Option<String>,
}

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(oidc_authorize).service(oidc_callback);
}
