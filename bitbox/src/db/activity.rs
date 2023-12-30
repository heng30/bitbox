use super::pool;
use anyhow::Result;

#[derive(Serialize, Deserialize, Debug, Clone, sqlx::FromRow)]
pub struct Activity {
    pub uuid: String,
    pub data: String,
}

pub async fn new() -> Result<()> {
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS activity (
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
    sqlx::query("DELETE FROM activity WHERE uuid=?")
        .bind(uuid)
        .execute(&pool())
        .await?;
    Ok(())
}

pub async fn delete_all() -> Result<()> {
    sqlx::query("DELETE FROM activity").execute(&pool()).await?;
    Ok(())
}

pub async fn insert(uuid: &str, network: &str, data: &str) -> Result<()> {
    sqlx::query("INSERT INTO activity (uuid, network, data) VALUES (?, ?, ?)")
        .bind(uuid)
        .bind(network)
        .bind(data)
        .execute(&pool())
        .await?;
    Ok(())
}

pub async fn select_all() -> Result<Vec<Activity>> {
    Ok(sqlx::query_as::<_, Activity>("SELECT * FROM activity")
        .fetch_all(&pool())
        .await?)
}

pub async fn select_all_network(network: &str) -> Result<Vec<Activity>> {
    Ok(
        sqlx::query_as::<_, Activity>("SELECT * FROM activity WHERE network=?")
            .bind(network)
            .fetch_all(&pool())
            .await?,
    )
}
