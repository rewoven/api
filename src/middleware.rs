//! HTTP middleware: per-IP rate limiting + response caching (Cache-Control + ETag).

use std::num::NonZeroU32;
use std::sync::Arc;

use axum::{
    body::Body,
    extract::State,
    http::{header, HeaderValue, Request, StatusCode},
    middleware::Next,
    response::Response,
};
use governor::{clock::DefaultClock, state::keyed::DefaultKeyedStateStore, Quota, RateLimiter};

/// Per-key (per-IP) rate limiter.
pub type KeyedLimiter = RateLimiter<String, DefaultKeyedStateStore<String>, DefaultClock>;

/// Build a limiter allowing `per_minute` requests per IP (with burst = per_minute).
pub fn new_limiter(per_minute: u32) -> Arc<KeyedLimiter> {
    let quota = Quota::per_minute(NonZeroU32::new(per_minute.max(1)).unwrap());
    Arc::new(RateLimiter::keyed(quota))
}

/// Extract the real client IP. The API sits behind nginx/Cloudflare, so the
/// socket peer is the proxy — we trust the standard forwarded headers instead.
fn client_key(req: &Request<Body>) -> String {
    let h = req.headers();
    if let Some(v) = h.get("cf-connecting-ip").and_then(|v| v.to_str().ok()) {
        return v.trim().to_string();
    }
    if let Some(v) = h.get("x-forwarded-for").and_then(|v| v.to_str().ok()) {
        if let Some(first) = v.split(',').next() {
            return first.trim().to_string();
        }
    }
    "unknown".to_string()
}

/// Reject requests from an IP that has exceeded its quota with 429.
pub async fn rate_limit(
    State(limiter): State<Arc<KeyedLimiter>>,
    req: Request<Body>,
    next: Next,
) -> Response {
    let key = client_key(&req);
    if limiter.check_key(&key).is_err() {
        let mut resp = Response::new(Body::from(
            r#"{"error":"Rate limit exceeded. Try again shortly.","status":429}"#,
        ));
        *resp.status_mut() = StatusCode::TOO_MANY_REQUESTS;
        resp.headers_mut().insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/json"),
        );
        resp.headers_mut()
            .insert(header::RETRY_AFTER, HeaderValue::from_static("30"));
        return resp;
    }
    next.run(req).await
}

/// Add `Cache-Control` + a content-hash `ETag` to GET responses, and short-circuit
/// to `304 Not Modified` when the client's `If-None-Match` already matches.
/// The brand/material data is static between deploys, so this saves real bandwidth.
pub async fn cache_and_etag(req: Request<Body>, next: Next) -> Response {
    let is_get = req.method() == axum::http::Method::GET;
    let inm = req
        .headers()
        .get(header::IF_NONE_MATCH)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());

    let resp = next.run(req).await;

    // Only cache successful GETs.
    if !is_get || resp.status() != StatusCode::OK {
        return resp;
    }

    let (mut parts, body) = resp.into_parts();
    let bytes = match axum::body::to_bytes(body, usize::MAX).await {
        Ok(b) => b,
        Err(_) => return Response::from_parts(parts, Body::empty()),
    };

    // Content hash → weak ETag.
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    bytes.as_ref().hash(&mut hasher);
    let etag_value = format!("\"{:016x}\"", hasher.finish());

    parts.headers.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("public, max-age=300"),
    );
    if let Ok(v) = HeaderValue::from_str(&etag_value) {
        parts.headers.insert(header::ETAG, v);
    }

    // Client already has this exact body → 304, no payload.
    if inm.as_deref() == Some(etag_value.as_str()) {
        parts.status = StatusCode::NOT_MODIFIED;
        return Response::from_parts(parts, Body::empty());
    }

    Response::from_parts(parts, Body::from(bytes))
}
