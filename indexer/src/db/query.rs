use solana_sdk::native_token::LAMPORTS_PER_SOL;
use sqlx::{Pool, Postgres};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

use crate::types::{BondStatus, Holding, Token, TokenDetails, Trade};

/// Fetches token data from the database and calculates various metrics such as volume, market cap, top_10_holding_percentage, and creator percentage etc.
pub async fn fetch_token_data(db: &Pool<Postgres>) -> Vec<TokenDetails> {
    let all_trades = sqlx::query_as::<_, Trade>(r#"SELECT * FROM trade"#)
        .fetch_all(&*db)
        .await
        .unwrap();
    let all_tokens = sqlx::query_as::<_, Token>(r#"SELECT * FROM token"#)
        .fetch_all(&*db)
        .await
        .unwrap();

    let token_map: HashMap<uuid::Uuid, Token> =
        all_tokens.iter().cloned().map(|t| (t.id, t)).collect();

    let mut volume: HashMap<Uuid, f64> = HashMap::new();

    let mut holdings_map: HashMap<Uuid, Vec<Holding>> = HashMap::new();

    for trade in all_trades {
        volume
            .entry(trade.token_id)
            .and_modify(|x| *x += trade.sol_amount as f64 / LAMPORTS_PER_SOL as f64)
            .or_insert(trade.sol_amount as f64 / LAMPORTS_PER_SOL as f64);

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

    let mut token_vec = Vec::new();

    let total_supply = 1000000000;

    // Then calculate per token:
    for (token_id, holders) in holdings_map {

        let mut sorted = holders.clone();

        sorted.sort_by(|a, b| b.net_tokens.cmp(&a.net_tokens));

        let top_10_total: i64 = sorted.iter().take(10).map(|h| h.net_tokens).sum();

        let holder_count = holders.iter().cloned().filter(|x| x.net_tokens > 0).count();

        let creator_balance = holders
            .iter()
            .find(|h| h.user == token_map.get(&token_id).unwrap().creator_address)
            .map(|h| h.net_tokens)
            .unwrap_or(0);

        let mut funds_percent_by_top_10 = if total_supply > 0 {
            ((top_10_total as f64 / 10f64.powi(6)) as f64 / total_supply as f64) * 100.0
        } else {
            0.0
        };

        if funds_percent_by_top_10 < 0.0 {
            funds_percent_by_top_10 = 0.0
        }

        let creator_percent = if total_supply > 0 {
            (((creator_balance as f64) / 10f64.powi(6)) / total_supply as f64) * 100.0
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
                bond_status: if token_details.bond_status == BondStatus::Graduated { BondStatus::Graduated } else if market_cap > 25000 && market_cap < 62500 {
                    BondStatus::Graduating
                } else if market_cap < 25000 {
                   BondStatus::NewlyLaunched
                } else {
                    BondStatus::Graduated
                },
                volume,
                market_cap: token_details.market_cap,
                uri: token_details.uri.clone(),
                bonding_curve_address: token_details.bonding_curve_address.clone(),
                creator_address: token_details.creator_address.clone(),
                funds_percent_by_top_10,
                holder_count,
                creator_percent,
            });
        }

        println!("Token: {token_id}, Top 10: {funds_percent_by_top_10:.2}%, Creator: {creator_percent:.2}%, Holders: {holder_count}");
    }

    let b_ids: HashSet<String> = token_vec.iter().map(|t| t.id.to_string()).collect();

    let not_included: Vec<Token> = all_tokens
        .iter()
        .cloned()
        .filter(|t| !b_ids.contains(&t.id.to_string()))
        .collect();

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
            volume: Some(0.0),
            market_cap: t.market_cap,
            uri: t.uri.clone(),
            bonding_curve_address: t.bonding_curve_address.clone(),
            creator_address: t.creator_address.clone(),
            funds_percent_by_top_10: 0.0,
            holder_count: 0,
            creator_percent: 0.0,
        });
    }

    token_vec
}
