use tokio_stream::StreamExt;
use crate::feeder::Feeder;
use crate::market::StreamRequest;
use crate::binance::BinanceFeeder;

mod feeder;
mod binance;
pub mod market {
    tonic::include_proto!("market");
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // let addr = "0.0.0.0:50051".parse()?;
    // let feeder = BinanceFeeder;

    // println!("Feeder Service running on {}", addr);

    // Server::builder()
    //     .add_service(feededr)
    //     .serve(addr)
    //     .await?;

    // Ok(())

    let feeder = BinanceFeeder;

    let request = StreamRequest {
        symbols: vec!["btcusdt".to_string()],
    };

    let mut stream = feeder.stream_klines(request).await?;

    println!("Listening to Binance klines...\n");

    while let Some(result) = stream.next().await {
        match result {
            Ok(kline) => {
                println!(
                    "{} | open_time:{} O:{} H:{} L:{} C:{} close_time:{} V:{}",
                    kline.symbol,
                    kline.open_time,
                    kline.open,
                    kline.high,
                    kline.low,
                    kline.close,
                    kline.close_time,
                    kline.volume
                );
            }
            Err(e) => {
                eprintln!("Stream error: {}", e);
            }
        }
    }

    Ok(())
}
