/// reference: https://github.com/rust-bitcoin/rust-bitcoin/blob/master/bitcoin/examples/ecdsa-psbt.rs
use crate::{util, wallet};
use anyhow::{anyhow, Context, Result};
use bitcoin::amount::Amount;
use bitcoin::bip32::{
    ChainCode, ChildNumber, DerivationPath, Fingerprint, IntoDerivationPath, Xpriv, Xpub,
};
use bitcoin::blockdata::locktime::absolute::LockTime;
use bitcoin::blockdata::transaction::Version;
use bitcoin::consensus::encode;
use bitcoin::psbt::{self, Input, Psbt, PsbtSighashType};
use bitcoin::secp256k1::{Secp256k1, Signing, Verification};
use bitcoin::{
    Address, Network, OutPoint, PrivateKey, PublicKey, ScriptBuf, Transaction, TxIn, TxOut, Txid,
    Witness,
};
use serde::Deserialize;
use std::time::Duration;

#[derive(Debug, Deserialize)]
pub struct UtxoStatus {
    pub confirmed: bool,
    pub block_height: Option<u64>,
    pub block_hash: Option<String>,
    pub block_time: Option<u64>,
}

#[derive(Debug, Deserialize)]
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

pub async fn fetch_balance(network: &str, address: &str) -> Result<u64> {
    let utxos = fetch_utxos(network, address).await?;
    Ok(utxos.iter().map(|item| item.value).sum())
}

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

// pub async fn build_transaction(
//     password: &str,
//     acnt: &wallet::account::Info,
//     tx_info: super::data::TxInfo,
// ) -> Result<(String, u64)> {
//     let secp = Secp256k1::new();

//     let (psbt, public_key, private_key, previous_output, fee) =
//         create_transaction(password, acnt, tx_info, &secp).await?;

//     let psbt = update_psbt(
//         psbt,
//         &secp,
//         &public_key,
//         &private_key,
//         previous_output.clone(),
//     )?;

//     let mut keys = BTreeMap::new();
//     keys.insert(public_key, private_key);
//     let psbt = sign(psbt, &secp, keys)?;

//     let psbt = finalize_psbt(psbt, &public_key)?;

//     let tx = psbt.extract_tx_unchecked_fee_rate();
//     tx.verify(|_| Some(previous_output.clone()))
//         .context("failed to verify transaction")?;

//     let hex = encode::serialize_hex(&tx);

//     Ok((hex, fee))
// }

// async fn create_transaction<C: Verification + Signing>(
//     password: &str,
//     acnt: &wallet::account::Info,
//     tx_info: super::data::TxInfo,
//     secp: &Secp256k1<C>,
// ) -> Result<(Psbt, PublicKey, PrivateKey, TxOut, u64)> {
//     let network = Network::from_core_arg(&acnt.address_info.network)?;
//     let private_key = PrivateKey::from_str(&acnt.decrypt_private_key(password)?)?;
//     let public_key = private_key.public_key(&secp);
//     let sender_address = Address::p2pkh(&public_key, network);

//     assert_eq!(public_key.to_string(), acnt.address_info.public_key);
//     assert_eq!(sender_address.to_string(), acnt.address_info.wallet_address);

//     let change_script_pubkey = sender_address.script_pubkey();
//     let recipient_script_pubkey = Address::from_str(&tx_info.recipient_address)?
//         .require_network(network)?
//         .script_pubkey();

//     let output = TxOut {
//         value: Amount::from_sat(tx_info.send_amount),
//         script_pubkey: recipient_script_pubkey,
//     };

//     let (inputs, total_input_amount, change_amount) = build_txins(&acnt, &tx_info, &output).await?;

//     // println!("{inputs:?}");

//     let change_output = TxOut {
//         value: Amount::from_sat(change_amount),
//         script_pubkey: change_script_pubkey.clone(),
//     };

//     let transaction = Transaction {
//         version: Version::TWO,
//         lock_time: LockTime::ZERO,
//         input: inputs,
//         output: vec![output, change_output],
//     };

//     let fee = transaction.total_size() as u64 * tx_info.fee_rate;

//     let wpkh = public_key.wpubkey_hash().context("a compressed pubkey")?;
//     let previous_output = TxOut {
//         value: Amount::from_sat(total_input_amount),
//         script_pubkey: ScriptBuf::new_p2wpkh(&wpkh),
//     };

//     let psbt = Psbt::from_unsigned_tx(transaction)?;
//     Ok((psbt, public_key, private_key, previous_output, fee))
// }


// // Updates the PSBT, in BIP174 parlance this is the 'Updater'.
// fn update_psbt<C: Verification + Signing>(
//     mut psbt: Psbt,
//     secp: &Secp256k1<C>,
//     public_key: &PublicKey,
//     private_key: &PrivateKey,
//     previous_output: TxOut,
// ) -> Result<Psbt> {
//     let mut input = Input {
//         witness_utxo: Some(previous_output),
//         ..Default::default()
//     };

//     let wpkh = public_key
//         .wpubkey_hash()
//         .ok_or(anyhow!("a compressed pubkey"))?;
//     let redeem_script = ScriptBuf::new_p2wpkh(&wpkh);
//     input.redeem_script = Some(redeem_script);

//     let master_xpriv = Xpriv {
//         network: private_key.network,
//         depth: 0,
//         parent_fingerprint: Default::default(),
//         child_number: ChildNumber::from_normal_idx(0)?,
//         private_key: private_key.inner,
//         chain_code: ChainCode::from([0; 32]),
//     };
//     let master_xpub = Xpub::from_priv(secp, &master_xpriv);

//     let fingerprint = master_xpub.fingerprint();
//     let path = input_derivation_path()?;
//     let mut map = BTreeMap::new();
//     map.insert(public_key.inner, (fingerprint, path));
//     input.bip32_derivation = map;

