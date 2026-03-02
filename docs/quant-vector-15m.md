# Quant Vector Analysis (15m Klines)

Quant vector analysis is now generated from **closed Binance 15m kline candles**.

## Data source

- Stream: `<symbol>@kline_15m`
- Event type: `kline`
- Emission condition: only when `k.x == true` (closed candle)

## Quant signal output

```text
[QUANT15M] <SYMBOL> <BULLISH|BEARISH|FLAT> | window=<open_ms>..<close_ms> | O:<open> C:<close> H:<high> L:<low> ret=<return>% range=<range>% taker_buy=<ratio>% qvol=<quote_volume> trades=<count>
```

## Metrics

- `ret`: `(close - open) / open * 100`
- `range`: `(high - low) / open * 100`
- `taker_buy`: `taker_buy_quote_volume / quote_volume * 100`
- `qvol`: quote asset volume over the closed candle
- `trades`: number of trades in the candle

## Notes

- Quant signals are broadcast to websocket clients over the existing broadcast channel.
- Open/in-progress klines are ignored to keep quant output deterministic on 15m boundaries.
