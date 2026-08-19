use axum::body::to_bytes;
use axum::extract::{Request, State};
use axum::{middleware::Next, response::IntoResponse};
use redis::AsyncCommands;
use serde_json::Value;
use sha3::{Digest, Sha3_256};
use std::collections::HashMap;

use crate::redis_state::RedisHandle;

#[derive(Clone)]
pub struct RateLimitState {
    pub redis: RedisHandle,
    pub limits: HashMap<String, i64>,
}

impl RateLimitState {
    pub fn new(redis: RedisHandle, limits: HashMap<String, i64>) -> Self {
        Self { redis, limits }
    }
}

pub async fn rate_limit_middleware(
    State(mut state): State<RateLimitState>,
    req: Request,
    next: Next,
) -> impl IntoResponse {
    // Extract token
    let token_opt = req
        .headers()
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer "))
        .map(|s| s.to_string());

    // Buffer body to inspect method
    let (parts, body) = req.into_parts();
    let bytes = match to_bytes(body, usize::MAX).await {
        Ok(b) => b,
        Err(_) => return axum::http::StatusCode::BAD_REQUEST.into_response(),
    };

    let method_name = if let Ok(json) = serde_json::from_slice::<Value>(&bytes) {
        if let Some(params) = json.get("params") {
            params.get("name").and_then(|v| v.as_str()).map(|s| s.to_string())
        } else {
            json.get("method").and_then(|v| v.as_str()).map(|s| s.to_string())
        }
    } else {
        None
    };

    if let Some(method) = method_name {
        if let Some(&limit) = state.limits.get(&method) {
            if let Some(token) = token_opt {
                let mut hasher = Sha3_256::new();
                hasher.update(token.as_bytes());
                let hashed = hex::encode(hasher.finalize());
                let rate_key = format!("rate:{method}:{hashed}");
                let count: i64 = state.redis.manager.incr(&rate_key, 1).await.unwrap_or(0);
                if count == 1 {
                    let _: Result<(), redis::RedisError> = state.redis.manager.expire(&rate_key, 3600).await;
                }
                if count > limit {
                    return axum::http::StatusCode::TOO_MANY_REQUESTS.into_response();
                }
            }
        }
    }

    // Reconstruct request
    let body = axum::body::Body::from(bytes);
    let req = Request::from_parts(parts, body);
    next.run(req).await
}
