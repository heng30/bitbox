use super::super::transaction::blockstream::{fetch_confirmed_utxos, Utxo};
use super::{address, sendinfo};
use anyhow::{anyhow, Context, Result};
use bip32::Seed;
use bitcoin::bip32::{Fingerprint, IntoDerivationPath, Xpriv, Xpub};
use bitcoin::blockdata::locktime::absolute::LockTime;
use bitcoin::blockdata::transaction::Version;
use bitcoin::consensus::{deserialize, encode};
use bitcoin::locktime::absolute;
use bitcoin::psbt::{self, Input, Psbt, PsbtSighashType};
use bitcoin::secp256k1::{Secp256k1, Signing, Verification};
use bitcoin::{
    transaction, Address, Amount, Network, OutPoint, ScriptBuf, Transaction, TxIn, TxOut, Txid,
    Witness,
};
use rand::seq::SliceRandom;
use std::collections::BTreeMap;
use std::str::FromStr;

#[derive(Debug, Clone)]
pub struct TxDetail {
    pub tx_hex: String,
    pub fee_amount: u64,
}

pub async fn build(
    password: &str,
    address_info: address::Info,
    tx_info: sendinfo::Info,
) -> Result<TxDetail> {
    let secp = Secp256k1::new();
    let network = Network::from_core_arg(&address_info.network)?;
    let seed = address_info.seed(password)?;

    let (offline, fingerprint, account_xpub) = ColdStorage::new(&secp, seed, network)?;

    tx_info.verify()?;
    address_info.verify(&account_xpub)?;

    let mut online = WatchOnly::new(account_xpub, fingerprint, address_info, tx_info);

    let created = online.create_psbt(&secp).await?;

    let updated = online.update_psbt(created)?;

    let signed = offline.sign_psbt(&secp, updated)?;

    let finalized = online.finalize_psbt(signed)?;

    let tx = finalized.extract_tx_unchecked_fee_rate();

    online.verify_transaction(&tx)?;

    Ok(TxDetail {
        tx_hex: encode::serialize_hex(&tx),
        fee_amount: online.fee_amount,
    })
}

struct ColdStorage {
    master_xpriv: Xpriv,
    master_xpub: Xpub,
}

type ExportData = (ColdStorage, Fingerprint, Xpub);

impl ColdStorage {
    fn new<C: Signing>(secp: &Secp256k1<C>, seed: Seed, network: Network) -> Result<ExportData> {
        let master_xpriv = Xpriv::new_master(network, seed.as_bytes())?;
        let master_xpub = Xpub::from_priv(secp, &master_xpriv);

        // Hardened children require secret data to derive.
        let path = address::ACCOUNT_DERIVATION_PATH.into_derivation_path()?;
        let account_xpriv = master_xpriv.derive_priv(secp, &path)?;
        let account_xpub = Xpub::from_priv(secp, &account_xpriv);

        let wallet = ColdStorage {
            master_xpriv,
            master_xpub,
        };
        let fingerprint = wallet.master_fingerprint();

        Ok((wallet, fingerprint, account_xpub))
    }

    fn master_fingerprint(&self) -> Fingerprint {
        self.master_xpub.fingerprint()
    }

    fn sign_psbt<C: Signing>(&self, secp: &Secp256k1<C>, mut psbt: Psbt) -> Result<Psbt> {
        match psbt.sign(&self.master_xpriv, secp) {
            Ok(_) => (),
            Err((_, e)) => {
                let e = e.get(&0).expect("at least one error");
                return Err(e.clone().into());
            }
        };
        Ok(psbt)
    }
}

#[derive(Debug, Clone)]
struct WatchOnly {
    account_xpub: Xpub,
    master_fingerprint: Fingerprint,

    raw_input_utxos: Vec<Utxo>,
    input_utxos: Vec<TxIn>,
    output_utxos: Vec<TxOut>,
    input_amount: u64,
    change_amount: u64,
    fee_amount: u64,

    address_info: address::Info,
    tx_info: sendinfo::Info,
}

impl WatchOnly {
    fn new(
        account_xpub: Xpub,
        master_fingerprint: Fingerprint,
        address_info: address::Info,
        tx_info: sendinfo::Info,
    ) -> Self {
        WatchOnly {
            account_xpub,
            master_fingerprint,

            raw_input_utxos: vec![],
            input_utxos: vec![],
            output_utxos: vec![],
            input_amount: 0,
            change_amount: 0,
            fee_amount: 0,

            address_info,
            tx_info,
        }
    }

    // Creates the PSBT, in BIP174 parlance this is the 'Creater'.
    async fn create_psbt<C: Verification>(&mut self, _secp: &Secp256k1<C>) -> Result<Psbt> {
        self.build_input_output_tx().await?;

        self.tx_info.verify_max_fee_amount(self.fee_amount)?;

        let tx = Transaction {
            version: transaction::Version::TWO,
            lock_time: absolute::LockTime::ZERO,
            input: self.input_utxos.clone(),
            output: self.output_utxos.clone(),
        };

        let psbt = Psbt::from_unsigned_tx(tx)?;

        Ok(psbt)
    }

