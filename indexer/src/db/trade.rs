use std::sync::Arc;

use redis::{aio::MultiplexedConnection, PushInfo, Value};
use sqlx::{types::chrono::Utc, PgPool};
use tokio::sync::mpsc::UnboundedReceiver;
use uuid::Uuid;

use crate::helpers::TradeInfo;

// Consumes messages from the Redis channel and stores them in the database once the trades count exceeds 10.
pub async fn consume_and_store(
    redis: &mut MultiplexedConnection,
    db: Arc<PgPool>,
    rx: &mut UnboundedReceiver<PushInfo>,
) {
    let _ = redis
        .psubscribe("trade")
        .await
        .expect("Failed to subscribe to trade channel");

    let mut cache_trades: Vec<TradeInfo> = Vec::new();

    while let Some(msg) = rx.recv().await {
        let message = msg.data;

        if message.len() < 3 {
            log::error!("Received message with insufficient data: {:?}", message);
            continue;
        }

        if let Value::BulkString(ref data) = message[2] {
            let Ok(str_data) = std::str::from_utf8(data) else {
                log::error!("Invalid UTF-8 in message data");
                return;
            };

            let Ok(parsed): Result<TradeInfo, _> = serde_json::from_str(str_data) else {
                log::error!("Failed to deserialize TradeInfo");
                return;
            };

            log::info!("Parsed TradeInfo: {:?}", parsed);
            cache_trades.push(parsed);

            if cache_trades.len() > 10 {
                let length = cache_trades.len();
                let temp_trades = cache_trades.clone();
                cache_trades.clear();

                let now = Utc::now();

                let db_clone = db.clone();

                tokio::spawn(async move {
                    let mut ids = Vec::with_capacity(length);
                    let mut sol_amounts = Vec::with_capacity(length);
                    let mut token_amounts = Vec::with_capacity(length);
                    let mut is_buys = Vec::with_capacity(length);
                    let mut users = Vec::with_capacity(length);
                    let mut created_ats = Vec::with_capacity(length);
                    let mut updated_ats = Vec::with_capacity(length);
                    let mut contract_addresses = Vec::with_capacity(length);

                    for trade in temp_trades {
                        ids.push(Uuid::new_v4());
                        sol_amounts.push(trade.sol_amount as i64);
                        token_amounts.push(trade.token_amount as i64);
                        is_buys.push(trade.is_buy);
                        users.push(trade.user.to_string());
                        created_ats.push(now);
                        updated_ats.push(now);
                        contract_addresses.push(trade.mint.to_string());
                    }

                    let query = r#"
                    INSERT INTO trade (id, sol_amount, token_amount, is_buy, user_address, created_at, updated_at, token_id)
                    SELECT 
                    i, s, t, b, u, c, up, tok.id
                    FROM 
                    UNNEST(
                    $1::uuid[], 
                    $2::bigint[], 
                    $3::bigint[], 
                    $4::bool[], 
                    $5::text[], 
                    $6::timestamptz[], 
                    $7::timestamptz[], 
                    $8::text[]
                    ) AS tmp(i, s, t, b, u, c, up, ca)
                    JOIN token tok ON tok.contract_address = tmp.ca
                    "#;

                    if let Err(err) = sqlx::query(query)
                        .bind(&ids)
                        .bind(&sol_amounts)
                        .bind(&token_amounts)
                        .bind(&is_buys)
                        .bind(&users)
                        .bind(&created_ats)
                        .bind(&updated_ats)
                        .bind(&contract_addresses)
                        .execute(&*db_clone)
                        .await
                    {
                        eprintln!("Failed to store trades batch: {:#?}", err);
                    } else {
                        log::info!("Stored trades batch successfully");
                    }
                });
            }
        } else {
            log::error!("Unexpected message format: {:?}", message);
        }
    }
}
