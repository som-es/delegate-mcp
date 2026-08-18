use std::borrow::Cow;

use rmcp::{Error as McpError, ServerHandler, model::*, schemars, tool};

use crate::state::SharedState;

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
    ) -> Result<CallToolResult, McpError> {
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
        .map_err(|_e| {
            McpError::new(
                ErrorCode::INTERNAL_ERROR,
                Cow::Borrowed("cannot search for points in qdrant"),
                None,
            )
        })?
        .into_iter()
        .map(|point| (point.score, point.payload))
        .collect::<Vec<_>>();

        let out = serde_json::to_string(&points).map_err(|_e| {
            McpError::new(
                ErrorCode::INTERNAL_ERROR,
                Cow::Borrowed("internal error"),
                None,
            )
        })?;

        Ok(CallToolResult::success(vec![Content::text(out)]))
    }

    #[tool(description = "Search over delegates")]
    async fn delegate_search(
        &self,
        #[tool(aggr)] args: DelSearchArgs,
    ) -> Result<CallToolResult, McpError> {
        let out = reqwest::get(&format!(
            "https://somes.at/api/{}/v1/delegates/search?page={}&search={}&entries_per_page={}",
            args.parliament.as_str(),
            args.page,
            args.delegate_search_terms,
            args.limit
        ))
        .await
        .map_err(|_e| {
            McpError::new(
                ErrorCode::INTERNAL_ERROR,
                Cow::Borrowed("internal error"),
                None,
            )
        })?
        .json::<serde_json::Value>()
        .await
        .map_err(|_e| {
            McpError::new(
                ErrorCode::INTERNAL_ERROR,
                Cow::Borrowed("internal error"),
                None,
            )
        })?;

        let out = serde_json::to_string(&out).map_err(|_e| {
            McpError::new(
                ErrorCode::INTERNAL_ERROR,
                Cow::Borrowed("internal error"),
                None,
            )
        })?;
        Ok(CallToolResult::success(vec![Content::text(out)]))
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

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct GetSpeechArgs {
    pub speech_id: i32,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct RagQueryArgs {
    pub question: String,
    pub delegate_id: i32,
    #[serde(default = "default_limit")]
    pub limit: u64,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub enum Parliament {
    At,
    Eu,
}
impl Parliament {
    pub fn as_str(&self) -> &str {
        match self {
            Parliament::At => "at",
            Parliament::Eu => "eu",
        }
    }

    pub fn from_council(council: &str) -> Parliament {
        match council {
            "ep" => Parliament::Eu,
            _ => Parliament::At,
        }
    }
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct DelSearchArgs {
    pub delegate_search_terms: String,
    pub parliament: Parliament,
    #[serde(default = "default_one")]
    #[schemars(range(min = 1))]
    pub page: u32,
    #[serde(default = "default_limit_del")]
    #[schemars(range(min = 1))]
    pub limit: u32,
}

fn default_limit() -> u64 {
    10
}

fn default_one() -> u32 {
    1
}

fn default_limit_del() -> u32 {
    3
}
