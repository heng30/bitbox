use super::pool;
use anyhow::Result;

#[derive(Serialize, Deserialize, Debug, Clone, sqlx::FromRow)]
pub struct AddressBook {
    pub uuid: String,
    pub data: String,
}

pub async fn new() -> Result<()> {
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS address_book (
             id INTEGER PRIMARY KEY,
             uuid TEXT NOT NULL UNIQUE,
             network TEXT NOT NULL,
             data TEXT NOT NULL)",
    )
    .execute(&pool())
    .await?;

    Ok(())
}

pub async fn delete(uuid: &str) -> Result<()> {
    sqlx::query("DELETE FROM address_book WHERE uuid=?")
        .bind(uuid)
        .execute(&pool())
        .await?;
    Ok(())
}

pub async fn insert(uuid: &str, network: &str, data: &str) -> Result<()> {
    sqlx::query("INSERT INTO address_book (uuid, network, data) VALUES (?, ?, ?)")
        .bind(uuid)
        .bind(network)
        .bind(data)
        .execute(&pool())
        .await?;
    Ok(())
}

#[allow(dead_code)]
pub async fn select_all() -> Result<Vec<AddressBook>> {
    Ok(
        sqlx::query_as::<_, AddressBook>("SELECT * FROM address_book")
            .fetch_all(&pool())
            .await?,
    )
}

pub async fn select_all_network(network: &str) -> Result<Vec<AddressBook>> {
    Ok(
        sqlx::query_as::<_, AddressBook>("SELECT * FROM address_book WHERE network=?")
            .bind(network)
            .fetch_all(&pool())
            .await?,
    )
}
