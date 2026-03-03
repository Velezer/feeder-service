use crate::binance_kline::{build_kline_streams, build_quant_signal_from_kline, parse_kline_event};

#[test]
fn parses_combined_kline_payload() {
    let payload = r#"{
        "stream":"btcusdt@kline_4h",
        "data":{
            "e":"kline",
            "E":1710000000000,
            "s":"BTCUSDT",
            "k":{
                "t":1710000000000,
                "T":1710000899999,
                "i":"4h",
                "o":"100.0",
                "c":"101.0",
                "h":"102.0",
                "l":"99.0",
                "v":"200.0",
                "q":"20100.0",
                "n":100,
                "x":true,
                "V":"110.0",
                "Q":"12000.0"
            }
        }
    }"#;

    let event = parse_kline_event(payload).expect("must parse");
    let signal = build_quant_signal_from_kline(&event).expect("must build signal");

    assert_eq!(signal.symbol, "BTCUSDT");
    assert!((signal.return_pct - 1.0).abs() < 1e-6);
    assert_eq!(signal.trade_count, 100);
}

#[test]
fn kline_stream_names_are_lowercased() {
    let streams = build_kline_streams(&["BTCUSDT".into(), "EthUsdt".into()], "4h");
    assert_eq!(streams, vec!["btcusdt@kline_4h", "ethusdt@kline_4h"]);
}
