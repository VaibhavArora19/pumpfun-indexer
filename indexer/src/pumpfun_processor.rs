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
    config::IndexerConfig,
    db::{change_status, create_token, BondingCurveAndMcInfo},
    helpers::{get_bonding_curve_progress, get_market_cap, store_in_redis},
    types::BondStatus,
    BondingMcStateMap,
};

pub struct PumpfunInstructionProcessor {
    pub db: Arc<PgPool>,
    pub config: IndexerConfig,
    pub redis: MultiplexedConnection,
    pub bonding_state_map: BondingMcStateMap,
    pub sol_price: Arc<RwLock<f64>>,
}

#[async_trait]
impl Processor for PumpfunInstructionProcessor {
    type InputType = InstructionProcessorInputType<PumpfunInstruction>;

    async fn process(
        &mut self,
        data: Self::InputType,
        _metrics: Arc<MetricsCollection>,
    ) -> CarbonResult<()> {
        let pumpfun_instruction: PumpfunInstruction = data.1.data;
        // println!("pump fun {:?}", pumpfun_instruction);

        //in case of sell event you are getting the creator address in create event so in every sell event, check if creator sold and store
        match pumpfun_instruction {
            PumpfunInstruction::CreateEvent(create_event) => {
                log::info!("New token created: {:#?}", create_event);
                create_token(self.db.clone(), &self.config, create_event.clone()).await;

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
            PumpfunInstruction::TradeEvent(trade_event) => {
                // log::info!("Big trade occured: {:#?}", trade_event);
                let mut map = self.bonding_state_map.write().await;

                // if it exists in our DB then only process it
                if let Some(event) = map.get_mut(&trade_event.mint.to_string()) {
                    let progress = get_bonding_curve_progress(
                        trade_event.real_token_reserves as i128,
                        793100000,
                    );

                    let curve_result = match i64::try_from(progress) {
                        Ok(value) => value,
                        Err(_) => {
                            log::error!(
                                "Failed to convert bonding curve progress: {}",
                                progress
                            );
                            0
                        }
                    };

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

                    event.bonding_curve_percentage = curve_result as i32;
                    event.market_cap = Some(market_cap);

                    let mut redis_clone = self.redis.clone();

                    //create a new thread to keep this block non-blocking
                    tokio::spawn(async move {
                        store_in_redis(&mut redis_clone, trade_event).await;
                    });
                }
            }
            PumpfunInstruction::CompleteEvent(complete_event) => {
                log::info!("Bonded: {:#?}", complete_event);

                change_status(BondStatus::Graduated, complete_event.mint, self.db.clone()).await;
            }
            _ => {}
        };

        Ok(())
    }
}
