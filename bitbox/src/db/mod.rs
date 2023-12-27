use anyhow::Result;
use sqlx::migrate::MigrateDatabase;
use sqlx::{
    sqlite::{Sqlite, SqlitePoolOptions},
    Pool,
};
use std::sync::Mutex;

pub mod account;
pub mod address_book;

const MAX_CONNECTIONS: u32 = 3;

lazy_static! {
    static ref POOL: Mutex<Option<Pool<Sqlite>>> = Mutex::new(None);
}

fn pool() -> Pool<Sqlite> {
    POOL.lock().unwrap().clone().unwrap()
}

async fn create_db(db_path: &str) -> Result<(), sqlx::Error> {
    Sqlite::create_database(db_path).await?;

    let pool = SqlitePoolOptions::new()
        .max_connections(MAX_CONNECTIONS)
        .connect(&format!("sqlite:{}", db_path))
        .await?;

    *POOL.lock().unwrap() = Some(pool);

    Ok(())
}

pub async fn init(db_path: &str) {
    create_db(db_path).await.expect("create db");
    account::new().await.expect("account new");
    address_book::new().await.expect("address_book new");
}

#[allow(dead_code)]
pub async fn is_table_exist(table_name: &str) -> Result<()> {
    sqlx::query("SELECT name FROM sqlite_master WHERE type='table' AND name=?")
        .bind(table_name)
        .fetch_one(&pool())
        .await?;

    Ok(())
}

pub async fn drop_table(table_name: &str) -> Result<()> {
    sqlx::query(&format!("DROP TABLE {}", table_name))
        .execute(&pool())
        .await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_db_is_table_exist() -> Result<()> {
        init("/tmp/bitbox-test.db").await;
        account::new().await?;
        assert!(is_table_exist("hello").await.is_err());
        assert!(is_table_exist("account").await.is_ok());
        Ok(())
    }

    #[tokio::test]
    async fn test_db_drop_table() -> Result<()> {
        init("/tmp/bitbox-test.db").await;
        account::new().await?;
        assert!(drop_table("hello").await.is_err());
        assert!(drop_table("account").await.is_ok());
        Ok(())
    }
}
