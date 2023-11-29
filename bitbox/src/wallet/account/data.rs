use super::super::address;
use crate::db;
use crate::util;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Deserialize, Serialize, Clone, Debug, Default, PartialEq)]
pub struct Info {
    pub uuid: String,
    pub name: String,
    pub address_info: address::Info,
    pub balance: u64, // satoshi
}

impl Info {
    pub fn new(network: &str, name: &str, password: &str) -> Result<Info> {
        let mut address_info = address::info(network)?;
        let encrypt_private_key = util::crypto::encrypt(password, &address_info.private_key)?;
        address_info.private_key = encrypt_private_key;

        Ok(Info {
            uuid: Uuid::new_v4().to_string(),
            name: name.to_string(),
            address_info,
            balance: 0,
        })
    }

    pub fn decrypt_private_key(&self, password: &str) -> Result<String> {
        util::crypto::decrypt(password, &self.address_info.private_key)
    }

    pub fn save(&self) -> Result<()> {
        let text = serde_json::to_string(self)?;
        db::account::save(&self.uuid, text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_info_new() -> Result<()>{
        let info = Info::new("test", "account1", "12345678")?;
        println!("{}", serde_json::to_string_pretty(&info)?);

        Ok(())
    }
}

