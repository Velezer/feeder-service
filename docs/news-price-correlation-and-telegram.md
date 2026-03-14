# News-Price Correlation + Telegram Alerts: Architecture & Operations

This document is the single operational reference for the news-correlation enrichment and Telegram alert pipeline.

## 1) Architecture

At runtime, the service has four cooperating subsystems:

1. **Market signal ingestion**
   - Binance websocket stream provides `aggTrade`, optional depth, and optional kline events.
   - Signal extraction happens in the main stream loop and per-signal processing functions.
2. **News ingestion and indexing**
   - Optional polling loop fetches news provider payloads.
   - News is normalized, symbol-tagged, and persisted in SQLite (`news_items` + `news_item_symbols`).
3. **Correlation enrichment**
   - Each qualifying signal asks `CorrelationService` for matching news in a configurable time window.
   - Enriched JSON includes matched headlines and a correlation score.
4. **Notification fanout**
   - Signal text + enriched payload are published to websocket clients.
   - Telegram notifier is optional and best-effort (rate-limited + retry path).

## 2) Data flow

```text
Binance streams
  └─> process_*_signal
        ├─> build plain signal line
        ├─> correlate symbol+timestamp against SQLite news index
        ├─> build enriched JSON payload
        ├─> broadcast websocket messages
        └─> (optional) send Telegram message if enabled and score threshold passes

News providers
  └─> run_news_ingest_loop
        ├─> fetch provider data
        ├─> normalize + tag symbols
        ├─> upsert into SQLite tables + indexes
        └─> retention cleanup
```

## 3) Alert decision rules

A signal reaches Telegram only when all of the following are true:

1. `ENABLE_TELEGRAM_NOTIFIER=true`.
2. `TELEGRAM_BOT_TOKEN` and `TELEGRAM_CHAT_ID` are present.
3. Correlation score is `>= TELEGRAM_MIN_CORRELATION_SCORE`.
4. Rate limiter allows send (1 message per `TELEGRAM_RATE_LIMIT_INTERVAL_SECS`).

Additional notes:
- Websocket publishing is attempted regardless of Telegram status.
- Telegram failures do not stop stream processing.
- Correlation score is currently `min(match_count, 5) / 5`.

## 4) Environment variable reference (single place)

Below are all relevant runtime env vars for this feature area, including values loaded via `src/config.rs` plus feature-adjacent toggles used by the runtime and correlation service.

### Core runtime + signal switches

| Variable | Default | Recommended starting value | Purpose |
|---|---:|---:|---|
| `SYMBOLS` | `btcusdt` | `btcusdt,ethusdt` | Comma-separated symbols to subscribe/process. |
| `PORT` | `9001` | `9001` | Websocket server bind port. |
| `BROADCAST_CAPACITY` | `16` | `64` | Internal broadcast buffer; increase for bursty clients. |
| `ENABLE_DEPTH` | `false` | `false` during rollout | Enables subscription to Binance diff depth streams. |
| `DISABLE_DEPTH_STREAM` | `false` | `true` for safe start, then `false` | Hard switch to skip depth parsing even if messages arrive. |
| `ENABLE_KLINE_QUANT` | `false` | `false` during initial rollout | Enables 4h kline quant signal stream processing. |
| `LOG_UNKNOWN_STREAM_MESSAGES` | unset | unset in prod | Optional debug logging for unknown inbound stream payloads. |

### Signal thresholds

| Variable | Default | Recommended starting value | Purpose |
|---|---:|---:|---|
| `BIG_TRADE_QTY` | `20.0` | symbol-dependent | Global agg-trade quantity trigger fallback. |
| `<SYMBOL>_BIG_TRADE_QTY` | inherits `BIG_TRADE_QTY` | tune per symbol (e.g. `BTCUSDT_BIG_TRADE_QTY=5`) | Per-symbol agg-trade trigger override. |
| `SPIKE_PCT` | `0.4` | `0.3` to `0.6` | Global move-percent threshold fallback. |
| `<SYMBOL>_SPIKE_PCT` | inherits `SPIKE_PCT` | tune per symbol | Per-symbol move-percent threshold override. |
| `BIG_DEPTH_MIN_QTY` | `0.0` | `0.0` then increase gradually | Min quantity filter for depth pressure signals. |
| `BIG_DEPTH_MIN_NOTIONAL` | `0.0` | `10000` (example for BTC) | Min notional filter for depth pressure signals. |
| `BIG_DEPTH_MIN_PRESSURE_PCT` | `0.0` | `5` to `15` | Min pressure percent for depth-based alerts. |

