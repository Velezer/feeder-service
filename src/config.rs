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
        let default_qty: f64 = env::var("BIG_TRADE_QTY")
            .unwrap_or_else(|_| "20".to_string())
            .parse()
            .expect("BIG_TRADE_QTY must be a number");

        let default_spike: f64 = env::var("SPIKE_PCT")
            .unwrap_or_else(|_| "0.4".to_string())
            .parse()
            .expect("SPIKE_PCT must be a number");

        let symbols_str = env::var("SYMBOLS").unwrap_or_else(|_| "btcusdt".to_string());
        let symbols: Vec<SymbolConfig> = symbols_str
            .split(',')
            .map(|s| s.trim().to_lowercase())
            .filter(|s| !s.is_empty())
            .map(|symbol| {
                let upper = symbol.to_uppercase();
                let big_trade_qty: f64 = env::var(format!("{}_BIG_TRADE_QTY", upper))
                    .ok()
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(default_qty);

                let spike_pct: f64 = env::var(format!("{}_SPIKE_PCT", upper))
                    .ok()
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(default_spike);

                SymbolConfig {
                    symbol,
                    big_trade_qty,
                    spike_pct,
                }
            })
            .collect();

        let port: u16 = env::var("PORT")
            .unwrap_or_else(|_| "9001".to_string())
            .parse()
            .expect("PORT must be a number");

        let broadcast_capacity: usize = env::var("BROADCAST_CAPACITY")
            .unwrap_or_else(|_| "16".to_string())
            .parse()
            .expect("BROADCAST_CAPACITY must be a number");

        Config {
            symbols,
            port,
            broadcast_capacity,
        }
    }
}