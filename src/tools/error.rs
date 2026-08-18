use std::borrow::Cow;
use rmcp::{Error as McpError, model::*, model::ErrorCode};

pub fn internal_error(msg: &str) -> McpError {
    McpError::new(ErrorCode::INTERNAL_ERROR, Cow::Owned(msg.to_string()), None)
}

pub fn json_text<T: serde::Serialize>(value: &T) -> Result<CallToolResult, McpError> {
    let s = serde_json::to_string(value).map_err(|_| internal_error("serialization error"))?;
    Ok(CallToolResult::success(vec![Content::text(s)]))
}
