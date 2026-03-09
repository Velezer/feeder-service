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
    pub corr_min_move_pct: f64,
    pub corr_max_lag_seconds: u64,
    pub corr_min_confidence: f64,
    /// Optional extra stream names (e.g. provider-specific news streams) appended verbatim.
    pub news_streams: Vec<String>,
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

        Config {
            symbols,
            port,
            broadcast_capacity,
            big_depth_min_qty,
            big_depth_min_notional,
            big_depth_min_pressure_pct,
            disable_depth_stream,
            corr_min_move_pct,
            corr_max_lag_seconds,
            corr_min_confidence,
            news_streams,
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
