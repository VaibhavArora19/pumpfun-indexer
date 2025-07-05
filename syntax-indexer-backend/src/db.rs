use std::sync::Arc;

use carbon_pumpfun_decoder::instructions::{create_event::CreateEvent, trade_event::TradeEvent};
use redis::{aio::MultiplexedConnection, AsyncCommands};
use serde::de::value;
use solana_pubkey::Pubkey;
use sqlx::{postgres::PgArguments, query_with, types::chrono::Utc, Arguments, PgPool};

use crate::{
    config::IndexerConfig, helpers::{get_creator_holding_percentage, TradeInfo}, types::BondStatus,
    BondingMcStateMap,
};

#[derive(sqlx::FromRow, Debug, Clone)]
pub struct BondingCurveAndMcInfo {
    pub contract_address: String,
    pub bonding_curve_address: String,
    pub bonding_curve_percentage: i32,
    pub market_cap: Option<i64>,
}

pub async fn create_token(db: Arc<PgPool>, config: &IndexerConfig, create_event: CreateEvent) {
    let id = uuid::Uuid::new_v4();
    let current_time = Utc::now();

    let holding_percentage =
        get_creator_holding_percentage(create_event.creator, create_event.mint, config).await;

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

        let query = r#"
        INSERT INTO trade (id, sol_amount, token_amount, is_buy, user_address, token_id)
        SELECT $1, $2, $3, $4, $5, id FROM token WHERE contract_address = $6
        "#;

        if let Err(err) = sqlx::query(query)
            .bind(id)
            .bind(trade.sol_amount as i64)
            .bind(trade.token_amount as i64)
            .bind(trade.is_buy)
            .bind(trade.user.to_string())
            .bind(trade.mint.to_string())
            .execute(&*db)
            .await
        {
            eprintln!("Failed to store trades. Failed with err: {:#?}", err);
        };
    }
}
