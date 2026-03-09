use crate::config::TelegramConfig;
use reqwest::Client;
use serde::Serialize;
use std::collections::HashMap;
use std::time::{Duration, Instant};
use tokio::sync::broadcast;

#[derive(Clone)]
pub struct TelegramNotifier {
    client: Client,
    config: TelegramConfig,
    dedupe_state: HashMap<String, Instant>,
}

#[derive(Serialize)]
struct SendMessagePayload<'a> {
    chat_id: &'a str,
    text: &'a str,
    disable_web_page_preview: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    message_thread_id: Option<i64>,
}

impl TelegramNotifier {
    pub fn new(config: TelegramConfig) -> Self {
        Self {
            client: Client::new(),
            config,
            dedupe_state: HashMap::new(),
        }
    }

    pub fn is_ready(&self) -> bool {
        self.config.enabled && self.config.bot_token.is_some() && self.config.chat_id.is_some()
    }

    pub async fn run(mut self, mut rx: broadcast::Receiver<String>) {
        if !self.is_ready() {
            eprintln!("[telegram] notifier disabled or misconfigured; skipping task startup");
            return;
        }

        loop {
            match rx.recv().await {
                Ok(msg) => {
                    if let Some(event) = ParsedEvent::from_broadcast(&msg, self.config.include_bigmove)
                    {
                        if self.should_debounce(&event.dedupe_key) {
                            continue;
                        }

                        let text = format_telegram_message(&event);
                        if let Err(err) = self.send_with_retry(&text).await {
                            eprintln!(
                                "[telegram] failed to deliver {} alert for {}: {}",
                                event.alert_type, event.symbol, err
                            );
                        }
                    }
                }
                Err(broadcast::error::RecvError::Lagged(skipped)) => {
                    eprintln!("[telegram] lagged on broadcast channel; skipped {} messages", skipped);
                }
                Err(broadcast::error::RecvError::Closed) => {
                    eprintln!("[telegram] broadcast channel closed; notifier exiting");
                    break;
                }
            }
        }
    }

    fn should_debounce(&mut self, key: &str) -> bool {
        let now = Instant::now();
        let debounce_window = Duration::from_secs(self.config.debounce_window_secs.max(1));

        if let Some(last) = self.dedupe_state.get(key)
            && now.duration_since(*last) < debounce_window
        {
            return true;
        }

        self.dedupe_state.insert(key.to_string(), now);
        false
    }

    async fn send_with_retry(&self, text: &str) -> Result<(), reqwest::Error> {
        let token = self.config.bot_token.as_deref().unwrap_or_default();
        let chat_id = self.config.chat_id.as_deref().unwrap_or_default();

        let endpoint = format!("https://api.telegram.org/bot{}/sendMessage", token);
        let payload = SendMessagePayload {
            chat_id,
            text,
            disable_web_page_preview: false,
            message_thread_id: self.config.thread_id,
        };

        let mut wait = Duration::from_millis(300);
        let max_attempts = 4;

        for attempt in 1..=max_attempts {
            match self.client.post(&endpoint).json(&payload).send().await {
                Ok(resp) => {
                    if resp.status().is_success() {
                        return Ok(());
                    }

                    eprintln!(
                        "[telegram] sendMessage non-success status={} attempt={}/{}",
                        resp.status(),
                        attempt,
                        max_attempts
                    );
                }
                Err(err) => {
                    if attempt == max_attempts {
                        return Err(err);
                    }
                    eprintln!(
                        "[telegram] transport error attempt={}/{}: {}",
                        attempt, max_attempts, err
                    );
                }
            }

            tokio::time::sleep(wait).await;
            wait = (wait * 2).min(Duration::from_secs(4));
        }

        Ok(())
    }
}

#[derive(Debug)]
struct ParsedEvent {
    alert_type: &'static str,
    symbol: String,
    move_pct: Option<f64>,
    confidence: Option<f64>,
    headline: String,
    links: Vec<String>,
    dedupe_key: String,
}

