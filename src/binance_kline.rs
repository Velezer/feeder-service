use crate::json_helpers::parse_combined_data;

#[derive(Debug, serde::Deserialize, Clone)]
pub struct KlineEvent {
    #[serde(rename = "s")]
    pub symbol: String,
    #[serde(rename = "E")]
    pub event_time: u64,
    #[serde(rename = "k")]
    pub kline: Kline,
}

#[derive(Debug, serde::Deserialize, Clone)]
pub struct Kline {
    #[serde(rename = "t")]
    pub open_time: u64,
    #[serde(rename = "T")]
    pub close_time: u64,
    #[serde(rename = "i")]
    pub interval: String,
    #[serde(rename = "o")]
    pub open: String,
    #[serde(rename = "c")]
    pub close: String,
    #[serde(rename = "h")]
    pub high: String,
    #[serde(rename = "l")]
    pub low: String,
    #[serde(rename = "v")]
    pub volume: String,
    #[serde(rename = "q")]
    pub quote_volume: String,
    #[serde(rename = "n")]
    pub trade_count: u64,
    #[serde(rename = "x")]
    pub is_closed: bool,
    #[serde(rename = "V")]
    pub taker_buy_base_volume: String,
    #[serde(rename = "Q")]
    pub taker_buy_quote_volume: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct QuantKlineSignal {
    pub symbol: String,
    pub interval_start_ms: u64,
    pub interval_end_ms: u64,
    pub open: f64,
    pub close: f64,
    pub high: f64,
    pub low: f64,
    pub volume: f64,
    pub quote_volume: f64,
    pub trade_count: u64,
    pub taker_buy_ratio_pct: f64,
    pub return_pct: f64,
    pub range_pct: f64,
}

pub fn build_kline_streams(symbols: &[String], interval: &str) -> Vec<String> {
    symbols
        .iter()
        .map(|symbol| format!("{}@kline_{}", symbol.to_lowercase(), interval))
        .collect()
}

pub fn parse_kline_event(msg: &str) -> Option<KlineEvent> {
    parse_combined_data(msg)
}

pub fn build_quant_signal_from_kline(event: &KlineEvent) -> Option<QuantKlineSignal> {
    if event.kline.interval != "4h" || !event.kline.is_closed {
        return None;
    }

    let open = event.kline.open.parse::<f64>().ok()?;
    let close = event.kline.close.parse::<f64>().ok()?;
    let high = event.kline.high.parse::<f64>().ok()?;
    let low = event.kline.low.parse::<f64>().ok()?;
    let volume = event.kline.volume.parse::<f64>().ok()?;
    let quote_volume = event.kline.quote_volume.parse::<f64>().ok()?;
    let taker_buy_quote_volume = event.kline.taker_buy_quote_volume.parse::<f64>().ok()?;

    if !open.is_finite()
        || !close.is_finite()
        || !high.is_finite()
        || !low.is_finite()
        || !volume.is_finite()
        || !quote_volume.is_finite()
        || !taker_buy_quote_volume.is_finite()
        || open <= 0.0
        || high <= 0.0
        || low <= 0.0
    {
        return None;
    }

    let return_pct = ((close - open) / open) * 100.0;
    let range_pct = ((high - low) / open) * 100.0;
    let taker_buy_ratio_pct = if quote_volume > 0.0 {
        (taker_buy_quote_volume / quote_volume * 100.0).clamp(0.0, 100.0)
    } else {
        0.0
    };

    Some(QuantKlineSignal {
        symbol: event.symbol.clone(),
        interval_start_ms: event.kline.open_time,
        interval_end_ms: event.kline.close_time,
        open,
        close,
        high,
        low,
        volume,
        quote_volume,
        trade_count: event.kline.trade_count,
        taker_buy_ratio_pct,
        return_pct,
        range_pct,
    })
}

#[cfg(test)]
#[path = "binance_kline_tests.rs"]
mod tests;
