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
