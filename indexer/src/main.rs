use std::{collections::HashMap, sync::Arc};

use crate::{
    config::IndexerConfig,
    db::{
        fetch_token_data, get_bonding_curve_and_mc_info, store_trades, update_bonding_curve_and_market_cap, BondingCurveAndMcInfo
    },
    helpers::get_latest_sol_price,
    pumpfun_processor::PumpfunInstructionProcessor,
    utils::connect_db,
};
use actix_cors::Cors;
use actix_web::{get, web, App, HttpResponse, HttpServer};
use carbon_core::pipeline::Pipeline;
use carbon_pumpfun_decoder::{PumpfunDecoder};
use dotenv::dotenv;
use redis::{AsyncCommands};
use sqlx::PgPool;
use tokio::{sync::RwLock, time};

mod config;
mod db;
mod helius_websocket;
mod helpers;
mod pumpfun_processor;
mod types;
mod utils;

pub type BondingMcStateMap = Arc<RwLock<HashMap<String, BondingCurveAndMcInfo>>>;

#[get("/tokens")]
async fn get_tokens(db : web::Data<Arc<PgPool>>) -> HttpResponse {

    let conn = db.get_ref();

    let result = fetch_token_data(conn).await;

    HttpResponse::Ok().json(&result)

}

#[tokio::main(flavor = "multi_thread")]
async fn main() -> std::io::Result<()> {
    dotenv().ok();
    tracing_subscriber::fmt::init();

    let config = IndexerConfig::get_config();

    let db = Arc::new(
        connect_db(&config.database_url)
            .await
            .map_err(|_| anyhow::Error::msg("Failed to connect to DB")).unwrap(),
    );

    log::info!("Database Connected");

    let redis_client = redis::Client::open(config.redis_url.clone()).unwrap();

    let mut connection = redis_client
        .get_multiplexed_async_connection()
        .await
        .unwrap();

    let _: () = connection.flushall().await.unwrap();
    
    let bonding_curve_and_mc_info = get_bonding_curve_and_mc_info(db.clone()).await.unwrap();

    println!("bonding curve info: {:#?}", bonding_curve_and_mc_info);

    let bonding_curve_and_mc_info_map: BondingMcStateMap = Arc::new(RwLock::new(HashMap::new()));

    //write this to the map and then pass the bonding curve info to the instruction processor
    {
        let mut map = bonding_curve_and_mc_info_map.write().await;

        for item in bonding_curve_and_mc_info.into_iter() {
            map.insert(
                item.contract_address.clone(),
                BondingCurveAndMcInfo {
                    contract_address: item.contract_address,
                    bonding_curve_address: item.bonding_curve_address,
                    bonding_curve_percentage: item.bonding_curve_percentage,
                    market_cap: item.market_cap,
                },
            );
        }
    }

    let sol_price = Arc::new(RwLock::new(0.0));

    let sol_price_clone = sol_price.clone();

    tokio::spawn(async move {
        loop {
            let price = get_latest_sol_price().await.unwrap();

            println!("price is: {:?}", price);

            let mut price_ref = sol_price_clone.write().await;

            *price_ref = price;

            tokio::time::sleep(time::Duration::from_secs(15)).await;
        }
    });

    let db_clone = db.clone();
    let info_map = bonding_curve_and_mc_info_map.clone();

    tokio::spawn(async move {
        loop {
            let db_clone = db_clone.clone();
            let info_map = info_map.clone();
            tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;
            update_bonding_curve_and_market_cap(db_clone, info_map).await;
        }
    });

    let db_clone_2 = db.clone();
    let connection_clone = connection.clone();

    //flush redis data into DB
    tokio::spawn(async move {
        loop {
            store_trades(&mut connection_clone.clone(), db_clone_2.clone()).await;

            tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;
        }
    });

    let instruction_processor = PumpfunInstructionProcessor {
        db: db.clone(),
        config,
        redis: connection,
        bonding_state_map: bonding_curve_and_mc_info_map,
        sol_price: sol_price,
    };

    Pipeline::builder()
        .datasource(helius_websocket::get_helius_websocket())
        .instruction(PumpfunDecoder, instruction_processor)
        .shutdown_strategy(carbon_core::pipeline::ShutdownStrategy::Immediate)
        .build().unwrap()
        .run()
        .await.unwrap();



    HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(db.clone()))
            .wrap(
                Cors::default()
                    .allow_any_origin()
                    .allowed_methods(vec!["GET"])
            )
            .service(get_tokens) 
    })
    .bind(("0.0.0.0", 8000))?
    .run()
    .await
}
