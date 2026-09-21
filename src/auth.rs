use axum::extract::{Request, State};
use axum::{middleware::Next, response::IntoResponse};
use redis::AsyncCommands;

use crate::identity::{hash_token, identity_from_stored};
use crate::redis_state::RedisHandle;

#[derive(Clone)]
pub struct AuthState {
    pub redis: RedisHandle,
}

impl AuthState {
    pub fn new(redis: RedisHandle) -> Self {
        Self { redis }
    }
}

pub async fn redis_auth_middleware(
    State(mut state): State<AuthState>,
    mut req: Request,
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

    let hashed = hash_token(token);

    let stored: Option<Vec<u8>> = state.redis.manager.get(&hashed).await.unwrap_or(None);
    if stored.is_none() {
        return axum::http::StatusCode::UNAUTHORIZED.into_response();
    }

    req.extensions_mut()
        .insert(identity_from_stored(stored.as_deref(), &hashed));

    next.run(req).await
}
