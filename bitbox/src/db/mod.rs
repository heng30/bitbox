use crate::config;
use sqlx::migrate::MigrateDatabase;
use sqlx::{
    sqlite::{Sqlite, SqlitePoolOptions},
    Pool,
};
use std::sync::Mutex;

pub mod account;

const MAX_CONNECTIONS: u32 = 3;

lazy_static! {
    pub static ref POOL: Mutex<Option<Pool<Sqlite>>> = Mutex::new(None);
}

async fn create_db() -> Result<(), sqlx::Error> {
    let db_path = config::db_path();

    Sqlite::create_database(&db_path).await?;

    let pool = SqlitePoolOptions::new()
        .max_connections(MAX_CONNECTIONS)
        .connect(&format!("sqlite:{}", db_path))
        .await?;

    *POOL.lock().unwrap() = Some(pool);

    Ok(())
}

pub async fn init() {
    if let Err(e) = create_db().await {
        panic!("create db: {}, Error: {e:?}", config::db_path());
    }
}
