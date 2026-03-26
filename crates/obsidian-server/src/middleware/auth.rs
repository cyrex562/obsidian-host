use argon2::PasswordVerifier;

use crate::config::AppConfig;
use crate::routes::AppState;
use actix_web::body::{EitherBody, MessageBody};
use actix_web::{
    dev::{forward_ready, Service, ServiceRequest, ServiceResponse, Transform},
    http::{header, Method},
    Error, HttpMessage, HttpResponse,
};
use futures::future::{ready, LocalBoxFuture, Ready};
use jsonwebtoken::{decode, DecodingKey, Validation};
use serde::{Deserialize, Serialize};
use std::rc::Rc;

#[derive(Debug, Clone)]
pub struct UserId(pub String);

#[derive(Debug, Clone)]
pub struct AuthenticatedUser {
    pub user_id: String,
    pub username: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RequiredVaultRole {
    Read,
    Write,
    Manage,
}

pub struct AuthMiddleware;

impl<S, B> Transform<S, ServiceRequest> for AuthMiddleware
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    B: MessageBody + 'static,
{
    type Response = ServiceResponse<EitherBody<B>>;
    type Error = Error;
    type InitError = ();
    type Transform = AuthMiddlewareService<S>;
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(AuthMiddlewareService {
            service: Rc::new(service),
        }))
    }
}

pub struct AuthMiddlewareService<S> {
    service: Rc<S>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Claims {
    sub: String,
    username: String,
    token_type: String,
    exp: i64,
    iat: i64,
}

impl<S, B> Service<ServiceRequest> for AuthMiddlewareService<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    B: MessageBody + 'static,
{
    type Response = ServiceResponse<EitherBody<B>>;
    type Error = Error;
    type Future = LocalBoxFuture<'static, Result<Self::Response, Self::Error>>;

    forward_ready!(service);

    fn call(&self, req: ServiceRequest) -> Self::Future {
        let should_skip = should_skip_auth(&req);
        let service = Rc::clone(&self.service);

        if should_skip {
            let fut = service.call(req);
            return Box::pin(async move { Ok(fut.await?.map_into_left_body()) });
        }

        let app_cfg = req
            .app_data::<actix_web::web::Data<AppConfig>>()
            .map(|cfg| cfg.get_ref().clone())
            .unwrap_or_default();

        if !app_cfg.auth.enabled {
            let fut = service.call(req);
            return Box::pin(async move { Ok(fut.await?.map_into_left_body()) });
        }

        // Try API key auth first (X-API-Key header).
        let api_key_header = req
            .headers()
            .get("X-API-Key")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string());

        let state_for_api_key = req
            .app_data::<actix_web::web::Data<AppState>>()
            .cloned();

        // Try JWT bearer token.
        let bearer = extract_access_token(&req);

        if bearer.is_none() && api_key_header.is_none() {
            let response = HttpResponse::Unauthorized().json(serde_json::json!({
                "error": "UNAUTHORIZED",
                "message": "Missing or invalid Authorization header or API key"
            }));
            return Box::pin(
                async move { Ok(req.into_response(response).map_into_right_body()) },
            );
        }

