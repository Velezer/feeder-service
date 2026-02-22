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


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_depth_update_from_combined_stream() {
        let msg = r#"{"stream":"btcusdt@depth20@100ms","data":{"e":"depthUpdate","E":1672515782136,"s":"BTCUSDT","U":157,"u":160,"b":[["24100.10","1.20"]],"a":[["24100.20","0.80"]]}}"#;
        let depth = parse_depth_update(msg).expect("depth should parse");

        assert_eq!(depth.symbol, "BTCUSDT");
        assert_eq!(depth.event_time, 1672515782136);
        assert_eq!(depth.first_update_id, 157);
        assert_eq!(depth.final_update_id, 160);
        assert_eq!(depth.bids[0], ["24100.10".to_string(), "1.20".to_string()]);
        assert_eq!(depth.asks[0], ["24100.20".to_string(), "0.80".to_string()]);
    }

    #[test]
    fn build_depth_stream_names() {
        let symbols = vec!["btcusdt".to_string(), "ETHUSDT".to_string()];
        let streams = build_depth_streams(&symbols, 20, 100);

        assert_eq!(
            streams,
            vec![
                "btcusdt@depth20@100ms".to_string(),
                "ethusdt@depth20@100ms".to_string(),
            ]
        );
    }
}
