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

    let parsed_hash = PasswordHash::new(&user.2)
        .map_err(|_| AppError::Unauthorized("Invalid username or password".to_string()))?;

    Argon2::default()
        .verify_password(password.as_bytes(), &parsed_hash)
        .map_err(|_| AppError::Unauthorized("Invalid username or password".to_string()))?;

    Ok(AuthenticatedPrincipal {
        user_id: user.0,
        username: user.1,
        auth_method: AuthProviderKind::Password.as_str().to_string(),
    })
}