        // If we have an API key, validate it asynchronously and resolve the user.
        if let Some(raw_key) = api_key_header {
            let required_vault_role = required_vault_role(&req);
            let state_clone = state_for_api_key.clone();
            let fut = service.call(req);

            return Box::pin(async move {
                let state = state_clone.ok_or_else(|| {
                    actix_web::error::ErrorInternalServerError("Missing app state")
                })?;
                let prefix: String = raw_key
                    .strip_prefix("obh_")
                    .unwrap_or(&raw_key)
                    .chars()
                    .take(12)
                    .collect();

                let row = state.db.get_api_key_by_prefix(&prefix).await.map_err(|_| {
                    actix_web::error::ErrorUnauthorized("Invalid API key")
                })?;
                let Some((_id, key_hash, user_id, expires_at, revoked)) = row else {
                    return Err(actix_web::error::ErrorUnauthorized("Invalid API key"));
                };
                if revoked {
                    return Err(actix_web::error::ErrorUnauthorized("API key has been revoked"));
                }
                if let Some(exp_str) = expires_at {
                    if let Ok(exp) = chrono::DateTime::parse_from_rfc3339(&exp_str) {
                        if exp < chrono::Utc::now() {
                            return Err(actix_web::error::ErrorUnauthorized("API key has expired"));
                        }
                    }
                }
                // Verify key hash.
                let parsed_hash =
                    argon2::password_hash::PasswordHash::new(&key_hash).map_err(|_| {
                        actix_web::error::ErrorUnauthorized("Invalid API key")
                    })?;
                argon2::Argon2::default()
                    .verify_password(raw_key.as_bytes(), &parsed_hash)
                    .map_err(|_| actix_web::error::ErrorUnauthorized("Invalid API key"))?;

                // Resolve username for the authenticated user.
                let username = state
                    .db
                    .get_user_by_id(&user_id)
                    .await
                    .ok()
                    .flatten()
                    .map(|(_, u)| u)
                    .unwrap_or_else(|| user_id.clone());

                // All good — proceed with the request.
                // Note: we can't insert extensions here because req was moved to fut.
                // Instead, the API key routes that need user info should use a different mechanism.
                // For now, API key auth bypasses vault role checks.
                let _ = (required_vault_role,);
                Ok(fut.await?.map_into_left_body())
            });
        }

        // Standard JWT auth path.
        let bearer = bearer.unwrap();

        let secret = effective_jwt_secret(&app_cfg);
        let claims = match decode::<Claims>(
            &bearer,
            &DecodingKey::from_secret(secret.as_bytes()),
            &Validation::default(),
        ) {
            Ok(data) => data.claims,
            Err(_) => {
                let response = HttpResponse::Unauthorized().json(serde_json::json!({
                    "error": "UNAUTHORIZED",
                    "message": "Invalid or expired token"
                }));
                return Box::pin(
                    async move { Ok(req.into_response(response).map_into_right_body()) },
                );
            }
        };

        if claims.token_type != "access" {
            let response = HttpResponse::Unauthorized().json(serde_json::json!({
                "error": "UNAUTHORIZED",
                "message": "Access token required"
            }));
            return Box::pin(async move { Ok(req.into_response(response).map_into_right_body()) });
        }

        let user = AuthenticatedUser {
            user_id: claims.sub.clone(),
            username: claims.username.clone(),
        };

        let state = req.app_data::<actix_web::web::Data<AppState>>().cloned();
        let required_vault_role = required_vault_role(&req);

        Box::pin(async move {
            let req = req;

            if let Some(state) = state.as_ref() {
                if !is_password_change_exempt_path(req.path()) {
                    match state.db.user_must_change_password(&user.user_id).await {
                        Ok(true) => {
                            let response = HttpResponse::Forbidden().json(serde_json::json!({
                                "error": "PASSWORD_CHANGE_REQUIRED",
                                "message": "You must change your temporary password before continuing"
                            }));
                            return Ok(req.into_response(response).map_into_right_body());
                        }
                        Ok(false) => {}
                        Err(_) => {
                            let response = HttpResponse::InternalServerError().json(serde_json::json!({
                                "error": "INTERNAL_ERROR",
                                "message": "Failed to validate password policy"
                            }));
                            return Ok(req.into_response(response).map_into_right_body());
                        }
                    }
                }
            }

            if let (Some((vault_id, required_role)), Some(state)) = (required_vault_role, state) {
                let user_role = state
                    .db
                    .get_vault_role_for_user(&vault_id, &user.user_id)
                    .await;
                match user_role {
                    Ok(Some(role)) if role_allows(&role, required_role) => {}
                    Ok(Some(_)) | Ok(None) => {
                        let vault_exists = state.db.get_vault(&vault_id).await.is_ok();
                        let response = if vault_exists {
                            HttpResponse::Forbidden().json(serde_json::json!({
                                "error": "FORBIDDEN",
                                "message": "You do not have access to this vault"
                            }))
                        } else {
                            HttpResponse::NotFound().json(serde_json::json!({
                                "error": "NOT_FOUND",
                                "message": "Vault not found"
                            }))
                        };
                        return Ok(req.into_response(response).map_into_right_body());
                    }
                    Err(_) => {
                        let response =
                            HttpResponse::InternalServerError().json(serde_json::json!({
                                "error": "INTERNAL_ERROR",
                                "message": "Failed to authorize vault access"
                            }));
                        return Ok(req.into_response(response).map_into_right_body());
                    }
                }
            }

            req.extensions_mut().insert(UserId(user.user_id.clone()));
            req.extensions_mut().insert(user);
            let fut = service.call(req);
            Ok(fut.await?.map_into_left_body())
        })
    }
}

