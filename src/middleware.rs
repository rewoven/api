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

pub type KeyedLimiter = RateLimiter<String, DefaultKeyedStateStore<String>, DefaultClock>;

pub fn new_limiter(per_minute: u32) -> Arc<KeyedLimiter> {
    let quota = Quota::per_minute(NonZeroU32::new(per_minute.max(1)).unwrap());
    Arc::new(RateLimiter::keyed(quota))
}

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

pub async fn cache_and_etag(req: Request<Body>, next: Next) -> Response {
    let is_get = req.method() == axum::http::Method::GET;
    let inm = req
        .headers()
        .get(header::IF_NONE_MATCH)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());

    let resp = next.run(req).await;

    if !is_get || resp.status() != StatusCode::OK {
        return resp;
    }

    let (mut parts, body) = resp.into_parts();
    let bytes = match axum::body::to_bytes(body, usize::MAX).await {
        Ok(b) => b,
        Err(_) => return Response::from_parts(parts, Body::empty()),
    };

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

    if inm.as_deref() == Some(etag_value.as_str()) {
        parts.status = StatusCode::NOT_MODIFIED;
        return Response::from_parts(parts, Body::empty());
    }

    Response::from_parts(parts, Body::from(bytes))
}
