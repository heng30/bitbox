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

pub async fn delete_all() -> Result<()> {
    sqlx::query("DELETE FROM address_book").execute(&pool()).await?;
    Ok(())
}

pub async fn insert(uuid: &str, data: &str) -> Result<()> {
    sqlx::query("INSERT INTO address_book (uuid, data) VALUES (?, ?)")
        .bind(uuid)
        .bind(data)
        .execute(&pool())
        .await?;
    Ok(())
}
pub async fn update(uuid: &str, data: &str) -> Result<()> {
    sqlx::query("UPDATE address_book SET data=? WHERE uuid=?")
        .bind(data)
        .bind(uuid)
        .execute(&pool())
        .await?;

    Ok(())
}

pub async fn select(uuid: &str) -> Result<AddressBook> {
    let pool = pool();
    let stream = sqlx::query_as::<_, AddressBook>("SELECT * FROM address_book WHERE uuid=?")
        .bind(uuid)
        .fetch_one(&pool);

    Ok(stream.await?)
}

pub async fn select_all() -> Result<Vec<AddressBook>> {
    Ok(sqlx::query_as::<_, AddressBook>("SELECT * FROM address_book")
        .fetch_all(&pool())
        .await?)
}

#[allow(dead_code)]
pub async fn is_exist(uuid: &str) -> Result<()> {
    select(uuid).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;

    #[tokio::test]
    async fn test_address_book_table_new() -> Result<()> {
        db::init("/tmp/bitbox-test.db").await;
        new().await
    }

    #[tokio::test]
    async fn test_address_book_table_delete_one() -> Result<()> {
        db::init("/tmp/bitbox-test.db").await;
        new().await?;
        delete_all().await?;
        insert("uuid-1", "data-1").await?;
        delete("uuid-1").await
    }

    #[tokio::test]
    async fn test_address_book_table_delete_all() -> Result<()> {
        db::init("/tmp/bitbox-test.db").await;
        new().await?;
        delete_all().await
    }

    #[tokio::test]
    async fn test_address_book_table_insert() -> Result<()> {
        db::init("/tmp/bitbox-test.db").await;
        new().await?;
        delete_all().await?;
        insert("uuid-1", "data-1").await?;
        insert("uuid-2", "data-2").await
    }

    #[tokio::test]
    async fn test_address_book_table_update() -> Result<()> {
        db::init("/tmp/bitbox-test.db").await;
        new().await?;
        delete_all().await?;
        insert("uuid-1", "data-1").await?;
        update("uuid-1", "data-1.1").await
    }

    #[tokio::test]
    async fn test_address_book_table_select_one() -> Result<()> {
        db::init("/tmp/bitbox-test.db").await;
        new().await?;
        delete_all().await?;
        assert!(select("uuid-1").await.is_err());

        insert("uuid-1", "data-1").await?;
        assert_eq!(select("uuid-1").await?.data, "data-1");
        Ok(())
    }

    #[tokio::test]
    async fn test_address_book_table_select_all() -> Result<()> {
        db::init("/tmp/bitbox-test.db").await;
        new().await?;
        delete_all().await?;
        insert("uuid-1", "data-1").await?;
        insert("uuid-2", "data-2").await?;
        let address_books = select_all().await?;

        assert_eq!(address_books.len(), 2);
        assert_eq!(address_books[0].uuid, "uuid-1");
        assert_eq!(address_books[1].uuid, "uuid-2");
        Ok(())
    }

    #[tokio::test]
    async fn test_address_book_table_is_exist() -> Result<()> {
        db::init("/tmp/bitbox-test.db").await;
        new().await?;
        delete_all().await?;
        insert("uuid-1", "data-1").await?;

        assert!(is_exist("uuid-0").await.is_err());
        assert!(is_exist("uuid-1").await.is_ok());
        Ok(())
    }
}
