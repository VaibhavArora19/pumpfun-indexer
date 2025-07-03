use std::sync::Arc;

use async_trait::async_trait;
use carbon_core::{error::CarbonResult, instruction::InstructionProcessorInputType, metrics::MetricsCollection, processor::Processor};
use carbon_pumpfun_decoder::instructions::PumpfunInstruction;
use solana_sdk::native_token::LAMPORTS_PER_SOL;
use sqlx::PgPool;

use crate::db::create_token;


pub struct PumpfunInstructionProcessor {
    pub db: Arc<PgPool>,
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
                create_token(self.db.clone(), create_event).await;
            }
            PumpfunInstruction::TradeEvent(trade_event) => {
                
                if trade_event.sol_amount > 10 * LAMPORTS_PER_SOL {
                    // log::info!("Big trade occured: {:#?}", trade_event);
                }
            }
            PumpfunInstruction::CompleteEvent(complete_event) => {
                log::info!("Bonded: {:#?}", complete_event);
            }
            _ => {}
        };

        Ok(())
    }
}
