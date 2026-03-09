use std::sync::Arc;
use std::time::{Duration, Instant};

use reqwest::Client;
use serde::Serialize;
use tokio::sync::Mutex;
use tokio::time::sleep;

use crate::config::TelegramConfig;

#[derive(Debug, Clone)]
pub struct TelegramNotifier {
    enabled: bool,
    bot_token: Option<String>,
    chat_id: Option<String>,
    min_correlation_score: f64,
    rate_limit_interval_secs: u64,
    api_base_url: String,
    http: Client,
    last_sent_at: Arc<Mutex<Option<Instant>>>,
}

#[derive(Debug, Serialize)]
struct SendMessageBody<'a> {
    chat_id: &'a str,
    text: &'a str,
    disable_web_page_preview: bool,
}

impl TelegramNotifier {
    pub fn new(config: TelegramConfig) -> Self {
        Self {
            enabled: config.enabled,
            bot_token: config.bot_token,
            chat_id: config.chat_id,
            min_correlation_score: config.min_correlation_score,
            rate_limit_interval_secs: config.rate_limit_interval_secs,
            api_base_url: config.api_base_url,
            http: Client::builder()
                .no_proxy()
                .build()
                .unwrap_or_else(|_| Client::new()),
            last_sent_at: Arc::new(Mutex::new(None)),
        }
    }

    pub async fn send_with_retry(&self, message: &str, correlation_score: f64) {
        if !self.enabled {
            return;
        }

        if correlation_score < self.min_correlation_score {
            return;
        }

        let Some(bot_token) = self.bot_token.as_deref() else {
            eprintln!("[notify/telegram] missing TELEGRAM_BOT_TOKEN, skipping notification");
            return;
        };

        let Some(chat_id) = self.chat_id.as_deref() else {
            eprintln!("[notify/telegram] missing TELEGRAM_CHAT_ID, skipping notification");
            return;
        };

        self.wait_for_rate_limit().await;

        let url = format!("{}/bot{}/sendMessage", self.api_base_url, bot_token);
        let body = SendMessageBody {
            chat_id,
            text: message,
            disable_web_page_preview: false,
        };

        let mut backoff_ms = 300_u64;
        let mut last_error = None;

        for _ in 0..3 {
            match self.http.post(&url).json(&body).send().await {
                Ok(resp) if resp.status().is_success() => {
                    let mut guard = self.last_sent_at.lock().await;
                    *guard = Some(Instant::now());
                    return;
                }
                Ok(resp) => {
                    last_error = Some(format!("telegram status {}", resp.status()));
                }
                Err(err) => {
                    last_error = Some(err.to_string());
                }
            }

            sleep(Duration::from_millis(backoff_ms)).await;
            backoff_ms *= 2;
        }

        if let Some(err) = last_error {
            eprintln!("[notify/telegram] delivery failed, continuing stream processing: {err}");
        }
    }

    async fn wait_for_rate_limit(&self) {
        if self.rate_limit_interval_secs == 0 {
            return;
        }

        let wait_duration = {
            let guard = self.last_sent_at.lock().await;
            match *guard {
                Some(last_sent) => {
                    let elapsed = last_sent.elapsed();
                    let min_interval = Duration::from_secs(self.rate_limit_interval_secs);
                    if elapsed < min_interval {
                        Some(min_interval - elapsed)
                    } else {
                        None
                    }
                }
                None => None,
            }
        };

        if let Some(wait) = wait_duration {
            sleep(wait).await;
        }
    }
}
