use crate::config::AuthConfig;
use crate::db::Database;
use crate::error::{AppError, AppResult};
use crate::models::auth::{Session, User};
use openidconnect::core::{CoreClient, CoreIdTokenClaims, CoreProviderMetadata, CoreResponseType};
use openidconnect::reqwest::async_http_client;
use openidconnect::{
    AuthenticationFlow, AuthorizationCode, ClientId, ClientSecret, CsrfToken, IssuerUrl, Nonce,
    PkceCodeChallenge, PkceCodeVerifier, RedirectUrl, Scope, TokenResponse,
};
use sha2::{Digest, Sha256};
use tracing::{error, info};

/// Service handling OIDC authentication flow and session management.
#[derive(Clone)]
pub struct AuthService {
    config: AuthConfig,
    client: Option<CoreClient>,
}

impl AuthService {
    /// Initialize the auth service. Performs OIDC discovery if auth is enabled.
    pub async fn new(config: AuthConfig) -> AppResult<Self> {
        if !config.enabled {
            info!("Authentication is disabled");
            return Ok(Self {
                config,
                client: None,
            });
        }

        if config.google_client_id.is_empty() || config.google_client_secret.is_empty() {
            return Err(AppError::InternalError(
                "Auth is enabled but google_client_id or google_client_secret is not set".into(),
            ));
        }

        let issuer_url =
            IssuerUrl::new("https://accounts.google.com".to_string()).map_err(|e| {
                AppError::InternalError(format!("Invalid Google issuer URL: {}", e))
            })?;

        info!("Discovering Google OIDC provider metadata...");
        let provider_metadata =
            CoreProviderMetadata::discover_async(issuer_url, async_http_client)
                .await
                .map_err(|e| {
                    AppError::InternalError(format!("OIDC discovery failed: {}", e))
                })?;

        let redirect_url = format!("{}/api/auth/callback", config.external_url);

        let client = CoreClient::from_provider_metadata(
            provider_metadata,
            ClientId::new(config.google_client_id.clone()),
            Some(ClientSecret::new(config.google_client_secret.clone())),
        )
        .set_redirect_uri(RedirectUrl::new(redirect_url).map_err(|e| {
            AppError::InternalError(format!("Invalid redirect URL: {}", e))
        })?);

        info!("OIDC client initialized successfully");

        Ok(Self {
            config,
            client: Some(client),
        })
    }

    /// Returns true if auth is enabled.
    pub fn is_enabled(&self) -> bool {
        self.config.enabled
    }

    /// Generate the authorization URL for the OIDC login flow.
    /// Returns (auth_url, csrf_token, nonce, pkce_verifier).
    pub fn generate_auth_url(&self) -> AppResult<(String, String, String, String)> {
        let client = self.client.as_ref().ok_or_else(|| {
            AppError::InternalError("Auth is not enabled".into())
        })?;

        let (pkce_challenge, pkce_verifier) = PkceCodeChallenge::new_random_sha256();

        let (auth_url, csrf_token, nonce) = client
            .authorize_url(
                AuthenticationFlow::<CoreResponseType>::AuthorizationCode,
                CsrfToken::new_random,
                Nonce::new_random,
            )
            .add_scope(Scope::new("email".to_string()))
            .add_scope(Scope::new("profile".to_string()))
            .set_pkce_challenge(pkce_challenge)
            .url();

        Ok((
            auth_url.to_string(),
            csrf_token.secret().clone(),
            nonce.secret().clone(),
            pkce_verifier.secret().clone(),
        ))
    }

    /// Exchange an authorization code for user info.
    /// Returns (email, name, picture, subject, issuer).
    pub async fn exchange_code(
        &self,
        code: &str,
        nonce_str: &str,
        pkce_verifier_str: &str,
    ) -> AppResult<(String, String, Option<String>, String, String)> {
        let client = self.client.as_ref().ok_or_else(|| {
            AppError::InternalError("Auth is not enabled".into())
        })?;

        let pkce_verifier = PkceCodeVerifier::new(pkce_verifier_str.to_string());

        let token_response = client
            .exchange_code(AuthorizationCode::new(code.to_string()))
            .set_pkce_verifier(pkce_verifier)
            .request_async(async_http_client)
            .await
            .map_err(|e| {
                error!("Token exchange failed: {}", e);
                AppError::Unauthorized(format!("Token exchange failed: {}", e))
            })?;

        let id_token = token_response.id_token().ok_or_else(|| {
            AppError::Unauthorized("No ID token in response".into())
        })?;

        let nonce = Nonce::new(nonce_str.to_string());

        let claims: &CoreIdTokenClaims = id_token
            .claims(&client.id_token_verifier(), &nonce)
            .map_err(|e| {
                error!("ID token verification failed: {}", e);
                AppError::Unauthorized(format!("ID token verification failed: {}", e))
            })?;

        let subject = claims.subject().to_string();
        let issuer = claims.issuer().to_string();

        let email = claims
            .email()
            .map(|e| e.as_str().to_string())
            .ok_or_else(|| AppError::Unauthorized("No email in ID token".into()))?;

        // Name and picture are localized claim maps - get the default (None) locale
        let name = claims
            .name()
            .and_then(|name_map| name_map.get(None))
            .map(|n| n.as_str().to_string())
            .unwrap_or_else(|| email.clone());

        let picture = claims
            .picture()
            .and_then(|pic_map| pic_map.get(None))
            .map(|p| p.as_str().to_string());

        Ok((email, name, picture, subject, issuer))
    }

    /// Hash a session token for storage.
    pub fn hash_token(token: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(token.as_bytes());
        hex::encode(hasher.finalize())
    }

    /// Generate a new random session token.
    pub fn generate_token() -> String {
        use rand::Rng;
        let mut rng = rand::rng();
        let bytes: Vec<u8> = (0..32).map(|_| rng.random::<u8>()).collect();
        hex::encode(bytes)
    }

    /// Create a session for a user and return the raw token (to be set as cookie).
    pub async fn create_session(&self, db: &Database, user_id: &str) -> AppResult<String> {
        let token = Self::generate_token();
        let token_hash = Self::hash_token(&token);

        db.create_session(user_id, &token_hash, self.config.session_duration_hours)
            .await?;

        Ok(token)
    }

    /// Validate a session token and return the associated user.
    pub async fn validate_session(
        &self,
        db: &Database,
        token: &str,
    ) -> AppResult<Option<(Session, User)>> {
        let token_hash = Self::hash_token(token);

        match db.get_valid_session(&token_hash).await? {
            Some(session) => {
                let user = db.get_user_by_id(&session.user_id).await?;
                Ok(Some((session, user)))
            }
            None => Ok(None),
        }
    }

    /// Invalidate a session by token.
    pub async fn logout(&self, db: &Database, token: &str) -> AppResult<()> {
        let token_hash = Self::hash_token(token);
        db.delete_session(&token_hash).await
    }

    pub fn session_duration_hours(&self) -> i64 {
        self.config.session_duration_hours
    }

    pub fn external_url(&self) -> &str {
        &self.config.external_url
    }
}
