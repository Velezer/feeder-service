# WebSocket disconnect reasons

The websocket client handler now emits a disconnect reason whenever a session ends.

## Behavior

When a client disconnects, server logs include a human-readable reason:

```text
Client disconnected: client sent close frame: browser tab closed
```

Possible reason categories include:
- client sent close frame (with or without reason text)
- heartbeat ping send failed
- heartbeat pong timeout
- broadcast channel closed
- broadcast forward failed
- websocket read error
- websocket stream ended

## Why this matters

This makes operational debugging easier by distinguishing intentional client closes from transport or server-side failures.

The heartbeat flow now also enforces a pong timeout, so stale clients that stop responding are cleaned up instead of being kept alive indefinitely.

## Validation

See end-to-end coverage in `tests/ws_disconnect_reason_e2e.rs`, which performs a real websocket handshake and close frame exchange and heartbeat-timeout detection without mocks.
