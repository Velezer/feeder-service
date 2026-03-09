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

The service queries a normalized symbol table (`news_item_symbols`) joined to `news_items`
with indexed predicates on symbol and publish timestamp. This avoids scanning and JSON symbol
parsing for every candidate row in the correlation window.

The schema includes:

- `news_item_symbols(news_item_id, symbol, published_at)`
- `idx_news_item_symbols_symbol_news_item (symbol, news_item_id)`
- `idx_news_item_symbols_symbol_published_at (symbol, published_at DESC, news_item_id)`

`NewsStore::init` uses idempotent migration statements (`CREATE TABLE IF NOT EXISTS` and
`CREATE INDEX IF NOT EXISTS`) and backfills symbol rows from existing `news_items` records,
so older databases are upgraded safely.

Environment variables:

- `NEWS_CORRELATION_LOOKBACK_SECS` (default: `900`, i.e. 15m)
- `NEWS_CORRELATION_LOOKAHEAD_SECS` (default: `1800`, i.e. 30m)
- `NEWS_CORRELATION_MAX_MATCHES` (default: `5`)

The score is currently a simple normalized match count (`matches / 5`, clamped to 1.0).
