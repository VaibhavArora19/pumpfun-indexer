use std::{collections::HashSet, sync::Arc};

use carbon_helius_atlas_ws_datasource::HeliusWebsocket;
use carbon_pumpfun_decoder::PROGRAM_ID;
use helius::types::{
    Cluster, RpcTransactionsConfig, TransactionCommitment, TransactionDetails,
    TransactionSubscribeFilter, TransactionSubscribeOptions, UiEnhancedTransactionEncoding,
};
use tokio::sync::RwLock;

use crate::config::IndexerConfig;

pub fn get_helius_websocket() -> HeliusWebsocket {
    let api_key = IndexerConfig::get_config().api_key;

    let helius_websocket = carbon_helius_atlas_ws_datasource::HeliusWebsocket::new(
        api_key,
        carbon_helius_atlas_ws_datasource::Filters {
            accounts: vec![],
            transactions: Some(RpcTransactionsConfig {
                filter: TransactionSubscribeFilter {
                    account_include: Some(vec![PROGRAM_ID.to_string().clone()]),
                    account_exclude: None,
                    account_required: None,
                    vote: None,
                    failed: None,
                    signature: None,
                },
                options: TransactionSubscribeOptions {
                    commitment: Some(TransactionCommitment::Confirmed),
                    encoding: Some(UiEnhancedTransactionEncoding::Base64),
                    transaction_details: Some(TransactionDetails::Full),
                    show_rewards: None,
                    max_supported_transaction_version: Some(0),
                },
            }),
        },
        Arc::new(RwLock::new(HashSet::new())),
        Cluster::MainnetBeta,
    );

    helius_websocket
}