impl ParsedEvent {
    fn from_broadcast(msg: &str, include_bigmove: bool) -> Option<Self> {
        if msg.starts_with("[NEWS_CORR]") {
            Self::parse_news_corr(msg)
        } else if include_bigmove && msg.starts_with("[BIGMOVE]") {
            Self::parse_bigmove(msg)
        } else {
            None
        }
    }

    fn parse_news_corr(msg: &str) -> Option<Self> {
        let body = msg.strip_prefix("[NEWS_CORR]")?.trim();
        let mut parts = body.split('|').map(str::trim);

        let left = parts.next()?.to_string();
        let symbol = left
            .split_whitespace()
            .next()
            .unwrap_or("UNKNOWN")
            .to_uppercase();

        let move_pct = parse_prefixed_percent(body, "move=");
        let confidence = parse_prefixed_percent(body, "confidence=")
            .or_else(|| parse_prefixed_percent(body, "conf="));

        let mut links = Vec::new();
        for token in msg.split_whitespace() {
            if token.starts_with("http://") || token.starts_with("https://") {
                links.push(token.trim_end_matches(',').to_string());
            }
        }

        Some(Self {
            alert_type: "NEWS_CORR",
            symbol: symbol.clone(),
            move_pct,
            confidence,
            headline: left,
            links,
            dedupe_key: format!("{}:{}", symbol, "NEWS_CORR"),
        })
    }

    fn parse_bigmove(msg: &str) -> Option<Self> {
        let body = msg.strip_prefix("[BIGMOVE]")?.trim();
        let symbol = body
            .split_whitespace()
            .next()
            .unwrap_or("UNKNOWN")
            .to_uppercase();

        let confidence = parse_prefixed_percent(body, "avg_pressure=");

        Some(Self {
            alert_type: "BIGMOVE",
            symbol: symbol.clone(),
            move_pct: None,
            confidence,
            headline: body.to_string(),
            links: vec![format!(
                "https://www.binance.com/en/trade/{}",
                symbol.replace("USDT", "_USDT")
            )],
            dedupe_key: format!("{}:{}", symbol, "BIGMOVE"),
        })
    }
}

fn parse_prefixed_percent(text: &str, prefix: &str) -> Option<f64> {
    for token in text.split_whitespace() {
        if let Some(rest) = token.strip_prefix(prefix) {
            let value = rest.trim_end_matches('%').trim_end_matches(',');
            if let Ok(v) = value.parse::<f64>() {
                return Some(v);
            }
        }
    }
    None
}

fn format_telegram_message(event: &ParsedEvent) -> String {
    let mut lines = vec![
        format!("🔔 {} {}", event.alert_type, event.symbol),
        format!("📰 Headline: {}", event.headline),
    ];

    if let Some(move_pct) = event.move_pct {
        lines.push(format!("📈 Move: {:+.2}%", move_pct));
    }

    if let Some(conf) = event.confidence {
        lines.push(format!("🎯 Confidence: {:.1}%", conf));
    }

    if !event.links.is_empty() {
        lines.push("🔗 Links:".to_string());
        lines.extend(event.links.iter().map(|l| format!("- {}", l)));
    }

    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_news_corr_message() {
        let msg = "[NEWS_CORR] BTCUSDT move=2.35% confidence=74.0 | ETF headline https://example.com";
        let event = ParsedEvent::from_broadcast(msg, true).expect("expected parse");
        assert_eq!(event.alert_type, "NEWS_CORR");
        assert_eq!(event.symbol, "BTCUSDT");
        assert_eq!(event.move_pct, Some(2.35));
        assert_eq!(event.confidence, Some(74.0));
        assert_eq!(event.links.len(), 1);
    }

    #[test]
    fn ignores_bigmove_when_disabled() {
        let msg = "[BIGMOVE] BTCUSDT BULLISH BREAKOUT likely! avg_pressure=80.2% notional=12345";
        assert!(ParsedEvent::from_broadcast(msg, false).is_none());
        assert!(ParsedEvent::from_broadcast(msg, true).is_some());
    }
}
