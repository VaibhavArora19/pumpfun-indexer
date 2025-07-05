use anyhow::Error;
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::commitment_config::CommitmentConfig;
use sqlx::PgPool;

use crate::config::IndexerConfig;

pub fn get_connection(config: &IndexerConfig) -> RpcClient {
    RpcClient::new_with_commitment(
        format!(
            "https://mainnet.helius-rpc.com/?api-key={}",
            config.api_key.clone()
        ),
        CommitmentConfig::confirmed(),
    )
}

pub async fn connect_db(database_url: &str) -> Result<PgPool, Error> {
    log::info!("Trying to connect with {:?}", database_url);
    let db = match sqlx::postgres::PgPool::connect(database_url).await {
        Ok(connection) => connection,
        Err(e) => {
            log::error!("Error: {:?}", e);
            return Err(Error::msg("Error: Unable to connect to db"));
        }
    };

    Ok(db)
}
