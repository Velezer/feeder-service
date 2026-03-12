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
        .filter(|last| *last > 0.0 && last.is_finite() && current.is_finite())
        .map(|last| ((current - last).abs() / last) * 100.0)
        .unwrap_or(0.0)
}

fn compute_delay_ms(now_ms: i64, event_ms: u64) -> i64 {
    let event_ms = event_ms.min(i64::MAX as u64) as i64;
    now_ms.saturating_sub(event_ms).max(0)
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
        let delay_ms = compute_delay_ms(Utc::now().timestamp_millis(), agg.t);

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
