# PLAN

## Goal
Implement high funding rate detection from Binance mark price updates.

## Steps
1. Add funding-rate parser and threshold helpers.
2. Subscribe to mark price streams and process high-funding signals in runtime loop.
3. Add configuration for enable/threshold/cooldown.
4. Increase test coverage for parser + threshold logic.
5. Document behavior and environment variables.
