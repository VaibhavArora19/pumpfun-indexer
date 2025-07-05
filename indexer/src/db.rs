use std::{collections::{HashMap, HashSet}, sync::Arc};

use carbon_pumpfun_decoder::instructions::{create_event::CreateEvent};
use redis::{aio::MultiplexedConnection, AsyncCommands};
use serde::{Deserialize, Serialize};
use solana_pubkey::Pubkey;
use sqlx::{
    postgres::PgArguments, prelude::FromRow, query_with, types::chrono::{DateTime, Utc}, Arguments, PgPool, Pool, Postgres,
};
use uuid::Uuid;

use crate::{
    config::IndexerConfig,
    helpers::{get_creator_holding_balance, get_total_supply, TradeInfo},
    types::BondStatus,
    BondingMcStateMap,
};

#[derive(sqlx::FromRow, Debug, Clone)]
pub struct BondingCurveAndMcInfo {
    pub contract_address: String,
    pub bonding_curve_address: String,
    pub bonding_curve_percentage: i32,
    pub market_cap: Option<i64>,
}

#[allow(dead_code)]
#[derive(FromRow)]
pub struct Trade {
    id: uuid::Uuid,
    sol_amount: i64,
    token_amount: i64,
    is_buy: bool,
    user_address: String,
    token_id: uuid::Uuid
}

#[allow(dead_code)]
#[derive(FromRow, Clone)]
pub struct Token {
    id: uuid::Uuid,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    name: String,
    ticker: String,
    contract_address: String,
    bonding_curve_percentage: i32,
    bond_status: String,
    volume: Option<i64>,
    market_cap: Option<i64>,
    uri: String,
    bonding_curve_address: String,
    creator_address: String,
    creator_holding_percentage: i64
}
#[allow(dead_code)]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TokenDetails {
    id: String,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    name: String,
    ticker: String,
    contract_address: String,
    bonding_curve_percentage: i32,
    bond_status: String,
    volume: Option<i64>,
    market_cap: Option<i64>,
    uri: String,
    bonding_curve_address: String,
    creator_address: String,
    funds_percent_by_top_10: f64,
    holder_count: usize,
    creator_percent: f64

}

#[allow(dead_code)]
#[derive(Debug, Clone)]
struct Holding {
    token_id: Uuid,
    user: String,
    net_tokens: i64,
}


pub async fn create_token(db: Arc<PgPool>, config: &IndexerConfig, create_event: CreateEvent) {
    let id = uuid::Uuid::new_v4();
    let current_time = Utc::now();

    let holding_percentage =
        get_creator_holding_balance(create_event.creator, create_event.mint, config).await;

    let insert_sql = r#"
    INSERT INTO token(
    id,
    created_at,
    updated_at,
    name,
    ticker,
    contract_address,
    bond_status,
    creator_holding_percentage,
    uri,
    bonding_curve_address,
    creator_address
    ) VALUES($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)"#;

    if let Err(err) = sqlx::query(insert_sql)
        .bind(id)
        .bind(current_time)
        .bind(current_time)
        .bind(create_event.name)
        .bind(create_event.symbol)
        .bind(create_event.mint.to_string())
        .bind(BondStatus::NewlyLaunched)
        .bind(holding_percentage.floor() as i64)
        .bind(create_event.uri)
        .bind(create_event.bonding_curve.to_string())
        .bind(create_event.user.to_string())
        .execute(&*db)
        .await
    {
        eprintln!("Failed to insert new token. Failed with err: {:?}", err);
    }
}

pub async fn change_status(bond_status: BondStatus, mint: Pubkey, db: Arc<PgPool>) {
    let update_sql = r#"
    UPDATE token SET bond_status = $1 WHERE contract_address = $2
    "#;

    if let Err(err) = sqlx::query(update_sql)
        .bind(bond_status)
        .bind(mint.to_string())
        .execute(&*db)
        .await
    {
        eprintln!(
            "Failed to update the bond status. Failed with err: {:?}",
            err
        );
    }
}

pub async fn get_bonding_curve_and_mc_info(
    db: Arc<PgPool>,
) -> Result<Vec<BondingCurveAndMcInfo>, anyhow::Error> {
    let query = r#"SELECT contract_address, bonding_curve_address, bonding_curve_percentage, market_cap FROM token"#;

    let bonding_curve_info = match sqlx::query_as::<_, BondingCurveAndMcInfo>(query)
        .fetch_all(&*db)
        .await
    {
        Ok(r) => r,
        Err(e) => {
            log::error!("{}", e);
            return Err(anyhow::Error::msg(
                "Error: Fail to fetch bonding curve info",
            ));
        }
    };

    return Ok(bonding_curve_info);
}

