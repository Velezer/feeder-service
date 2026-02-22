// File: src/binance.rs
use crate::config::SymbolConfig;
use crate::time_helpers::{format_ts, utc_to_utc7};
use chrono::Utc;
use simd_json::serde::from_slice;
use tokio::sync::broadcast;

#[derive(Debug, serde::Deserialize)]
pub struct AggTrade {
    pub s: String,
    pub p: String,
    pub q: String,
    pub T: u64,
    pub m: bool,
}

#[derive(Debug, serde::Deserialize)]
struct CombinedStreamMsg {
    pub data: AggTrade,
}

pub fn parse_agg_trade(msg: &str) -> Option<AggTrade> {
    let mut bytes = msg.as_bytes().to_vec();
    let wrapper: CombinedStreamMsg = from_slice(&mut bytes).ok()?;
    Some(wrapper.data)
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
        let dt_utc7 = utc_to_utc7(agg.T as i64);
        let ts_str = format_ts(dt_utc7);
        let delay_ms = Utc::now().timestamp_millis() - agg.T as i64;

        let log_msg = format!(
            "[{}] {} - Price: {:.2}, Qty: {:.4}, Spike: {:.4}%, BuyerMaker: {}, Delay: {} ms",
            ts_str,
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
