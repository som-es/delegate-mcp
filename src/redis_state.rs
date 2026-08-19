use redis::aio::ConnectionManager;

#[derive(Clone)]
pub struct RedisHandle {
    pub client: redis::Client,
    pub manager: ConnectionManager,
}

impl RedisHandle {
    pub fn new(client: redis::Client, manager: ConnectionManager) -> Self {
        Self { client, manager }
    }
}
