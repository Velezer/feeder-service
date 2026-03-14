use feeder_service::binance::AggTrade;
use feeder_service::config::{Config, NewsConfig, SymbolConfig, TelegramConfig};
use feeder_service::news::store::NewsStore;
use feeder_service::news::tagging::tag_symbols;
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
async fn correlates_real_tagged_news_for_app_state_signals() {
    let db_path = test_db_path("news-price-correlation");
    let store = NewsStore::new(db_path.clone());
    store.init().expect("db initialized");

    let mut btc_story = NewsItem {
        id: "article-btc-1".to_string(),
        source: "seed".to_string(),
        published_at: 1_710_000_000_500,
        title: "Bitcoin derivatives open interest climbs".to_string(),
        summary: "Traders expect momentum continuation".to_string(),
        url: "https://example.com/bitcoin-open-interest".to_string(),
        symbols: vec![],
        sentiment_score: Some(0.7),
    };
    tag_symbols(&mut btc_story);

    let mut second_btc_story = NewsItem {
        id: "article-btc-2".to_string(),
        source: "seed".to_string(),
        published_at: 1_710_000_000_700,
        title: "Bitcoin ETF inflows stay elevated".to_string(),
        summary: "Institutional demand remains strong".to_string(),
        url: "https://example.com/bitcoin-etf-inflows".to_string(),
        symbols: vec![],
        sentiment_score: Some(0.8),
    };
    tag_symbols(&mut second_btc_story);

    let mut unrelated_story = NewsItem {
        id: "article-eth-1".to_string(),
        source: "seed".to_string(),
        published_at: 1_710_000_000_900,
        title: "Ethereum staking yields flatten".to_string(),
        summary: "Yield seekers rotate to L2s".to_string(),
        url: "https://example.com/ethereum-staking".to_string(),
        symbols: vec![],
        sentiment_score: Some(0.2),
    };
    tag_symbols(&mut unrelated_story);

    store
        .upsert_many(&[btc_story, second_btc_story, unrelated_story])
        .expect("seed tagged news");

    let config = Config {
        symbols: vec![SymbolConfig {
            symbol: "btcusdt".to_string(),
            big_trade_qty: 0.1,
            spike_pct: 0.0,
        }],
        port: 9001,
        broadcast_capacity: 32,
        big_depth_min_qty: 0.0,
        big_depth_min_notional: 0.0,
        big_depth_min_pressure_pct: 0.0,
        disable_depth_stream: false,
        enable_funding_rate: false,
        funding_rate_alert_pct: 0.1,
        funding_rate_cooldown_secs: 300,
        corr_min_move_pct: 0.25,
        corr_max_lag_seconds: 300,
        corr_min_confidence: 0.6,
        news_streams: vec![],
        news: NewsConfig {
            enabled: true,
            db_path: db_path.clone(),
            poll_interval_secs: 60,
            retention_hours: 24,
            finnhub_api_key: None,
            newsapi_api_key: None,
        },
        telegram: TelegramConfig {
            enabled: false,
            bot_token: None,
            chat_id: None,
            thread_id: None,
            include_bigmove: false,
            debounce_window_secs: 45,
            min_correlation_score: 0.0,
            rate_limit_interval_secs: 0,
            api_base_url: "https://api.telegram.org".to_string(),
        },
    };

    let mut app_state = AppState::new(config);
    let (tx, mut rx) = broadcast::channel(32);

    app_state
        .process_agg_trade(
            &AggTrade {
                s: "BTCUSDT".to_string(),
                p: "43000.0".to_string(),
                q: "0.25".to_string(),
                t: 1_710_000_001_000,
                m: false,
            },
            &tx,
        )
        .await;

    let first = rx.recv().await.expect("plain-text signal");
    assert!(first.contains("[AGG_TRADE]"));

    let second = rx.recv().await.expect("enriched signal");
    let payload: serde_json::Value = serde_json::from_str(&second).expect("json payload");

    assert_eq!(payload["signal_type"], "agg_trade");
    assert_eq!(payload["symbol"], "BTCUSDT");

    let matches = payload["matched_news"].as_array().expect("news array");
    assert_eq!(matches.len(), 2);
    assert_eq!(matches[0]["headline"], "Bitcoin ETF inflows stay elevated");
    assert_eq!(
        matches[1]["headline"],
        "Bitcoin derivatives open interest climbs"
    );

    let score = payload["correlation_score"]
        .as_f64()
        .expect("correlation score");
    assert!(
        (score - 0.4).abs() < f64::EPSILON,
        "unexpected score: {score}"
    );

    let _ = std::fs::remove_file(db_path);
}
