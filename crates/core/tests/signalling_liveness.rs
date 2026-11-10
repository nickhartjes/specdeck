//! The app's connection to the sharing service answers the service's pings, so the service keeps
//! it open while the app has nothing to send (room-ownership decision 5).

use std::sync::Arc;
use std::time::Duration;

use specdeck_core::signalling_client;
use specdeck_protocol::signalling::{ClientMessage, ServerMessage};
use specdeck_server::{Config, Service, serve};
use tokio::net::TcpListener;

#[tokio::test]
async fn a_quiet_connection_stays_open() {
    // A connection that did not answer would be closed after 3 seconds; the app's own keepalive
    // comes only every 30.
    let config = Config {
        ping_interval: Duration::from_secs(1),
        unresponsive_after: Duration::from_secs(3),
        ..Config::default()
    };
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("ws://{}/signal", listener.local_addr().unwrap());
    let service = Arc::new(Service::new(config));
    tokio::spawn(serve(listener, service.clone()));

    let (sender, mut receiver) = signalling_client::connect(&url).await.unwrap();
    for _ in 0..10 {
        tokio::time::sleep(Duration::from_secs(1)).await;
        assert_eq!(service.connection_count(), 1);
    }
    sender.send(ClientMessage::Host { room: "r".into(), claim: None });
    let opened = tokio::time::timeout(Duration::from_secs(5), receiver.recv()).await.unwrap();
    assert_eq!(opened, Some(ServerMessage::RoomOpened { room: "r".into() }));
}
