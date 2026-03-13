use std::sync::{Arc, Mutex};
use std::time::Duration;

use feeder_service::config::TelegramConfig;
use feeder_service::news::correlation::MatchedNews;
use feeder_service::notify::{
    NotificationFanout, build_signal_notification, telegram::TelegramNotifier,
};
use serde_json::json;
use tokio::sync::broadcast;
use warp::Filter;

async fn wait_for_message_count(
    captured: &Arc<Mutex<Vec<String>>>,
    expected: usize,
    timeout: Duration,
) {
    let start = std::time::Instant::now();
    loop {
        if captured.lock().unwrap().len() >= expected {
            return;
        }

        assert!(
            start.elapsed() < timeout,
            "timed out waiting for {expected} Telegram message(s)"
        );

        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

#[tokio::test]
async fn sends_telegram_message_with_compact_template_and_links() {
    let captured: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let captured_filter = warp::any().map({
        let captured = captured.clone();
        move || captured.clone()
    });

    let route = warp::path!("bottoken123" / "sendMessage")
        .and(warp::post())
        .and(warp::body::json())
        .and(captured_filter)
        .map(
            |body: serde_json::Value, captured: Arc<Mutex<Vec<String>>>| {
                let text = body["text"].as_str().unwrap_or_default().to_string();
                captured.lock().unwrap().push(text);
                warp::reply::json(&json!({"ok": true}))
            },
        );

    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind test listener");
    let addr = listener.local_addr().expect("local addr");
    drop(listener);

    let server = tokio::spawn(warp::serve(route).run(addr));

    let notifier = TelegramNotifier::new(TelegramConfig {
        enabled: true,
        bot_token: Some("token123".to_string()),
        chat_id: Some("-100123".to_string()),
        thread_id: None,
        include_bigmove: false,
        debounce_window_secs: 45,
        min_correlation_score: 0.4,
        rate_limit_interval_secs: 0,
        api_base_url: format!("http://{}", addr),
    });

    let fanout = NotificationFanout::new(Some(notifier));
    let (tx, mut rx) = broadcast::channel(8);

    let notification = build_signal_notification(
        "kline_quant",
        "btcusdt",
        1_710_000_000_000,
        json!({"return_pct": 2.31}),
        &[MatchedNews {
            headline: "ETF approvals drive demand".to_string(),
            url: "https://example.com/story".to_string(),
            published_at: 1_710_000_000_100,
        }],
        0.8,
    );

    fanout.dispatch(&tx, notification).await;

    let ws_payload = rx.recv().await.expect("ws payload");
    assert!(ws_payload.contains("\"signal_type\":\"kline_quant\""));

    wait_for_message_count(&captured, 1, Duration::from_secs(2)).await;

    let sent = captured.lock().unwrap().clone();
    assert_eq!(sent.len(), 1);
    assert!(sent[0].contains("BTCUSDT KLINE_QUANT"));
    assert!(sent[0].contains("BULLISH | move +2.31%"));
    assert!(sent[0].contains("ETF approvals drive demand"));
    assert!(sent[0].contains("https://example.com/story"));

    server.abort();
}

#[tokio::test]
async fn telegram_failures_do_not_interrupt_ws_delivery() {
    let notifier = TelegramNotifier::new(TelegramConfig {
        enabled: true,
        bot_token: Some("token123".to_string()),
        chat_id: Some("-100123".to_string()),
        thread_id: None,
        include_bigmove: false,
        debounce_window_secs: 45,
        min_correlation_score: 0.0,
        rate_limit_interval_secs: 0,
        api_base_url: "http://127.0.0.1:9".to_string(),
    });

    let fanout = NotificationFanout::new(Some(notifier));
    let (tx, mut rx) = broadcast::channel(8);

    let notification = build_signal_notification(
        "agg_trade",
        "ethusdt",
        1_710_000_000_000,
        json!({"spike_pct": -0.67}),
        &[],
        0.2,
    );

    let dispatch = tokio::spawn(async move {
        fanout.dispatch(&tx, notification).await;
    });

    let result = tokio::time::timeout(Duration::from_millis(200), rx.recv())
        .await
        .expect("ws message should be available quickly")
        .expect("payload");

    assert!(result.contains("\"signal_type\":\"agg_trade\""));

    dispatch.await.expect("dispatch task complete");
}
