pub mod auth;
pub mod rate_limit;
pub mod state;
pub mod tools;
pub mod redis_state;

pub fn redis_url_with_db() -> String {
    std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://127.0.0.1:6379/255".to_string())
}
