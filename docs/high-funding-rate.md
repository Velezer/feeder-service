# High funding rate detection

The feeder service can monitor Binance `@markPrice@1s` streams and emit a `funding_rate` signal when funding is elevated.

## Behavior

- Subscribes to `<symbol>@markPrice@1s` for each configured symbol.
- Parses mark price updates and extracts the `r` (funding rate) field.
- Converts funding to percent (`r * 100`).
- Triggers when `abs(funding_rate_pct) >= FUNDING_RATE_ALERT_PCT`.
- Applies per-symbol cooldown (`FUNDING_RATE_COOLDOWN_SECS`) to reduce alert spam.

## Output

When triggered, the service emits:

- log line:
  - `[FUNDING] BTCUSDT HIGH LONG_BIASED funding=+0.1200% threshold=0.1000% next=...`
- websocket/notification signal type:
  - `funding_rate`
- metrics payload fields:
  - `funding_rate_pct`
  - `funding_rate_threshold_pct`
  - `next_funding_time`
  - `cooldown_secs`

## Environment variables

- `ENABLE_FUNDING_RATE` (default: `true`)
- `FUNDING_RATE_ALERT_PCT` (default: `0.10`)
- `FUNDING_RATE_COOLDOWN_SECS` (default: `300`)
