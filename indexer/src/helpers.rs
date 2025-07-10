use std::{collections::HashMap, sync::Arc};

use anyhow::Error;
use carbon_pumpfun_decoder::instructions::trade_event::TradeEvent;
use redis::{aio::MultiplexedConnection, AsyncCommands};
use serde::{Deserialize, Serialize};
use solana_client::client_error::reqwest::{
    self,
    header::{HeaderMap, HeaderValue, CONTENT_TYPE},
};
use solana_sdk::native_token::LAMPORTS_PER_SOL;
use tokio::sync::RwLock;

use crate::{config::IndexerConfig};

#[derive(Debug, Serialize, Deserialize)]
pub struct CoinPriceData {
    pub usd: f64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TradeInfo {
    pub sol_amount: u64,
    pub token_amount: u64,
    pub is_buy: bool,
    pub user: String,
    pub mint: String,
}

pub type CoinPriceResponse = HashMap<String, CoinPriceData>;

// Function to calculate the bonding curve progress based on the virtual token reserve
pub fn get_bonding_curve_progress(virtual_token_reserve: i128) -> i128 {
    let progress =
        (1073000000 * 10i128.pow(6) - virtual_token_reserve) * 100 / (793100000 * 10i128.pow(6));

    return progress;
}

// Function to calculate the market cap based on the virtual reserves, total supply, and latest SOL price in USD
pub async fn get_market_cap(
    virtual_sol_reserves: u64,
    virtual_token_reserve: u64,
    decimals: i64,
    total_supply: u64,
    sol_price_usd: Arc<RwLock<f64>>,
) -> i64 {
    let sol_price_usd = {
        let read_guard = sol_price_usd.read().await;
        *read_guard
    };

    println!("sol price usd: {}", sol_price_usd);

    let sol_reserve = virtual_sol_reserves / LAMPORTS_PER_SOL;
    let token_reserve = virtual_token_reserve / 10u64.pow(decimals as u32);

    println!("sol reserve, {} {}", sol_reserve, token_reserve);

    let token_price_sol: f64 = sol_reserve as f64 / token_reserve as f64;

    let token_price_usd = token_price_sol * sol_price_usd as f64;

    println!("token price usd: {}", token_price_usd);

    let mc = token_price_usd * total_supply as f64;

    return mc as i64;
}

// Function to fetch the latest SOL price from the CoinGecko API
pub async fn get_latest_sol_price() -> Result<f64, Error> {
    let api_key = IndexerConfig::get_config().coingecko_api;

    let coingecko_url =
        "https://api.coingecko.com/api/v3/simple/price?ids=solana&vs_currencies=usd";

    let mut headers = HeaderMap::new();

    headers.insert("x-cg-api-key", HeaderValue::from_str(&api_key).unwrap());
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));

    let client = reqwest::Client::new();

    let res = client
        .get(coingecko_url)
        .headers(headers)
        .send()
        .await
        .unwrap();

    let response: CoinPriceResponse = match res.json().await {
        Ok(r) => r,
        Err(err) => {
            log::error!("Failed to parse api response. Failed with error: {}", err);
            return Err(Error::msg("Failed to parse api response"));
        }
    };

    return Ok(response.get("solana").unwrap().usd);
}

pub async fn store_in_redis(redis: &mut MultiplexedConnection, data: TradeEvent) {
    log::info!("Entering data into redis");

    let trade_info = TradeInfo {
        sol_amount: data.sol_amount,
        token_amount: data.token_amount,
        is_buy: data.is_buy,
        user: data.user.to_string(),
        mint: data.mint.to_string(),
    };

    let trade_details = serde_json::to_string(&trade_info).unwrap();

    log::info!("mint address: {}", data.mint.to_string());
    log::info!("trade details: {:?}", trade_details);

    let _: () = redis
        .publish("trade", trade_details)
        .await
        .expect("Failed to publish trade details");

    log::info!("redis published into trade channel");
}
