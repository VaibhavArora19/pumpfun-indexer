use std::sync::Arc;

use carbon_pumpfun_decoder::instructions::create_event::CreateEvent;
use sqlx::{types::chrono::Utc, PgPool};

use crate::{config::IndexerConfig, helpers::get_creator_holding_percentage, types::BondStatus};

pub async fn create_token(db: Arc<PgPool>, config: &IndexerConfig, create_event: CreateEvent) {
    let id = uuid::Uuid::new_v4();
    let current_time = Utc::now();

    let holding_percentage = get_creator_holding_percentage(create_event.creator, create_event.mint, config).await;

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
    .await {
        eprintln!("Failed to insert new token. Failed with err: {:?}", err);
    }

    
}