### News ingestion + storage

| Variable | Default | Recommended starting value | Purpose |
|---|---:|---:|---|
| `ENABLE_NEWS_INGEST` | `false` | `true` in canary | Enables periodic provider polling + DB upserts. |
| `NEWS_DB_PATH` | `news.sqlite` | explicit volume path (e.g. `/data/news.sqlite`) | SQLite path used by ingest and correlation lookup. |
| `NEWS_POLL_INTERVAL_SECS` | `300` | `300` initially, then `120` if needed | Provider polling frequency. |
| `NEWS_RETENTION_HOURS` | `168` | `168` (7 days) | Retention cleanup window in hours. |
| `FINNHUB_API_KEY` | unset | required if Finnhub enabled | Finnhub auth key. |
| `NEWSAPI_API_KEY` | unset | required if NewsAPI enabled | NewsAPI auth key. |

### Correlation window controls

| Variable | Default | Recommended starting value | Purpose |
|---|---:|---:|---|
| `NEWS_CORRELATION_LOOKBACK_SECS` | `900` | `900` | Search window before signal timestamp. |
| `NEWS_CORRELATION_LOOKAHEAD_SECS` | `1800` | `1800` | Search window after signal timestamp. |
| `NEWS_CORRELATION_MAX_MATCHES` | `5` | `5` | Max matched headlines returned per signal. |

### Telegram notifier

| Variable | Default | Recommended starting value | Purpose |
|---|---:|---:|---|
| `ENABLE_TELEGRAM_NOTIFIER` | `false` | `false` at first, enable in canary | Turns on Telegram fanout path. |
| `TELEGRAM_BOT_TOKEN` | unset | required when enabled | Telegram Bot token used for sendMessage. |
| `TELEGRAM_CHAT_ID` | unset | required when enabled | Target chat/channel identifier. |
| `TELEGRAM_MIN_CORRELATION_SCORE` | `0.0` | `0.4` to `0.6` for early rollout | Suppresses low-confidence alerts. |
| `TELEGRAM_RATE_LIMIT_INTERVAL_SECS` | `30` | `30` (or `60` for noisy symbols) | Min interval between outbound Telegram messages. |
| `TELEGRAM_API_BASE_URL` | `https://api.telegram.org` | default unless self-host/proxy | Telegram API base URL. |

## 5) Rollout guidance

### Disabled-by-default safety posture

Start with all optional outputs and high-noise streams disabled:

- `ENABLE_NEWS_INGEST=false`
- `ENABLE_TELEGRAM_NOTIFIER=false`
- `ENABLE_DEPTH=false`
- `ENABLE_KLINE_QUANT=false`
- optionally `DISABLE_DEPTH_STREAM=true`

This keeps existing agg-trade behavior intact while you validate baseline stability.

### Threshold tuning order

1. **Turn on ingest first (`ENABLE_NEWS_INGEST=true`)** and verify DB freshness.
2. **Observe enriched websocket payloads** with Telegram still disabled.
3. **Tune correlation threshold** (`TELEGRAM_MIN_CORRELATION_SCORE`) based on observed noise.
4. **Tune symbol thresholds** (`<SYMBOL>_BIG_TRADE_QTY`, `<SYMBOL>_SPIKE_PCT`) to reduce chatter.
5. **Only then enable Telegram**.

### Safe canary strategy

1. Deploy canary instance with a narrow symbol list (e.g. `SYMBOLS=btcusdt`).
2. Enable `ENABLE_NEWS_INGEST=true`, keep Telegram disabled for burn-in.
3. Verify:
   - DB writes/retention healthy.
   - Enriched websocket payload shape and score distribution.
4. Enable Telegram in canary with conservative threshold, e.g. `TELEGRAM_MIN_CORRELATION_SCORE=0.6`.
5. Monitor rate limiting and error logs for at least one market session.
6. Expand symbols and/or lower threshold incrementally.

