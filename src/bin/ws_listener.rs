use futures_util::{SinkExt, StreamExt};
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;

#[tokio::main]
async fn main() {
    let url = "ws://localhost:8080/aggTrade";
    println!("Connecting to {}", url);

    let (ws_stream, _) = connect_async(url).await.expect("Failed to connect");
    println!("Connected!");

    let (mut write, mut read) = ws_stream.split();

    while let Some(msg) = read.next().await {
        match msg {
            Ok(Message::Text(txt)) => println!("Received: {}", txt),
            Ok(Message::Binary(bin)) => println!("Received binary: {:?}", bin),
            Ok(Message::Ping(payload)) => {
                println!("Ping received, sending Pong...");
                // Respond to ping
                if let Err(e) = write.send(Message::Pong(payload)).await {
                    eprintln!("Failed to send Pong: {}", e);
                    break;
                }
            }
            Ok(Message::Pong(_)) => println!("Pong received"),
            Ok(Message::Close(frame)) => {
                println!("Connection closed by server: {:?}", frame);
                // Reply with Close to complete handshake
                let _ = write.send(Message::Close(frame)).await;
                break;
            }
            Ok(other) => println!("Other message received: {:?}", other),
            Err(e) => {
                eprintln!("WebSocket error: {}", e);
                break;
            }
        }
    }

    println!("Listener exited");
}
