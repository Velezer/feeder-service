use std::time::{SystemTime, UNIX_EPOCH};

use super::feeder::Feeder;
use crate::market::{Kline, StreamRequest};
use reqwest::Client;
use serde_json::Value;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tokio_tungstenite::connect_async;
use futures_util::StreamExt;
use tonic::Status;
use serde::Deserialize;

pub struct BinanceFeeder;

#[tonic::async_trait]
impl Feeder for BinanceFeeder {
    type StreamKlinesStream = ReceiverStream<Result<Kline, Status>>;

    async fn stream_klines(
        &self,
        request: StreamRequest,
    ) -> Result<ReceiverStream<Result<Kline, Status>>, Status> {
        let symbols = request.symbols;
        if symbols.is_empty() {
            return Err(Status::invalid_argument("No symbols provided"));
        }

        let (tx, rx) = mpsc::channel(100);

        tokio::spawn(async move {
            let stream_list: Vec<String> =
                symbols.iter().map(|s| format!("{}@kline_1m", s.to_lowercase())).collect();
            let url = format!(
                "wss://stream.binance.com:9443/stream?streams={}",
                stream_list.join("/")
            );

            let (ws_stream, _) = connect_async(url)
                .await
                .expect("Failed to connect to Binance WS");

            let (_, mut read) = ws_stream.split();

            while let Some(msg) = read.next().await {
                if let Ok(msg) = msg {
                    if msg.is_text() {
                        if let Ok(kline) = parse_kline(msg.to_text().unwrap()) {
                            let _ = tx.send(Ok(kline)).await;
                        }
                    }
                }
            }
        });

        Ok(ReceiverStream::new(rx))
    }

      async fn historical_klines(
        &self,
        symbol: &str,
        interval: &str,
        days: u64,
    ) -> Result<Vec<Kline>, Box<dyn std::error::Error>> {
        let client = Client::new();

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)?
            .as_millis() as u64;

        let start_time = now - days * 24 * 60 * 60 * 1000;

        let url = format!(
            "https://api.binance.com/api/v3/klines?symbol={}&interval={}&startTime={}&limit=1000",
            symbol.to_uppercase(),
            interval,
            start_time
        );

        let response: Vec<Vec<Value>> = client
            .get(url)
            .send()
            .await?
            .json()
            .await?;

        let mut klines = Vec::new();

        for entry in response {
            let kline = Kline {
                symbol: symbol.to_uppercase(),
                open_time: entry[0].as_i64().unwrap(),
                open: entry[1].as_str().unwrap().parse().unwrap(),
                high: entry[2].as_str().unwrap().parse().unwrap(),
                low: entry[3].as_str().unwrap().parse().unwrap(),
                close: entry[4].as_str().unwrap().parse().unwrap(),
                volume: entry[5].as_str().unwrap().parse().unwrap(),
                close_time: entry[6].as_i64().unwrap(),
            };

            klines.push(kline);
        }

        Ok(klines)
    }
}

// Binance WS JSON structs
#[derive(Debug, Deserialize)]
struct BinanceWrapper {
    data: BinanceKlineWrapper,
}

#[derive(Debug, Deserialize)]
struct BinanceKlineWrapper {
    k: BinanceKline,
}

#[derive(Debug, Deserialize)]
struct BinanceKline {
    s: String,
    t: i64,
    #[serde(rename = "T")]
    close_time: i64,
    o: String,
    h: String,
    l: String,
    c: String,
    v: String,
}

fn parse_kline(text: &str) -> Result<Kline, serde_json::Error> {
    let wrapper: BinanceWrapper = serde_json::from_str(text)?;
    let k = wrapper.data.k;

    Ok(Kline {
        symbol: k.s,
        open_time: k.t,
        close_time: k.close_time,
        open: k.o.parse().unwrap_or(0.0),
        high: k.h.parse().unwrap_or(0.0),
        low: k.l.parse().unwrap_or(0.0),
        close: k.c.parse().unwrap_or(0.0),
        volume: k.v.parse().unwrap_or(0.0),
    })
}
