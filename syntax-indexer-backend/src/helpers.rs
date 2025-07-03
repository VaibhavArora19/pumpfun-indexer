use solana_pubkey::Pubkey;
use spl_associated_token_account_client::address::get_associated_token_address;

use crate::{config::IndexerConfig, utils::get_connection};

pub async fn get_creator_holding_percentage(wallet_address: Pubkey, mint: Pubkey, config: &IndexerConfig) -> f64 {
    let rpc_client = get_connection(config);

    let total_supply = rpc_client.get_token_supply(&mint).await.expect("Failed to fetch total supply of the token");

    let ata = get_associated_token_address(&wallet_address, &mint);

    let balance = rpc_client.get_token_account_balance(&ata).await.expect("Failed to get ata balance"); //this might throw error if creator closed the account

    let holding_percentage = (balance.amount.parse::<f64>().unwrap() / total_supply.amount.parse::<f64>().unwrap()) * 100.0;

    return holding_percentage
}