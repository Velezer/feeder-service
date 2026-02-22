// File: src/binance_depth.rs
use crate::json_helpers::parse_combined_data;

#[derive(Debug, serde::Deserialize)]
pub struct DepthUpdate {
    #[serde(rename = "s")]
    pub symbol: String,
    #[serde(rename = "b")]
    pub bids: Vec<[String; 2]>,
    #[serde(rename = "a")]
    pub asks: Vec<[String; 2]>,
    #[serde(rename = "E")]
    pub event_time: u64,
    #[serde(rename = "U")]
    pub first_update_id: u64,
    #[serde(rename = "u")]
    pub final_update_id: u64,
}

pub fn parse_depth_update(msg: &str) -> Option<DepthUpdate> {
    parse_combined_data(msg)
}

pub fn build_depth_streams(symbols: &[String], levels: u16, speed_ms: u16) -> Vec<String> {
    symbols
        .iter()
        .map(|symbol| format!("{}@depth{}@{}ms", symbol.to_lowercase(), levels, speed_ms))
        .collect()
}

pub fn build_diff_depth_streams(symbols: &[String], speed_ms: u16) -> Vec<String> {
    symbols
        .iter()
        .map(|symbol| format!("{}@depth@{}ms", symbol.to_lowercase(), speed_ms))
        .collect()
}

#[cfg(test)]
#[path = "binance_depth_tests.rs"]
mod tests;
