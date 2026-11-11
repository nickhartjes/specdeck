//! The service binary stops gracefully on SIGTERM (`sharing-service-limits`, Graceful stop): it
//! closes its connections with a WebSocket close and exits within 5 seconds.
#![cfg(unix)]

use std::process::{Command, Stdio};
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use specdeck_protocol::signalling::{ClientMessage, ServerMessage};
use tokio_tungstenite::tungstenite::Message;

#[tokio::test]
async fn sigterm_closes_connections_and_exits() {
    let port = std::net::TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port();
    let mut service = Command::new(env!("CARGO_BIN_EXE_specdeck-server"))
        .env("SPECDECK_LISTEN", format!("127.0.0.1:{port}"))
        .stdout(Stdio::null())
        .spawn()
        .unwrap();
    let url = format!("ws://127.0.0.1:{port}/signal");
    let mut client = None;
    for _ in 0..100 {
        if let Ok((socket, _)) = tokio_tungstenite::connect_async(&url).await {
            client = Some(socket);
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    let mut client = client.expect("the service did not start");
    let host = ClientMessage::Host { room: "r".into(), claim: None };
    client.send(Message::Text(serde_json::to_string(&host).unwrap().into())).await.unwrap();
    let opened = client.next().await.unwrap().unwrap();
    let opened: ServerMessage = serde_json::from_str(opened.to_text().unwrap()).unwrap();
    assert!(matches!(opened, ServerMessage::RoomOpened { .. }));

    let asked = tokio::time::Instant::now();
    let killed = Command::new("kill").args(["-TERM", &service.id().to_string()]).status().unwrap();
    assert!(killed.success());
    let close = tokio::time::timeout(Duration::from_secs(5), client.next()).await;
    assert!(matches!(close, Ok(Some(Ok(Message::Close(_))))), "{close:?}");
    loop {
        if let Some(status) = service.try_wait().unwrap() {
            assert!(status.success(), "{status}");
            break;
        }
        assert!(asked.elapsed() < Duration::from_secs(5), "still running");
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}
