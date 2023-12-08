extern crate rand;

use bitcoin::amount::Amount;
use bitcoin::Denomination;
use super::super::address;
use crate::db;
use crate::util;
use anyhow::Result;
use bip32::{Language, Mnemonic};
use bitcoin::bip32::{
    ChainCode, ChildNumber, DerivationPath, Fingerprint, IntoDerivationPath, Xpriv, Xpub,
};
use rand_core::OsRng;
use serde::{Deserialize, Serialize};
use std::convert::TryInto;
use uuid::Uuid;

#[derive(Deserialize, Serialize, Clone, Debug, Default, PartialEq)]
pub struct Info {
    pub uuid: String,
    pub name: String,
    pub balance: u64, // satoshi
    pub encrypt_seed: String,
}

impl Info {
    pub fn new(name: &str, password: &str, mnemonic: &str) -> Result<Info> {
        let mnemonic = Mnemonic::new(mnemonic, Language::English)?;
        let seed = mnemonic.to_seed("");

        Ok(Info {
            uuid: Uuid::new_v4().to_string(),
            name: name.to_string(),
            encrypt_seed: util::crypto::encrypt(password, seed.as_ref())?,
            ..Default::default()
        })
    }

    pub fn recover(password: &str, mnemonic: &str) -> Result<Info> {
        let name = util::crypto::random_string(5);
        Info::new(&name, password, mnemonic)
    }


    pub fn mnemonic() -> String {
        let mnemonic = Mnemonic::random(&mut OsRng, Default::default());
        mnemonic.phrase().to_string()
    }

    pub fn decrypt_seed(&self, password: &str) -> Result<Vec<u8>> {
        util::crypto::decrypt(password, &self.encrypt_seed)
    }

    pub fn save(&self) -> Result<()> {
        let text = serde_json::to_string(self)?;
        db::account::save(&self.uuid, text)
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct TxInfo {
    pub recipient_address: String,
    pub send_amount: u64,     // satoshi
    pub max_send_amount: u64, // satoshi
    pub fee_rate: u64,        // Set your desired fee rate here (in satoshis per byte)
    pub max_fee_rate: u64,    // satoshi
    pub max_fee_amount: u64,  // satoshi
}

impl TxInfo {
    pub fn amount_from_btc(mut self, send_amount: &str, max_send_amount: &str) -> Result<Self> {
        self.send_amount = Amount::from_str_in(send_amount, Denomination::Bitcoin)?.to_sat();
        self.max_send_amount =
            Amount::from_str_in(max_send_amount, Denomination::Bitcoin)?.to_sat();
        Ok(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PASSWORD: &str = "12345678";

    #[test]
    fn test_info_new() -> Result<()> {
        let mnemonic = Info::mnemonic();
        let info = Info::new("account1", PASSWORD, &mnemonic)?;
        let seed = info.decrypt_seed(PASSWORD)?;
        println!("{}", serde_json::to_string_pretty(&info)?);
        println!("{seed:?}");

        Ok(())
    }

    #[test]
    fn test_mnemonic() {
        let mnemonic = Info::mnemonic();
        println!("{mnemonic}");
    }

    #[test]
    fn test_amount_from_btc() -> Result<()> {
        let txinfo = TxInfo::default().amount_from_btc("0.12345678", "1.2345678")?;
        assert_eq!(txinfo.send_amount, 12345678);
        assert_eq!(txinfo.max_send_amount, 123456780);
        Ok(())
    }
}
