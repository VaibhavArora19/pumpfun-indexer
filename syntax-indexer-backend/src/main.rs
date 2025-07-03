use std::sync::Arc;

use carbon_core::pipeline::Pipeline;
use carbon_pumpfun_decoder::PumpfunDecoder;
use dotenv::dotenv;

use crate::{config::IndexerConfig, pumpfun_processor::PumpfunInstructionProcessor, utils::connect_db};

mod config;
mod pumpfun_processor;
mod helius_websocket;
mod types;
mod utils;

//first thing tommorow is to sort the bonding curve i.e. get data from bonding curve account and check how much bonded and how much not
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv().ok();
    tracing_subscriber::fmt::init();

    let config = IndexerConfig::get_config();

    let db = Arc::new(
        connect_db(&config.database_url)
            .await
            .map_err(|_| anyhow::Error::msg("Failed to connect to DB"))?,
    );

    log::info!("Database Connected");

    let instruction_processor = PumpfunInstructionProcessor {
        db
    };

    Pipeline::builder()
        .datasource(helius_websocket::get_helius_websocket())
        .instruction(PumpfunDecoder, instruction_processor)
        .shutdown_strategy(carbon_core::pipeline::ShutdownStrategy::Immediate)
        .build()?
        .run()
        .await?;

    // Keep the main task alive indefinitely
    tokio::signal::ctrl_c()
        .await
        .expect("Failed to listen for shutdown signal");

    Ok(())
}
