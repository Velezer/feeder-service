# Telegram notifier

`feeder-service` can fan out enriched signal payloads to both:

- websocket clients (`broadcast::Sender<String>`)
- Telegram Bot API (`sendMessage`)

The shared formatting path is implemented in `src/notify/mod.rs` so websocket and Telegram consume the same correlation context without duplicating signal assembly logic.

## Environment variables

- `ENABLE_TELEGRAM_NOTIFIER` (`true|false`, default `false`)
- `TELEGRAM_ENABLED` (`true|false`, default `false`, alias supported for compatibility)
- `TELEGRAM_BOT_TOKEN` (required only when Telegram fanout is enabled)
- `TELEGRAM_CHAT_ID` (required only when Telegram fanout is enabled)
- `TELEGRAM_THREAD_ID` (optional forum topic thread id for `sendMessage`)
- `TELEGRAM_INCLUDE_BIGMOVE` (`true|false`, default `false`)
- `TELEGRAM_DEBOUNCE_WINDOW_SECS` (default `45`, dedupe window for repeated alerts)
- `TELEGRAM_MIN_CORRELATION_SCORE` (default `0.0`)
- `TELEGRAM_RATE_LIMIT_INTERVAL_SECS` (default `30`)
- `TELEGRAM_API_BASE_URL` (default `https://api.telegram.org`; surrounding spaces and trailing `/` are trimmed)

## Optional Telegram behavior

Telegram fanout is treated as **optional**. If `ENABLE_TELEGRAM_NOTIFIER`/`TELEGRAM_ENABLED` is true but credentials are missing, the service logs a warning and continues with websocket-only fanout. This prevents startup/runtime failures in environments where Telegram is intentionally not configured.

## Message template

Telegram messages are concise and include:

1. symbol + signal type
2. direction and move magnitude (return %, spike %, or pressure)
3. correlation score
4. top matched news headlines with links

Example:

```text
🔔 BTCUSDT KLINE_QUANT
BULLISH | move +2.31%
Correlation score: 0.80
News:
• ETF approvals drive demand
  https://example.com/story
```

## Delivery behavior

- Websocket publish always happens first.
- Telegram publish is best-effort and never aborts stream processing.
- Telegram delivery retries up to 3 attempts with exponential backoff (`300ms`, `600ms`, `1200ms`).
- If all attempts fail, the system logs the error and continues processing.

## End-to-end tests and safety gating

Two e2e tests cover notifier behavior:

- `tests/telegram_notifier_e2e.rs`: non-destructive local integration against a loopback HTTP endpoint (safe in CI by default).
- `tests/telegram_alert_e2e.rs`: real Telegram Bot API integration.

`tests/telegram_alert_e2e.rs` is explicitly gated and will only send messages when `TELEGRAM_E2E=1`.
If the gate is enabled, these environment variables are required:

- `TELEGRAM_BOT_TOKEN`
- `TELEGRAM_CHAT_ID`
- `TELEGRAM_API_BASE_URL` (optional, defaults to `https://api.telegram.org`)

Safety behavior:

- default behavior is **skip** (no outbound Telegram message)
- CI only enables this test when Telegram secrets are configured
- websocket delivery assertions still run to ensure signal fanout is not broken
