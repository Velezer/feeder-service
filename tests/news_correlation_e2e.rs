use feeder_service::binance::AggTrade;
use feeder_service::config::{Config, NewsConfig, SymbolConfig};
use feeder_service::news::store::NewsStore;
use feeder_service::news::types::NewsItem;
use feeder_service::refactor::AppState;
use tokio::sync::broadcast;

fn test_db_path(name: &str) -> String {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("valid time")
        .as_nanos();
    std::env::temp_dir()
        .join(format!("feeder-service-{name}-{nanos}.sqlite"))
        .to_string_lossy()
        .to_string()
}

#[tokio::test]
async fn emits_enriched_json_with_news_matches_for_agg_trade() {
    let db_path = test_db_path("e2e-correlation");
    let store = NewsStore::new(db_path.clone());
    store.init().expect("db initialized");

    store
        .upsert_many(&[NewsItem {
            id: "article-1".to_string(),
            source: "test".to_string(),
            published_at: 1_710_000_000_500,
            title: "Bitcoin ETF inflow surges".to_string(),
            summary: "Demand rises".to_string(),
            url: "https://example.com/bitcoin-etf".to_string(),
            symbols: vec!["BTCUSDT".to_string()],
            sentiment_score: Some(0.7),
        }])
        .expect("seed news");

    let config = Config {
        symbols: vec![SymbolConfig {
            symbol: "btcusdt".to_string(),
            big_trade_qty: 0.1,
            spike_pct: 0.2,
        }],
        port: 9001,
        broadcast_capacity: 32,
        big_depth_min_qty: 0.0,
        big_depth_min_notional: 0.0,
        big_depth_min_pressure_pct: 0.0,
        disable_depth_stream: false,
        news: NewsConfig {
            enabled: true,
            db_path: db_path.clone(),
            poll_interval_secs: 60,
            retention_hours: 24,
            finnhub_api_key: None,
            newsapi_api_key: None,
        },
    };

    let mut app_state = AppState::new(config);
    let (tx, mut rx) = broadcast::channel(32);

    let agg = AggTrade {
        s: "BTCUSDT".to_string(),
        p: "43000.0".to_string(),
        q: "0.2".to_string(),
        t: 1_710_000_001_000,
        m: false,
    };

    app_state.process_agg_trade(&agg, &tx).await;

    let first = rx.recv().await.expect("plain-text signal");
    assert!(first.contains("[AGG_TRADE]"));

    let second = rx.recv().await.expect("enriched signal");
    let payload: serde_json::Value = serde_json::from_str(&second).expect("json payload");
    assert_eq!(payload["signal_type"], "agg_trade");
    assert_eq!(payload["symbol"], "BTCUSDT");
    assert_eq!(
        payload["matched_news"][0]["headline"],
        "Bitcoin ETF inflow surges"
    );
    assert_eq!(
        payload["matched_news"][0]["url"],
        "https://example.com/bitcoin-etf"
    );
    assert!(payload["correlation_score"].as_f64().unwrap_or_default() > 0.0);

    let _ = std::fs::remove_file(db_path);
}