## 6) Example enriched websocket payload

```json
{
  "signal_type": "agg_trade",
  "symbol": "BTCUSDT",
  "event_timestamp": 1710000001000,
  "move_metrics": {
    "price": 43000.0,
    "qty": 0.25,
    "notional": 10750.0,
    "is_buyer_maker": false
  },
  "matched_news": [
    {
      "headline": "Bitcoin ETF inflows stay elevated",
      "url": "https://example.com/bitcoin-etf-inflows",
      "published_at": 1710000000700
    },
    {
      "headline": "Bitcoin derivatives open interest climbs",
      "url": "https://example.com/bitcoin-open-interest",
      "published_at": 1710000000500
    }
  ],
  "correlation_score": 0.4
}
```

## 7) Example Telegram message

```text
🔔 BTCUSDT AGG_TRADE
BULLISH | qty 0.25 @ 43000.00
Correlation score: 0.40
News:
• Bitcoin ETF inflows stay elevated
  https://example.com/bitcoin-etf-inflows
• Bitcoin derivatives open interest climbs
  https://example.com/bitcoin-open-interest
```

## 8) Troubleshooting

### A) Missing news matches

Symptoms:
- `matched_news` empty despite obvious related headlines.

Checks:
1. Confirm `ENABLE_NEWS_INGEST=true` and at least one provider API key exists (`FINNHUB_API_KEY` or `NEWSAPI_API_KEY`).
   - Keys containing only spaces behave as unset values and will not fetch news.
2. Verify correlation windows are wide enough (`NEWS_CORRELATION_LOOKBACK_SECS`, `NEWS_CORRELATION_LOOKAHEAD_SECS`).
3. Confirm symbol tagging contains the traded symbol (`BTC` vs `BTCUSDT` normalization expectations).
4. Check event/news timestamps are in milliseconds and in expected wall-clock range.

### B) Stale or empty DB

Symptoms:
- Same old headlines only; no fresh rows.

Checks:
1. Confirm `NEWS_DB_PATH` points to a persistent writable location.
2. Inspect DB file mtime and row counts for `news_items` and `news_item_symbols`.
3. Check provider responses and auth quotas.
4. Ensure `NEWS_POLL_INTERVAL_SECS` is not too large for your expectations.

### D) `[news] fetched=0 inserted=0 pruned=0 db=news.sqlite` appears repeatedly

The ingest loop now prints a reason and provider status so operators can immediately see why the counters are zero:

```text
[news] fetched=0 inserted=0 pruned=0 db=news.sqlite reason=<reason_code> providers=finnhub=<state>;newsapi=<state>
```

Reason codes:
- `no_provider_api_key`: both providers are disabled because `FINNHUB_API_KEY` and `NEWSAPI_API_KEY` are unset/empty.
- `provider_failed`: exactly one provider is configured and its fetch failed.
- `providers_failed`: both configured providers failed in the current poll cycle.
- `partial_provider_failure`: one provider failed but another provider succeeded.
- `providers_returned_no_articles`: providers responded successfully but returned zero rows.
- `ok`: at least one article was fetched.

Provider states:
- `disabled`: no API key configured for that provider.
- `ok`: fetch call succeeded.
- `failed`: fetch call errored (network/auth/rate-limit/etc.).

### C) Telegram auth or chat errors

Symptoms:
- 401/403/400 responses in notifier logs.

Checks:
1. Validate `TELEGRAM_BOT_TOKEN` format and token freshness.
2. Validate `TELEGRAM_CHAT_ID` (chat/channel IDs can be negative for channels).
3. Ensure bot is added to target chat/channel and has posting permission.
4. If using custom `TELEGRAM_API_BASE_URL`, verify TLS/proxy routing.

### D) Rate limiting / message drops

Symptoms:
- Signals visible on websocket but not every signal reaches Telegram.

Checks:
1. Verify intended behavior: notifier throttles by `TELEGRAM_RATE_LIMIT_INTERVAL_SECS`.
2. Reduce alert volume by raising `TELEGRAM_MIN_CORRELATION_SCORE`.
3. Reduce source noise by tuning per-symbol thresholds.
4. If needed, lower rate limit interval cautiously and monitor bot/provider limits.