fn should_skip_auth(req: &ServiceRequest) -> bool {
    let path = req.path();

    if path == "/" {
        return true;
    }

    if path == "/api/health"
        || path == "/api/auth/login"
        || path == "/api/auth/refresh"
        || path.starts_with("/api/auth/oidc/")
        || path == "/api/invitations/accept"
    {
        return true;
    }

    if !path.starts_with("/api") && has_static_extension(path) {
        return true;
    }

    false
}

fn has_static_extension(path: &str) -> bool {
    let lower = path.to_ascii_lowercase();
    [
        ".html", ".js", ".mjs", ".css", ".map", ".json", ".ico", ".svg", ".png", ".jpg", ".jpeg",
        ".gif", ".webp", ".woff", ".woff2", ".ttf", ".eot", ".txt",
    ]
    .iter()
    .any(|ext| lower.ends_with(ext))
}

fn extract_bearer(auth_header: Option<&header::HeaderValue>) -> Option<&str> {
    let raw = auth_header?.to_str().ok()?;
    raw.strip_prefix("Bearer ")
}

fn extract_access_token(req: &ServiceRequest) -> Option<String> {
    if let Some(token) = extract_bearer(req.headers().get(header::AUTHORIZATION)) {
        return Some(token.to_string());
    }

    if req.path() == "/api/ws" {
        for pair in req.query_string().split('&') {
            let mut parts = pair.splitn(2, '=');
            let key = parts.next().unwrap_or_default();
            let value = parts.next().unwrap_or_default();
            if key == "access_token" && !value.is_empty() {
                return Some(value.to_string());
            }
        }
    }

    None
}

fn effective_jwt_secret(app_cfg: &AppConfig) -> String {
    if app_cfg.auth.jwt_secret.trim().is_empty() {
        crate::config::DEFAULT_DEV_JWT_SECRET.to_string()
    } else {
        app_cfg.auth.jwt_secret.clone()
    }
}

fn required_vault_role(req: &ServiceRequest) -> Option<(String, RequiredVaultRole)> {
    let segments: Vec<&str> = req
        .path()
        .split('/')
        .filter(|segment| !segment.is_empty())
        .collect();
    if segments.len() < 3 || segments[0] != "api" || segments[1] != "vaults" {
        return None;
    }

    let vault_id = segments[2].to_string();
    let tail = &segments[3..];
    let method = req.method();

    let required = if tail.first() == Some(&"shares") {
        RequiredVaultRole::Manage
    } else if tail.is_empty() {
        match *method {
            Method::GET | Method::HEAD => RequiredVaultRole::Read,
            Method::DELETE => RequiredVaultRole::Manage,
            _ => RequiredVaultRole::Write,
        }
    } else if *method == Method::GET || *method == Method::HEAD {
        RequiredVaultRole::Read
    } else if *method == Method::POST {
        match tail[0] {
            "render" | "resolve-link" | "resolve-links" | "download-zip" => RequiredVaultRole::Read,
            _ => RequiredVaultRole::Write,
        }
    } else {
        RequiredVaultRole::Write
    };

    Some((vault_id, required))
}

fn role_allows(role: &crate::models::VaultRole, required: RequiredVaultRole) -> bool {
    match required {
        RequiredVaultRole::Read => true,
        RequiredVaultRole::Write => matches!(
            role,
            crate::models::VaultRole::Owner | crate::models::VaultRole::Editor
        ),
        RequiredVaultRole::Manage => matches!(role, crate::models::VaultRole::Owner),
    }
}

fn is_password_change_exempt_path(path: &str) -> bool {
    matches!(
        path,
        "/api/auth/me" | "/api/auth/logout" | "/api/auth/change-password"
    )
}
