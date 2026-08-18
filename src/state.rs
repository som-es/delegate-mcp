use qdrant_client::Qdrant;
use somai_client::SomAiClient;
use std::{env, sync::Arc};
use tokio::sync::RwLock;

pub struct AppState {
    pub somai_client: SomAiClient,
    pub qdrant_client: Qdrant,
}

impl AppState {
    pub async fn new() -> anyhow::Result<Self> {
        let somai_client =
            SomAiClient::new(&env::var("SOMAI_URL")?, "API_USER", &env::var("SOMAI_KEY")?).await?;
        let qdrant_client = Qdrant::from_url(&env::var("QDRANT_URL")?).build()?;
        Ok(Self {
            somai_client,
            qdrant_client,
        })
    }
}

pub type SharedState = Arc<RwLock<AppState>>;
