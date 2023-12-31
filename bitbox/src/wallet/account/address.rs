extern crate rand;

use crate::util;
use anyhow::{anyhow, Result};
use bip32::{Language, Mnemonic, Seed};
use bitcoin::bip32::{IntoDerivationPath, Xpriv, Xpub};
use bitcoin::secp256k1::Secp256k1;
use bitcoin::{Address, Network};
use rand_core::OsRng;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const ACCOUNT_DERIVATION_PATH: &str = "m/0'/0'";

#[derive(Deserialize, Serialize, Clone, Debug, Default, PartialEq)]
pub struct Info {
    pub uuid: String,
    pub name: String,
    pub mnemonic: String,
    pub network: String,
    pub address: (String, String),
}

impl Info {
    pub fn new(name: &str, password: &str, mnemonic: &str) -> Result<Self> {
        let mn = Mnemonic::new(mnemonic, Language::English)?;
        let seed = mn.to_seed("");
        let secp = Secp256k1::new();

        let mut address = vec![];
        for network_core_arg in ["main", "test"] {
            let network = Network::from_core_arg(network_core_arg)?;
            let xpriv = Xpriv::new_master(network, seed.as_bytes())?;

            let path = ACCOUNT_DERIVATION_PATH.into_derivation_path()?;
            let account_xpriv = xpriv.derive_priv(&secp, &path)?;
            let account_xpub = Xpub::from_priv(&secp, &account_xpriv);

            address.push(Address::p2wpkh(&account_xpub.to_pub(), network)?.to_string());
        }

        Ok(Self {
            uuid: Uuid::new_v4().to_string(),
            name: name.to_string(),
            mnemonic: util::crypto::encrypt(password, mnemonic.as_bytes())?,
            network: "main".to_string(),
            address: (address[0].clone(), address[1].clone()),
        })
    }
    #[allow(dead_code)]
    pub fn recover(password: &str, mnemonic: &str) -> Result<Self> {
        Self::new("account_0", password, mnemonic)
    }

    pub fn mnemonic() -> String {
        let mnemonic = Mnemonic::random(OsRng, Default::default());
        mnemonic.phrase().to_string()
    }

    pub fn decrypt_mnemonic(&self, password: &str) -> Result<String> {
        let mnemonic = util::crypto::decrypt(password, &self.mnemonic)?;
        Ok(String::from_utf8_lossy(&mnemonic).to_string())
    }

    pub fn seed(&self, password: &str) -> Result<Seed> {
        let mnemonic = self.decrypt_mnemonic(password)?;
        let mnemonic = Mnemonic::new(mnemonic, Language::English)?;
        Ok(mnemonic.to_seed(""))
    }

    pub fn verify(&self, account_xpub: &Xpub) -> Result<()> {
        let mut address = vec![];
        for network_core_arg in ["main", "test"] {
            let network = Network::from_core_arg(network_core_arg)?;
            address.push(Address::p2wpkh(&account_xpub.to_pub(), network)?.to_string());
        }

        if self.address.0 != address[0] {
            return Err(anyhow!(format!(
                "main network different address: {} <-> {}",
                self.address.0, address[0]
            )));
        }

        if self.address.1 != address[1] {
            return Err(anyhow!(format!(
                "test network different address: {} <-> {}",
                self.address.1, address[1]
            )));
        }

        Ok(())
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
        println!("{}", serde_json::to_string_pretty(&info)?);

        Ok(())
    }

    #[test]
    fn test_mnemonic() {
        let mnemonic = Info::mnemonic();
        println!("{mnemonic}");
    }

    #[test]
    fn test_decrypt_mnemonic() -> Result<()> {
        let mnemonic_1 = Info::mnemonic();
        let info = Info::new("account1", PASSWORD, &mnemonic_1)?;
        let mnemonic_2 = info.decrypt_mnemonic(PASSWORD)?;

        println!("{mnemonic_2}");
        assert_eq!(mnemonic_1, mnemonic_2);

        Ok(())
    }
}
