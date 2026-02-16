use crate::models::auth::{User, UserRole};
use crate::routes::AppState;
use actix_web::dev::Payload;
use actix_web::{web, FromRequest, HttpRequest};
use std::future::Future;
use std::pin::Pin;

/// Extractor that requires an authenticated, approved user.
///
/// Add this as a parameter to any route handler that needs authentication:
/// ```
/// async fn my_handler(user: AuthenticatedUser, ...) -> ... { }
/// ```
///
/// Returns 401 if not logged in or session expired.
/// Returns 403 if user account is pending approval.
pub struct AuthenticatedUser(pub User);

impl FromRequest for AuthenticatedUser {
    type Error = actix_web::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self, Self::Error>>>>;

    fn from_request(req: &HttpRequest, _payload: &mut Payload) -> Self::Future {
        let req = req.clone();

        Box::pin(async move {
            let state = req
                .app_data::<web::Data<AppState>>()
                .expect("AppState not configured");

            let auth = match &state.auth_service {
                Some(auth) => auth,
                None => {
                    // Auth disabled - return synthetic admin
                    return Ok(AuthenticatedUser(User {
                        id: "anonymous".to_string(),
                        email: "anonymous@localhost".to_string(),
                        name: "Anonymous".to_string(),
                        picture: None,
                        role: UserRole::Admin,
                        oidc_subject: String::new(),
                        oidc_issuer: String::new(),
                        created_at: chrono::Utc::now(),
                        updated_at: chrono::Utc::now(),
                    }));
                }
            };

            let token = req
                .cookie("obsidian_session")
                .map(|c| c.value().to_string())
                .ok_or_else(|| {
                    crate::error::AppError::Unauthorized("Not logged in".into())
                })?;

            let (_session, user) = auth
                .validate_session(&state.db, &token)
                .await
                .map_err(|_| {
                    crate::error::AppError::Unauthorized("Session validation failed".into())
                })?
                .ok_or_else(|| {
                    crate::error::AppError::Unauthorized(
                        "Invalid or expired session".into(),
                    )
                })?;

            if user.role == UserRole::Pending {
                return Err(crate::error::AppError::Forbidden(
                    "Your account is pending admin approval".into(),
                )
                .into());
            }

            if user.role == UserRole::Suspended {
                return Err(crate::error::AppError::Forbidden(
                    "Your account has been suspended".into(),
                )
                .into());
            }

            Ok(AuthenticatedUser(user))
        })
    }
}

/// Extractor that requires an admin user.
///
/// Returns 401 if not logged in, 403 if not admin.
pub struct AdminUser(pub User);

impl FromRequest for AdminUser {
    type Error = actix_web::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self, Self::Error>>>>;

    fn from_request(req: &HttpRequest, payload: &mut Payload) -> Self::Future {
        let auth_fut = AuthenticatedUser::from_request(req, payload);

        Box::pin(async move {
            let AuthenticatedUser(user) = auth_fut.await?;

            if user.role != UserRole::Admin {
                return Err(
                    crate::error::AppError::Forbidden("Admin access required".into()).into(),
                );
            }

            Ok(AdminUser(user))
        })
    }
}
