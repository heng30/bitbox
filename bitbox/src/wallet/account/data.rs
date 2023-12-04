extern crate rand;

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
}
