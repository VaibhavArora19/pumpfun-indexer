use std::sync::{Arc, RwLock};

use async_trait::async_trait;
use carbon_core::{
    error::CarbonResult, instruction::InstructionProcessorInputType, metrics::MetricsCollection,
    processor::Processor,
};
use carbon_pumpfun_decoder::instructions::PumpfunInstruction;
use redis::Connection;
use solana_sdk::native_token::LAMPORTS_PER_SOL;
use sqlx::PgPool;

use crate::{
    config::IndexerConfig, db::{change_status, create_token, BondingCurveInfo}, helpers::get_bonding_curve_progress, types::BondStatus, BondingStateMap
};

pub struct PumpfunInstructionProcessor {
    pub db: Arc<PgPool>,
    pub config: IndexerConfig,
    pub redis: Arc<Connection>,
    pub bonding_state_map: BondingStateMap
}

#[async_trait]
impl Processor for PumpfunInstructionProcessor {
    type InputType = InstructionProcessorInputType<PumpfunInstruction>;

    async fn process(
        &mut self,
        data: Self::InputType,
        _metrics: Arc<MetricsCollection>,
    ) -> CarbonResult<()> {
        println!("entered here");
        let pumpfun_instruction: PumpfunInstruction = data.1.data;
        // println!("pump fun {:?}", pumpfun_instruction);
        println!("Triggered");

        //in case of sell event you are getting the creator address in create event so in every sell event, check if creator sold and store
        match pumpfun_instruction {
            PumpfunInstruction::CreateEvent(create_event) => {
                log::info!("New token created: {:#?}", create_event);
                create_token(self.db.clone(), &self.config, create_event.clone()).await;
            
                let mut map = self.bonding_state_map.write().unwrap();
            
                map.insert(create_event.mint.to_string(), BondingCurveInfo {
                    contract_address: create_event.mint.to_string(),
                    bonding_curve_address: create_event.bonding_curve.to_string(),
                    bonding_curve_percentage: 0
                });
            }
            PumpfunInstruction::TradeEvent(trade_event) => {
                    // log::info!("Big trade occured: {:#?}", trade_event);
                    let mut map = self.bonding_state_map.write().unwrap();

                    let progress = get_bonding_curve_progress(trade_event.real_token_reserves as u128, 0); //todo: get initial token reserve                
                    
                    let result = i64::try_from(progress).unwrap();

                    if let Some(value) = map.get_mut(&trade_event.mint.to_string()) {
                        value.bonding_curve_percentage = result;
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
