use feeder_service::ws_helpers::handle_client_with_notifier;
use futures_util::SinkExt;
use std::net::TcpListener;
use std::sync::Arc;
use tokio::sync::{Mutex, broadcast, oneshot};
use tokio_tungstenite::{connect_async, tungstenite::protocol::CloseFrame};
use url::Url;
use warp::Filter;

#[tokio::test]
async fn reports_client_close_reason_on_disconnect() {
    let (broadcast_tx, _broadcast_rx) = broadcast::channel::<String>(8);
    let (disconnect_tx, disconnect_rx) = oneshot::channel::<String>();
    let disconnect_tx = Arc::new(Mutex::new(Some(disconnect_tx)));

    let ws_route = warp::path("aggTrade").and(warp::ws()).map({
        let tx = broadcast_tx.clone();
        let disconnect_tx = Arc::clone(&disconnect_tx);
        move |ws: warp::ws::Ws| {
            let tx_inner = tx.clone();
            let disconnect_tx = Arc::clone(&disconnect_tx);
            ws.on_upgrade(move |socket| async move {
                let notifier = disconnect_tx.lock().await.take();
                handle_client_with_notifier(socket, tx_inner, notifier).await;
            })
        }
    });

    let std_listener = TcpListener::bind("127.0.0.1:0").expect("bind ephemeral port");
    let addr = std_listener.local_addr().expect("read local addr");
    drop(std_listener);

    tokio::task::spawn(warp::serve(ws_route).run(([127, 0, 0, 1], addr.port())));

    let ws_url = Url::parse(&format!("ws://127.0.0.1:{}/aggTrade", addr.port())).expect("build ws url");

    let mut connected = None;
    for _ in 0..20 {
        match connect_async(ws_url.clone()).await {
            Ok(client) => {
                connected = Some(client);
                break;
            }
            Err(_) => tokio::time::sleep(std::time::Duration::from_millis(50)).await,
        }
    }

    let (mut client, _) = connected.expect("connect websocket");

    client
        .send(tokio_tungstenite::tungstenite::Message::Close(Some(CloseFrame {
            code: tokio_tungstenite::tungstenite::protocol::frame::coding::CloseCode::Normal,
            reason: "browser tab closed".into(),
        })))
        .await
        .expect("send close frame");

    let disconnect_reason = tokio::time::timeout(std::time::Duration::from_secs(3), disconnect_rx)
        .await
        .expect("disconnect timeout")
        .expect("disconnect signal");

    assert!(disconnect_reason.contains("client sent close frame"));
    assert!(disconnect_reason.contains("browser tab closed"));
}
