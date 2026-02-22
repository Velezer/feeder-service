// File: src/ws_helpers.rs
use warp::ws::{Message, WebSocket};
use tokio::sync::broadcast;
use tokio::time::{interval, Duration};
use futures_util::{StreamExt, SinkExt};

pub type BroadcastRx = broadcast::Receiver<String>;
pub type WsTx = futures_util::stream::SplitSink<WebSocket, Message>;

pub async fn send_heartbeat(ws_tx: &mut WsTx) -> Result<(), ()> {
    ws_tx.send(Message::ping(vec![])).await.map_err(|_| ())
}

pub async fn forward_broadcast(ws_tx: &mut WsTx, rx: &mut BroadcastRx) -> Result<(), ()> {
    if let Ok(msg) = rx.recv().await {
        ws_tx.send(Message::text(msg)).await.map_err(|_| ())?;
    }
    Ok(())
}

pub async fn handle_incoming(ws_tx: &mut WsTx, msg: Message) -> bool {
    if msg.is_ping() {
        let _ = ws_tx.send(Message::pong(msg.as_bytes().to_vec())).await;
    } else if msg.is_close() {
        let _ = ws_tx.send(Message::close()).await;
        return false;
    }
    true
}

pub async fn handle_client(ws: WebSocket, tx: broadcast::Sender<String>) {
    let (mut ws_tx, mut ws_rx) = ws.split();
    let mut rx = tx.subscribe();
    let mut heartbeat = interval(Duration::from_secs(15));

    loop {
        tokio::select! {
            _ = heartbeat.tick() => {
                if send_heartbeat(&mut ws_tx).await.is_err() {
                    break;
                }
            }
            Ok(_) = rx.recv() => {
                if forward_broadcast(&mut ws_tx, &mut rx).await.is_err() {
                    break;
                }
            }
            Some(Ok(msg)) = ws_rx.next() => {
                if !handle_incoming(&mut ws_tx, msg).await {
                    break;
                }
            }
        }
    }

    println!("Client disconnected");
}