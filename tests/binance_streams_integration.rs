use feeder_service::binance::{calc_spike, parse_agg_trade};
use feeder_service::binance_depth::{build_depth_streams, parse_depth_update};

#[test]
fn parse_agg_trade_valid_payload() {
    let payload = r#"{"stream":"btcusdt@aggTrade","data":{"e":"aggTrade","E":1710000000000,"s":"BTCUSDT","p":"43000.50","q":"0.1200","T":1710000000010,"m":true}}"#;
    let trade = parse_agg_trade(payload).expect("expected a parsed aggTrade payload");

    assert_eq!(trade.s, "BTCUSDT");
    assert_eq!(trade.p, "43000.50");
    assert_eq!(trade.q, "0.1200");
    assert_eq!(trade.T, 1710000000010);
    assert!(trade.m);
}

#[test]
fn parse_agg_trade_rejects_depth_payload() {
    let payload = r#"{"stream":"btcusdt@depth20@100ms","data":{"e":"depthUpdate","E":1672515782136,"s":"BTCUSDT","U":157,"u":160,"b":[["24100.10","1.20"]],"a":[["24100.20","0.80"]]}}"#;
    assert!(parse_agg_trade(payload).is_none());
}

#[test]
fn parse_agg_trade_rejects_invalid_json() {
    let payload = "this is not json";
    assert!(parse_agg_trade(payload).is_none());
}

#[test]
fn calc_spike_returns_zero_without_previous_price() {
    assert_eq!(calc_spike(None, 100.0), 0.0);
}

#[test]
fn calc_spike_for_upward_move() {
    let spike = calc_spike(Some(100.0), 105.0);
    assert!((spike - 5.0).abs() < f64::EPSILON);
}

#[test]
fn calc_spike_for_downward_move() {
    let spike = calc_spike(Some(100.0), 95.0);
    assert!((spike - 5.0).abs() < f64::EPSILON);
}

#[test]
fn parse_depth_update_valid_payload() {
    let payload = r#"{"stream":"btcusdt@depth20@100ms","data":{"e":"depthUpdate","E":1672515782136,"s":"BTCUSDT","U":157,"u":160,"b":[["24100.10","1.20"],["24100.00","2.10"]],"a":[["24100.20","0.80"],["24100.30","1.75"]]}}"#;
    let depth = parse_depth_update(payload).expect("expected a parsed depth payload");

    assert_eq!(depth.symbol, "BTCUSDT");
    assert_eq!(depth.event_time, 1672515782136);
    assert_eq!(depth.first_update_id, 157);
    assert_eq!(depth.final_update_id, 160);
    assert_eq!(depth.bids.len(), 2);
    assert_eq!(depth.asks.len(), 2);
}

#[test]
fn parse_depth_update_handles_empty_levels() {
    let payload = r#"{"stream":"btcusdt@depth20@100ms","data":{"e":"depthUpdate","E":1672515782136,"s":"BTCUSDT","U":157,"u":160,"b":[],"a":[]}}"#;
    let depth = parse_depth_update(payload).expect("expected a parsed depth payload");

    assert!(depth.bids.is_empty());
    assert!(depth.asks.is_empty());
}

#[test]
fn parse_depth_update_rejects_agg_trade_payload() {
    let payload = r#"{"stream":"btcusdt@aggTrade","data":{"e":"aggTrade","E":1710000000000,"s":"BTCUSDT","p":"43000.50","q":"0.1200","T":1710000000010,"m":true}}"#;
    assert!(parse_depth_update(payload).is_none());
}

#[test]
fn parse_depth_update_rejects_invalid_json() {
    assert!(parse_depth_update("not json").is_none());
}

#[test]
fn build_depth_streams_lowercases_symbols() {
    let symbols = vec!["BTCUSDT".to_string(), "EthUsdt".to_string()];
    let streams = build_depth_streams(&symbols, 20, 100);

    assert_eq!(
        streams,
        vec![
            "btcusdt@depth20@100ms".to_string(),
            "ethusdt@depth20@100ms".to_string(),
        ]
    );
}

#[test]
fn build_depth_streams_supports_custom_speed_and_levels() {
    let symbols = vec!["solusdt".to_string()];
    let streams = build_depth_streams(&symbols, 5, 250);

    assert_eq!(streams, vec!["solusdt@depth5@250ms".to_string()]);
}
