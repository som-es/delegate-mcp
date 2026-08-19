use anyhow::Result;
use clap::Parser;
use delegate_mcp::auth::redis_auth_middleware;
use delegate_mcp::rate_limit::{RateLimitState, rate_limit_middleware};
use delegate_mcp::redis_state::RedisHandle;
use delegate_mcp::state::{AppState, SharedState};
use delegate_mcp::tools::speech::{SpeechTools, TOOL_DELEGATE_SEARCH, TOOL_RAG_QUERY_SPEECHES};
use dotenv::dotenv;
use rmcp::transport::streamable_http_server::{
    StreamableHttpServerConfig, StreamableHttpService, session::local::LocalSessionManager,
};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing_subscriber::{self, EnvFilter};

#[derive(Parser, Debug)]
struct Cli {
    /// Disable authentication
    #[arg(long, default_value_t = false)]
    no_auth: bool,
}

const MCP_PATH: &str = "/mcp";

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive(tracing::Level::INFO.into()))
        .init();

    dotenv()?;

    let cli = Cli::parse();

    tracing::info!("Starting Delegate MCP Streamable HTTP");

    // Redis state for auth & rate limiting
    let redis_url =
        std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://127.0.0.1:6379/255".to_string());
    let redis_client = redis::Client::open(redis_url)?;
    let redis_manager = redis::aio::ConnectionManager::new(redis_client.clone()).await?;
    let redis_handle = RedisHandle::new(redis_client, redis_manager);

    // Rate limits per tool method
    let mut limits = std::collections::HashMap::new();
    let rag_limit = std::env::var("RAG_RATE_LIMIT")
        .ok()
        .and_then(|v| v.parse::<i64>().ok())
        .unwrap_or(15);
    limits.insert(TOOL_RAG_QUERY_SPEECHES.to_string(), rag_limit);

    let rate_limit_state = RateLimitState::new(redis_handle.clone(), limits);

    let shared_state: SharedState = Arc::new(RwLock::new(AppState::new().await?));
    let server_config = StreamableHttpServerConfig::default().with_allowed_hosts([
        "localhost",
        "127.0.0.1",
        "::1",
        "somes.at:3001",
        "somes.at",
    ]);
    let service = StreamableHttpService::new(
        move || Ok(SpeechTools::new(shared_state.clone())),
        LocalSessionManager::default().into(),
        server_config,
    );

    let mut router = axum::Router::new().nest_service(MCP_PATH, service);

    // Rate limiting middleware with configurable limits
    router = router.layer(axum::middleware::from_fn_with_state(
        rate_limit_state.clone(),
        rate_limit_middleware,
    ));

    if !cli.no_auth {
        router = router.layer(axum::middleware::from_fn_with_state(
            redis_handle.clone(),
            redis_auth_middleware,
        ));
        tracing::info!("Redis token validation enabled");
    } else {
        tracing::info!("Authentication disabled via --no-auth");
    }

    let bind_addr = std::env::var("ADDR")?;
    let listener = tokio::net::TcpListener::bind(&bind_addr).await?;
    tracing::info!("MCP Streamable HTTP listening on http://{bind_addr}{MCP_PATH}");

    axum::serve(listener, router)
        .with_graceful_shutdown(async {
            tokio::signal::ctrl_c().await.ok();
        })
        .await?;

    tracing::info!("Shutting down");
    Ok(())
}
