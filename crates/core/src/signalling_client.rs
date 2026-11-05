//! WebSocket connection to the signalling service (design decision 9).

use futures_util::{SinkExt, StreamExt};
use specdeck_protocol::signalling::{ClientMessage, ServerMessage};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use tokio::sync::mpsc;
use tokio::time::Instant;
use tokio_tungstenite::tungstenite::Message;

use crate::diag::{self, Reason};

/// How often a quiet connection is pinged. A host's connection carries nothing for most of a
/// session, and proxies in front of the service close quiet connections (Envoy after 5 minutes,
/// Cloudflare after 100 seconds).
const KEEPALIVE: Duration = Duration::from_secs(30);

/// Sends messages to the signalling service. Dropping it closes the connection.
#[derive(Clone)]
pub struct SignallingSender(mpsc::UnboundedSender<ClientMessage>);

impl SignallingSender {
    /// Returns false if the connection is gone.
    pub fn send(&self, message: ClientMessage) -> bool {
        self.0.send(message).is_ok()
    }
}

/// Connects to `url` (`ws://` or `wss://…/signal`). The receiver ends when the connection closes.
pub async fn connect(
    url: &str,
) -> Result<
    (SignallingSender, mpsc::UnboundedReceiver<ServerMessage>),
    tokio_tungstenite::tungstenite::Error,
> {
    let socket = match tokio_tungstenite::connect_async(url).await {
        Ok((socket, _)) => socket,
        Err(error) => {
            diag::signalling_connect_failed(Reason::from_websocket(&error));
            return Err(error);
        }
    };
    diag::signalling_connected();
    let (mut sink, mut stream) = socket.split();
    let (out_tx, mut out_rx) = mpsc::unbounded_channel::<ClientMessage>();
    let (in_tx, in_rx) = mpsc::unbounded_channel();
    // Set when this side closes, so the reader does not report the close as a failure.
    let closing = Arc::new(AtomicBool::new(false));

    let closed_here = closing.clone();
    diag::spawn(async move {
        let mut keepalive = tokio::time::interval_at(Instant::now() + KEEPALIVE, KEEPALIVE);
        loop {
            let (message, ping) = tokio::select! {
                message = out_rx.recv() => match message {
                    Some(message) => (Message::Text(
                        serde_json::to_string(&message).expect("client messages always encode").into(),
                    ), false),
                    None => break,
                },
                _ = keepalive.tick() => (Message::Ping(Default::default()), true),
            };
            if let Err(error) = sink.send(message).await {
                let reason = Reason::from_websocket(&error);
                if ping {
                    diag::signalling_ping_failed(reason);
                }
                if !closed_here.swap(true, Ordering::Relaxed) {
                    diag::signalling_closed(Some(reason));
                }
                return;
            }
        }
        if !closed_here.swap(true, Ordering::Relaxed) {
            diag::signalling_closed(None);
        }
        let _ = sink.close().await;
    });
    diag::spawn(async move {
        let ended = loop {
            match stream.next().await {
                Some(Ok(Message::Text(text))) => {
                    // A message this version does not understand is skipped, not fatal.
                    if let Ok(message) = serde_json::from_str(&text)
                        && in_tx.send(message).is_err()
                    {
                        break None;
                    }
                }
                Some(Ok(_)) => {}
                Some(Err(error)) => break Some(Reason::from_websocket(&error)),
                None => break Some(Reason::Closed),
            }
        };
        if let Some(reason) = ended
            && !closing.swap(true, Ordering::Relaxed)
        {
            diag::signalling_closed(Some(reason));
        }
    });
    Ok((SignallingSender(out_tx), in_rx))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::net::TcpListener;

    #[tokio::test(start_paused = true)]
    async fn pings_a_quiet_connection() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("ws://{}/signal", listener.local_addr().unwrap());
        let server = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            let mut socket = tokio_tungstenite::accept_async(stream).await.unwrap();
            socket.next().await.unwrap().unwrap()
        });
        let (_sender, _receiver) = connect(&url).await.unwrap();
        let started = Instant::now();
        let message = server.await.unwrap();
        assert!(matches!(message, Message::Ping(_)), "got {message:?}");
        assert!(started.elapsed() >= KEEPALIVE - Duration::from_secs(1));
    }

    #[tokio::test]
    async fn records_connecting_and_closing_without_the_address() {
        let (capture, _guard) = crate::capture::Capture::start();
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("ws://{}/signal", listener.local_addr().unwrap());
        drop(listener);
        assert!(connect(&url).await.is_err());
        let failed = capture.named("signalling.connect_failed");
        assert_eq!(failed[0].field("reason"), Some("refused"));

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("ws://{}/signal", listener.local_addr().unwrap());
        let server = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            let mut socket = tokio_tungstenite::accept_async(stream).await.unwrap();
            socket.close(None).await.unwrap();
        });
        let (_sender, mut receiver) = connect(&url).await.unwrap();
        server.await.unwrap();
        assert!(receiver.recv().await.is_none());
        tokio::time::sleep(Duration::from_millis(50)).await;
        assert_eq!(capture.named("signalling.connected").len(), 1);
        assert_eq!(capture.named("signalling.closed")[0].field("reason"), Some("closed"));
        assert!(capture.events().iter().all(|e| !format!("{e:?}").contains("127.0.0.1")));
    }
}
