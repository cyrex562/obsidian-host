use actix_web::{
    dev::{forward_ready, Service, ServiceRequest, ServiceResponse, Transform},
    Error,
};
use std::future::{ready, Ready};
use std::pin::Pin;
use std::time::Instant;
use tracing::{info, warn};

/// Middleware for logging API requests with detailed context
pub struct RequestLogging;

impl<S, B> Transform<S, ServiceRequest> for RequestLogging
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error>,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type InitError = ();
    type Transform = RequestLoggingMiddleware<S>;
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(RequestLoggingMiddleware { service }))
    }
}

pub struct RequestLoggingMiddleware<S> {
    service: S,
}

impl<S, B> Service<ServiceRequest> for RequestLoggingMiddleware<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error>,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Future = Pin<Box<dyn std::future::Future<Output = Result<Self::Response, Self::Error>>>>;

    forward_ready!(service);

    fn call(&self, req: ServiceRequest) -> Self::Future {
        let start = Instant::now();
        let method = req.method().to_string();
        let path = req.path().to_string();
        let query = req.query_string().to_string();
        let remote_addr = req.peer_addr().map(|addr| addr.to_string());

        // Extract user agent if present
        let user_agent = req
            .headers()
            .get("user-agent")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("unknown")
            .to_string();

        let fut = self.service.call(req);

        Box::pin(async move {
            let res = fut.await?;
            let duration = start.elapsed();
            let status = res.status();

            // Log the request with context
            if status.is_success() {
                info!(
                    method = %method,
                    path = %path,
                    query = %query,
                    status = %status.as_u16(),
                    duration_ms = %duration.as_millis(),
                    remote_addr = ?remote_addr,
                    user_agent = %user_agent,
                    "API request completed"
                );
            } else if status.is_client_error() || status.is_server_error() {
                warn!(
                    method = %method,
                    path = %path,
                    query = %query,
                    status = %status.as_u16(),
                    duration_ms = %duration.as_millis(),
                    remote_addr = ?remote_addr,
                    user_agent = %user_agent,
                    "API request failed"
                );
            }

            Ok(res)
        })
    }
}