    // Updates the PSBT, in BIP174 parlance this is the 'Updater'.
    fn update_psbt(&self, mut psbt: Psbt) -> Result<Psbt> {
        let mut inputs = vec![];

        for index in 0..self.input_utxos.len() {
            let mut input = Input {
                witness_utxo: Some(self.previous_output(index)?),
                ..Default::default()
            };

            input.redeem_script = Some(self.input_utxo_script_pubkey()?);

            let fingerprint = self.master_fingerprint;
            let path = address::ACCOUNT_DERIVATION_PATH.into_derivation_path()?;
            let mut map = BTreeMap::new();
            map.insert(self.account_xpub.to_pub().inner, (fingerprint, path));
            input.bip32_derivation = map;

            let ty = PsbtSighashType::from_str("SIGHASH_ALL")?;
            input.sighash_type = Some(ty);

            inputs.push(input);
        }

        psbt.inputs = inputs;

        Ok(psbt)
    }

    // Finalizes the PSBT, in BIP174 parlance this is the 'Finalizer'.
    fn finalize_psbt(&self, mut psbt: Psbt) -> Result<Psbt> {
        if psbt.inputs.is_empty() {
            return Err(psbt::SignError::MissingInputUtxo.into());
        }

        for i in 0..psbt.inputs.len() {
            let sigs: Vec<_> = psbt.inputs[i].partial_sigs.values().collect();
            let mut script_witness: Witness = Witness::new();
            script_witness.push(&sigs[0].to_vec());
            script_witness.push(self.account_xpub.to_pub().to_bytes());

            psbt.inputs[i].final_script_witness = Some(script_witness);

            // Clear all the data fields as per the spec.
            psbt.inputs[i].partial_sigs = BTreeMap::new();
            psbt.inputs[i].sighash_type = None;
            psbt.inputs[i].redeem_script = None;
            psbt.inputs[i].witness_script = None;
            psbt.inputs[i].bip32_derivation = BTreeMap::new();
        }

        Ok(psbt)
    }

    fn wallet_address(&self) -> Result<Address> {
        let network = Network::from_core_arg(&self.address_info.network)?;
        let pk = self.account_xpub.to_pub();
        Ok(Address::p2wpkh(&pk, network)?)
    }

    fn recipient_address(&self) -> Result<Address> {
        let network = Network::from_core_arg(&self.address_info.network)?;
        let addr = Address::from_str(&self.tx_info.recipient_address)?.require_network(network)?;
        Ok(addr)
    }

    fn input_utxo_script_pubkey(&self) -> Result<ScriptBuf> {
        let wpkh = self
            .account_xpub
            .to_pub()
            .wpubkey_hash()
            .context("failed to get input utxo script pubkey")?;

        Ok(ScriptBuf::new_p2wpkh(&wpkh))
    }

    fn previous_output(&self, index: usize) -> Result<TxOut> {
        Ok(TxOut {
            value: Amount::from_sat(self.raw_input_utxos[index].value),
            script_pubkey: self.input_utxo_script_pubkey()?,
        })
    }

    async fn build_input_output_tx(&mut self) -> Result<()> {
        let wallet_address = self.wallet_address()?.to_string();
        let mut utxos = fetch_confirmed_utxos(&self.address_info.network, &wallet_address).await?;
        utxos.shuffle(&mut rand::thread_rng());

        let (mut raw_inputs, mut inputs, mut outputs) = (vec![], vec![], vec![]);
        outputs.push(TxOut {
            value: Amount::from_sat(self.tx_info.send_amount),
            script_pubkey: self.recipient_address()?.script_pubkey(),
        });

        let (mut total_input_amount, mut change_amount, mut fee_amount) = (0, 0, 0);
        for utxo in utxos.iter() {
            let input = TxIn {
                previous_output: OutPoint::new(Txid::from_str(&utxo.txid)?, utxo.vout),
                ..Default::default()
            };
            inputs.push(input);
            raw_inputs.push(utxo.clone());

            total_input_amount += utxo.value;
            if self.tx_info.send_amount >= total_input_amount {
                continue;
            }

            fee_amount = Transaction {
                version: Version::TWO,
                lock_time: LockTime::ZERO,
                input: inputs.clone(),
                output: vec![outputs[0].clone(), outputs[0].clone()], // one for recipient, another for change
            }
            .total_size() as u64
                * self.tx_info.fee_rate;

            if total_input_amount > self.tx_info.send_amount + fee_amount {
                change_amount = total_input_amount - self.tx_info.send_amount - fee_amount;
                break;
            }
        }

        if change_amount == 0 {
            return Err(anyhow!("insufficient balance"));
        }

        outputs.push(TxOut {
            value: Amount::from_sat(change_amount),
            script_pubkey: self.wallet_address()?.script_pubkey(),
        });

        self.raw_input_utxos = raw_inputs;
        self.input_utxos = inputs;
        self.output_utxos = outputs;
        self.input_amount = total_input_amount;
        self.change_amount = change_amount;
        self.fee_amount = fee_amount;
        Ok(())
    }

