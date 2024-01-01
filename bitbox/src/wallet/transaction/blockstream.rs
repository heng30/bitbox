use crate::{config, util};
use anyhow::{anyhow, Result};
use serde::Deserialize;
use std::time::Duration;

const TIMEOUT_SECS: u64 = 15;

#[derive(Debug, Clone, Deserialize)]
pub struct UtxoStatus {
    pub confirmed: bool,
    pub block_height: Option<u64>,
    pub block_hash: Option<String>,
    pub block_time: Option<u64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Utxo {
    pub txid: String,
    pub vout: u32,
    pub value: u64,
    pub status: UtxoStatus,
}

fn utxo_url(network: &str, address: &str) -> String {
    match network {
        "main" => format!("https://blockstream.info/api/address/{}/utxo", address),
        "test" => format!(
            "https://blockstream.info/testnet/api/address/{}/utxo",
            address
        ),
        _ => unimplemented!(),
    }
}

fn broadcast_url(network: &str) -> &str {
    match network {
        "main" => "https://blockstream.info/api/tx",
        "test" => "https://blockstream.info/testnet/api/tx",
        _ => unimplemented!(),
    }
}

fn tx_status_url(network: &str, txid: &str) -> String {
    match network {
        "main" => format!("https://blockstream.info/api/tx/{}/status", txid),
        "test" => format!("https://blockstream.info/testnet/api/tx/{}/status", txid),
        _ => unimplemented!(),
    }
}

pub async fn fetch_utxos(network: &str, address: &str) -> Result<Vec<Utxo>> {
    let url = utxo_url(network, address);

    let client = util::http::client()?;
    let response = client
        .get(&url)
        .timeout(Duration::from_secs(TIMEOUT_SECS))
        .send()
        .await?
        .json::<Vec<Utxo>>()
        .await?;

    Ok(response)
}

pub async fn fetch_confirmed_utxos(network: &str, address: &str) -> Result<Vec<Utxo>> {
    let skip_utxo_amount = config::account().skip_utxo_amount as u64;

    Ok(fetch_utxos(network, address)
        .await?
        .into_iter()
        .filter(|item| item.status.confirmed && item.value > skip_utxo_amount)
        .collect())
}

#[allow(dead_code)]
pub async fn fetch_unconfirmed_utxos(network: &str, address: &str) -> Result<Vec<Utxo>> {
    let skip_utxo_amount = config::account().skip_utxo_amount as u64;

    Ok(fetch_utxos(network, address)
        .await?
        .into_iter()
        .filter(|item| !item.status.confirmed && item.value > skip_utxo_amount)
        .collect())
}

pub async fn fetch_balance(network: &str, address: &str) -> Result<u64> {
    Ok(fetch_confirmed_utxos(network, address)
        .await?
        .into_iter()
        .map(|item| item.value)
        .sum())
}

// return txid
pub async fn broadcast_transaction(network: &str, tx: String) -> Result<String> {
    let url = broadcast_url(network);
    let client = util::http::client()?;
    let response = client.post(url).body(tx).send().await?;

    if response.status().is_success() {
        Ok(response.text().await?)
    } else {
        Err(anyhow!(response.text().await?))
    }
}

pub async fn is_tx_confirmed(network: &str, txid: &str) -> Result<bool> {
    let url = tx_status_url(network, txid);

    let client = util::http::client()?;
    let response = client
        .get(&url)
        .timeout(Duration::from_secs(TIMEOUT_SECS))
        .send()
        .await?
        .json::<UtxoStatus>()
        .await?;

    Ok(response.confirmed)
}

#[cfg(test)]
mod tests {
    use super::*;

    const MAIN_ADDRESS: &str = "36LjFk7tAn6j93nKBHcvtXd88wFGSPDtZG";
    const TEST_ADDRESS: &str = "tb1q5sulqc5lq048s25jtcdv34fhxq7s68uk6m2nl0";

    #[tokio::test]
    async fn test_fetch_utxos() -> Result<()> {
        let utxos = super::fetch_utxos("main", MAIN_ADDRESS).await?;
        assert!(!utxos.is_empty());

        let utxos = super::fetch_utxos("test", TEST_ADDRESS).await?;
        assert!(!utxos.is_empty());

        Ok(())
    }

    #[tokio::test]
    async fn test_fetch_balance() -> Result<()> {
        let mb = fetch_balance("main", MAIN_ADDRESS).await?;
        assert!(mb > 0);

        let tb = fetch_balance("test", TEST_ADDRESS).await?;
        assert!(tb > 0);

        // println!("{mb}, {tb}");
        Ok(())
    }

    #[tokio::test]
    async fn test_is_tx_confirmed() -> Result<()> {
        let txid = "32f1ae8a69040b3f56b29e4e44e288e3712be0887d05e812155bb7984a71d49e";
        let ret = is_tx_confirmed("test", txid).await?;
        assert!(ret);

        Ok(())
    }
}
