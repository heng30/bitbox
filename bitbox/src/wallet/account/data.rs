use super::super::address;
use crate::db;
use crate::util;
use anyhow::Result;
use uuid::Uuid;
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Clone, Debug, Default, PartialEq)]
pub struct Info {
    pub uuid: String,
    pub name: String,
    pub address_info: address::Info,
}

impl Info {
    pub fn new(network: String, name: String, password: String) -> Result<Info> {
        let mut address_info = address::info(&network)?;
        let encrypt_password = util::crypto::encrypt(&password, &address_info.private_key);
        address_info.private_key = encrypt_password;

        Ok(Info {
            uuid: Uuid::new_v4().to_string(),
            name,
            address_info,
        })
    }

    pub fn save(&self) -> Result<()> {
        let text = serde_json::to_string(self)?;
        db::account::save(&self.uuid, text)
    }
}
