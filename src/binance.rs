use super::feeder::Feeder;
use crate::market::{Kline, StreamRequest};
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
