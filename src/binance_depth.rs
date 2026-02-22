// File: src/binance_depth.rs
use crate::json_helpers::parse_combined_data;

#[derive(Debug, serde::Deserialize)]
pub struct DepthUpdate {
    #[serde(rename = "s")]
    pub symbol: String,
    #[serde(rename = "b")]
    pub bids: Vec<[String; 2]>,
    #[serde(rename = "a")]
    pub asks: Vec<[String; 2]>,
    #[serde(rename = "E")]
    pub event_time: u64,
    #[serde(rename = "U")]
    pub first_update_id: u64,
    #[serde(rename = "u")]
    pub final_update_id: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ParsedDepthLevel {
    pub price: f64,
    pub qty: f64,
    pub notional: f64,
}

pub fn parse_depth_update(msg: &str) -> Option<DepthUpdate> {
    parse_combined_data(msg)
}

pub fn parse_depth_level(level: &[String; 2]) -> Option<ParsedDepthLevel> {
    let price = level[0].parse::<f64>().ok()?;
    let qty = level[1].parse::<f64>().ok()?;

    if !price.is_finite() || !qty.is_finite() || price <= 0.0 || qty <= 0.0 {
        return None;
    }

    Some(ParsedDepthLevel {
        price,
        qty,
        notional: price * qty,
    })
}

pub fn collect_big_levels(
    levels: &[[String; 2]],
    min_qty: f64,
    max_matches: usize,
) -> Vec<ParsedDepthLevel> {
    let mut matches: Vec<ParsedDepthLevel> = levels
        .iter()
        .filter_map(parse_depth_level)
        .filter(|level| level.qty >= min_qty)
        .collect();

    matches.sort_by(|a, b| b.notional.total_cmp(&a.notional));
    matches.truncate(max_matches);
    matches
}

pub fn is_big_depth_update(bids: &[ParsedDepthLevel], asks: &[ParsedDepthLevel]) -> bool {
    !bids.is_empty() || !asks.is_empty()
}

pub fn passes_pressure_filter(
    bid_pressure_pct: f64,
    sell_pressure_pct: f64,
    min_pressure_pct: f64,
) -> bool {
    if min_pressure_pct <= 0.0 {
        return true;
    }

    let threshold = min_pressure_pct.clamp(0.0, 100.0);
    let bid_pressure = bid_pressure_pct.clamp(0.0, 100.0);
    let sell_pressure = sell_pressure_pct.clamp(0.0, 100.0);

    bid_pressure >= threshold || sell_pressure >= threshold
}

pub fn format_depth_levels(levels: &[ParsedDepthLevel]) -> String {
    if levels.is_empty() {
        return "-".to_string();
    }

    levels
        .iter()
        .map(|level| format!("{:.2} x {:.4}", level.price, level.qty))
        .collect::<Vec<_>>()
        .join(",")
}

pub fn build_depth_streams(symbols: &[String], levels: u16, speed_ms: u16) -> Vec<String> {
    symbols
        .iter()
        .map(|symbol| format!("{}@depth{}@{}ms", symbol.to_lowercase(), levels, speed_ms))
        .collect()
}

pub fn build_diff_depth_streams(symbols: &[String], speed_ms: u16) -> Vec<String> {
    symbols
        .iter()
        .map(|symbol| format!("{}@depth@{}ms", symbol.to_lowercase(), speed_ms))
        .collect()
}

#[cfg(test)]
#[path = "binance_depth_tests.rs"]
mod tests;
