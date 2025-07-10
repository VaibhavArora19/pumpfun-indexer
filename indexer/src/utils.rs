use anyhow::Error;
use sqlx::PgPool;

// Connects to the PostgreSQL database using the provided database URL.
pub async fn connect_db(database_url: &str) -> Result<PgPool, Error> {
    log::info!("Trying to connect with {:?}", database_url);
    let db = match sqlx::postgres::PgPool::connect(database_url).await {
        Ok(connection) => connection,
        Err(e) => {
            log::error!("Error: {:?}", e);
            return Err(Error::msg("Error: Unable to connect to db"));
        }
    };

    Ok(db)
}
