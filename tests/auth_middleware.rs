//! Integration tests for the MCP auth + rate limit middleware.
//!
//! These tests need a reachable Redis server (`MCP_TEST_REDIS_URL`, default
//! `redis://127.0.0.1:6379/254`). They are skipped when Redis is unavailable.

use axum::{Router, body::Body, extract::Request, http::StatusCode, middleware, routing::get};
use delegate_mcp::auth::{AuthState, redis_auth_middleware};
use delegate_mcp::identity::hash_token;
use delegate_mcp::rate_limit::{RateLimitState, rate_limit_middleware};
use delegate_mcp::redis_state::RedisHandle;
use redis::AsyncCommands;
use std::collections::HashMap;
use tower::ServiceExt;

const TEST_DB: &str = "254";
const RAG_METHOD: &str = "rag_query_speeches";

fn redis_url() -> String {
    std::env::var("MCP_TEST_REDIS_URL")
        .unwrap_or_else(|_| format!("redis://127.0.0.1:6379/{TEST_DB}"))
}

async fn conn() -> Option<redis::aio::ConnectionManager> {
    let client = redis::Client::open(redis_url()).ok()?;
    redis::aio::ConnectionManager::new(client).await.ok()
}

async fn handle() -> &'static str {
    "ok"
}

async fn router() -> Option<Router> {
    let client = redis::Client::open(redis_url()).ok()?;
    let manager = redis::aio::ConnectionManager::new(client.clone())
        .await
        .ok()?;
    let state = AuthState::new(RedisHandle::new(client, manager));

    Some(
        Router::new()
            .route("/mcp", get(handle).post(handle))
            .layer(middleware::from_fn_with_state(state, redis_auth_middleware)),
    )
}

async fn rate_limited_router(limit: i64) -> Option<Router> {
    let client = redis::Client::open(redis_url()).ok()?;
    let manager = redis::aio::ConnectionManager::new(client.clone())
        .await
        .ok()?;
    let redis_handle = RedisHandle::new(client.clone(), manager);

    let mut limits = HashMap::new();
    limits.insert(RAG_METHOD.to_string(), limit);

    Some(
        Router::new()
            .route("/mcp", get(handle).post(handle))
            .layer(middleware::from_fn_with_state(
                RateLimitState::new(redis_handle.clone(), limits),
                rate_limit_middleware,
            ))
            .layer(middleware::from_fn_with_state(
                AuthState::new(redis_handle),
                redis_auth_middleware,
            )),
    )
}

async fn status(router: Router, auth: Option<&str>) -> StatusCode {
    let mut builder = Request::builder().uri("/mcp");
    if let Some(auth) = auth {
        builder = builder.header(axum::http::header::AUTHORIZATION, auth);
    }
    router
        .oneshot(builder.body(Body::empty()).unwrap())
        .await
        .unwrap()
        .status()
}

async fn rag_call_status(router: Router, token: &str) -> StatusCode {
    let req = Request::builder()
        .uri("/mcp")
        .method("POST")
        .header(axum::http::header::AUTHORIZATION, format!("Bearer {token}"))
        .body(Body::from(
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": "tools/call",
                "params": { "name": RAG_METHOD }
            })
            .to_string(),
        ))
        .unwrap();
    router.oneshot(req).await.unwrap().status()
}

async fn issue_legacy_token(token: &str) -> Option<String> {
    let mut c = conn().await?;
    let hashed = hash_token(token);
    let _: () = c.set(&hashed, "").await.ok()?;
    Some(hashed)
}

async fn issue_token_for_user(token: &str, user_id: &str) -> Option<String> {
    let mut c = conn().await?;
    let hashed = hash_token(token);
    let _: () = c.set(&hashed, user_id).await.ok()?;
    Some(hashed)
}

async fn delete_keys(keys: &[String]) {
    let Some(mut c) = conn().await else { return };
    for key in keys {
        let _: () = redis::cmd("DEL")
            .arg(key)
            .query_async(&mut c)
            .await
            .unwrap_or(());
    }
}

async fn flush_rate_keys(identities: &[&str]) {
    let Some(mut c) = conn().await else { return };
    for identity in identities {
        let key = format!("rate:{RAG_METHOD}:{identity}");
        let _: () = redis::cmd("DEL")
            .arg(key)
            .query_async(&mut c)
            .await
            .unwrap_or(());
    }
}

