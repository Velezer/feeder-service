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
