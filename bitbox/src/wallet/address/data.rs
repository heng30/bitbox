use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Clone, Debug, Default, PartialEq)]
pub struct Info {
    pub network: String,
    pub private_key: String,
    pub public_key: String,
    pub wallet_address: String,
}