    fn verify_transaction(&self, tx: &Transaction) -> Result<()> {
        let raw_input_utxos = self.raw_input_utxos.clone();
        let input_utxo_script_pubkey = self.input_utxo_script_pubkey()?;

        match tx.verify(|outpoint| {
            for utxo in raw_input_utxos.iter() {
                if utxo.txid == outpoint.txid.to_string() && utxo.vout == outpoint.vout {
                    return Some(TxOut {
                        value: Amount::from_sat(utxo.value),
                        script_pubkey: input_utxo_script_pubkey.clone(),
                    });
                }
            }

            None
        }) {
            Err(e) => Err(anyhow!(format!(
                "failed to verify transaction. Reason: {:?}",
                e
            ))),
            _ => Ok(()),
        }
    }
}

pub fn parse_tx(hex_tx: &str) -> Result<String> {
    let bytes = hex_tx
        .as_bytes()
        .chunks(2)
        .map(|chunk| {
            u8::from_str_radix(std::str::from_utf8(chunk).unwrap_or("00"), 16).unwrap_or(0_u8)
        })
        .collect::<Vec<u8>>();

    let transaction: Transaction = deserialize(&bytes)?;
    Ok(format!("{transaction:?}"))
}

pub fn is_valid_txid(txid: &str) -> bool {
    if txid.len() != 64 || !txid.chars().all(|c| c.is_ascii_hexdigit()) {
        return false;
    }

    true
}

#[cfg(test)]
mod tests {
    use super::super::super::transaction::blockstream::broadcast_transaction;
    use super::*;

    const PASSWORD: &str = "12345678";

    const TEST_ACCOUNT_1: &str = r#"
    {
        "uuid":"2a42cc5b-1663-424d-a391-cd700b5c2f25",
        "name":"account1",
        "mnemonic": "b9cd46a3394670456d56a9d47c38da1df0e0ea226c270b08770c20dd738ee69ee54ae400283d587973ffc653eb48b5e46d1bc4153d433ce6bc5db90dc5f02b01cdb47eb9cffad0043587d941469f20c272045151446e9ceb872495c99d7f4353f4b9b88ea02d92645eb6863c85ac6a73ced1d0224b8b2783c72bf3911e15971d5a152d56e383abbe48c6d87b1ed4d4953e40fd465ee125e48379abb039128f04",
        "network": "test",
        "address": [
            "bc1qq8jfvz4fzc83jrysyrtj83sn4ps0zenwc7zccv",
            "tb1qq8jfvz4fzc83jrysyrtj83sn4ps0zenwjcetrl"
        ]
    }"#;

    const TEST_ACCOUNT_2: &str = r#"
    {
        "uuid": "0d2fe06d-570f-4eda-9746-1316685ba75b",
        "name": "account2",
        "mnemonic": "9dfb1e817d1d0b261a62cba4681c92c5982ca59f7ad5be5087107ab26fedd2c996481a34f5968ebbdcf4f53f00a03ed3b1afdbcd2fd8c45559ccdc354f6c4ee02bf33ca1c4b3d9790b3b741771ed518d1b5ed6a2a9458c19f82a20fabaf8220d79083531cc3be119d30a575dc3b20c6e734238a25bf68051881480bfb5f86310c49b13e19d4b5ee7fedb23070ff1b89bf26c091e0f879052007141dd416a37ac",
        "network": "test",
        "address": [
            "bc1qll0600sj066n8lptxnshuu24vq3hgvs7vrs6f8",
            "tb1qll0600sj066n8lptxnshuu24vq3hgvs7x9tfj5"
        ]
    }
    "#;

    async fn build_transaction_1to2() -> Result<TxDetail> {
        let acnt_1: address::Info = serde_json::from_str(TEST_ACCOUNT_1)?;
        let acnt_2: address::Info = serde_json::from_str(TEST_ACCOUNT_2)?;

        let mnemonic = acnt_1.decrypt_mnemonic(PASSWORD)?;
        println!("{mnemonic}");

        let tx_info = sendinfo::Info {
            recipient_address: acnt_2.address.1.clone(),
            send_amount: 10_000,
            max_send_amount: 100_000,
            fee_rate: 3,
            max_fee_rate: 10,
            max_fee_amount: 1_000_000,
        };

        build(PASSWORD, acnt_1, tx_info).await
    }

