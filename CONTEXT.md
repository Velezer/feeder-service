# CONTEXT

- Existing service already ingests `aggTrade`, depth, and optional kline streams.
- Notifications are sent through websocket broadcast and optional Telegram fanout.
- Correlation engine can receive generic market events, so funding-rate events can reuse this path.
