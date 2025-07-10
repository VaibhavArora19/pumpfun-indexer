use std::sync::Arc;

use carbon_pumpfun_decoder::instructions::create_event::CreateEvent;
use solana_pubkey::Pubkey;
use sqlx::{postgres::PgArguments, query_with, types::chrono::Utc, Arguments, PgPool};

use crate::{
    config::IndexerConfig,
    types::{BondStatus, BondingCurveAndMcInfo},
    BondingMcStateMap,
};

pub async fn create_token(db: Arc<PgPool>, create_event: CreateEvent) {
    let id = uuid::Uuid::new_v4();
    let current_time = Utc::now();

    // let creator_initial_balance =
    //     get_creator_holding_balance(create_event.creator, create_event.mint, config).await;

    let insert_sql = r#"
    INSERT INTO token(
    id,
    created_at,
    updated_at,
    name,
    ticker,
    contract_address,
    bond_status,
    uri,
    bonding_curve_address,
    creator_address
    ) VALUES($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)"#;

    if let Err(err) = sqlx::query(insert_sql)
        .bind(id)
        .bind(current_time)
        .bind(current_time)
        .bind(create_event.name)
        .bind(create_event.symbol)
        .bind(create_event.mint.to_string())
        .bind(BondStatus::NewlyLaunched)
        .bind(create_event.uri)
        .bind(create_event.bonding_curve.to_string())
        .bind(create_event.user.to_string())
        .execute(&*db)
        .await
    {
        eprintln!("Failed to insert new token. Failed with error: {:?}", err);
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
