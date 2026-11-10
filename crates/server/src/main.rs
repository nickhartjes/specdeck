//! Runs the sharing service. Configuration comes from the environment:
//!
//! - `SPECDECK_LISTEN`: address to listen on (default `0.0.0.0:8080`)
//! - `SPECDECK_TURN_SECRET`: coturn's `static-auth-secret`; without it, no TURN credentials
//! - `SPECDECK_TURN_URLS`: comma-separated TURN URLs handed to clients
//! - `SPECDECK_TURN_TTL_SECONDS`: how long credentials stay valid (default 43200, 12 hours)
//! - `SPECDECK_TRUSTED_PROXIES`: reverse proxies in front of the service, such as an ingress
//!   (default 0); the client address for rate limiting is then read from `X-Forwarded-For`
//! - `SPECDECK_MAX_CONNECTIONS`: open connections in total (default 2000)
//! - `SPECDECK_MAX_CONNECTIONS_PER_IP`: open connections per client network, an IPv4 address or an
//!   IPv6 /64 (default 100)
//! - `SPECDECK_MAX_ROOMS`: open rooms in total, rooms kept for a host that is away included
//!   (default 4000)
//! - `SPECDECK_ROOM_HOLD_SECONDS`: how long a room is kept for a host whose connection ended,
//!   when the host opened it with a claim (default 300); 0 keeps none
//! - `SPECDECK_LOG_LEVEL`: `error`, `warn`, `info` or `debug` (default `info`); when it is unset,
//!   a `RUST_LOG` that is a plain level is used
//! - `SPECDECK_LOG_FORMAT`: `plain` (default) or `json`, one object per line
//!
//! Built with the `web-viewer` feature, it also serves the web viewer at `/`.
//!
//! On SIGTERM or SIGINT it stops accepting connections, closes the open ones, and exits within
//! `STOP_TIMEOUT`.

use std::sync::Arc;
use std::time::Duration;

use specdeck_server::{Config, Service, TurnConfig, log, serve_until};
use tokio::net::TcpListener;

/// How long the service waits for its connections to close when it is asked to stop.
const STOP_TIMEOUT: Duration = Duration::from_secs(5);

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let var = |name| std::env::var(name).ok();
    log::init(&log::settings(
        var("SPECDECK_LOG_LEVEL").as_deref(),
        var("RUST_LOG").as_deref(),
        var("SPECDECK_LOG_FORMAT").as_deref(),
    ));
    let address = std::env::var("SPECDECK_LISTEN").unwrap_or_else(|_| "0.0.0.0:8080".into());
    let defaults = Config::default();
    let config = Config {
        turn: turn_config(),
        trusted_proxies: number("SPECDECK_TRUSTED_PROXIES", defaults.trusted_proxies),
        web: web_viewer(),
        max_connections: number("SPECDECK_MAX_CONNECTIONS", defaults.max_connections),
        max_connections_per_ip: number(
            "SPECDECK_MAX_CONNECTIONS_PER_IP",
            defaults.max_connections_per_ip,
        ),
        max_rooms: number("SPECDECK_MAX_ROOMS", defaults.max_rooms),
        room_hold: seconds(var("SPECDECK_ROOM_HOLD_SECONDS").as_deref(), defaults.room_hold),
        ..defaults
    };
    if config.turn.is_none() {
        log::no_turn();
    }
    let listener = TcpListener::bind(&address).await?;
    log::started(config.turn.is_some(), config.web.is_some());
    let (stop, stopped) = tokio::sync::oneshot::channel::<()>();
    let mut server = tokio::spawn(serve_until(listener, Arc::new(Service::new(config)), async {
        let _ = stopped.await;
    }));
    tokio::select! {
        result = &mut server => return result.expect("the service does not panic"),
        () = stop_requested() => {}
    }
    let _ = stop.send(());
    // The connections had their close; whatever is left is cut off.
    let _ = tokio::time::timeout(STOP_TIMEOUT, server).await;
    Ok(())
}

/// Waits for SIGTERM or SIGINT; Ctrl-C on Windows.
async fn stop_requested() {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{SignalKind, signal};
        let mut terminate = signal(SignalKind::terminate()).expect("signal handlers can be set");
        let mut interrupt = signal(SignalKind::interrupt()).expect("signal handlers can be set");
        tokio::select! {
            _ = terminate.recv() => {}
            _ = interrupt.recv() => {}
        }
    }
    #[cfg(not(unix))]
    {
        let _ = tokio::signal::ctrl_c().await;
    }
}

/// A number from the environment, or `default` when it is not set or not a number.
fn number(name: &str, default: usize) -> usize {
    parse_number(std::env::var(name).ok().as_deref(), default)
}

fn parse_number(value: Option<&str>, default: usize) -> usize {
    value.and_then(|s| s.trim().parse().ok()).unwrap_or(default)
}

/// A whole number of seconds, or `default` when it is not set or not a number.
fn seconds(value: Option<&str>, default: Duration) -> Duration {
    value.and_then(|s| s.trim().parse().ok()).map_or(default, Duration::from_secs)
}

#[cfg(feature = "web-viewer")]
fn web_viewer() -> Option<specdeck_server::WebViewer> {
    Some(specdeck_server::WebViewer::embedded())
}

#[cfg(not(feature = "web-viewer"))]
fn web_viewer() -> Option<specdeck_server::WebViewer> {
    None
}

fn turn_config() -> Option<TurnConfig> {
    let secret = std::env::var("SPECDECK_TURN_SECRET").ok().filter(|s| !s.is_empty())?;
    let urls: Vec<String> = std::env::var("SPECDECK_TURN_URLS")
        .ok()?
        .split(',')
        .map(|url| url.trim().to_owned())
        .filter(|url| !url.is_empty())
        .collect();
    if urls.is_empty() {
        return None;
    }
    let ttl = std::env::var("SPECDECK_TURN_TTL_SECONDS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(12 * 60 * 60);
    Some(TurnConfig { secret, urls, ttl: Duration::from_secs(ttl) })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_from_the_environment() {
        assert_eq!(parse_number(Some("250"), 100), 250);
        assert_eq!(parse_number(Some("many"), 100), 100);
        assert_eq!(parse_number(None, 100), 100);
    }

    #[test]
    fn room_hold_from_the_environment() {
        let default = Config::default().room_hold;
        assert_eq!(default, Duration::from_secs(300));
        assert_eq!(seconds(Some("900"), default), Duration::from_secs(900));
        assert_eq!(seconds(Some("0"), default), Duration::ZERO);
        assert_eq!(seconds(Some("five minutes"), default), default);
        assert_eq!(seconds(Some("-1"), default), default);
        assert_eq!(seconds(None, default), default);
    }
}
