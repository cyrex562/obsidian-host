use crate::config::AuthConfig;
use crate::db::Database;
use crate::error::{AppError, AppResult};
use argon2::{password_hash::PasswordHash, Argon2, PasswordVerifier};

/// Validate a new password against the configured password policy.
pub fn validate_password_policy(password: &str, auth_cfg: &AuthConfig) -> AppResult<()> {
    if password.len() < auth_cfg.min_password_length {
        return Err(AppError::InvalidInput(format!(
            "Password must be at least {} characters",
            auth_cfg.min_password_length
        )));
    }
    if auth_cfg.require_uppercase && !password.chars().any(|c| c.is_uppercase()) {
        return Err(AppError::InvalidInput(
            "Password must contain at least one uppercase letter".to_string(),
        ));
    }
    if auth_cfg.require_lowercase && !password.chars().any(|c| c.is_lowercase()) {
        return Err(AppError::InvalidInput(
            "Password must contain at least one lowercase letter".to_string(),
        ));
    }
    if auth_cfg.require_digit && !password.chars().any(|c| c.is_ascii_digit()) {
        return Err(AppError::InvalidInput(
            "Password must contain at least one digit".to_string(),
        ));
    }
    if auth_cfg.require_special
        && !password
            .chars()
            .any(|c| !c.is_alphanumeric() && !c.is_whitespace())
    {
        return Err(AppError::InvalidInput(
            "Password must contain at least one special character".to_string(),
        ));
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthProviderKind {
    Password,
    Oidc,
    Ldap,
}

impl AuthProviderKind {
    pub fn from_config_value(value: &str) -> Self {
        match value.trim().to_ascii_lowercase().as_str() {
            "oidc" => Self::Oidc,
            "ldap" | "active_directory" | "ad" => Self::Ldap,
            // "mtls" was removed — mutual TLS requires transport-layer cert
            // extraction (e.g. a reverse proxy forwarding X-Client-Cert headers)
            // and cannot be implemented as a plain auth provider.
            _ => Self::Password,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Password => "password",
            Self::Oidc => "oidc",
            Self::Ldap => "ldap",
        }
    }
}

#[derive(Debug, Clone)]
pub struct AuthenticatedPrincipal {
    pub user_id: String,
    pub username: String,
    pub auth_method: String,
    /// If true, the user has TOTP enabled and a second factor code is required.
    pub totp_required: bool,
}

pub async fn authenticate_username_password(
    db: &Database,
    auth_cfg: &AuthConfig,
    username: &str,
    password: &str,
) -> AppResult<AuthenticatedPrincipal> {
    match AuthProviderKind::from_config_value(&auth_cfg.provider) {
        AuthProviderKind::Password => {
            authenticate_with_password_provider(db, auth_cfg, username, password).await
        }
        AuthProviderKind::Ldap => {
            authenticate_with_ldap_provider(db, auth_cfg, username, password).await
        }
        AuthProviderKind::Oidc => Err(AppError::Unauthorized(
            "OIDC uses redirect-based login. Use /api/auth/oidc/authorize instead.".to_string(),
        )),
    }
}

async fn authenticate_with_password_provider(
    db: &Database,
    auth_cfg: &AuthConfig,
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
        let max_attempts = if auth_cfg.max_failed_logins > 0 {
            auth_cfg.max_failed_logins as i64
        } else {
            5
        };
        let lockout_minutes = if auth_cfg.lockout_minutes > 0 {
            auth_cfg.lockout_minutes as i64
        } else {
            15
        };

        if max_attempts > 0 && attempts >= max_attempts {
            let until = chrono::Utc::now()
                + chrono::Duration::minutes(lockout_minutes);
            let _ = db.lock_user_until(user_id, until).await;
            let _ = db
                .write_audit_log(
                    Some(user_id),
                    Some(username),
                    "account_locked",
                    Some(&format!(
                        "Locked after {attempts} failed attempts for {lockout_minutes} minutes"
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

    // Check if TOTP is enabled for this user.
    let totp_required = db
        .get_totp_state(user_id)
        .await
        .map(|(enabled, _, _)| enabled)
        .unwrap_or(false);

    let _ = db
        .write_audit_log(
            Some(user_id),
            Some(username),
            "login_success",
            if totp_required {
                Some("TOTP verification pending")
            } else {
                None
            },
            None,
            true,
        )
        .await;

    Ok(AuthenticatedPrincipal {
        user_id: user.0,
        username: user.1,
        auth_method: AuthProviderKind::Password.as_str().to_string(),
        totp_required,
    })
}

/// Authenticate via LDAP: verify credentials against the directory, then
/// find-or-create a local user record so the rest of the app works normally.
async fn authenticate_with_ldap_provider(
    db: &Database,
    auth_cfg: &AuthConfig,
    username: &str,
    password: &str,
) -> AppResult<AuthenticatedPrincipal> {
    // Verify credentials against LDAP.
    let canonical_username =
        crate::services::ldap_provider::authenticate_ldap(auth_cfg, username, password).await?;

    // Find or create local user row (LDAP users get a placeholder password hash
    // since their password is managed by the directory).
    let local_user = db.get_user_auth_by_username(&canonical_username).await?;
    let user_id = match local_user {
        Some((id, _, _)) => id,
        None => {
            // Auto-provision the LDAP user locally.
            let placeholder_hash = "ldap-managed";
            let (id, _, _, _) = db
                .create_user_with_options(&canonical_username, placeholder_hash, false, false)
                .await?;
            let _ = db
                .write_audit_log(
                    Some(&id),
                    Some(&canonical_username),
                    "ldap_user_provisioned",
                    Some("Auto-provisioned from LDAP directory"),
                    None,
                    true,
                )
                .await;
            id
        }
    };

    let _ = db
        .write_audit_log(
            Some(&user_id),
            Some(&canonical_username),
            "login_success",
            Some("Authenticated via LDAP"),
            None,
            true,
        )
        .await;

    Ok(AuthenticatedPrincipal {
        user_id,
        username: canonical_username,
        auth_method: AuthProviderKind::Ldap.as_str().to_string(),
        totp_required: false,
    })
}
