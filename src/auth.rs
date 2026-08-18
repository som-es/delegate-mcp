use axum::extract::Request;
use axum::{middleware::Next, response::IntoResponse};
use redis::AsyncCommands;
use sha3::{Digest, Sha3_256};

fn redis_url_with_db() -> String {
    std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://127.0.0.1:6379/255".to_string())
}

pub async fn redis_auth_middleware(req: Request, next: Next) -> impl IntoResponse {
    let Some(auth_header) = req.headers().get(axum::http::header::AUTHORIZATION) else {
        return axum::http::StatusCode::UNAUTHORIZED.into_response();
    };

    let Ok(auth_str) = auth_header.to_str() else {
        return axum::http::StatusCode::UNAUTHORIZED.into_response();
    };

    let Some(token) = auth_str.strip_prefix("Bearer ") else {
        return axum::http::StatusCode::UNAUTHORIZED.into_response();
    };

    let redis_url = redis_url_with_db();
    let Ok(client) = redis::Client::open(redis_url) else {
        return axum::http::StatusCode::UNAUTHORIZED.into_response();
    };

    let Ok(mut conn) = client.get_multiplexed_async_connection().await else {
        return axum::http::StatusCode::UNAUTHORIZED.into_response();
    };

    let mut hasher = Sha3_256::new();
    hasher.update(token.as_bytes());
    let hashed = hex::encode(hasher.finalize());
    // Token existence check
    let exists: bool = conn.exists(&hashed).await.unwrap_or(false);
    if !exists {
        return axum::http::StatusCode::UNAUTHORIZED.into_response();
    }
    // Rate limiting: 15 per hour per token
    let rate_key = format!("rate:token:{}", hashed);
    let count: i64 = conn.incr(&rate_key, 1).await.unwrap_or(0);
    if count == 1 {
        let _: Result<(), redis::RedisError> = conn.expire(&rate_key, 3600).await;
    }
    if count > 15 {
        return axum::http::StatusCode::TOO_MANY_REQUESTS.into_response();
    }
    next.run(req).await
}
