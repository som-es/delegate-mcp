use anyhow::Result;
use delegate_mcp::state::{AppState, SharedState};
use delegate_mcp::tools::speech::SpeechTools;
use dotenv::dotenv;
use rmcp::transport::sse_server::{SseServer, SseServerConfig};
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;
use tracing_subscriber::{self, EnvFilter};

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive(tracing::Level::INFO.into()))
        .init();

    dotenv()?;

    tracing::info!("Starting Delegate MCP SSE");

    let shared_state: SharedState = Arc::new(RwLock::new(AppState::new().await?));
    let speech_tools = SpeechTools::new(shared_state);

    let bind = "0.0.0.0:8080".parse()?;
    let config = SseServerConfig {
        bind,
        sse_path: "/mcp".to_string(),
        post_path: "/mcp/message".to_string(),
        ct: CancellationToken::new(),
    };
    let sse_server = SseServer::serve_with_config(config).await?;
    sse_server.with_service(move || speech_tools.clone());

    tracing::info!("MCP SSE listening on http://0.0.0.0:8080/mcp");
    tracing::info!("POST messages to /mcp/message?sessionId=...");
    tokio::signal::ctrl_c().await?;
    tracing::info!("Shutting down");
    Ok(())
}
