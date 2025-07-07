use std::{collections::HashMap, sync::Arc};

use anyhow::Error;
use carbon_pumpfun_decoder::instructions::trade_event::TradeEvent;
use redis::{aio::MultiplexedConnection, AsyncCommands};
use serde::{Deserialize, Serialize};
use solana_account_decoder::parse_token::UiTokenAmount;
use solana_client::{
    client_error::reqwest::{
        self,
        header::{HeaderMap, HeaderValue, CONTENT_TYPE},
    },
    nonblocking::rpc_client,
};
use solana_pubkey::Pubkey;
use solana_sdk::native_token::LAMPORTS_PER_SOL;
use spl_associated_token_account_client::address::get_associated_token_address;
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::{config::IndexerConfig, utils::get_connection};

#[derive(Debug, Serialize, Deserialize)]
pub struct CoinPriceData {
    pub usd: f64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TradeInfo {
    pub sol_amount: u64,
    pub token_amount: u64,
    pub is_buy: bool,
    pub user: String,
    pub mint: String,
}

#[derive(Debug)]
struct Holding {
    token_id: Uuid,
    user: String,
    net_tokens: i64,
}

pub type CoinPriceResponse = HashMap<String, CoinPriceData>;

pub async fn get_total_supply(mint: Pubkey, config: &IndexerConfig) -> f64 {
    let rpc_client = get_connection(config);

    let total_supply = rpc_client
        .get_token_supply(&mint)
        .await
        .expect("Failed to fetch total supply of the token");

    total_supply.amount.parse::<f64>().unwrap()
}

pub async fn get_creator_holding_balance(
    wallet_address: Pubkey,
    mint: Pubkey,
    config: &IndexerConfig,
) -> f64 {
    let rpc_client = get_connection(config);

    let ata = get_associated_token_address(&wallet_address, &mint);

    let balance = match rpc_client.get_token_account_balance(&ata).await {
        Ok(bal) => bal,
        Err(_) => UiTokenAmount {
            ui_amount: Some(0.0),
            decimals: 6,
            amount: "0".to_string(),
            ui_amount_string: "0".to_string(),
        },
    };

    // let holding_percentage = (balance.amount.parse::<f64>().unwrap()
    //     / total_supply.amount.parse::<f64>().unwrap())
    //     * 100.0;

    balance.amount.parse::<f64>().unwrap()
    // return holding_percentage;
}

pub fn get_bonding_curve_progress(
    real_token_reserves: u128,
    initial_real_token_reserves: u128,
) -> u128 {
    log::info!("real token reserves: {}", real_token_reserves);

    let left_tokens = (real_token_reserves / 10u128.pow(6)) - 206900000;

    println!("details: {} {}", left_tokens, initial_real_token_reserves);

    let bonding_curve: f64 =
        100.0 - ((left_tokens as f64 / initial_real_token_reserves as f64) * 100.0);

    return bonding_curve.floor() as u128;
}

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
        .set(data.mint.to_string(), trade_details)
        .await
        .unwrap();

    log::info!("stored in redis");
}
