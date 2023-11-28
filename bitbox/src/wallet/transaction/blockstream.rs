use crate::util;
use crate::wallet;
use anyhow::{anyhow, Result};
use bitcoin::amount::Amount;
use bitcoin::blockdata::locktime::absolute::LockTime;
use bitcoin::blockdata::transaction::Version;
use bitcoin::psbt::{Psbt, PsbtSighashType};
use bitcoin::secp256k1::Secp256k1;
use bitcoin::{Address, Network, OutPoint, PrivateKey, ScriptBuf, Transaction, TxIn, TxOut, Txid};
use rand::seq::SliceRandom;
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

    let client = util::http::client()?;
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

    let client = util::http::client()?;
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
) -> Result<(Psbt, u64)> {
    let network = Network::from_core_arg(&acnt.address_info.network)?;
    let private_key = util::crypto::decrypt(password, &acnt.address_info.private_key);
    let private_key = PrivateKey::from_str(&private_key)?;
    let public_key = private_key.public_key(&Secp256k1::new());
    let sender_address = Address::p2pkh(&public_key, network);

    assert_eq!(public_key.to_string(), acnt.address_info.public_key);
    assert_eq!(sender_address.to_string(), acnt.address_info.wallet_address);

    let change_script_pubkey = sender_address.script_pubkey();
    let recipient_script_pubkey = Address::from_str(&tx_info.recipient_address)?
        .require_network(network)?
        .script_pubkey();

    let output = TxOut {
        value: Amount::from_sat(tx_info.send_amount),
        script_pubkey: recipient_script_pubkey,
    };

    let (inputs, change_amount) = build_txins(&acnt, &tx_info, &output).await?;

    let change_output = TxOut {
        value: Amount::from_sat(change_amount),
        script_pubkey: change_script_pubkey,
    };

    let transaction = Transaction {
        version: Version::TWO,
        lock_time: LockTime::ZERO,
        input: inputs,
        output: vec![output, change_output],
    };

    let fee = transaction.total_size() as u64 * tx_info.fee_rate;

    let psbt = Psbt::from_unsigned_tx(transaction)?;
    let mut keys = BTreeMap::new();
    keys.insert(public_key, private_key);
    Ok((sign(psbt, keys)?, fee))
}

async fn build_txins(
    acnt: &wallet::account::Info,
    tx_info: &super::data::TxInfo,
    output: &TxOut,
) -> Result<(Vec<TxIn>, u64)> {
    let mut utxos = fetch_utxos(
        &acnt.address_info.network,
        &acnt.address_info.wallet_address,
    )
    .await?;
    utxos.shuffle(&mut rand::thread_rng());

    let mut inputs: Vec<TxIn> = Vec::new();
    let (mut total_input_sat, mut change_amount) = (0, 0);
    for utxo in utxos.iter() {
        let mut input = TxIn::default();
        input.previous_output = OutPoint::new(Txid::from_str(&utxo.txid)?, utxo.vout);
        inputs.push(input);

        let fee_amount = Transaction {
            version: Version::TWO,
            lock_time: LockTime::ZERO,
            input: inputs.clone(),
            output: vec![output.clone(), output.clone()], // one for recipient, another for change
        }
        .total_size() as u64 * tx_info.fee_rate;

        total_input_sat += utxo.value;
        if total_input_sat > tx_info.send_amount + fee_amount {
            change_amount = total_input_sat - tx_info.send_amount - fee_amount;
            break;
        }
    }

    if change_amount == 0 {
        Err(anyhow!("insufficient balance"))
    } else {
        Ok((inputs, change_amount))
    }

}

fn sign(mut psbt: Psbt, keys: BTreeMap<bitcoin::PublicKey, PrivateKey>) -> Result<Psbt> {
    let secp = Secp256k1::new();
    if let Err((_, e)) = psbt.sign(&keys, &secp) {
        return Err(anyhow!("{:?}", e));
    }
    Ok(psbt)
}

pub fn verify_tx_info(
    tx_info: &super::data::TxInfo,
) -> Result<()> {
    if tx_info.send_amount > tx_info.max_send_amount {
        return Err(anyhow!(
            "send amount: {} is bigger than max send amount: {}",
            tx_info.send_amount,
            tx_info.max_send_amount
        ));
    }

    if tx_info.fee_rate > tx_info.max_fee_rate {
        return Err(anyhow!(
            "fee rate: {} is bigger than max fee rate: {}",
            tx_info.fee_rate,
            tx_info.max_fee_rate
        ));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    const ADDRESS: &'static str = "36LjFk7tAn6j93nKBHcvtXd88wFGSPDtZG";

    #[tokio::test]
    async fn test_fetch_utxos() {
        let utxos = super::fetch_utxos("main", ADDRESS).await.unwrap();
        assert!(!utxos.is_empty());
        // println!("{:?}", utxos[0]);
    }
}
