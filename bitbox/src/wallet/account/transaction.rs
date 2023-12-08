use super::super::transaction::blockstream::fetch_utxos;
use super::data::TxInfo;
use anyhow::{anyhow, Context, Result};
use bitcoin::bip32::{
    ChainCode, ChildNumber, DerivationPath, Fingerprint, IntoDerivationPath, Xpriv, Xpub,
};
use bitcoin::blockdata::locktime::absolute::LockTime;
use bitcoin::blockdata::transaction::Version;
use bitcoin::consensus::encode;
use bitcoin::locktime::absolute;
use bitcoin::psbt::{self, Input, Psbt, PsbtSighashType};
use bitcoin::secp256k1::{Secp256k1, Signing, Verification};
use bitcoin::{
    transaction, Address, Amount, Network, OutPoint, PrivateKey, ScriptBuf, Transaction, TxIn,
    TxOut, Txid, Witness,
};
use rand::seq::SliceRandom;
use std::collections::BTreeMap;
use std::str::FromStr;

const INPUT_UTXO_DERIVATION_PATH: &str = "m/0h/0h/0h";

pub async fn build(private_key: &str, tx_info: TxInfo) -> Result<String> {
    let secp = Secp256k1::new();

    let (offline, fingerprint, input_xpub) = ColdStorage::new(&secp, private_key)?;

    let mut online = WatchOnly::new(offline.master_xpub, input_xpub, fingerprint, tx_info);

    let created = online.create_psbt(&secp).await?;

    let updated = online.update_psbt(created)?;

    let signed = offline.sign_psbt(&secp, updated)?;

    let finalized = online.finalize_psbt(signed)?;

    // println!("{finalized:?}");

    let previous_output = online.previous_output()?;
    let tx = finalized.extract_tx_unchecked_fee_rate();
    tx.verify(|_| Some(previous_output.clone()))
        .expect("failed to verify transaction");

    let hex = encode::serialize_hex(&tx);
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
    fn new<C: Signing>(secp: &Secp256k1<C>, private_key: &str) -> Result<ExportData> {
        let private_key = PrivateKey::from_str(private_key)?;
        let master_xpriv = Xpriv {
            network: private_key.network,
            depth: 0,
            parent_fingerprint: Default::default(),
            child_number: ChildNumber::from_normal_idx(0)?,
            private_key: private_key.inner,
            chain_code: ChainCode::from([0; 32]),
        };

        let master_xpub = Xpub::from_priv(secp, &master_xpriv);

        // Hardened children require secret data to derive.
        let path = INPUT_UTXO_DERIVATION_PATH.into_derivation_path()?;
        let input_xpriv = master_xpriv.derive_priv(secp, &path)?;
        let input_xpub = Xpub::from_priv(secp, &input_xpriv);

        let wallet = ColdStorage {
            master_xpriv,
            master_xpub,
        };
        let fingerprint = wallet.master_fingerprint();

        Ok((wallet, fingerprint, input_xpub))
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
    master_xpub: Xpub,
    input_xpub: Xpub,
    master_fingerprint: Fingerprint,

    network: String,
    input_utxos: Vec<TxIn>,
    output_utxos: Vec<TxOut>,
    input_amount: u64,
    change_amount: u64,
    fee_amount: u64,

    tx_info: TxInfo,
}

impl WatchOnly {
    fn new(
        master_xpub: Xpub,
        input_xpub: Xpub,
        master_fingerprint: Fingerprint,
        tx_info: TxInfo,
    ) -> Self {
        WatchOnly {
            master_xpub,
            input_xpub,
            master_fingerprint,

            network: "main".to_string(),
            input_utxos: vec![],
            output_utxos: vec![],
            input_amount: 0,
            change_amount: 0,
            fee_amount: 0,

            tx_info,
        }
    }

    // Creates the PSBT, in BIP174 parlance this is the 'Creater'.
    async fn create_psbt<C: Verification>(&mut self, secp: &Secp256k1<C>) -> Result<Psbt> {
        self.build_input_output_tx().await?;

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

        for i in 0..self.input_utxos.len() {
            let mut input = Input {
                witness_utxo: Some(self.previous_output()?),
                ..Default::default()
            };

            input.redeem_script = Some(self.input_utxo_script_pubkey()?);

            let fingerprint = self.master_fingerprint;
            let path = INPUT_UTXO_DERIVATION_PATH.into_derivation_path()?;
            let mut map = BTreeMap::new();
            map.insert(self.input_xpub.to_pub().inner, (fingerprint, path));
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
            script_witness.push(self.input_xpub.to_pub().to_bytes());

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

    fn change_address(&self) -> Result<Address> {
        let network = Network::from_core_arg(&self.network)?;
        let pk = self.master_xpub.to_pub();
        Ok(Address::p2wpkh(&pk, network)?)
    }

    fn recipient_address(&self) -> Result<Address> {
        let network = Network::from_core_arg(&self.network)?;
        let addr = Address::from_str(&self.tx_info.recipient_address)?.require_network(network)?;
        Ok(addr)
    }

    fn input_utxo_script_pubkey(&self) -> Result<ScriptBuf> {
        let wpkh = self
            .input_xpub
            .to_pub()
            .wpubkey_hash()
            .context("a compressed pubkey")?;

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

    fn wallet_address(&self) -> String {
        self.master_xpub.to_pub().to_string()
    }

    async fn build_input_output_tx(&mut self) -> Result<()> {
        let mut utxos = fetch_utxos(&self.network, &self.wallet_address()).await?;
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
            script_pubkey: self.change_address()?.script_pubkey(),
        });

        self.input_utxos = inputs;
        self.output_utxos = outputs;
        self.input_amount = total_input_amount;
        self.change_amount = change_amount;
        self.fee_amount = fee_amount;
        Ok(())
    }
}
