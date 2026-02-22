// File: src/binance_depth.rs
use simd_json::serde::from_slice;

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

#[derive(Debug, serde::Deserialize)]
struct CombinedDepthStreamMsg {
    data: DepthUpdate,
}

pub fn parse_depth_update(msg: &str) -> Option<DepthUpdate> {
    let mut bytes = msg.as_bytes().to_vec();
    let wrapper: CombinedDepthStreamMsg = from_slice(&mut bytes).ok()?;
    Some(wrapper.data)
}

pub fn build_depth_streams(symbols: &[String], levels: u16, speed_ms: u16) -> Vec<String> {
    symbols
        .iter()
        .map(|symbol| format!("{}@depth{}@{}ms", symbol.to_lowercase(), levels, speed_ms))
        .collect()
}
