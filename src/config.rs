// File: src/config.rs
use std::env;

#[derive(Debug, Clone)]
pub struct SymbolConfig {
    pub symbol: String,
    pub big_trade_qty: f64,
    pub spike_pct: f64,
}

#[derive(Debug, Clone)]
pub struct Config {
    pub symbols: Vec<SymbolConfig>,
    pub port: u16,
    pub broadcast_capacity: usize,
    pub big_depth_min_qty: f64,
    pub big_depth_min_notional: f64,
    pub big_depth_min_pressure_pct: f64,
    /// When `true`, depth streams are not subscribed and depth messages are not processed.
    pub disable_depth_stream: bool,
    pub telegram: TelegramConfig,
    pub news: NewsConfig,
}

#[derive(Debug, Clone, Default)]
pub struct TelegramConfig {
    pub enabled: bool,
    pub bot_token: Option<String>,
    pub chat_id: Option<String>,
    pub thread_id: Option<i64>,
    pub include_bigmove: bool,
    pub debounce_window_secs: u64,
}

    pub corr_min_move_pct: f64,
    pub corr_max_lag_seconds: u64,
    pub corr_min_confidence: f64,
    /// Optional extra stream names (e.g. provider-specific news streams) appended verbatim.
    pub news_streams: Vec<String>,
    pub news: NewsConfig,
    pub telegram: TelegramConfig,
}

#[derive(Debug, Clone)]
pub struct NewsConfig {
    pub enabled: bool,
    pub db_path: String,
    pub poll_interval_secs: u64,
    pub retention_hours: i64,
    pub finnhub_api_key: Option<String>,
    pub newsapi_api_key: Option<String>,
}

#[derive(Debug, Clone)]
pub struct TelegramConfig {
    pub enabled: bool,
    pub bot_token: Option<String>,
    pub chat_id: Option<String>,
    pub min_correlation_score: f64,
    pub rate_limit_interval_secs: u64,
    pub api_base_url: String,
}

