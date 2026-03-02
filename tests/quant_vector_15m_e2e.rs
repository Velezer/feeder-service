use feeder_service::{
    binance_kline::parse_kline_event,
    config::{Config, SymbolConfig},
    refactor::AppState,
};
use tokio::sync::broadcast;

#[tokio::test]
async fn quant_vector_uses_closed_15m_kline_and_broadcasts_signal() {
    let config = Config {
        symbols: vec![SymbolConfig {
            symbol: "btcusdt".to_string(),
            big_trade_qty: 1.0,
            spike_pct: 0.4,
        }],
        port: 9001,
        broadcast_capacity: 32,
        big_depth_min_qty: 0.0,
        big_depth_min_notional: 0.0,
        big_depth_min_pressure_pct: 0.0,
        disable_depth_stream: false,
    };

    let app = AppState::new(config);
    let (tx, mut rx) = broadcast::channel(32);

    let payload = r#"{
        "stream": "btcusdt@kline_15m",
        "data": {
            "e": "kline",
            "E": 1710000900000,
            "s": "BTCUSDT",
            "k": {
                "t": 1710000000000,
                "T": 1710000899999,
                "i": "15m",
                "o": "100.0",
                "c": "102.5",
                "h": "103.0",
                "l": "99.5",
                "v": "250.0",
                "q": "25500.0",
                "n": 120,
                "x": true,
                "V": "140.0",
                "Q": "14500.0"
            }
        }
    }"#;

    let event = parse_kline_event(payload).expect("expected kline event");
    app.process_kline_event(&event, &tx);

    let mut quant_msg = None;
    while let Ok(msg) = rx.try_recv() {
        if msg.contains("[QUANT15M]") {
            quant_msg = Some(msg);
            break;
        }
    }

    let quant_msg = quant_msg.expect("expected quant15m message");
    assert!(quant_msg.contains("BTCUSDT"));
    assert!(quant_msg.contains("window=1710000000000..1710000899999"));
    assert!(quant_msg.contains("ret=+2.50%"));
    assert!(quant_msg.contains("taker_buy=56.9%"));
}
