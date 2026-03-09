use feeder_service::config::TelegramConfig;
use feeder_service::news::correlation::MatchedNews;
use feeder_service::notify::{
    NotificationFanout, build_signal_notification, telegram::TelegramNotifier,
};
use serde_json::json;
use tokio::sync::broadcast;

fn env_flag_enabled(name: &str) -> bool {
    matches!(std::env::var(name).ok().as_deref(), Some("1"))
}

#[tokio::test]
async fn sends_real_telegram_alert_when_explicitly_enabled() {
    if !env_flag_enabled("TELEGRAM_E2E") {
        eprintln!("skipping Telegram e2e test; set TELEGRAM_E2E=1 to enable");
        return;
    }

    let bot_token = std::env::var("TELEGRAM_BOT_TOKEN")
        .expect("TELEGRAM_BOT_TOKEN is required when TELEGRAM_E2E=1");
    let chat_id = std::env::var("TELEGRAM_CHAT_ID")
        .expect("TELEGRAM_CHAT_ID is required when TELEGRAM_E2E=1");
    let api_base_url = std::env::var("TELEGRAM_API_BASE_URL")
        .unwrap_or_else(|_| "https://api.telegram.org".to_string());

    let notifier = TelegramNotifier::new(TelegramConfig {
        enabled: true,
        bot_token: Some(bot_token),
        chat_id: Some(chat_id),
        thread_id: None,
        include_bigmove: false,
        debounce_window_secs: 45,
        min_correlation_score: 0.0,
        rate_limit_interval_secs: 0,
        api_base_url,
    });

    let fanout = NotificationFanout::new(Some(notifier));
    let (tx, mut rx) = broadcast::channel(8);

    let notification = build_signal_notification(
        "agg_trade",
        "btcusdt",
        1_710_000_000_000,
        json!({"spike_pct": 0.67}),
        &[MatchedNews {
            headline: "Bitcoin ETF inflow remains strong".to_string(),
            url: "https://example.com/live-check".to_string(),
            published_at: 1_710_000_000_100,
        }],
        0.9,
    );

    fanout.dispatch(&tx, notification).await;

    let ws_payload = rx.recv().await.expect("ws payload");
    assert!(ws_payload.contains("\"signal_type\":\"agg_trade\""));
}
