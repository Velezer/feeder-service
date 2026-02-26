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

        Config {
            symbols,
            port,
            broadcast_capacity,
            big_depth_min_qty,
            big_depth_min_notional,
            big_depth_min_pressure_pct,
            disable_depth_stream,
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
