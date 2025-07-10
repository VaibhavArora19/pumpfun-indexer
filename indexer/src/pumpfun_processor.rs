use std::sync::Arc;

use async_trait::async_trait;
use carbon_core::{
    error::CarbonResult, instruction::InstructionProcessorInputType, metrics::MetricsCollection,
    processor::Processor,
};
use carbon_pumpfun_decoder::instructions::PumpfunInstruction;
use redis::aio::MultiplexedConnection;
use sqlx::PgPool;
use tokio::sync::RwLock;

use crate::{
    db::token::{change_status, create_token},
    helpers::{get_bonding_curve_progress, get_market_cap, store_in_redis},
    types::{BondStatus, BondingCurveAndMcInfo},
    BondingMcStateMap,
};

pub struct PumpfunInstructionProcessor {
    pub db: Arc<PgPool>,
    pub redis: MultiplexedConnection,
    pub bonding_state_map: BondingMcStateMap,
    pub sol_price: Arc<RwLock<f64>>,
}

#[async_trait]
impl Processor for PumpfunInstructionProcessor {
    type InputType = InstructionProcessorInputType<PumpfunInstruction>;

    //This function is called whenever any kind of event is emitted from the Pumpfun program.
    async fn process(
        &mut self,
        data: Self::InputType,
        _metrics: Arc<MetricsCollection>,
    ) -> CarbonResult<()> {
        let pumpfun_instruction: PumpfunInstruction = data.1.data;

        //Pattern matching to check which event is being processed
        match pumpfun_instruction {
            // This is the event when a new token is created
            PumpfunInstruction::CreateEvent(create_event) => {
                log::info!("New token created: {:#?}", create_event);
                create_token(self.db.clone(), create_event.clone()).await;

                let mut map = self.bonding_state_map.write().await;

                map.insert(
                    create_event.mint.to_string(),
                    BondingCurveAndMcInfo {
                        contract_address: create_event.mint.to_string(),
                        bonding_curve_address: create_event.bonding_curve.to_string(),
                        bonding_curve_percentage: 0,
                        market_cap: Some(0),
                    },
                );
            }
            // This is the event when a trade event occurs for any token
            PumpfunInstruction::TradeEvent(trade_event) => {
                let mut map = self.bonding_state_map.write().await;

                // if the token exists in our DB and here in our Hashmap, then only process it
                if let Some(event) = map.get_mut(&trade_event.mint.to_string()) {
                    let progress = get_bonding_curve_progress(
                        trade_event.virtual_token_reserves as i128,
                    );

                    let curve_result = match i64::try_from(progress) {
                        Ok(value) => value,
                        Err(_) => {
                            log::error!("Failed to convert bonding curve progress: {}", progress);
                            0
                        }
                    };

                    //Get the market cap based on the virtual reserves, total supply, and latest SOL price in USD
                    let market_cap = get_market_cap(
                        trade_event.virtual_sol_reserves,
                        trade_event.virtual_token_reserves,
                        6,
                        1000000000,
                        self.sol_price.clone(),
                    )
                    .await;

                    log::info!(
                        "details for mint: {:#?} {:?} {:?}",
                        trade_event.mint,
                        market_cap,
                        curve_result
                    );

                    //Update the hashmap key-value pair with the new bonding curve percentage and market cap
                    event.bonding_curve_percentage = curve_result as i32;
                    event.market_cap = Some(market_cap);

                    let mut redis_clone = self.redis.clone();

                    //create a new thread that publishes the data in the "trade" channel to keep this block non-blocking
                    tokio::spawn(async move {
                        store_in_redis(&mut redis_clone, trade_event).await;
                    });
                }
            }
            // This is the event when a token is graduated
            PumpfunInstruction::CompleteEvent(complete_event) => {
                log::info!("Bonded: {:#?}", complete_event);

                //Change the status of the token to "Graduated" in the DB
                change_status(BondStatus::Graduated, complete_event.mint, self.db.clone()).await;
            }
            _ => {}
        };

        Ok(())
    }
}
