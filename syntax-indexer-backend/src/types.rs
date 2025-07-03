use solana_pubkey::Pubkey;
use sqlx::Type;

pub struct BondingCurveAccount {
    pub discriminator: u64,
    pub virtual_token_reserves: u64,
    pub virtual_sol_reserves: u64,
    pub real_token_reserves: u64,
    pub real_sol_reserves: u64,
    pub token_total_supply: u64,
    pub complete: bool,
    pub creator: Pubkey,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Type)]
#[sqlx(type_name = "Text")]
#[sqlx(rename_all = "snake_case")]
pub enum BondStatus {
    NewlyLaunched,
    Graduating,
    Graduated,
}
