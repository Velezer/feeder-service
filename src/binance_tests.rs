use super::*;

#[test]
fn parse_agg_trade_from_combined_stream() {
    let msg = r#"{"stream":"btcusdt@aggTrade","data":{"e":"aggTrade","E":1710000000000,"s":"BTCUSDT","p":"43000.50","q":"0.1200","T":1710000000010,"m":true}}"#;
    let agg = parse_agg_trade(msg).expect("agg trade should parse");

    assert_eq!(agg.s, "BTCUSDT");
    assert_eq!(agg.p, "43000.50");
    assert_eq!(agg.q, "0.1200");
    assert_eq!(agg.t, 1710000000010);
    assert!(agg.m);
}

#[test]
fn calc_spike_is_zero_when_no_previous_price() {
    assert_eq!(calc_spike(None, 100.0), 0.0);
}

#[test]
fn calc_spike_works_with_previous_price() {
    let spike = calc_spike(Some(100.0), 101.0);
    assert!((spike - 1.0).abs() < f64::EPSILON);
}
