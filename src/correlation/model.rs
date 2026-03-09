use serde_json::Value;

#[derive(Debug, Clone)]
pub enum MarketEventKind {
    AggTrade,
    DepthPressure,
    KlineClose,
}

#[derive(Debug, Clone)]
pub struct MarketEvent {
    pub symbol: String,
    pub timestamp_ms: u64,
    pub kind: MarketEventKind,
    pub move_pct: f64,
    pub notional: f64,
    /// +1 for bullish, -1 for bearish, 0 for neutral/unknown.
    pub direction: i8,
}

#[derive(Debug, Clone)]
pub struct NewsEvent {
    pub symbol: String,
    pub timestamp_ms: u64,
    pub headline: String,
    /// Optional sentiment polarity in range [-1, 1].
    pub sentiment: Option<f64>,
}

#[derive(Debug, Clone)]
pub struct CorrelationSignal {
    pub symbol: String,
    pub market_event_kind: MarketEventKind,
    pub news_headline: String,
    pub lag_ms: i64,
    pub confidence: f64,
    pub move_pct: f64,
    pub notional: f64,
    pub window_5m_count: usize,
    pub window_15m_count: usize,
    pub window_1h_count: usize,
}

pub fn parse_news_event(msg: &str) -> Option<NewsEvent> {
    let root: Value = serde_json::from_str(msg).ok()?;
    let data = root.get("data").unwrap_or(&root);

    let event_type = data.get("e").and_then(Value::as_str).unwrap_or_default();
    if !event_type.eq_ignore_ascii_case("news") {
        return None;
    }

    let symbol = data
        .get("s")
        .or_else(|| data.get("symbol"))
        .and_then(Value::as_str)?
        .trim()
        .to_lowercase();
    if symbol.is_empty() {
        return None;
    }

    let headline = data
        .get("headline")
        .or_else(|| data.get("title"))
        .and_then(Value::as_str)?
        .trim()
        .to_string();
    if headline.is_empty() {
        return None;
    }

    let timestamp_ms = data
        .get("T")
        .or_else(|| data.get("ts"))
        .or_else(|| data.get("timestamp"))
        .and_then(Value::as_u64)?;

    let sentiment = data
        .get("sentiment")
        .and_then(Value::as_f64)
        .map(|v| v.clamp(-1.0, 1.0));

    Some(NewsEvent {
        symbol,
        timestamp_ms,
        headline,
        sentiment,
    })
}

#[cfg(test)]
mod tests {
    use super::parse_news_event;

    #[test]
    fn parses_news_event_from_combined_payload() {
        let payload = r#"{
          "stream": "btcusdt@news",
          "data": {
            "e": "news",
            "s": "BTCUSDT",
            "T": 1710010000000,
            "headline": "Spot ETF momentum increases",
            "sentiment": 0.75
          }
        }"#;

        let event = parse_news_event(payload).expect("news event should parse");
        assert_eq!(event.symbol, "btcusdt");
        assert_eq!(event.timestamp_ms, 1710010000000);
        assert_eq!(event.headline, "Spot ETF momentum increases");
        assert_eq!(event.sentiment, Some(0.75));
    }

    #[test]
    fn rejects_non_news_payload() {
        let payload = r#"{"data":{"e":"aggTrade","s":"BTCUSDT"}}"#;
        assert!(parse_news_event(payload).is_none());
    }
}
