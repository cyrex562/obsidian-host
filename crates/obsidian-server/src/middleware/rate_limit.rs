use crate::middleware::auth::UserId;
use actix_web::body::{EitherBody, MessageBody};
use actix_web::dev::{forward_ready, Service, ServiceRequest, ServiceResponse, Transform};
use actix_web::{Error, HttpMessage, HttpResponse};
use futures::future::{ready, LocalBoxFuture, Ready};
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Mutex;
use std::time::Instant;

/// Per-IP and per-user token-bucket rate limiter.
///
/// Authenticated requests are bucketed by user ID; unauthenticated requests
/// fall back to IP address. This middleware must run *after* `AuthMiddleware`
/// in the actix-web stack so that `UserId` is already in request extensions.
///
/// Configured via environment variables or defaults:
/// - `RATE_LIMIT_REQUESTS`: max requests per window for **IP** buckets (default 120)
/// - `RATE_LIMIT_USER_REQUESTS`: max requests per window for **user** buckets (default 300)
/// - `RATE_LIMIT_WINDOW_SECS`: shared window size in seconds (default 60)
pub struct RateLimitMiddleware;

impl<S, B> Transform<S, ServiceRequest> for RateLimitMiddleware
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    B: MessageBody + 'static,
{
    type Response = ServiceResponse<EitherBody<B>>;
    type Error = Error;
    type Transform = RateLimitService<S>;
    type InitError = ();
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        let max_requests = std::env::var("RATE_LIMIT_REQUESTS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(120u64);
        let max_user_requests = std::env::var("RATE_LIMIT_USER_REQUESTS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(300u64);
        let window_secs = std::env::var("RATE_LIMIT_WINDOW_SECS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(60u64);

        ready(Ok(RateLimitService {
            service: Rc::new(service),
            buckets: Rc::new(Mutex::new(HashMap::new())),
            max_requests,
            max_user_requests,
            window_secs,
        }))
    }
}

struct BucketEntry {
    count: u64,
    window_start: Instant,
}

pub struct RateLimitService<S> {
    service: Rc<S>,
    buckets: Rc<Mutex<HashMap<String, BucketEntry>>>,
    max_requests: u64,
    max_user_requests: u64,
    window_secs: u64,
}

impl<S, B> Service<ServiceRequest> for RateLimitService<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    B: MessageBody + 'static,
{
    type Response = ServiceResponse<EitherBody<B>>;
    type Error = Error;
    type Future = LocalBoxFuture<'static, Result<Self::Response, Self::Error>>;

    forward_ready!(service);

    fn call(&self, req: ServiceRequest) -> Self::Future {
        // Skip rate limiting for health checks and static assets.
        let path = req.path().to_string();
        if path == "/" || path.starts_with("/assets") {
            let fut = self.service.call(req);
            return Box::pin(async move { Ok(fut.await?.map_into_left_body()) });
        }

        // If AuthMiddleware has already run, prefer per-user bucketing.
        // Prefix keys so IP and user buckets never collide.
        let (key, max) = if let Some(user_id) = req.extensions().get::<UserId>().cloned() {
            (format!("user:{}", user_id.0), self.max_user_requests)
        } else {
            let ip = req
                .connection_info()
                .peer_addr()
                .unwrap_or("unknown")
                .to_string();
            (format!("ip:{}", ip), self.max_requests)
        };

        let now = Instant::now();
        let window = std::time::Duration::from_secs(self.window_secs);

        let allowed = {
            let mut buckets = self.buckets.lock().unwrap();
            let entry = buckets.entry(key).or_insert(BucketEntry {
                count: 0,
                window_start: now,
            });

            // Reset window if expired.
            if now.duration_since(entry.window_start) >= window {
                entry.count = 0;
                entry.window_start = now;
            }

            entry.count += 1;
            entry.count <= max
        };

        if !allowed {
            let window_secs = self.window_secs;
            let response = HttpResponse::TooManyRequests()
                .insert_header(("Retry-After", window_secs.to_string()))
                .json(serde_json::json!({
                    "error": "RATE_LIMITED",
                    "message": format!("Rate limit exceeded. Max {} requests per {} seconds.", max, window_secs),
                }));
            return Box::pin(async move { Ok(req.into_response(response).map_into_right_body()) });
        }

        let fut = self.service.call(req);
        Box::pin(async move { Ok(fut.await?.map_into_left_body()) })
    }
}
