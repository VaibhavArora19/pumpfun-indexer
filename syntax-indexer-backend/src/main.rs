use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
};

use crate::{
    config::IndexerConfig,
    db::{get_bonding_curve_info, BondingCurveInfo},
    pumpfun_processor::PumpfunInstructionProcessor,
    utils::connect_db,
};
use carbon_core::pipeline::Pipeline;
use carbon_pumpfun_decoder::PumpfunDecoder;
use dotenv::dotenv;
use redis::AsyncCommands;

mod config;
mod db;
mod helius_websocket;
mod helpers;
mod pumpfun_processor;
mod types;
mod utils;

pub type BondingStateMap = Arc<RwLock<HashMap<String, BondingCurveInfo>>>;

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

    let redis_client = redis::Client::open(config.redis_url.clone()).unwrap();

    let redis = Arc::new(redis_client.get_connection().unwrap());

    let bonding_curve_info = get_bonding_curve_info(db.clone()).await.unwrap();

    let mut bonding_curve_info_map: BondingStateMap = Arc::new(RwLock::new(HashMap::new()));

    //write this to the map and then pass the bonding curve info to the instruction processor
    {
        let mut map = bonding_curve_info_map.write().unwrap();

        for item in bonding_curve_info.into_iter() {
            map.insert(
                item.contract_address.clone(),
                BondingCurveInfo {
                    contract_address: item.contract_address,
                    bonding_curve_address: item.bonding_curve_address,
                    bonding_curve_percentage: item.bonding_curve_percentage,
                },
            );
        }
    }

    // create_token(db.clone(), CreateEvent { name: "hello".to_string(), symbol: "helllo".to_string(), uri: "https".to_string(), mint: Pubkey::from_str("B1Ni4dMNFvRXXV6qCtC98vSYNy6fKidsUtQrs5mqpump").unwrap(), bonding_curve: Pubkey::from_str("FdGc4Ns4dKbFv8ADKgS66tWR8CyxPJbtJcJHheNM3C8N").unwrap(), user: Pubkey::from_str("3BJ6qHbt6U7EnUAFP2cwP8ydz5iiGfReRvGnnB3uygfd").unwrap(), creator: Pubkey::from_str("3BJ6qHbt6U7EnUAFP2cwP8ydz5iiGfReRvGnnB3uygfd").unwrap(), timestamp: 1751536646 }).await;

    let instruction_processor = PumpfunInstructionProcessor { db, config, redis, bonding_state_map: bonding_curve_info_map };

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
