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
//! - `SPECDECK_MAX_ROOMS`: open rooms in total (default 4000)
//! - `SPECDECK_LOG_LEVEL`: `error`, `warn`, `info` or `debug` (default `info`); when it is unset,
//!   a `RUST_LOG` that is a plain level is used
//! - `SPECDECK_LOG_FORMAT`: `plain` (default) or `json`, one object per line
//!
//! Built with the `web-viewer` feature, it also serves the web viewer at `/`.

use std::sync::Arc;
use std::time::Duration;

use specdeck_server::{Config, Service, TurnConfig, log, serve};
use tokio::net::TcpListener;

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
        ..defaults
    };
    if config.turn.is_none() {
        log::no_turn();
    }
    let listener = TcpListener::bind(&address).await?;
    log::started(config.turn.is_some(), config.web.is_some());
    serve(listener, Arc::new(Service::new(config))).await
}

/// A number from the environment, or `default` when it is not set or not a number.
fn number(name: &str, default: usize) -> usize {
    std::env::var(name).ok().and_then(|s| s.parse().ok()).unwrap_or(default)
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
