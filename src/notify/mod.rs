use serde_json::json;
use tokio::sync::broadcast;

use crate::news::correlation::MatchedNews;

pub mod telegram;

#[derive(Debug, Clone)]
pub struct SignalNotification {
    pub ws_payload: String,
    pub telegram_message: String,
    pub correlation_score: f64,
}

#[derive(Debug, Clone)]
pub struct NotificationFanout {
    telegram: Option<telegram::TelegramNotifier>,
}

impl NotificationFanout {
    pub fn new(telegram: Option<telegram::TelegramNotifier>) -> Self {
        Self { telegram }
    }

    pub async fn dispatch(&self, tx: &broadcast::Sender<String>, notification: SignalNotification) {
        let _ = tx.send(notification.ws_payload);

        if let Some(telegram) = &self.telegram {
            telegram
                .send_with_retry(
                    &notification.telegram_message,
                    notification.correlation_score,
                )
                .await;
        }
    }
}

pub fn build_signal_notification(
    signal_type: &str,
    symbol: &str,
    event_timestamp: i64,
    move_metrics: serde_json::Value,
    matches: &[MatchedNews],
    correlation_score: f64,
) -> SignalNotification {
    let ws_payload = json!({
        "signal_type": signal_type,
        "symbol": symbol.to_uppercase(),
        "event_timestamp": event_timestamp,
        "move_metrics": move_metrics,
        "matched_news": matches,
        "correlation_score": correlation_score,
    })
    .to_string();

    let telegram_message = format_telegram_message(
        signal_type,
        &symbol.to_uppercase(),
        &move_metrics,
        matches,
        correlation_score,
    );

    SignalNotification {
        ws_payload,
        telegram_message,
        correlation_score,
    }
}

fn format_telegram_message(
    signal_type: &str,
    symbol: &str,
    move_metrics: &serde_json::Value,
    matches: &[MatchedNews],
    correlation_score: f64,
) -> String {
    let (direction, magnitude) = direction_and_magnitude(move_metrics);

    let mut lines = vec![
        format!("🔔 {} {}", symbol, signal_type.to_uppercase()),
        format!("{} | {}", direction, magnitude),
        format!("Correlation score: {:.2}", correlation_score),
        "News:".to_string(),
    ];

    for news in matches.iter().take(3) {
        lines.push(format!("• {}", news.headline));
        lines.push(format!("  {}", news.url));
    }

    if matches.is_empty() {
        lines.push("• No matched headlines".to_string());
    }

    lines.join("\n")
}

fn direction_and_magnitude(move_metrics: &serde_json::Value) -> (String, String) {
    if let Some(ret) = move_metrics.get("return_pct").and_then(|v| v.as_f64()) {
        let direction = if ret > 0.0 {
            "BULLISH"
        } else if ret < 0.0 {
            "BEARISH"
        } else {
            "FLAT"
        };
        return (direction.to_string(), format!("move {:+.2}%", ret));
    }

    if let Some(spike) = move_metrics.get("spike_pct").and_then(|v| v.as_f64()) {
        let direction = if spike > 0.0 {
            "UP"
        } else if spike < 0.0 {
            "DOWN"
        } else {
            "FLAT"
        };
        return (direction.to_string(), format!("spike {:+.2}%", spike));
    }

    if let Some(pressure) = move_metrics
        .get("bid_pressure_pct")
        .and_then(|v| v.as_f64())
    {
        let direction = if pressure > 50.0 { "BUY" } else { "SELL" };
        return (
            direction.to_string(),
            format!("bid pressure {:.1}%", pressure),
        );
    }

    ("N/A".to_string(), "move metrics unavailable".to_string())
}
