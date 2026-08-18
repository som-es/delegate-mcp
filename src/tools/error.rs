use rmcp::{model::*, ErrorData};

pub fn internal_error(msg: &str) -> ErrorData {
    ErrorData::new(ErrorCode::INTERNAL_ERROR, msg.to_string(), None)
}

pub fn json_text<T: serde::Serialize>(value: &T) -> Result<CallToolResult, ErrorData> {
    let s = serde_json::to_string(value).map_err(|_| internal_error("serialization error"))?;
    Ok(CallToolResult::success(vec![ContentBlock::text(s)]))
}
