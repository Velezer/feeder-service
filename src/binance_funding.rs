use crate::json_helpers::parse_combined_data;

#[derive(Debug, serde::Deserialize)]
pub struct FundingRateUpdate {
    #[serde(rename = "E")]
    pub event_time: u64,
    #[serde(rename = "s")]
    pub symbol: String,
    #[serde(rename = "r")]
    pub funding_rate: String,
    #[serde(rename = "T")]
    pub next_funding_time: u64,
}

pub fn parse_funding_rate_update(msg: &str) -> Option<FundingRateUpdate> {
    parse_combined_data(msg)
}

pub fn funding_rate_pct(raw_rate: &str) -> Option<f64> {
    raw_rate.parse::<f64>().ok().map(|v| v * 100.0)
}

pub fn is_high_funding_rate(rate_pct: f64, threshold_pct: f64) -> bool {
    rate_pct.is_finite() && threshold_pct.is_finite() && threshold_pct >= 0.0 && rate_pct.abs() >= threshold_pct
}

#[cfg(test)]
#[path = "binance_funding_tests.rs"]
mod tests;
