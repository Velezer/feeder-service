// File: src/ws_helpers.rs
use futures_util::{SinkExt, StreamExt};
use tokio::sync::{broadcast, oneshot};
use tokio::time::{Duration, interval};
use warp::ws::{Message, WebSocket};

pub type BroadcastRx = broadcast::Receiver<String>;
pub type WsTx = futures_util::stream::SplitSink<WebSocket, Message>;

#[derive(Debug)]
pub enum DisconnectReason {
    ClientClosed(Option<String>),
    HeartbeatSendFailed,
    BroadcastReceiveClosed,
    BroadcastForwardFailed,
    ReceiveError(String),
    ClientStreamEnded,
}

impl DisconnectReason {
    pub fn describe(&self) -> String {
        match self {
            Self::ClientClosed(Some(reason)) if !reason.is_empty() => {
                format!("client sent close frame: {reason}")
            }
            Self::ClientClosed(_) => "client sent close frame without reason".to_string(),
            Self::HeartbeatSendFailed => "failed to send heartbeat ping".to_string(),
            Self::BroadcastReceiveClosed => "broadcast channel closed".to_string(),
            Self::BroadcastForwardFailed => "failed to forward broadcast message to client".to_string(),
            Self::ReceiveError(err) => format!("error reading from websocket client: {err}"),
            Self::ClientStreamEnded => "websocket stream ended".to_string(),
        }
    }
}

pub async fn send_heartbeat(ws_tx: &mut WsTx) -> Result<(), ()> {
    ws_tx.send(Message::ping(vec![])).await.map_err(|_| ())
}

pub async fn forward_broadcast(ws_tx: &mut WsTx, msg: String) -> Result<(), ()> {
    ws_tx.send(Message::text(msg)).await.map_err(|_| ())
}

pub async fn handle_incoming(ws_tx: &mut WsTx, msg: Message) -> Option<DisconnectReason> {
    if msg.is_ping() {
        let _ = ws_tx.send(Message::pong(msg.as_bytes().to_vec())).await;
        return None;
    }

    if msg.is_close() {
        let reason = msg
            .close_frame()
            .map(|(_, reason)| reason.to_string())
            .filter(|text| !text.is_empty());
        let _ = ws_tx.send(Message::close()).await;
        return Some(DisconnectReason::ClientClosed(reason));
    }

    None
}

pub async fn handle_client(ws: WebSocket, tx: broadcast::Sender<String>) {
    handle_client_with_notifier(ws, tx, None).await;
}

pub async fn handle_client_with_notifier(
    ws: WebSocket,
    tx: broadcast::Sender<String>,
    disconnect_notifier: Option<oneshot::Sender<String>>,
) {
    let (mut ws_tx, mut ws_rx) = ws.split();
    let mut rx = tx.subscribe();
    let mut heartbeat = interval(Duration::from_secs(15));

    let disconnect_reason = loop {
        tokio::select! {
            _ = heartbeat.tick() => {
                if send_heartbeat(&mut ws_tx).await.is_err() {
                    break DisconnectReason::HeartbeatSendFailed;
                }
            }
            recv_result = rx.recv() => {
                match recv_result {
                    Ok(msg) => {
                        if forward_broadcast(&mut ws_tx, msg).await.is_err() {
                            break DisconnectReason::BroadcastForwardFailed;
                        }
                    }
                    Err(_) => break DisconnectReason::BroadcastReceiveClosed,
                }
            }
            maybe_msg = ws_rx.next() => {
                match maybe_msg {
                    Some(Ok(msg)) => {
                        if let Some(reason) = handle_incoming(&mut ws_tx, msg).await {
                            break reason;
                        }
                    }
                    Some(Err(err)) => break DisconnectReason::ReceiveError(err.to_string()),
                    None => break DisconnectReason::ClientStreamEnded,
                }
            }
        }
    };

    let reason_text = disconnect_reason.describe();
    println!("Client disconnected: {reason_text}");
    if let Some(notifier) = disconnect_notifier {
        let _ = notifier.send(reason_text);
    }
}