    async fn build_transaction_2to1() -> Result<TxDetail> {
        let acnt_1: address::Info = serde_json::from_str(TEST_ACCOUNT_1)?;
        let acnt_2: address::Info = serde_json::from_str(TEST_ACCOUNT_2)?;

        let tx_info = sendinfo::Info {
            recipient_address: acnt_1.address.1.clone(),
            send_amount: 1_0000,
            max_send_amount: 100_000,
            fee_rate: 3,
            max_fee_rate: 10,
            max_fee_amount: 1_000_000,
        };

        build(PASSWORD, acnt_2, tx_info).await
    }

    async fn build_transaction_to_pkh() -> Result<TxDetail> {
        let acnt_1: address::Info = serde_json::from_str(TEST_ACCOUNT_1)?;

        let tx_info = sendinfo::Info {
            recipient_address: "msFbCzXbGxdeFRp6zm4WJZozm7akFSGRXg".to_string(),
            send_amount: 1,
            max_send_amount: 100_000,
            fee_rate: 3,
            max_fee_rate: 10,
            max_fee_amount: 1_000_000,
        };

        build(PASSWORD, acnt_1, tx_info).await
    }

    #[tokio::test]
    async fn test_build_transaction() -> Result<()> {
        let tx_detail = build_transaction_1to2().await?;

        println!(
            "You should now be able to broadcast the following transaction: \n{}\nfee:{}",
            tx_detail.tx_hex, tx_detail.fee_amount
        );

        Ok(())
    }

    #[tokio::test]
    async fn test_broadcast_transaction_1to2() -> Result<()> {
        let tx_detail = build_transaction_1to2().await?;
        println!(
            "tx hex: \n{}\nfee:{}",
            tx_detail.tx_hex, tx_detail.fee_amount
        );

        let res = broadcast_transaction("test", tx_detail.tx_hex).await?;
        println!("txid: {res}");

        Ok(())
    }

    #[tokio::test]
    async fn test_broadcast_transaction_2to1() -> Result<()> {
        let tx_detail = build_transaction_2to1().await?;
        println!(
            "tx hex: \n{}\nfee:{}",
            tx_detail.tx_hex, tx_detail.fee_amount
        );

        let res = broadcast_transaction("test", tx_detail.tx_hex).await?;
        println!("txid: {res}");

        Ok(())
    }

    #[tokio::test]
    async fn test_broadcast_transaction_to_pkh() -> Result<()> {
        let tx_detail = build_transaction_to_pkh().await?;
        println!(
            "tx hex: \n{}\nfee:{}",
            tx_detail.tx_hex, tx_detail.fee_amount
        );

        let res = broadcast_transaction("test", tx_detail.tx_hex).await?;
        println!("txid: {res}");

        Ok(())
    }

    #[test]
    fn test_parse_tx() -> Result<()> {
        let hex_tx = "0200000000010298375ac8abe48596820881bb3680f3b0c188b951aba9bef71da9132aa72f51e40000000000ffffffffbffba2c7217503b46cf36816e04d717f523945d8ed634b8ddbf17fc6de1306a50000000000ffffffff02102700000000000016001401e4960aa9160f190c9020d723c613a860f1666e4225000000000000160014ffdfa7be127eb533fc2b34e17e7155602374321e0247304402201f3e52861f0f8b00f6c7e765d08f4b35e1389036a3ab3b6f48bebba8135ad5bb02204226566461e1a308f8df5b9e27aaea3e263fad3ccbaecbc271255c9b434fa3040121031a88cf3b115c06567ed3c3c4e997c52bd37f012f220c238e2e957871ab16989602483045022100a4bf46848bf833c4b72550f326d26e92b4b9ad203ef3af05e06282a7c1f5d22d02205751131ea0aebb7fd5bad56cd83c4d193fa4af64be4dadee42767374388d3fda0121031a88cf3b115c06567ed3c3c4e997c52bd37f012f220c238e2e957871ab16989600000000";

        let raw_tx = parse_tx(hex_tx)?;
        println!("{raw_tx}");

        Ok(())
    }

    #[test]
    fn test_is_valid_txid() {
        assert!(is_valid_txid(
            "8a3c64494a2a9815e5116de1007a95dcb637e86ec4b6654356e5b485c7bbaa14"
        ));
        assert!(!is_valid_txid(
            "8a3c64494a2a9815e5116de1007a95dcb637e86ec4b6654356e5b4=5c7bbaa14"
        ));
        assert!(!is_valid_txid(
            "8a3c64494a2a9815e5116de1007a95dcb637e86ec4b6654356e5b485 7bbaa14"
        ));
        assert!(!is_valid_txid(
            "8a3c64494a2a9815e5116de1007a95dcb637e86ec4b6654356e5b4857bbaa1"
        ));
    }
}
