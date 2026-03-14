use super::*;

#[test]
fn parse_funding_rate_from_combined_stream() {
    let msg = r#"{"stream":"btcusdt@markPrice@1s","data":{"e":"markPriceUpdate","E":1710000000100,"s":"BTCUSDT","p":"43000.5","i":"42990.0","P":"44000.0","r":"0.00120000","T":1710003600000}}"#;
    let funding = parse_funding_rate_update(msg).expect("funding rate should parse");

    assert_eq!(funding.symbol, "BTCUSDT");
    assert_eq!(funding.event_time, 1710000000100);
    assert_eq!(funding.funding_rate, "0.00120000");
    assert_eq!(funding.next_funding_time, 1710003600000);
}

#[test]
fn parse_funding_rate_rejects_other_payload_type() {
    let msg = r#"{"stream":"btcusdt@aggTrade","data":{"e":"aggTrade","E":1710000000000,"s":"BTCUSDT"}}"#;
    assert!(parse_funding_rate_update(msg).is_none());
}

#[test]
fn funding_rate_pct_converts_decimal_to_percent() {
    assert_eq!(funding_rate_pct("0.001"), Some(0.1));
    assert_eq!(funding_rate_pct("-0.0025"), Some(-0.25));
}

#[test]
fn high_funding_rate_detection_uses_absolute_threshold() {
    assert!(is_high_funding_rate(0.16, 0.1));
    assert!(is_high_funding_rate(-0.25, 0.1));
    assert!(!is_high_funding_rate(0.05, 0.1));
    assert!(!is_high_funding_rate(0.2, -0.1));
}
