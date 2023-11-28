pub mod data;
pub use data::Info;

use anyhow::Result;
use bitcoin::address::NetworkUnchecked;
use bitcoin::secp256k1::{rand, Secp256k1};
use bitcoin::{Address, Network, PrivateKey, PublicKey};
use std::str::FromStr;

// "main" => Bitcoin, "test" => Testnet,
// "signet" => Signet, "regtest" => Regtest,
pub fn info(network_core_arg: &str) -> Result<Info> {
    let network = Network::from_core_arg(network_core_arg)?;

    let keypair = Secp256k1::new().generate_keypair(&mut rand::thread_rng());
    let private_key = PrivateKey::new(keypair.0, network);
    let public_key = PublicKey::new(keypair.1);
    let wallet_address = Address::p2pkh(&public_key, network);

    Ok(Info {
        network: network_core_arg.to_string(),
        private_key: private_key.to_string(),
        public_key: public_key.to_string(),
        wallet_address: wallet_address.to_string(),
    })
}

pub fn recover(network_core_arg: &str, private_key: &str) -> Result<Info> {
    let s = Secp256k1::new();
    let network = Network::from_core_arg(network_core_arg)?;

    let private_key = PrivateKey::from_str(private_key)?;
    let public_key = private_key.public_key(&s);
    let wallet_address = Address::p2pkh(&public_key, network);

    Ok(Info {
        network: network_core_arg.to_string(),
        private_key: private_key.to_string(),
        public_key: public_key.to_string(),
        wallet_address: wallet_address.to_string(),
    })
}

pub fn is_valid_wallet_address(network_core_arg: &str, wallet_address: &str) -> Result<bool> {
    let network = Network::from_core_arg(network_core_arg)?;
    let address: Address<NetworkUnchecked> = wallet_address.parse()?;
    Ok(address.is_valid_for_network(network))
}

#[cfg(test)]
mod tests {
    use bitcoin::PrivateKey;
    use std::env;
    use std::path::Path;
    use tokio::fs::File;
    use tokio::io::{self, AsyncBufReadExt};

    #[test]
    fn test_info_and_recover() {
        for _ in 0..100 {
            let addr_info = super::info("main").unwrap();

            let recover_addr_info =
                super::recover(&addr_info.network, &addr_info.private_key).unwrap();

            assert_eq!(addr_info, recover_addr_info);
        }
    }

    #[tokio::test]
    async fn test_recover() -> anyhow::Result<()> {
        let mut dir = env::current_exe()?;
        dir.pop();
        let working_dir =
            Path::new(dir.to_str().unwrap()).join("../../../testdata/wallet-addr.csv");

        let file = File::open(working_dir).await?;
        let reader = io::BufReader::new(file);

        let mut lines = reader.lines();
        while let Some(line) = lines.next_line().await? {
            let fields: Vec<&str> = line.split(',').collect();
            // println!("{:?}", fields);

            let private_key = PrivateKey::from_wif(fields[2].trim_matches('"'))?;

            let info = super::recover("main", &private_key.to_string())?;

            assert_eq!(fields[1].trim_matches('"'), info.wallet_address);
        }

        Ok(())
    }

    #[test]
    fn test_different_address() {
        let addr_info = super::info("main").unwrap();
        for core_arg in vec!["test", "signet", "regtest"] {
            let recover_addr_info = super::recover(core_arg, &addr_info.private_key).unwrap();

            assert_ne!(addr_info.wallet_address, recover_addr_info.wallet_address);
            assert_ne!(addr_info.network, recover_addr_info.network);
            assert_eq!(addr_info.private_key, recover_addr_info.private_key);
            assert_eq!(addr_info.public_key, recover_addr_info.public_key);
        }
    }

    #[test]
    fn test_is_valid_wallet_address() {
        for core_arg in vec!["main", "test", "signet", "regtest"] {
            let addr_info = super::info(core_arg).unwrap();
            assert!(super::is_valid_wallet_address(
                addr_info.network.as_str(),
                addr_info.wallet_address.as_str(),
            )
            .unwrap());

            for core_arg in vec!["main", "test", "signet", "regtest"] {
                if addr_info.network == core_arg {
                    continue;
                }

                match addr_info.network.as_str() {
                    // main network has different wallet_address with other network
                    "main" => {
                        assert!(!super::is_valid_wallet_address(
                            core_arg,
                            addr_info.wallet_address.as_str(),
                        )
                        .unwrap());
                    }

                    _ => match core_arg {
                        // main network has different wallet_address with other network
                        "main" => {
                            assert!(!super::is_valid_wallet_address(
                                core_arg,
                                addr_info.wallet_address.as_str(),
                            )
                            .unwrap());
                        }

                        // expect for main network, other networks have the same wallet_address
                        _ => {
                            assert!(super::is_valid_wallet_address(
                                core_arg,
                                addr_info.wallet_address.as_str(),
                            )
                            .unwrap());
                        }
                    },
                }
            }
        }
    }
}
