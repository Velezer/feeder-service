use feeder_service::{
    binance_depth::DepthUpdate,
    binance_kline::parse_kline_event,
    config::{Config, SymbolConfig, TelegramConfig},
    refactor::AppState,
};
use tokio::sync::broadcast;

fn depth(symbol: &str, event_time: u64, bid_qty: &str, ask_qty: &str) -> DepthUpdate {
    DepthUpdate {
        symbol: symbol.to_string(),
        bids: vec![["100.0".to_string(), bid_qty.to_string()]],
        asks: vec![["100.0".to_string(), ask_qty.to_string()]],
        event_time,
        first_update_id: 1,
        final_update_id: 2,
    }
}

#[tokio::test]
async fn quant_vector_uses_closed_4h_kline_and_stays_separate_from_depth() {
    let config = Config {
        symbols: vec![SymbolConfig {
            symbol: "btcusdt".to_string(),
            big_trade_qty: 1.0,
            spike_pct: 0.4,
        }],
        port: 9001,
        broadcast_capacity: 64,
        big_depth_min_qty: 0.0,
        big_depth_min_notional: 0.0,
        big_depth_min_pressure_pct: 0.0,
        disable_depth_stream: false,
        telegram: TelegramConfig {
            enabled: false,
            bot_token: None,
            chat_id: None,
            thread_id: None,
            include_bigmove: false,
            debounce_window_secs: 45,
        },
    };

    let mut app = AppState::new(config);
    let (tx, mut rx) = broadcast::channel(64);

    // Depth activity should not produce QUANT4H messages.
    app.process_depth_update(&depth("BTCUSDT", 1710000000000, "8.0", "6.0"), &tx);

    // Closed 4h kline should produce QUANT4H message.
    let payload = r#"{
        "stream": "btcusdt@kline_4h",
        "data": {
            "e": "kline",
            "E": 1710000900000,
            "s": "BTCUSDT",
            "k": {
                "t": 1710000000000,
                "T": 1710014399999,
                "i": "4h",
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

    let mut seen_quant = false;
    while let Ok(msg) = rx.try_recv() {
        if msg.contains("[QUANT4H]") {
            seen_quant = true;
            assert!(msg.contains("BTCUSDT"));
            assert!(msg.contains("window=1710000000000..1710014399999"));
            assert!(msg.contains("ret=+2.50%"));
            assert!(msg.contains("taker_buy=56.9%"));
        }
    }

    assert!(seen_quant, "expected quant signal from closed 4h kline");
}
