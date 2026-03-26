use crate::config::AuthConfig;
use crate::db::Database;
use crate::error::{AppError, AppResult};
use argon2::{password_hash::PasswordHash, Argon2, PasswordVerifier};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthProviderKind {
    Password,
    Oidc,
    Mtls,
}

impl AuthProviderKind {
    pub fn from_config_value(value: &str) -> Self {
        match value.trim().to_ascii_lowercase().as_str() {
            "oidc" => Self::Oidc,
            "mtls" | "m_tls" | "mutual_tls" => Self::Mtls,
            _ => Self::Password,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Password => "password",
            Self::Oidc => "oidc",
            Self::Mtls => "mtls",
        }
    }
}

#[derive(Debug, Clone)]
pub struct AuthenticatedPrincipal {
    pub user_id: String,
    pub username: String,
    pub auth_method: String,
}

pub async fn authenticate_username_password(
    db: &Database,
    auth_cfg: &AuthConfig,
    username: &str,
    password: &str,
) -> AppResult<AuthenticatedPrincipal> {
    match AuthProviderKind::from_config_value(&auth_cfg.provider) {
        AuthProviderKind::Password => {
            authenticate_with_password_provider(db, username, password).await
        }
        AuthProviderKind::Oidc => Err(AppError::Unauthorized(
            "OIDC authentication is not yet implemented".to_string(),
        )),
        AuthProviderKind::Mtls => Err(AppError::Unauthorized(
            "mTLS authentication is not yet implemented".to_string(),
        )),
    }
}

async fn authenticate_with_password_provider(
    db: &Database,
    username: &str,
    password: &str,
) -> AppResult<AuthenticatedPrincipal> {
    let user = db
        .get_user_auth_by_username(username)
        .await?
        .ok_or_else(|| AppError::Unauthorized("Invalid username or password".to_string()))?;

    let user_id = &user.0;

    // Check if the account is active.
    if !db.is_user_active(user_id).await? {
        return Err(AppError::Unauthorized(
            "Account is deactivated. Contact an administrator.".to_string(),
        ));
    }

    // Check if the account is currently locked out.
    if let Some(locked_until) = db.get_lockout_status(user_id).await? {
        return Err(AppError::Unauthorized(format!(
            "Account is temporarily locked until {}. Try again later.",
            locked_until.format("%Y-%m-%d %H:%M:%S UTC")
        )));
    }

    let parsed_hash = PasswordHash::new(&user.2)
        .map_err(|_| AppError::Unauthorized("Invalid username or password".to_string()))?;

    if Argon2::default()
        .verify_password(password.as_bytes(), &parsed_hash)
        .is_err()
    {
        // Record failed attempt and optionally lock the account.
        let attempts = db.record_failed_login(user_id).await.unwrap_or(0);
        const MAX_ATTEMPTS: i64 = 5;
        const LOCKOUT_MINUTES: i64 = 15;

        if attempts >= MAX_ATTEMPTS {
            let until = chrono::Utc::now()
                + chrono::Duration::minutes(LOCKOUT_MINUTES);
            let _ = db.lock_user_until(user_id, until).await;
            let _ = db
                .write_audit_log(
                    Some(user_id),
                    Some(username),
                    "account_locked",
                    Some(&format!(
                        "Locked after {attempts} failed attempts for {LOCKOUT_MINUTES} minutes"
                    )),
                    None,
                    false,
                )
                .await;
        }

        let _ = db
            .write_audit_log(
                Some(user_id),
                Some(username),
                "login_failed",
                None,
                None,
                false,
            )
            .await;

        return Err(AppError::Unauthorized(
            "Invalid username or password".to_string(),
        ));
    }

    // Successful login — clear any failed attempts.
    let _ = db.clear_failed_logins(user_id).await;
    let _ = db
        .write_audit_log(
            Some(user_id),
            Some(username),
            "login_success",
            None,
            None,
            true,
        )
        .await;

    Ok(AuthenticatedPrincipal {
        user_id: user.0,
        username: user.1,
        auth_method: AuthProviderKind::Password.as_str().to_string(),
    })
}
