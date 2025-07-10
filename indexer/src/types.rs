use serde::{Deserialize, Serialize};
use sqlx::{
    prelude::FromRow,
    types::chrono::{DateTime, Utc},
    Type,
};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Type)]
#[sqlx(type_name = "Text")]
#[sqlx(rename_all = "snake_case")]
pub enum BondStatus {
    NewlyLaunched,
    Graduating,
    Graduated,
}

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
    pub id: uuid::Uuid,
    pub sol_amount: i64,
    pub token_amount: i64,
    pub is_buy: bool,
    pub user_address: String,
    pub token_id: uuid::Uuid,
}

#[allow(dead_code)]
#[derive(FromRow, Clone)]
pub struct Token {
    pub id: uuid::Uuid,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub name: String,
    pub ticker: String,
    pub contract_address: String,
    pub bonding_curve_percentage: i32,
    pub bond_status: String,
    pub market_cap: Option<i64>,
    pub uri: String,
    pub bonding_curve_address: String,
    pub creator_address: String,
}
#[allow(dead_code)]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TokenDetails {
    pub id: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub name: String,
    pub ticker: String,
    pub contract_address: String,
    pub bonding_curve_percentage: i32,
    pub bond_status: String,
    pub volume: Option<f64>,
    pub market_cap: Option<i64>,
    pub uri: String,
    pub bonding_curve_address: String,
    pub creator_address: String,
    pub funds_percent_by_top_10: f64,
    pub holder_count: usize,
    pub creator_percent: f64,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct Holding {
    pub token_id: Uuid,
    pub user: String,
    pub net_tokens: i64,
}