#[tokio::test]
async fn missing_authorization_header_is_rejected() {
    let Some(router) = router().await else {
        eprintln!("skipping: redis not reachable at {}", redis_url());
        return;
    };
    assert_eq!(status(router, None).await, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn unknown_token_is_rejected() {
    let Some(router) = router().await else {
        eprintln!("skipping: redis not reachable at {}", redis_url());
        return;
    };
    assert_eq!(
        status(router, Some("Bearer mcp_not_a_real_token")).await,
        StatusCode::UNAUTHORIZED
    );
}

#[tokio::test]
async fn redis_mcp_token_authenticates_without_oauth() {
    let Some(router) = router().await else {
        eprintln!("skipping: redis not reachable at {}", redis_url());
        return;
    };
    let token = "mcp_test_token_valid";
    let key = issue_legacy_token(token).await;
    let out = status(router, Some(&format!("Bearer {token}"))).await;
    if let Some(key) = key {
        delete_keys(&[key]).await;
    }
    assert_eq!(out, StatusCode::OK);
}

#[tokio::test]
async fn jwt_is_not_accepted_while_oauth_is_disabled() {
    unsafe { std::env::set_var("OAUTH_JWT_SECRET", "test-secret") };
    unsafe { std::env::set_var("MCP_RESOURCE_URI", "https://mcp.example.com/mcp") };

    let Some(router) = router().await else {
        eprintln!("skipping: redis not reachable at {}", redis_url());
        return;
    };

    let claims = serde_json::json!({
        "sub": "42",
        "aud": "https://mcp.example.com/mcp",
        "exp": usize::MAX / 2,
        "iat": 1,
    });
    let token = jsonwebtoken::encode(
        &jsonwebtoken::Header::new(jsonwebtoken::Algorithm::HS256),
        &claims,
        &jsonwebtoken::EncodingKey::from_secret(b"test-secret"),
    )
    .unwrap();

    assert_eq!(
        status(router, Some(&format!("Bearer {token}"))).await,
        StatusCode::UNAUTHORIZED
    );
}

#[tokio::test]
async fn rate_limit_is_shared_between_tokens_of_the_same_user() {
    let Some(router) = rate_limited_router(1).await else {
        eprintln!("skipping: redis not reachable at {}", redis_url());
        return;
    };
    flush_rate_keys(&["77"]).await;

    let token_a = "mcp_rate_test_token_a";
    let token_b = "mcp_rate_test_token_b";
    let mut keys = Vec::new();
    let Some(k1) = issue_token_for_user(token_a, "77").await else {
        return;
    };
    let Some(k2) = issue_token_for_user(token_b, "77").await else {
        return;
    };
    keys.append(&mut vec![k1, k2]);

    let first = rag_call_status(router.clone(), token_a).await;
    let second = rag_call_status(router.clone(), token_b).await;
    let third = rag_call_status(router.clone(), token_a).await;

    delete_keys(&keys).await;
    flush_rate_keys(&["77"]).await;

    assert_eq!(first, StatusCode::OK, "first call is allowed");
    assert_eq!(
        second,
        StatusCode::TOO_MANY_REQUESTS,
        "recreated token of the same user must not reset the limit"
    );
    assert_eq!(third, StatusCode::TOO_MANY_REQUESTS);
}

#[tokio::test]
async fn rate_limit_is_isolated_between_users() {
    let Some(router) = rate_limited_router(1).await else {
        eprintln!("skipping: redis not reachable at {}", redis_url());
        return;
    };
    flush_rate_keys(&["1", "2"]).await;

    let mut keys = Vec::new();
    let Some(k1) = issue_token_for_user("mcp_rate_test_user_a", "1").await else {
        return;
    };
    let Some(k2) = issue_token_for_user("mcp_rate_test_user_b", "2").await else {
        return;
    };
    keys.append(&mut vec![k1, k2]);

    let first = rag_call_status(router.clone(), "mcp_rate_test_user_a").await;
    let second = rag_call_status(router.clone(), "mcp_rate_test_user_b").await;

    delete_keys(&keys).await;
    flush_rate_keys(&["1", "2"]).await;

    assert_eq!(first, StatusCode::OK);
    assert_eq!(second, StatusCode::OK, "other user must not be limited");
}

#[tokio::test]
async fn legacy_tokens_are_still_rate_limited() {
    let Some(router) = rate_limited_router(1).await else {
        eprintln!("skipping: redis not reachable at {}", redis_url());
        return;
    };
    let legacy_identity = hash_token("mcp_rate_test_legacy");
    flush_rate_keys(&[&legacy_identity]).await;

    let key = issue_legacy_token("mcp_rate_test_legacy").await;
    let first = rag_call_status(router.clone(), "mcp_rate_test_legacy").await;
    let second = rag_call_status(router.clone(), "mcp_rate_test_legacy").await;
    if let Some(key) = key {
        delete_keys(&[key]).await;
    }
    flush_rate_keys(&[&legacy_identity]).await;

    assert_eq!(first, StatusCode::OK);
    assert_eq!(second, StatusCode::TOO_MANY_REQUESTS);
}
