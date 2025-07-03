use std::env;

use thiserror::Error;

#[derive(Debug, Default)]
pub struct IndexerConfig {
    pub api_key: String,
    pub database_url: String,
    pub redis_url: String,
}

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("Error: Invalid API Key")]
    InvalidAPIKey,
    #[error("Error: Invalid Database URL")]
    InvalidDatabseURL,
    #[error("Error: Invalid Redis URL")]
    InvalidRedisURL,
}

impl IndexerConfig {
    pub fn get_config() -> Self {
        let api_key =
            env::var("API_KEY").unwrap_or_else(|_| ConfigError::InvalidAPIKey.to_string());

        let database_url =
            env::var("DATABASE_URL").unwrap_or_else(|_| ConfigError::InvalidDatabseURL.to_string());

        let redis_url =
            env::var("REDIS_URL").unwrap_or_else(|_| ConfigError::InvalidRedisURL.to_string());

        Self {
            api_key,
            database_url,
            redis_url,
        }
    }
}