pub async fn update_bonding_curve_and_market_cap(db: Arc<PgPool>, updates: BondingMcStateMap) {
    log::info!("Entered into sql function");
    let mut sql = String::from("UPDATE token AS t SET market_cap = u.market_cap, bonding_curve_percentage = u.bonding_curve_percentage FROM (VALUES");

    let updates_ref = {
        let reference = updates.read().await.clone();
        reference
    };

    if updates_ref.len() == 0 {
        return;
    }

    //clear the map after flushing into DB to avoid memory issues
    // {
    //     let mut clean_map = updates.write().await;
    //     clean_map.clear();
    // }

    let mut args = PgArguments::default();

    for (i, update) in updates_ref.iter().enumerate() {
        if i > 0 {
            sql.push_str(", ");
        }

        let base = i * 3;

        sql.push_str(&format!("(${}, ${}, ${})", base + 1, base + 2, base + 3));

        args.add(&update.0);
        args.add(&update.1.market_cap);
        args.add(&update.1.bonding_curve_percentage);
    }

    sql.push_str(") AS u(contract_address, market_cap, bonding_curve_percentage) WHERE u.contract_address = t.contract_address");

    if let Err(err) = query_with(&sql, args).execute(&*db).await {
        eprintln!("Failed to save the data. Failed with error: {:?}", err);
    };

    log::info!("Update data with updates: {:#?}", updates);
}

pub async fn store_trades(redis: &mut MultiplexedConnection, db: Arc<PgPool>) {
    let keys: Vec<String> = redis::cmd("KEYS")
        .arg("*")
        .query_async(redis)
        .await
        .unwrap();

    if keys.len() == 0 {
        return;
    }

    let values: Vec<String> = redis::cmd("MGET")
        .arg(keys.clone())
        .query_async(redis)
        .await
        .unwrap();

    log::info!("keys are: {:?}", keys);
    log::info!("values are {:?}", values);

    //flush redis here
    let _: () = redis.flushall().await.unwrap();

    let mut parsed_info = Vec::new();

    for item in values.into_iter() {
        let parsed: TradeInfo = serde_json::from_str(&item).unwrap();

        parsed_info.push(parsed);
    }

    for trade in parsed_info {
        let id = uuid::Uuid::new_v4();
        let time = Utc::now();

        let query = r#"
        INSERT INTO trade (id, sol_amount, token_amount, is_buy, user_address, created_at, updated_at, token_id)
        SELECT $1, $2, $3, $4, $5, $6, $7, id FROM token WHERE contract_address = $8
        "#;

        if let Err(err) = sqlx::query(query)
            .bind(id)
            .bind(trade.sol_amount as i64)
            .bind(trade.token_amount as i64)
            .bind(trade.is_buy)
            .bind(trade.user.to_string())
            .bind(time)
            .bind(time)
            .bind(trade.mint.to_string())
            .execute(&*db)
            .await
        {
            eprintln!("Failed to store trades. Failed with err: {:#?}", err);
        };
    }
}

