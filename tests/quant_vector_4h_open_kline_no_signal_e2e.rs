use feeder_service::{
    binance_kline::parse_kline_event,
    config::{Config, SymbolConfig, TelegramConfig},
    refactor::AppState,
};
use tokio::sync::broadcast;

#[tokio::test]
async fn quant_vector_ignores_open_4h_kline_events() {
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

    let app = AppState::new(config);
    let (tx, mut rx) = broadcast::channel(64);

    let payload = r#"{
        "stream": "btcusdt@kline_4h",
        "data": {
            "e": "kline",
            "E": 1710014399000,
            "s": "BTCUSDT",
            "k": {
                "t": 1710000000000,
                "T": 1710014399999,
                "i": "4h",
                "o": "100.0",
                "c": "99.8",
                "h": "101.0",
                "l": "99.0",
                "v": "450.0",
                "q": "44910.0",
                "n": 220,
                "x": false,
                "V": "190.0",
                "Q": "18850.0"
            }
        }
    }"#;

    let event = parse_kline_event(payload).expect("expected kline event");
    app.process_kline_event(&event, &tx);

    let mut saw_quant = false;
    while let Ok(msg) = rx.try_recv() {
        if msg.contains("[QUANT4H]") {
            saw_quant = true;
        }
    }

    assert!(
        !saw_quant,
        "open (x=false) 4h candles must not emit quant signals"
    );
}
