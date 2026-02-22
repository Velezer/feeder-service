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

        Config {
            symbols,
            port,
            broadcast_capacity,
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
