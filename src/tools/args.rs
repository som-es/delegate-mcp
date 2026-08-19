use rmcp::schemars;
use serde::Deserialize;

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct RagQueryArgs {
    pub question: String,
    pub delegate_id: i32,
    #[serde(default = "default_limit")]
    pub limit: u64,
    pub parliament: Parliament,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
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
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct DelSearchArgs {
    pub delegate_search_terms: String,
    pub parliament: Parliament,
    #[serde(default = "default_one")]
    pub page: u32,
    #[serde(default = "default_limit_del")]
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
