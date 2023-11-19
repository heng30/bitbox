mod data;

use anyhow::Result;
use bitcoin::secp256k1::{rand, Secp256k1};
use bitcoin::{Address, Network, PrivateKey, PublicKey};
pub use data::Info;
use std::str::FromStr;

// "main" => Bitcoin, "test" => Testnet,
// "signet" => Signet, "regtest" => Regtest,
pub fn info(network_core_arg: &str) -> Result<Info> {
    let network = Network::from_core_arg(network_core_arg.to_lowercase().as_str())?;

    let keypair = Secp256k1::new().generate_keypair(&mut rand::thread_rng());
    let private_key = PrivateKey::new(keypair.0, network);
    let public_key = PublicKey::new(keypair.1);
    let pay_address = Address::p2pkh(&public_key, network);

    Ok(Info {
        network: network_core_arg.to_string(),
        private_key: private_key.to_string(),
        public_key: public_key.to_string(),
        pay_address: pay_address.to_string(),
    })
}

pub fn recover(network_core_arg: &str, private_key: &str) -> Result<Info> {
    let s = Secp256k1::new();
    let network = Network::from_core_arg(network_core_arg.to_lowercase().as_str())?;

    let private_key = PrivateKey::from_str(private_key)?;
    let public_key = private_key.public_key(&s);
    let pay_address = Address::p2pkh(&public_key, network);

    Ok(Info {
        network: network_core_arg.to_string(),
        private_key: private_key.to_string(),
        public_key: public_key.to_string(),
        pay_address: pay_address.to_string(),
    })
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_all() {
        for _ in 0..100 {
            let addr_info = super::info("main").unwrap();

            // println!("addr_info: {:?}", addr_info);

            let recover_addr_info =
                super::recover(&addr_info.network, &addr_info.private_key).unwrap();

            assert_eq!(addr_info, recover_addr_info);
        }
    }
}
