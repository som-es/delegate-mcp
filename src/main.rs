use anyhow::Result;
use clap::Parser;
use delegate_mcp::auth::redis_auth_middleware;
use delegate_mcp::state::{AppState, SharedState};
use delegate_mcp::tools::speech::SpeechTools;
use dotenv::dotenv;
use rmcp::transport::streamable_http_server::{
    StreamableHttpService, session::local::LocalSessionManager,
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

    let shared_state: SharedState = Arc::new(RwLock::new(AppState::new().await?));
    let service = StreamableHttpService::new(
        move || Ok(SpeechTools::new(shared_state.clone())),
        LocalSessionManager::default().into(),
        Default::default(),
    );

    let mut router = axum::Router::new().nest_service(MCP_PATH, service);

    if !cli.no_auth {
        router = router.layer(axum::middleware::from_fn(redis_auth_middleware));
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
