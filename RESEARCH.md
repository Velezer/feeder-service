# RESEARCH

## Binance funding data

Binance mark price stream (`<symbol>@markPrice` / `@markPrice@1s`) includes:
- `e`: `markPriceUpdate`
- `E`: event time (ms)
- `s`: symbol
- `r`: funding rate (decimal, e.g. `0.001` = `0.1%`)
- `T`: next funding time (ms)

Detection uses absolute percent value (`abs(r * 100)`) against a configurable threshold.
