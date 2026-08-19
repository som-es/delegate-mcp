use rmcp::{
    ErrorData, ServerHandler, handler::server::wrapper::Parameters, model::*, tool, tool_handler,
    tool_router,
};

use crate::state::SharedState;
use crate::tools::args::{DelSearchArgs, RagQueryArgs};
use crate::tools::error::{internal_error, json_text};

pub const TOOL_RAG_QUERY_SPEECHES: &str = "rag_query_speeches";
pub const TOOL_DELEGATE_SEARCH: &str = "delegate_search";

#[derive(Clone)]
pub struct SpeechTools {
    state: SharedState,
}

#[tool_router]
impl SpeechTools {
    pub fn new(state: SharedState) -> Self {
        Self { state }
    }

    #[tool(name = "rag_query_speeches", description = "RAG query over speeches")]
    async fn rag_query_speeches(
        &self,
        Parameters(args): Parameters<RagQueryArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        let limit = args.limit.min(35);
        let state = self.state.read().await;
        let points = dataservice::political_analysis::delegate_context::search_points_for_entry(
            &state.qdrant_client,
            &state.somai_client,
            args.delegate_id,
            &args.question,
            limit,
            match args.parliament {
                super::args::Parliament::At => dataservice::combx::Parliament::At,
                super::args::Parliament::Eu => dataservice::combx::Parliament::Eu,
            },
        )
        .await
        .map_err(|e| {
            tracing::error!("Qdrant search failed: {e}");
            internal_error("cannot search for points in qdrant")
        })?
        .into_iter()
        .map(|point| (point.score, point.payload))
        .collect::<Vec<_>>();

        json_text(&points)
    }

    #[tool(name = "delegate_search", description = "Search over delegates")]
    async fn delegate_search(
        &self,
        Parameters(args): Parameters<DelSearchArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        let url = format!(
            "https://somes.at/api/{}/v1/delegates/search?page={}&search={}&entries_per_page={}",
            args.parliament.as_str(),
            args.page,
            args.delegate_search_terms,
            args.limit
        );
        let value = reqwest::get(&url)
            .await
            .map_err(|e| {
                tracing::error!("HTTP request failed: {e}");
                internal_error("internal error")
            })?
            .json::<serde_json::Value>()
            .await
            .map_err(|e| {
                tracing::error!("JSON decode failed: {e}");
                internal_error("internal error")
            })?;

        json_text(&value)
    }
}

#[tool_handler]
impl ServerHandler for SpeechTools {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::from_build_env())
            .with_protocol_version(ProtocolVersion::V_2024_11_05)
            .with_instructions("Tools for searching parliamentary speeches and RAG.".to_string())
    }
}
