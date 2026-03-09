use crate::news::store::{NewsRecord, NewsStore};
use anyhow::Result;
use serde::Serialize;

#[derive(Debug, Clone)]
pub struct CorrelationService {
    store: NewsStore,
    default_lookback_ms: i64,
    default_lookahead_ms: i64,
    max_matches: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct MatchedNews {
    pub headline: String,
    pub url: String,
    pub published_at: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct CorrelationResult {
    pub lookback_ms: i64,
    pub lookahead_ms: i64,
    pub matches: Vec<MatchedNews>,
    pub score: f64,
}

impl CorrelationService {
    pub fn new(
        store: NewsStore,
        default_lookback_ms: i64,
        default_lookahead_ms: i64,
        max_matches: usize,
    ) -> Self {
        Self {
            store,
            default_lookback_ms,
            default_lookahead_ms,
            max_matches,
        }
    }

    pub fn from_env(store: NewsStore) -> Self {
        let lookback_secs = std::env::var("NEWS_CORRELATION_LOOKBACK_SECS")
            .ok()
            .and_then(|v| v.parse::<i64>().ok())
            .unwrap_or(15 * 60);
        let lookahead_secs = std::env::var("NEWS_CORRELATION_LOOKAHEAD_SECS")
            .ok()
            .and_then(|v| v.parse::<i64>().ok())
            .unwrap_or(30 * 60);
        let max_matches = std::env::var("NEWS_CORRELATION_MAX_MATCHES")
            .ok()
            .and_then(|v| v.parse::<usize>().ok())
            .unwrap_or(5);

        Self::new(
            store,
            lookback_secs * 1000,
            lookahead_secs * 1000,
            max_matches,
        )
    }

    pub fn correlate(&self, symbol: &str, event_ts_ms: i64) -> Result<CorrelationResult> {
        let from_ts = event_ts_ms - self.default_lookback_ms;
        let to_ts = event_ts_ms + self.default_lookahead_ms;
        let records = self
            .store
            .get_recent_by_symbol(symbol, from_ts, to_ts, self.max_matches)?;

        Ok(CorrelationResult {
            lookback_ms: self.default_lookback_ms,
            lookahead_ms: self.default_lookahead_ms,
            score: correlation_score(&records),
            matches: records
                .into_iter()
                .map(|record| MatchedNews {
                    headline: record.title,
                    url: record.url,
                    published_at: record.published_at,
                })
                .collect(),
        })
    }
}

fn correlation_score(records: &[NewsRecord]) -> f64 {
    (records.len() as f64).min(5.0) / 5.0
}
