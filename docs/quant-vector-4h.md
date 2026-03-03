# Quant Vector Analysis (4h Klines)

Quant vector analysis is now generated from **closed Binance 4h kline candles**.

## Runtime activation

- Quant kline analysis is **separate from depth analysis**.
- Production default is inactive: `ENABLE_KLINE_QUANT=false`.
- To enable quant kline stream subscription and emission, set:

```bash
ENABLE_KLINE_QUANT=true
```

Depth processing and depth alerts continue to work independently under their own flags/settings.

## Data source

- Stream: `<symbol>@kline_4h`
- Event type: `kline`
- Emission condition: only when `k.x == true` (closed candle)

## Quant signal output

```text
[QUANT4H] <SYMBOL> <BULLISH|BEARISH|FLAT> | window=<open_ms>..<close_ms> | O:<open> C:<close> H:<high> L:<low> ret=<return>% range=<range>% taker_buy=<ratio>% qvol=<quote_volume> trades=<count>
```

## Metrics

- `ret`: `(close - open) / open * 100`
- `range`: `(high - low) / open * 100`
- `taker_buy`: `taker_buy_quote_volume / quote_volume * 100`
- `qvol`: quote asset volume over the closed candle
- `trades`: number of trades in the candle

## Notes

- Quant signals are broadcast to websocket clients over the existing broadcast channel.
- Open/in-progress klines are ignored to keep quant output deterministic on 4h boundaries.
