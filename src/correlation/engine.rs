use std::collections::{HashMap, VecDeque};

use super::model::{CorrelationSignal, MarketEvent, NewsEvent};

const WINDOW_5M_MS: u64 = 5 * 60 * 1000;
const WINDOW_15M_MS: u64 = 15 * 60 * 1000;
const WINDOW_1H_MS: u64 = 60 * 60 * 1000;

#[derive(Debug, Clone)]
pub struct CorrelationEngine {
    min_move_pct: f64,
    max_lag_ms: i64,
    min_confidence: f64,
    market_by_symbol: HashMap<String, VecDeque<MarketEvent>>,
    news_by_symbol: HashMap<String, VecDeque<NewsEvent>>,
}

impl CorrelationEngine {
    pub fn new(min_move_pct: f64, max_lag_seconds: u64, min_confidence: f64) -> Self {
        Self {
            min_move_pct,
            max_lag_ms: (max_lag_seconds as i64).max(0) * 1_000,
            min_confidence,
            market_by_symbol: HashMap::new(),
            news_by_symbol: HashMap::new(),
        }
    }

    pub fn ingest_news(&mut self, news: NewsEvent) {
        let symbol = news.symbol.to_lowercase();
        let queue = self.news_by_symbol.entry(symbol).or_default();
        queue.push_back(news);
        let now_ms = queue.back().map(|n| n.timestamp_ms).unwrap_or(0);
        Self::trim_news_window(queue, now_ms, WINDOW_1H_MS);
    }

    pub fn on_market_event(&mut self, event: MarketEvent) -> Option<CorrelationSignal> {
        let symbol = event.symbol.to_lowercase();
        let market_queue = self.market_by_symbol.entry(symbol.clone()).or_default();
        market_queue.push_back(event.clone());
        Self::trim_market_window(market_queue, event.timestamp_ms, WINDOW_1H_MS);

        if event.move_pct.abs() < self.min_move_pct {
            return None;
        }

        let news_queue = self.news_by_symbol.get(&symbol)?;
        let (news, lag_ms) = Self::closest_news(news_queue, event.timestamp_ms, self.max_lag_ms)?;

        let recency_score =
            (1.0 - (lag_ms.unsigned_abs() as f64 / self.max_lag_ms.max(1) as f64)).clamp(0.0, 1.0);
        let magnitude_score =
            (event.move_pct.abs() / (self.min_move_pct * 4.0).max(0.0001)).clamp(0.0, 1.0);
        let notional_score = ((event.notional + 1.0).log10() / 7.0).clamp(0.0, 1.0);
        let sentiment_score = Self::sentiment_alignment_score(event.direction, news.sentiment);

        let confidence = (0.35 * magnitude_score)
            + (0.30 * notional_score)
            + (0.25 * recency_score)
            + (0.10 * sentiment_score);

        if confidence < self.min_confidence {
            return None;
        }

        let (window_5m_count, window_15m_count, window_1h_count) =
            self.window_counts(&symbol, event.timestamp_ms);

        Some(CorrelationSignal {
            symbol: event.symbol,
            market_event_kind: event.kind,
            news_headline: news.headline.clone(),
            lag_ms,
            confidence,
            move_pct: event.move_pct,
            notional: event.notional,
            window_5m_count,
            window_15m_count,
            window_1h_count,
        })
    }

    pub fn window_counts(&self, symbol: &str, now_ms: u64) -> (usize, usize, usize) {
        let Some(queue) = self.market_by_symbol.get(&symbol.to_lowercase()) else {
            return (0, 0, 0);
        };

        let c5 = queue
            .iter()
            .filter(|e| now_ms.saturating_sub(e.timestamp_ms) <= WINDOW_5M_MS)
            .count();
        let c15 = queue
            .iter()
            .filter(|e| now_ms.saturating_sub(e.timestamp_ms) <= WINDOW_15M_MS)
            .count();
        let c60 = queue
            .iter()
            .filter(|e| now_ms.saturating_sub(e.timestamp_ms) <= WINDOW_1H_MS)
            .count();

        (c5, c15, c60)
    }

    fn closest_news(
        news: &VecDeque<NewsEvent>,
        ts_ms: u64,
        max_lag_ms: i64,
    ) -> Option<(&NewsEvent, i64)> {
        news.iter()
            .filter_map(|n| {
                let lag = ts_ms as i64 - n.timestamp_ms as i64;
                if lag.abs() <= max_lag_ms {
                    Some((n, lag))
                } else {
                    None
                }
            })
            .min_by_key(|(_, lag)| lag.abs())
    }

    fn trim_market_window(queue: &mut VecDeque<MarketEvent>, now_ms: u64, window_ms: u64) {
        while let Some(front) = queue.front() {
            if now_ms.saturating_sub(front.timestamp_ms) > window_ms {
                queue.pop_front();
            } else {
                break;
            }
        }
    }

    fn trim_news_window(queue: &mut VecDeque<NewsEvent>, now_ms: u64, window_ms: u64) {
        while let Some(front) = queue.front() {
            if now_ms.saturating_sub(front.timestamp_ms) > window_ms {
                queue.pop_front();
            } else {
                break;
            }
        }
    }

    fn sentiment_alignment_score(direction: i8, sentiment: Option<f64>) -> f64 {
        let Some(sentiment) = sentiment else {
            return 0.5;
        };

        let sentiment = sentiment.clamp(-1.0, 1.0);
        let sentiment_dir = if sentiment > 0.05 {
            1
        } else if sentiment < -0.05 {
            -1
        } else {
            0
        };

        if direction == 0 || sentiment_dir == 0 {
            0.5
        } else if direction == sentiment_dir {
            1.0
        } else {
            0.0
        }
    }
}