impl Config {
    pub fn load() -> Self {
        // Global defaults (optional)
        let default_qty = env::var("BIG_TRADE_QTY")
            .ok()
            .and_then(|v| v.parse::<f64>().ok())
            .unwrap_or(20.0);

        let default_spike = env::var("SPIKE_PCT")
            .ok()
            .and_then(|v| v.parse::<f64>().ok())
            .unwrap_or(0.4);

        // Parse symbols
        let symbols_str = env::var("SYMBOLS").unwrap_or_else(|_| "btcusdt".to_string());

        let symbols: Vec<SymbolConfig> = symbols_str
            .split(',')
            .map(|s| s.trim().to_lowercase())
            .filter(|s| !s.is_empty())
            .map(|symbol| {
                let big_trade_qty = Self::load_symbol_env(&symbol, "BIG_TRADE_QTY", default_qty);
                let spike_pct = Self::load_symbol_env(&symbol, "SPIKE_PCT", default_spike);
                SymbolConfig {
                    symbol,
                    big_trade_qty,
                    spike_pct,
                }
            })
            .collect();

        // Load server config
        let port = env::var("PORT")
            .ok()
            .and_then(|v| v.parse::<u16>().ok())
            .unwrap_or(9001);

        let broadcast_capacity = env::var("BROADCAST_CAPACITY")
            .ok()
            .and_then(|v| v.parse::<usize>().ok())
            .unwrap_or(16);

        let big_depth_min_qty = env::var("BIG_DEPTH_MIN_QTY")
            .ok()
            .and_then(|v| v.parse::<f64>().ok())
            .unwrap_or(0.0);

        let big_depth_min_notional = env::var("BIG_DEPTH_MIN_NOTIONAL")
            .ok()
            .and_then(|v| v.parse::<f64>().ok())
            .unwrap_or(0.0);

        let big_depth_min_pressure_pct = env::var("BIG_DEPTH_MIN_PRESSURE_PCT")
            .ok()
            .and_then(|v| v.parse::<f64>().ok())
            .unwrap_or(0.0);

        let disable_depth_stream = env::var("DISABLE_DEPTH_STREAM")
            .map(|v| {
                // Trim surrounding whitespace and optional surrounding quotes
                // (e.g. DISABLE_DEPTH_STREAM="true" in some .env parsers keeps the quotes)
                let trimmed = v.trim().trim_matches('"').trim_matches('\'').to_lowercase();
                matches!(trimmed.as_str(), "1" | "true" | "yes")
            })
            .unwrap_or(false);

        let telegram = TelegramConfig {
            enabled: env::var("TELEGRAM_ENABLED")
                .map(|v| {
                    let v = v.trim().trim_matches('"').trim_matches('\'').to_lowercase();
                    matches!(v.as_str(), "1" | "true" | "yes")
                })
                .unwrap_or(false),
            bot_token: env::var("TELEGRAM_BOT_TOKEN")
                .ok()
                .filter(|v| !v.trim().is_empty()),
            chat_id: env::var("TELEGRAM_CHAT_ID")
                .ok()
                .filter(|v| !v.trim().is_empty()),
            thread_id: env::var("TELEGRAM_THREAD_ID")
                .ok()
                .and_then(|v| v.parse::<i64>().ok()),
            include_bigmove: env::var("TELEGRAM_INCLUDE_BIGMOVE")
                .map(|v| {
                    let v = v.trim().trim_matches('"').trim_matches('\'').to_lowercase();
                    matches!(v.as_str(), "1" | "true" | "yes")
                })
                .unwrap_or(false),
            debounce_window_secs: env::var("TELEGRAM_DEBOUNCE_WINDOW_SECS")
                .ok()
                .and_then(|v| v.parse::<u64>().ok())
                .unwrap_or(45),
        };

        let corr_min_move_pct = env::var("CORR_MIN_MOVE_PCT")
            .ok()
            .and_then(|v| v.parse::<f64>().ok())
            .unwrap_or(0.25);

        let corr_max_lag_seconds = env::var("CORR_MAX_LAG_SECONDS")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(300);

        let corr_min_confidence = env::var("CORR_MIN_CONFIDENCE")
            .ok()
            .and_then(|v| v.parse::<f64>().ok())
            .unwrap_or(0.60);

        let news_streams = env::var("NEWS_STREAMS")
            .ok()
            .map(|raw| {
                raw.split(',')
                    .map(|s| s.trim().to_lowercase())
                    .filter(|s| !s.is_empty())
                    .collect::<Vec<String>>()
            })
            .unwrap_or_default();
        let news = NewsConfig {
            enabled: env::var("ENABLE_NEWS_INGEST")
                .map(|v| {
                    let value = v.trim().trim_matches('"').trim_matches('\'').to_lowercase();
                    matches!(value.as_str(), "1" | "true" | "yes")
                })
                .unwrap_or(false),
            db_path: env::var("NEWS_DB_PATH").unwrap_or_else(|_| "news.sqlite".to_string()),
            poll_interval_secs: env::var("NEWS_POLL_INTERVAL_SECS")
                .ok()
                .and_then(|v| v.parse::<u64>().ok())
                .unwrap_or(300),
            retention_hours: env::var("NEWS_RETENTION_HOURS")
                .ok()
                .and_then(|v| v.parse::<i64>().ok())
                .unwrap_or(24 * 7),
            finnhub_api_key: env::var("FINNHUB_API_KEY").ok(),
            newsapi_api_key: env::var("NEWSAPI_API_KEY").ok(),
        };

        let telegram = TelegramConfig {
            enabled: env::var("ENABLE_TELEGRAM_NOTIFIER")
                .map(|v| {
                    let value = v.trim().trim_matches('"').trim_matches('\'').to_lowercase();
                    matches!(value.as_str(), "1" | "true" | "yes")
                })
                .unwrap_or(false),
            bot_token: env::var("TELEGRAM_BOT_TOKEN").ok(),
            chat_id: env::var("TELEGRAM_CHAT_ID").ok(),
            min_correlation_score: env::var("TELEGRAM_MIN_CORRELATION_SCORE")
                .ok()
                .and_then(|v| v.parse::<f64>().ok())
                .unwrap_or(0.0),
            rate_limit_interval_secs: env::var("TELEGRAM_RATE_LIMIT_INTERVAL_SECS")
                .ok()
                .and_then(|v| v.parse::<u64>().ok())
                .unwrap_or(30),
            api_base_url: env::var("TELEGRAM_API_BASE_URL")
                .unwrap_or_else(|_| "https://api.telegram.org".to_string()),
        };

        Config {
            symbols,
            port,
            broadcast_capacity,
            big_depth_min_qty,
            big_depth_min_notional,
            big_depth_min_pressure_pct,
            disable_depth_stream,
            telegram,
            corr_min_move_pct,
            corr_max_lag_seconds,
            corr_min_confidence,
            news_streams,
            news,
            telegram,
        }
    }

    /// Helper: load per-symbol env variable, fallback to default
    fn load_symbol_env(symbol: &str, key: &str, default: f64) -> f64 {
        let env_key = format!("{}_{}", symbol.to_uppercase(), key);
        env::var(&env_key)
            .ok()
            .and_then(|v| v.parse::<f64>().ok())
            .unwrap_or(default)
    }
}
