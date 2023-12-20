use anyhow::{anyhow, Result};
use bitcoin::amount::Amount;
use bitcoin::Denomination;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Info {
    pub recipient_address: String,
    pub send_amount: u64,     // satoshi
    pub max_send_amount: u64, // satoshi
    pub fee_rate: u64,        // Set your desired fee rate here (in satoshis per byte)
    pub max_fee_rate: u64,    // satoshi
    pub max_fee_amount: u64,  // satoshi
}

impl Info {
    pub fn amount_from_btc(mut self, send_amount: &str, max_send_amount: &str) -> Result<Self> {
        self.send_amount = Amount::from_str_in(send_amount, Denomination::Bitcoin)?.to_sat();
        self.max_send_amount =
            Amount::from_str_in(max_send_amount, Denomination::Bitcoin)?.to_sat();
        Ok(self)
    }

    pub fn verify(&self) -> Result<()> {
        if self.send_amount > self.max_send_amount {
            return Err(anyhow!(
                "send_amount > max_send_amount. {} > {}",
                self.send_amount,
                self.max_send_amount
            ));
        }

        if self.fee_rate > self.max_fee_rate {
            return Err(anyhow!(
                "fee_rate > max_fee_rate. {} > {}",
                self.fee_rate,
                self.max_fee_rate
            ));
        }

        Ok(())
    }

    pub fn verify_max_fee_amount(&self, fee_amount: u64) -> Result<()> {
        if fee_amount > self.max_fee_amount {
            return Err(anyhow!(
                "fee_amount > max_fee_amount. {} > {}",
                fee_amount,
                self.max_fee_amount
            ));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_amount_from_btc() -> Result<()> {
        let txinfo = Info::default().amount_from_btc("0.12345678", "1.2345678")?;
        assert_eq!(txinfo.send_amount, 12345678);
        assert_eq!(txinfo.max_send_amount, 123456780);
        Ok(())
    }
}