//     let ty = PsbtSighashType::from_str("SIGHASH_ALL")?;
//     input.sighash_type = Some(ty);

//     psbt.inputs = vec![input];

//     Ok(psbt)
// }

// fn input_derivation_path() -> Result<DerivationPath> {
//     let path = INPUT_UTXO_DERIVATION_PATH.into_derivation_path()?;
//     Ok(path)
// }

// // Finalizes the PSBT, in BIP174 parlance this is the 'Finalizer'.
// fn finalize_psbt(mut psbt: Psbt, pk: &PublicKey) -> Result<Psbt> {
//     if psbt.inputs.is_empty() {
//         return Err(psbt::SignError::MissingInputUtxo.into());
//     }

//     let input_len = psbt.inputs.len();
//     for i in 0..input_len {
//         let sigs: Vec<_> = psbt.inputs[i].partial_sigs.values().collect();
//         let mut script_witness: Witness = Witness::new();
//         script_witness.push(&sigs[i].to_vec());
//         script_witness.push(pk.to_bytes());

//         psbt.inputs[i].final_script_witness = Some(script_witness);
//         psbt.inputs[i].partial_sigs = BTreeMap::new();
//         psbt.inputs[i].sighash_type = None;
//         psbt.inputs[i].redeem_script = None;
//         psbt.inputs[i].witness_script = None;
//         psbt.inputs[i].bip32_derivation = BTreeMap::new();
//     }

//     Ok(psbt)
// }

// fn sign<C: Verification + Signing>(
//     mut psbt: Psbt,
//     secp: &Secp256k1<C>,
//     keys: BTreeMap<bitcoin::PublicKey, PrivateKey>,
// ) -> Result<Psbt> {
//     if let Err((_, e)) = psbt.sign(&keys, secp) {
//         return Err(anyhow!("{:?}", e));
//     }
//     Ok(psbt)
// }

// pub fn verify_tx_info(tx_info: &super::data::TxInfo) -> Result<()> {
//     if tx_info.send_amount > tx_info.max_send_amount {
//         return Err(anyhow!(
//             "send amount: {} is bigger than max send amount: {}",
//             tx_info.send_amount,
//             tx_info.max_send_amount
//         ));
//     }

//     if tx_info.fee_rate > tx_info.max_fee_rate {
//         return Err(anyhow!(
//             "fee rate: {} is bigger than max fee rate: {}",
//             tx_info.fee_rate,
//             tx_info.max_fee_rate
//         ));
//     }

//     Ok(())
// }

#[cfg(test)]
mod tests {
    use super::super::super::account;
    use super::super::data::TxInfo;
    use super::*;

    const PASSWORD: &str = "12345678";
    const MAIN_ADDRESS: &str = "36LjFk7tAn6j93nKBHcvtXd88wFGSPDtZG";
    const TEST_ADDRESS: &str = "tb1q5sulqc5lq048s25jtcdv34fhxq7s68uk6m2nl0";

    const TEST_ACCOUNT_1: &str = r#"
    {
        "uuid":"2a42cc5b-1663-424d-a391-cd700b5c2f25",
        "name":"account1",
        "address_info":{
            "network":"test",
            "private_key":"eee3574e2f327fbf5489a9479aeae5473713ddf8aa2d259d3908173302fdbd7292d2cea8edc5db5bc60df9f5c12395f20306d5ab1f0dbf2d8d5f7a83f770cce4",
            "public_key":"0312914cf39329afe5180bfa0f69d9d67da3685a5c8d28673199ae973f38ac4148",
            "wallet_address":"msFbCzXbGxdeFRp6zm4WJZozm7akFSGRXg"
            },
        "balance":0
    }"#;

    const TEST_ACCOUNT_2: &str = r#"
    {
      "uuid": "0d2fe06d-570f-4eda-9746-1316685ba75b",
      "name": "account2",
      "address_info": {
        "network": "test",
        "private_key": "02036909611bcb3451dfecf968214ee20b004ed18819aac73a1f275ff580e1520429cacb578e50a0adf7084adb28e9b3b525f1186bac81badb4fd74a64045c6e",
        "public_key": "03be87566556380896352da2d62b9699a37904312166108de7d6a3f890b103a7c5",
        "wallet_address": "mv545czau2FymXWRv2EoypVJXuLfLMBR7Q"
      },
      "balance": 0
    }
    "#;

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

    // async fn _build_transaction() -> Result<(String, u64)> {
    //     let acnt_1: account::Info = serde_json::from_str(TEST_ACCOUNT_1)?;
    //     let acnt_2: account::Info = serde_json::from_str(TEST_ACCOUNT_2)?;

    //     let tx_info = TxInfo {
    //         recipient_address: acnt_2.address_info.wallet_address.clone(),
    //         send_amount: 10,
    //         max_send_amount: 1000,
    //         fee_rate: 10,
    //         max_fee_rate: 20,
    //         max_fee_amount: 1_000_000,
    //     };

    //     build_transaction(PASSWORD, &acnt_1, tx_info).await
    // }

    // #[tokio::test]
    // async fn test_build_transaction() -> Result<()> {
    //     let (tx, fee) = _build_transaction().await?;

    //     println!("You should now be able to broadcast the following transaction: \n{tx}\n fee:{fee}");

    //     Ok(())
    // }

    // #[tokio::test]
    // async fn test_broadcast_transaction() -> Result<()> {
    //     let (tx, fee) = _build_transaction().await?;
    //     println!("You should now be able to broadcast the following transaction: \n{tx}\n fee:{fee}");

    //     let res = broadcast_transaction("test", tx).await?;
    //     println!("{res}");

    //     Ok(())
    // }
}
