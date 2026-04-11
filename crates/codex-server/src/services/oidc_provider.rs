//! OAuth2 / OIDC authentication provider.
//!
//! Implements the Authorization Code flow:
//! 1. Client redirects user to `GET /api/auth/oidc/authorize`
//! 2. Server redirects to the OIDC provider's authorization endpoint
//! 3. Provider redirects back to `GET /api/auth/oidc/callback?code=...&state=...`
//! 4. Server exchanges code for tokens, extracts user info, creates/finds local user

use crate::config::AuthConfig;
use crate::error::{AppError, AppResult};
use serde::{Deserialize, Serialize};

/// Minimal OIDC discovery document (only the fields we need).
#[derive(Debug, Deserialize)]
pub struct OidcDiscovery {
    pub authorization_endpoint: String,
    pub token_endpoint: String,
    pub userinfo_endpoint: String,
    #[serde(default)]
    pub issuer: String,
}

/// Token response from the OIDC provider.
#[derive(Debug, Deserialize)]
pub struct OidcTokenResponse {
    pub access_token: String,
    #[serde(default)]
    pub id_token: Option<String>,
    #[serde(default)]
    pub token_type: String,
    #[serde(default)]
    pub expires_in: Option<u64>,
}

/// User info from the OIDC provider.
#[derive(Debug, Deserialize)]
pub struct OidcUserInfo {
    pub sub: String,
    #[serde(default)]
    pub preferred_username: Option<String>,
    #[serde(default)]
    pub email: Option<String>,
    #[serde(default)]
    pub name: Option<String>,
}

/// OIDC authorize URL response sent to the client.
#[derive(Debug, Serialize)]
pub struct OidcAuthorizeResponse {
    pub authorize_url: String,
    pub state: String,
}

/// Fetch the OIDC discovery document from `{issuer}/.well-known/openid-configuration`.
pub async fn fetch_discovery(issuer_url: &str) -> AppResult<OidcDiscovery> {
    let url = format!(
        "{}/.well-known/openid-configuration",
        issuer_url.trim_end_matches('/')
    );
    let client = reqwest::Client::new();
    let resp = client
        .get(&url)
        .send()
        .await
        .map_err(|e| AppError::InternalError(format!("OIDC discovery request failed: {e}")))?;

    if !resp.status().is_success() {
        return Err(AppError::InternalError(format!(
            "OIDC discovery returned status {}",
            resp.status()
        )));
    }

    resp.json::<OidcDiscovery>()
        .await
        .map_err(|e| AppError::InternalError(format!("Failed to parse OIDC discovery: {e}")))
}

/// Build the authorization URL that the user should be redirected to.
pub fn build_authorize_url(
    discovery: &OidcDiscovery,
    auth_cfg: &AuthConfig,
    state_token: &str,
) -> AppResult<String> {
    let client_id = auth_cfg
        .oidc_client_id
        .as_deref()
        .ok_or_else(|| AppError::InternalError("oidc_client_id not configured".to_string()))?;
    let redirect_uri = auth_cfg
        .oidc_redirect_uri
        .as_deref()
        .ok_or_else(|| AppError::InternalError("oidc_redirect_uri not configured".to_string()))?;

    let url = format!(
        "{}?response_type=code&client_id={}&redirect_uri={}&scope=openid%20profile%20email&state={}",
        discovery.authorization_endpoint,
        urlencoding::encode(client_id),
        urlencoding::encode(redirect_uri),
        urlencoding::encode(state_token),
    );
    Ok(url)
}

/// Exchange an authorization code for tokens.
pub async fn exchange_code(
    discovery: &OidcDiscovery,
    auth_cfg: &AuthConfig,
    code: &str,
) -> AppResult<OidcTokenResponse> {
    let client_id = auth_cfg
        .oidc_client_id
        .as_deref()
        .ok_or_else(|| AppError::InternalError("oidc_client_id not configured".to_string()))?;
    let client_secret = auth_cfg
        .oidc_client_secret
        .as_deref()
        .ok_or_else(|| AppError::InternalError("oidc_client_secret not configured".to_string()))?;
    let redirect_uri = auth_cfg
        .oidc_redirect_uri
        .as_deref()
        .ok_or_else(|| AppError::InternalError("oidc_redirect_uri not configured".to_string()))?;

    let client = reqwest::Client::new();
    let resp = client
        .post(&discovery.token_endpoint)
        .form(&[
            ("grant_type", "authorization_code"),
            ("code", code),
            ("redirect_uri", redirect_uri),
            ("client_id", client_id),
            ("client_secret", client_secret),
        ])
        .send()
        .await
        .map_err(|e| AppError::InternalError(format!("OIDC token exchange failed: {e}")))?;

    if !resp.status().is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(AppError::Unauthorized(format!(
            "OIDC token exchange rejected: {body}"
        )));
    }

    resp.json::<OidcTokenResponse>()
        .await
        .map_err(|e| AppError::InternalError(format!("Failed to parse OIDC token response: {e}")))
}

/// Fetch user info using the access token.
pub async fn fetch_userinfo(
    discovery: &OidcDiscovery,
    access_token: &str,
) -> AppResult<OidcUserInfo> {
    let client = reqwest::Client::new();
    let resp = client
        .get(&discovery.userinfo_endpoint)
        .bearer_auth(access_token)
        .send()
        .await
        .map_err(|e| AppError::InternalError(format!("OIDC userinfo request failed: {e}")))?;

    if !resp.status().is_success() {
        return Err(AppError::Unauthorized(
            "Failed to fetch user info from OIDC provider".to_string(),
        ));
    }

    resp.json::<OidcUserInfo>()
        .await
        .map_err(|e| AppError::InternalError(format!("Failed to parse OIDC userinfo: {e}")))
}

/// Derive a local username from the OIDC user info.
pub fn derive_username(info: &OidcUserInfo) -> String {
    info.preferred_username
        .clone()
        .or_else(|| info.email.clone())
        .unwrap_or_else(|| format!("oidc_{}", &info.sub[..8.min(info.sub.len())]))
}