pub async fn fetch_token_data(db: &Pool<Postgres>) -> Vec<TokenDetails> {
let all_trades = sqlx::query_as::<_, Trade>(r#"SELECT * FROM trade"#).fetch_all(&*db).await.unwrap();
let all_tokens = sqlx::query_as::<_, Token>(r#"SELECT * FROM token"#).fetch_all(&*db).await.unwrap();

let token_map: HashMap<uuid::Uuid,Token > = all_tokens.iter().cloned().map(|t| (t.id, t)).collect();

let mut volume: HashMap<Uuid, i64> = HashMap::new();

let mut holdings_map: HashMap<Uuid, Vec<Holding>> = HashMap::new();

for trade in all_trades {

    volume.entry(trade.token_id).and_modify(|x| *x += trade.sol_amount).or_insert(trade.sol_amount);

    let entry = holdings_map.entry(trade.token_id).or_default();
    if let Some(user_holding) = entry.iter_mut().find(|h| h.user == trade.user_address) {
        if trade.is_buy {
            user_holding.net_tokens += trade.token_amount as i64;
        } else {
            user_holding.net_tokens -= trade.token_amount as i64;
        }
    } else {
        entry.push(Holding {
            token_id: trade.token_id,
            user: trade.user_address.clone(),
            net_tokens: if trade.is_buy {
                trade.token_amount as i64
            } else {
                -(trade.token_amount as i64)
            },
        });
    }
}

println!("holding map: {:#?}", holdings_map);

let mut token_vec = Vec::new();


// Then calculate per token:
for (token_id, holders) in holdings_map {
    // let total_supply: i64 = holders.iter().map(|h| h.net_tokens).sum();
    let total_supply =1000000000;

    let mut sorted = holders.clone();
    println!("sorted: {:#?}", sorted);
    sorted.sort_by(|a, b| b.net_tokens.cmp(&a.net_tokens));

    let top_10_total: i64 = sorted.iter().take(10).map(|h| h.net_tokens).sum();

    let holder_count = holders.iter().cloned().filter(|x| x.net_tokens > 0).count();

    log::info!("holders: {:?}", holders);

    let mut creator_balance = holders
        .iter()
        .find(|h| h.user == token_map.get(&token_id).unwrap().creator_address)
        .map(|h| h.net_tokens)
        .unwrap_or(0);

    let mut funds_percent_by_top_10 = if total_supply > 0 {
        (((top_10_total as f64 / 10f64.powi(6)) as f64 / total_supply as f64) * 100.0)
    } else {
        0.0
    };

    log::info!("token id: {} and funds_percent_top_10: {}", token_id, funds_percent_by_top_10);

    if funds_percent_by_top_10 < 0.0 {
        funds_percent_by_top_10 = 0.0
    }

    let initial_creator_balance = all_tokens.iter().cloned().find(|x| x.id == token_id).unwrap();

    let creator_percent = if total_supply > 0 {
        ((((initial_creator_balance.creator_holding_percentage as f64) + creator_balance as f64) /  10f64.powi(6)) / total_supply as f64) * 100.0
    } else {
        0.0
    };

    if let Some(token_details) = all_tokens.iter().find(|x| x.id == token_id) {
        let volume = volume.get(&token_id).cloned();

        let market_cap = token_details.market_cap.unwrap_or_else(|| 0);

        token_vec.push(TokenDetails {
            id: token_id.to_string(),
            created_at: token_details.created_at,
            updated_at: token_details.updated_at,
            name: token_details.name.clone(),
            ticker: token_details.ticker.clone(),
            contract_address: token_details.contract_address.clone(),
            bonding_curve_percentage: token_details.bonding_curve_percentage,
            bond_status: if market_cap > 25000 && market_cap < 62500 { "Graduating".to_string() } else if market_cap < 25000 { "Newly launched".to_string()} else { "Graduated".to_string() },
            volume,
            market_cap: token_details.market_cap,
            uri: token_details.uri.clone(),
            bonding_curve_address: token_details.bonding_curve_address.clone(),
            creator_address: token_details.creator_address.clone(),
            funds_percent_by_top_10,
            holder_count: if creator_percent > 0.0 { holder_count + 1 } else { holder_count },
            creator_percent
        });
    }

    println!("Token: {token_id}, Top 10: {funds_percent_by_top_10:.2}%, Creator: {creator_percent:.2}%, Holders: {holder_count}");
}

    let b_ids: HashSet<String> = token_vec.iter().map(|t| t.id.to_string()).collect();

    let not_included: Vec<Token> = all_tokens.iter().cloned().filter(|t| !b_ids.contains(&t.id.to_string())).collect();

    for t in not_included {
        token_vec.push(TokenDetails {
            id: t.id.to_string(),
            created_at: t.created_at,
            updated_at: t.updated_at,
            name: t.name.clone(),
            ticker: t.ticker.clone(),
            contract_address: t.contract_address.clone(),
            bonding_curve_percentage: t.bonding_curve_percentage,
            bond_status: t.bond_status.clone(),
            volume: Some(0),
            market_cap: t.market_cap,
            uri: t.uri.clone(),
            bonding_curve_address: t.bonding_curve_address.clone(),
            creator_address: t.creator_address.clone(),
            funds_percent_by_top_10: 0.0,
            holder_count: 0,
            creator_percent: 0.0
        });
    }

    token_vec
}
