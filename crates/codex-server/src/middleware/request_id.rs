use actix_web::{
    dev::{forward_ready, Service, ServiceRequest, ServiceResponse, Transform},
    http::header::{HeaderName, HeaderValue},
    Error, HttpMessage,
};
use std::future::{ready, Ready};
use std::pin::Pin;
use uuid::Uuid;

static X_REQUEST_ID: HeaderName = HeaderName::from_static("x-request-id");

/// Middleware that assigns a unique `X-Request-ID` to every request.
///
/// If the client sends an `X-Request-ID` header that is a valid non-empty
/// ASCII string (≤ 128 chars) the value is passed through unchanged.
/// Otherwise a fresh UUID v4 is generated.
///
/// The chosen ID is:
/// - Stored in `req.extensions()` as a `RequestId` newtype for handlers.
/// - Echoed back in the response `X-Request-ID` header.
/// - Logged alongside every request by `RequestLogging`.
pub struct RequestIdMiddleware;

/// Newtype wrapper stored in request extensions.
#[derive(Clone, Debug)]
pub struct RequestId(pub String);

impl<S, B> Transform<S, ServiceRequest> for RequestIdMiddleware
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error>,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type InitError = ();
    type Transform = RequestIdMiddlewareService<S>;
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(RequestIdMiddlewareService { service }))
    }
}

pub struct RequestIdMiddlewareService<S> {
    service: S,
}

impl<S, B> Service<ServiceRequest> for RequestIdMiddlewareService<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error>,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Future = Pin<Box<dyn std::future::Future<Output = Result<Self::Response, Self::Error>>>>;

    forward_ready!(service);

    fn call(&self, mut req: ServiceRequest) -> Self::Future {
        // Use client-supplied ID if it's a short, printable ASCII string;
        // generate a fresh UUID otherwise.
        let id = req
            .headers()
            .get(&X_REQUEST_ID)
            .and_then(|v| v.to_str().ok())
            .filter(|s| !s.is_empty() && s.len() <= 128 && s.is_ascii())
            .map(str::to_owned)
            .unwrap_or_else(|| Uuid::new_v4().to_string());

        // Make the ID available to handlers via extensions.
        req.extensions_mut().insert(RequestId(id.clone()));

        let fut = self.service.call(req);

        Box::pin(async move {
            let mut res = fut.await?;

            // Echo the ID back in the response so clients can correlate logs.
            if let Ok(value) = HeaderValue::from_str(&id) {
                res.headers_mut().insert(X_REQUEST_ID.clone(), value);
            }

            Ok(res)
        })
    }
}
