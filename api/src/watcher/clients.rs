use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde_json::Value;
use crate::config::Config;
use super::matching::IncomingTransfer;

pub const USDT_TRC20: &str = "TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t";
pub const USDT_ERC20: &str = "0xdAC17F958D2ee523a2206206994597C13D831ec7";
pub const USDT_BEP20: &str = "0x55d398326f99059fF775485246999027B3197955";

pub struct ChainClient {
    http: reqwest::Client,
    etherscan_key: Option<String>,
    trongrid_key: Option<String>,
}

impl ChainClient {
    pub fn new(config: &Config) -> Self {
        Self {
            http: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(10))
                .build()
                .expect("http client"),
            etherscan_key: config.etherscan_api_key.clone(),
            trongrid_key: config.trongrid_api_key.clone(),
        }
    }

    pub async fn trc20_transfers(&self, address: &str, since: DateTime<Utc>) -> Result<Vec<IncomingTransfer>, String> {
        let url = format!(
            "https://api.trongrid.io/v1/accounts/{address}/transactions/trc20\
             ?only_to=true&contract_address={USDT_TRC20}&min_timestamp={}&limit=50&only_confirmed=true",
            since.timestamp_millis()
        );
        let mut req = self.http.get(&url);
        if let Some(k) = &self.trongrid_key {
            req = req.header("TRON-PRO-API-KEY", k);
        }
        let v: Value = req.send().await.map_err(|e| e.to_string())?
            .json().await.map_err(|e| e.to_string())?;
        Ok(parse_trongrid(&v))
    }

    pub async fn evm_transfers(&self, chain_id: u32, usdt_contract: &str, address: &str) -> Result<Vec<IncomingTransfer>, String> {
        let key = self.etherscan_key.clone().unwrap_or_default();
        // Etherscan has no server-side time filter for tokentx, so we page the newest
        // transfers and rely on client-side timestamp matching in `find_matching_transfer`.
        // offset=100 (up from 50) reduces, but does not eliminate, the chance that a busy
        // address pages the matching transfer out before we see it; acceptable for MVP.
        let url = format!(
            "https://api.etherscan.io/v2/api?chainid={chain_id}&module=account&action=tokentx\
             &contractaddress={usdt_contract}&address={address}&page=1&offset=100&sort=desc&apikey={key}"
        );
        let v: Value = self.http.get(&url).send().await.map_err(|e| e.to_string())?
            .json().await.map_err(|e| e.to_string())?;
        Ok(parse_etherscan(&v))
    }
}

/// TronGrid TRC20 payload: data[].{transaction_id, to, value (int string, 6 dp), block_timestamp (ms)}
pub fn parse_trongrid(v: &Value) -> Vec<IncomingTransfer> {
    v["data"].as_array().map(|arr| {
        arr.iter().filter_map(|t| {
            Some(IncomingTransfer {
                txid: t["transaction_id"].as_str()?.to_string(),
                to: t["to"].as_str()?.to_string(),
                amount: Decimal::from_i128_with_scale(t["value"].as_str()?.parse::<i128>().ok()?, 6),
                timestamp: DateTime::from_timestamp_millis(t["block_timestamp"].as_i64()?)?,
            })
        }).collect()
    }).unwrap_or_default()
}

/// Etherscan V2 tokentx payload: result[].{hash, to, value (int string), tokenDecimal, timeStamp (s)}
pub fn parse_etherscan(v: &Value) -> Vec<IncomingTransfer> {
    v["result"].as_array().map(|arr| {
        arr.iter().filter_map(|t| {
            let decimals: u32 = t["tokenDecimal"].as_str()?.parse().ok()?;
            Some(IncomingTransfer {
                txid: t["hash"].as_str()?.to_string(),
                to: t["to"].as_str()?.to_string(),
                amount: Decimal::from_i128_with_scale(t["value"].as_str()?.parse::<i128>().ok()?, decimals),
                timestamp: DateTime::from_timestamp(t["timeStamp"].as_str()?.parse::<i64>().ok()?, 0)?,
            })
        }).collect()
    }).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn parses_trongrid_fixture() {
        let v: Value = serde_json::from_str(r#"{
            "data": [{
                "transaction_id": "aabbcc",
                "to": "TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t",
                "from": "TSender",
                "value": "92500000",
                "block_timestamp": 1752700000000,
                "token_info": {"symbol": "USDT"}
            }]
        }"#).unwrap();
        let transfers = parse_trongrid(&v);
        assert_eq!(transfers.len(), 1);
        assert_eq!(transfers[0].amount, Decimal::from_str("92.5").unwrap());
        assert_eq!(transfers[0].txid, "aabbcc");
    }

    #[test]
    fn parses_etherscan_fixture_with_18_decimals() {
        let v: Value = serde_json::from_str(r#"{
            "status": "1",
            "result": [{
                "hash": "0xdeadbeef",
                "to": "0x55d398326f99059ff775485246999027b3197955",
                "value": "92500000000000000000",
                "tokenDecimal": "18",
                "timeStamp": "1752700000"
            }]
        }"#).unwrap();
        let transfers = parse_etherscan(&v);
        assert_eq!(transfers.len(), 1);
        assert_eq!(transfers[0].amount, Decimal::from_str("92.5").unwrap());
    }

    #[test]
    fn parses_etherscan_fixture_with_6_decimals() {
        // Native ERC20 USDT reports tokenDecimal "6", unlike the 18-decimal BEP20 wrapper above.
        let v: Value = serde_json::from_str(r#"{
            "status": "1",
            "result": [{
                "hash": "0xfeedface",
                "to": "0xdAC17F958D2ee523a2206206994597C13D831ec7",
                "value": "92500000",
                "tokenDecimal": "6",
                "timeStamp": "1752700000"
            }]
        }"#).unwrap();
        let transfers = parse_etherscan(&v);
        assert_eq!(transfers.len(), 1);
        assert_eq!(transfers[0].amount, Decimal::from_str("92.5").unwrap());
        assert_eq!(transfers[0].txid, "0xfeedface");
    }

    #[test]
    fn tolerates_error_payloads() {
        let v: Value = serde_json::from_str(r#"{"status":"0","message":"NOTOK","result":"Max rate limit reached"}"#).unwrap();
        assert!(parse_etherscan(&v).is_empty());
        let v: Value = serde_json::from_str(r#"{"success": false}"#).unwrap();
        assert!(parse_trongrid(&v).is_empty());
    }
}
