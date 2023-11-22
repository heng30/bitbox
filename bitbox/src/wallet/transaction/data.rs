use anyhow::Result;
use bitcoin::amount::Amount;

#[derive(Clone, Debug, PartialEq)]
pub struct TxInfo {
    pub recipient_address: String,
    pub send_amount: u64,     // satoshi
    pub max_send_amount: u64, // satoshi
    pub fee_rate: u64,        //Set your desired fee rate here (in satoshis per byte)
    pub max_fee_rate: u64,    // satoshi
    pub max_fee_amount: u64,  // satoshi
}

impl TxInfo {
    pub fn from_btc(mut self, send_amount: f64, max_send_amount: f64) -> Result<Self> {
        self.send_amount = Amount::from_btc(send_amount)?.to_sat();
        self.max_send_amount = Amount::from_btc(max_send_amount)?.to_sat();
        Ok(self)
    }
}
