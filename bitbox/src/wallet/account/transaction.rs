use super::super::transaction::blockstream::fetch_utxos;
use super::{address, tx};
use anyhow::{anyhow, Context, Result};
use bip32::Seed;
use bitcoin::bip32::{Fingerprint, IntoDerivationPath, Xpriv, Xpub};
use bitcoin::blockdata::locktime::absolute::LockTime;
use bitcoin::blockdata::transaction::Version;
use bitcoin::consensus::encode;
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

pub async fn build(
    password: &str,
    address_info: address::Info,
    tx_info: tx::Info,
) -> Result<String> {
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

    // println!("{finalized:?}");

    let previous_output = online.previous_output()?;
    let tx = finalized.extract_tx_unchecked_fee_rate();
    let hex = encode::serialize_hex(&tx);

    tx.verify(|_| Some(previous_output.clone()))
        .context(format!("failed to verify transaction. hex: {}", hex))?;

    println!(
        "You should now be able to broadcast the following transaction: \n\n{}",
        hex
    );

    Ok(hex)
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

    input_utxos: Vec<TxIn>,
    output_utxos: Vec<TxOut>,
    input_amount: u64,
    change_amount: u64,
    fee_amount: u64,

    address_info: address::Info,
    tx_info: tx::Info,
}

impl WatchOnly {
    fn new(
        account_xpub: Xpub,
        master_fingerprint: Fingerprint,
        address_info: address::Info,
        tx_info: tx::Info,
    ) -> Self {
        WatchOnly {
            account_xpub,
            master_fingerprint,

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

        for _ in 0..self.input_utxos.len() {
            let mut input = Input {
                witness_utxo: Some(self.previous_output()?),
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

    fn previous_output(&self) -> Result<TxOut> {
        Ok(TxOut {
            value: Amount::from_sat(self.input_amount),
            script_pubkey: self
                .input_utxo_script_pubkey()
                .context("failed to parse input utxo scriptPubkey")?,
        })
    }

    async fn build_input_output_tx(&mut self) -> Result<()> {
        let wallet_address = self.wallet_address()?.to_string();
        let mut utxos = fetch_utxos(&self.address_info.network, &wallet_address).await?;
        utxos.shuffle(&mut rand::thread_rng());

        let (mut inputs, mut outputs) = (vec![], vec![]);
        outputs.push(TxOut {
            value: Amount::from_sat(self.tx_info.send_amount),
            script_pubkey: self.recipient_address()?.script_pubkey(),
        });

        let (mut total_input_amount, mut change_amount, mut fee_amount) = (0, 0, 0);
        for utxo in utxos.iter() {
            let mut input = TxIn::default();
            input.previous_output = OutPoint::new(Txid::from_str(&utxo.txid)?, utxo.vout);
            inputs.push(input);

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

        self.input_utxos = inputs;
        self.output_utxos = outputs;
        self.input_amount = total_input_amount;
        self.change_amount = change_amount;
        self.fee_amount = fee_amount;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
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
