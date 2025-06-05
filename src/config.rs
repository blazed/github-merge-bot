// config.rs
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::env;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub github_token: String,
    pub webhook_secret: String,
    pub database_url: String,
    pub bind_address: String,
    pub bot_name: String,
}

impl Config {
    pub fn load() -> Result<Self> {
        Ok(Config {
            github_token: env::var("GITHUB_TOKEN")
                .map_err(|_| anyhow::anyhow!("GITHUB_TOKEN not set"))?,
            webhook_secret: env::var("WEBHOOK_SECRET")
                .map_err(|_| anyhow::anyhow!("WEBHOOK_SECRET not set"))?,
            database_url: env::var("DATABASE_URL")
                .unwrap_or_else(|_| "postgresql://localhost/github_bot".to_string()),
            bind_address: env::var("BIND_ADDRESS").unwrap_or_else(|_| "0.0.0.0:3000".to_string()),
            bot_name: env::var("BOT_NAME").unwrap_or_else(|_| "bot".to_string()),
        })
    }
}
