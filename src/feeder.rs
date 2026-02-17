use tonic::Status;
use tokio_stream::wrappers::ReceiverStream;

use crate::market::{Kline, StreamRequest};

// Trait acts like an interface
#[tonic::async_trait]
pub trait Feeder: Send + Sync + 'static {
    type StreamKlinesStream: tokio_stream::Stream<Item = Result<Kline, Status>> + Send + 'static;

    async fn stream_klines(
        &self,
        request: StreamRequest,
    ) -> Result<ReceiverStream<Result<Kline, Status>>, Status>;

    async fn historical_klines(
        &self,
        symbol: &str,
        interval: &str,
        days: u64,
    ) -> Result<Vec<Kline>, Box<dyn std::error::Error>>;
}
