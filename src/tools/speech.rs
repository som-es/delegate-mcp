use rmcp::{ServerHandler, model::*, tool};

use crate::state::SharedState;
use crate::tools::args::{DelSearchArgs, RagQueryArgs};
use crate::tools::error::{internal_error, json_text};

#[derive(Clone)]
pub struct SpeechTools {
    state: SharedState,
}

#[tool(tool_box)]
impl SpeechTools {
    pub fn new(state: SharedState) -> Self {
        Self { state }
    }

    #[tool(description = "RAG query over speeches")]
    async fn rag_query_speeches(
        &self,
        #[tool(aggr)] args: RagQueryArgs,
    ) -> Result<CallToolResult, rmcp::Error> {
        let limit = args.limit.min(35);
        let state = self.state.read().await;
        let points = dataservice::political_analysis::delegate_context::search_points_for_entry(
            &state.qdrant_client,
            &state.somai_client,
            args.delegate_id,
            &args.question,
            limit,
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

    #[tool(description = "Search over delegates")]
    async fn delegate_search(
        &self,
        #[tool(aggr)] args: DelSearchArgs,
    ) -> Result<CallToolResult, rmcp::Error> {
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

#[tool(tool_box)]
impl ServerHandler for SpeechTools {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            protocol_version: ProtocolVersion::V_2024_11_05,
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            server_info: Implementation::from_build_env(),
            instructions: Some("Tools for searching parliamentary speeches and RAG.".into()),
        }
    }
}
