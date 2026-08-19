use axum::extract::{Request, State};
use axum::{middleware::Next, response::IntoResponse};
use redis::AsyncCommands;
use sha3::{Digest, Sha3_256};

use crate::redis_state::RedisHandle;

pub async fn redis_auth_middleware(
    State(mut state): State<RedisHandle>,
    req: Request,
    next: Next,
) -> impl IntoResponse {
    let Some(auth_header) = req.headers().get(axum::http::header::AUTHORIZATION) else {
        return axum::http::StatusCode::UNAUTHORIZED.into_response();
    };

    let Ok(auth_str) = auth_header.to_str() else {
        return axum::http::StatusCode::UNAUTHORIZED.into_response();
    };

    let Some(token) = auth_str.strip_prefix("Bearer ") else {
        return axum::http::StatusCode::UNAUTHORIZED.into_response();
    };

    let mut hasher = Sha3_256::new();
    hasher.update(token.as_bytes());
    let hashed = hex::encode(hasher.finalize());

    // Use shared ConnectionManager
    let exists: bool = state.manager.exists(&hashed).await.unwrap_or(false);
    if !exists {
        return axum::http::StatusCode::UNAUTHORIZED.into_response();
    }

    next.run(req).await
}
