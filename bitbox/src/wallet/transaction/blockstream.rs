use crate::util::http;
use anyhow::Result;
use bitcoin::{BlockHash, OutPoint, Script, Transaction, TxIn, TxOut};
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug, Deserialize)]
struct UtxoStatus {
    confirmed: bool,
    block_height: u64,
    block_hash: String,
    block_time: u64,
}

#[derive(Debug, Deserialize)]
struct Utxo {
    txid: String,
    vout: u32,
    value: u64,
    status: UtxoStatus,
}

pub async fn fetch_utxos(address: &str) -> Result<Vec<Utxo>> {
    let url = format!("https://blockstream.info/api/address/{}/utxo", address);

    let client = http::client(None)?;
    let response = client
        .get(&url)
        .timeout(Duration::from_secs(15))
        .send()
        .await?
        .json::<Vec<Utxo>>()
        .await?;

    Ok(response)
}

#[derive(Debug, Serialize)]
struct BroadcastRequest {
    tx: String,
}

#[derive(Debug, Deserialize)]
struct BroadcastResponse {
    success: bool,
    error: Option<String>,
}

// TODO: test
pub async fn broadcast_transaction(transaction: &Transaction) -> Result<()> {
    let url = "https://blockstream.info/api/tx";
    let hex_tx = hex::encode(serde_json::to_string(transaction)?);
    let request_body = BroadcastRequest { tx: hex_tx };

    let client = http::client(None)?;
    let response = client
        .post(url)
        .json(&request_body)
        .send()
        .await?
        .json::<BroadcastResponse>()
        .await?;

    if response.success {
        log::debug!("Transaction successfully broadcasted!");
        Ok(())
    } else if let Some(error) = response.error {
        log::warn!("Failed to broadcast transaction: {}", error);
        Err(anyhow::Error::msg(error))
    } else {
        log::warn!("Failed to broadcast transaction");
        Err(anyhow::Error::msg("Unknown error"))
    }
}

// TODO: construct a transaction

#[cfg(test)]
mod tests {
    const ADDRESS: &'static str = "36LjFk7tAn6j93nKBHcvtXd88wFGSPDtZG";

    #[tokio::test]
    async fn test_fetch_utxos() {
        let utxos = super::fetch_utxos(ADDRESS).await.unwrap();
        assert!(!utxos.is_empty());
        println!("{:?}", utxos[0]);
    }
}
