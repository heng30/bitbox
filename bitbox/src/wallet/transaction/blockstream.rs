use crate::util;
use crate::wallet;
use anyhow::{anyhow, Result};
use bitcoin::amount::Amount;
use bitcoin::blockdata::locktime::absolute::LockTime;
use bitcoin::blockdata::transaction::Version;
use bitcoin::psbt::{Psbt, PsbtSighashType};
use bitcoin::secp256k1::Secp256k1;
use bitcoin::{Address, Network, OutPoint, PrivateKey, ScriptBuf, Transaction, TxIn, TxOut, Txid};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::str::FromStr;
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

pub async fn fetch_utxos(network: &str, address: &str) -> Result<Vec<Utxo>> {
    let url = utxo_url(network, address);
    let proxy_info = None; // TODO

    let client = util::http::client(proxy_info)?;
    let response = client
        .get(&url)
        .timeout(Duration::from_secs(15))
        .send()
        .await?
        .json::<Vec<Utxo>>()
        .await?;

    Ok(response)
}

// TODO: need test
pub async fn broadcast_transaction(network: &str, psbt: &Psbt) -> Result<String> {
    let url = broadcast_url(network);
    let hex_tx = psbt.serialize_hex();

    let proxy_info = None; // TODO
    let client = util::http::client(proxy_info)?;
    let response = client.post(url).body(hex_tx).send().await?;

    if response.status().is_success() {
        Ok(response.text().await?)
    } else {
        Err(anyhow!(response.text().await?))
    }
}

// TODO: need test
pub async fn build_transaction(
    password: &str,
    acnt: &wallet::account::Info,
    tx_info: super::data::TxInfo,
) -> Result<Psbt> {
    let network = Network::from_core_arg(&acnt.address_info.network)?;

    let private_key = util::crypto::decrypt(password, &acnt.address_info.private_key);
    let private_key = PrivateKey::from_str(&private_key)?;

    let public_key = private_key.public_key(&Secp256k1::new());
    let sender_address = Address::p2pkh(&public_key, network);

    assert_eq!(public_key.to_string(), acnt.address_info.public_key);
    assert_eq!(sender_address.to_string(), acnt.address_info.wallet_address);

    let recipient_script_pubkey = Address::from_str(&tx_info.recipient_address)?
        .require_network(network)?
        .script_pubkey();

    let mut inputs: Vec<TxIn> = Vec::new();
    let utxos = fetch_utxos(&acnt.address_info.network, &sender_address.to_string()).await?;
    for utxo in utxos.iter() {
        let mut input = TxIn::default();
        input.previous_output = OutPoint::new(Txid::from_str(&utxo.txid)?, utxo.vout);
        inputs.push(input);
    }

    let output = build_recipient_txout(&tx_info, recipient_script_pubkey)?;

    let change_script_pubkey = sender_address.script_pubkey();
    let total_input_sat: u64 = utxos.iter().map(|utxo| utxo.value).sum();

    let change_output = build_change_txout(
        &tx_info,
        change_script_pubkey,
        total_input_sat,
        &inputs,
        &output,
    )?;

    let transaction = Transaction {
        version: Version::TWO,
        lock_time: LockTime::ZERO,
        input: inputs,
        output: vec![output, change_output],
    };

    let psbt = Psbt::from_unsigned_tx(transaction)?;
    let mut keys = BTreeMap::new();
    keys.insert(public_key, private_key);
    sign(psbt, keys)
}

fn sign(mut psbt: Psbt, keys: BTreeMap<bitcoin::PublicKey, PrivateKey>) -> Result<Psbt> {
    let secp = Secp256k1::new();
    if let Err((_, e)) = psbt.sign(&keys, &secp) {
        return Err(anyhow!("{:?}", e));
    }
    Ok(psbt)
}

fn build_recipient_txout(
    tx_info: &super::data::TxInfo,
    recipient_script_pubkey: ScriptBuf,
) -> Result<TxOut> {
    if tx_info.send_amount > tx_info.max_send_amount {
        return Err(anyhow!(
            "send amount: {} is bigger than max send amount: {}",
            tx_info.send_amount,
            tx_info.max_send_amount
        ));
    }

    Ok(TxOut {
        value: Amount::from_sat(tx_info.send_amount),
        script_pubkey: recipient_script_pubkey,
    })
}

fn build_change_txout(
    tx_info: &super::data::TxInfo,
    change_script_pubkey: ScriptBuf,
    total_input_sat: u64,
    inputs: &Vec<TxIn>,
    output: &TxOut,
) -> Result<TxOut> {
    if tx_info.fee_rate > tx_info.max_fee_rate {
        return Err(anyhow!(
            "fee rate: {} is bigger than max fee rate: {}",
            tx_info.fee_rate,
            tx_info.max_fee_rate
        ));
    }

    let estimated_size: usize = Transaction {
        version: Version::TWO,
        lock_time: LockTime::ZERO,
        input: inputs.clone(),
        output: vec![output.clone()],
    }
    .total_size();
    let fee_amount = estimated_size as u64 * tx_info.fee_rate;

    let change_amount = total_input_sat - tx_info.send_amount - fee_amount;

    Ok(TxOut {
        value: Amount::from_sat(change_amount),
        script_pubkey: change_script_pubkey,
    })
}

#[cfg(test)]
mod tests {
    const ADDRESS: &'static str = "36LjFk7tAn6j93nKBHcvtXd88wFGSPDtZG";

    #[tokio::test]
    async fn test_fetch_utxos() {
        let utxos = super::fetch_utxos("main", ADDRESS).await.unwrap();
        assert!(!utxos.is_empty());
        println!("{:?}", utxos[0]);
    }
}
