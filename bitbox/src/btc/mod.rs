use crate::util;
use anyhow::{anyhow, Result};
use serde_json::Value;
use std::collections::HashMap;
use std::time::Duration;

// return satoshi
pub async fn price() -> Result<f64> {
    const URL: &str = "https://api.alternative.me/v2/ticker/bitcoin/";
    let client = util::http::client()?;

    let response = client
        .get(URL)
        .timeout(Duration::from_secs(15))
        .send()
        .await?
        .json::<Value>()
        .await?;

    // let price = response["data"]["1"]["quotes"]["USD"]["price"]
    let price = response
        .get("data")
        .ok_or(anyhow!("no data"))?
        .get("1")
        .ok_or(anyhow!("no index"))?
        .get("quotes")
        .ok_or(anyhow!("no quotes"))?
        .get("USD")
        .ok_or(anyhow!("no USD"))?
        .get("price")
        .ok_or(anyhow!("no price"))?
        .as_f64()
        .ok_or(anyhow!("invalid price format"))?;

    Ok(price)
}

// (low, middle, high)
pub async fn feerate() -> Result<(u64, u64, u64)> {
    const URL: &str = "https://blockstream.info/api/fee-estimates";
    let client = util::http::client()?;

    let mut response = client
        .get(URL)
        .timeout(Duration::from_secs(15))
        .send()
        .await?
        .json::<HashMap<String, f64>>()
        .await?
        .into_values()
        .map(|v| v as u64)
        .collect::<Vec<u64>>();

    response.sort_by(|a, b| a.partial_cmp(b).unwrap());
    match response.len() {
        0 => Err(anyhow!("no feerate provided")),
        1 => Ok((response[0], response[0], response[0])),
        2 => Ok((response[0], response[0], response[1])),
        _ => {
            let low = response[0];
            let middle = response[response.len() / 2];
            let high = *response.last().unwrap();
            Ok((low, middle, high))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_price() -> Result<()> {
        let price = super::price().await?;
        assert!(price > 0);

        println!("{}", price);
        Ok(())
    }

    #[tokio::test]
    async fn test_feerate() -> Result<()> {
        let (low, middle, high) = super::feerate().await?;
        assert!(low > 0);
        assert!(middle >= low);
        assert!(high >= middle);

        // println!("{}, {}, {}", low, middle, high);
        Ok(())
    }
}
