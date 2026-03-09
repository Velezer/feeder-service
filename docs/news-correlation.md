# News Correlation Enrichment

The websocket stream now emits **two messages** for qualifying signals:

1. Existing plain-text signal log line (backward compatible).
2. Enriched JSON payload for downstream consumers.

## Enriched payload shape

```json
{
  "signal_type": "agg_trade|depth_update|kline_quant",
  "symbol": "BTCUSDT",
  "event_timestamp": 1710000001000,
  "move_metrics": {
    "...": "signal-specific metrics"
  },
  "matched_news": [
    {
      "headline": "Bitcoin ETF inflow surges",
      "url": "https://example.com/article",
      "published_at": 1710000000500
    }
  ],
  "correlation_score": 0.2
}
```

## Correlation window

The service queries `news_items` by `published_at` and symbol tag match in a configurable
window around the signal event timestamp.

Environment variables:

- `NEWS_CORRELATION_LOOKBACK_SECS` (default: `900`, i.e. 15m)
- `NEWS_CORRELATION_LOOKAHEAD_SECS` (default: `1800`, i.e. 30m)
- `NEWS_CORRELATION_MAX_MATCHES` (default: `5`)

The score is currently a simple normalized match count (`matches / 5`, clamped to 1.0).
