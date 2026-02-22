// File: src/binance.rs
use crate::config::SymbolConfig;
use crate::json_helpers::parse_combined_data;
use chrono::Utc;
use tokio::sync::broadcast;

#[derive(Debug, serde::Deserialize)]
pub struct AggTrade {
    pub s: String,
    pub p: String,
    pub q: String,
    #[serde(rename = "T")]
    pub t: u64,
    pub m: bool,
}

pub fn parse_agg_trade(msg: &str) -> Option<AggTrade> {
    parse_combined_data(msg)
}

pub fn calc_spike(last_price: Option<f64>, current: f64) -> f64 {
    last_price
        .map(|last| ((current - last).abs() / last) * 100.0)
        .unwrap_or(0.0)
}

pub async fn log_and_broadcast(
    tx: &broadcast::Sender<String>,
    agg: &AggTrade,
    spike: f64,
    cfg: &SymbolConfig,
) {
    let price: f64 = agg.p.parse().unwrap_or(0.0);
    let qty: f64 = agg.q.parse().unwrap_or(0.0);

    if qty >= cfg.big_trade_qty || spike >= cfg.spike_pct {
        let delay_ms = Utc::now().timestamp_millis() - agg.t as i64;

        let log_msg = format!(
            "[AGG_TRADE] {} - Price: {:.2}, Qty: {:.4}, Spike: {:.4}%, BuyerMaker: {}, Delay: {} ms",
            agg.s.to_uppercase(),
            price,
            qty,
            spike,
            agg.m,
            delay_ms
        );

        println!("{}", log_msg);
        let _ = tx.send(log_msg);
    }
}

#[cfg(test)]
#[path = "binance_tests.rs"]
mod tests;
