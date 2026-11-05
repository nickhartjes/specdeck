//! Runs the signalling service. Configuration comes from the environment:
//!
//! - `SPECDECK_LISTEN`: address to listen on (default `0.0.0.0:8080`)
//! - `SPECDECK_TURN_SECRET`: coturn's `static-auth-secret`; without it, no TURN credentials
//! - `SPECDECK_TURN_URLS`: comma-separated TURN URLs handed to clients
//! - `SPECDECK_TURN_TTL_SECONDS`: how long credentials stay valid (default 43200, 12 hours)
//! - `SPECDECK_TRUSTED_PROXIES`: reverse proxies in front of the service, such as an ingress
//!   (default 0); the client address for rate limiting is then read from `X-Forwarded-For`
//!
//! Built with the `web-viewer` feature, it also serves the web viewer at `/`.

use std::sync::Arc;
use std::time::Duration;

use specdeck_signalling::{Config, Service, TurnConfig, serve};
use tokio::net::TcpListener;

#[tokio::main]
async fn main() -> std::io::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();
    let address = std::env::var("SPECDECK_LISTEN").unwrap_or_else(|_| "0.0.0.0:8080".into());
    let trusted_proxies =
        std::env::var("SPECDECK_TRUSTED_PROXIES").ok().and_then(|s| s.parse().ok()).unwrap_or(0);
    let config =
        Config { turn: turn_config(), trusted_proxies, web: web_viewer(), ..Config::default() };
    if config.turn.is_none() {
        tracing::warn!("no TURN relay configured (SPECDECK_TURN_SECRET, SPECDECK_TURN_URLS)");
    }
    let listener = TcpListener::bind(&address).await?;
    tracing::info!("signalling service listening on {address}");
    serve(listener, Arc::new(Service::new(config))).await
}

#[cfg(feature = "web-viewer")]
fn web_viewer() -> Option<specdeck_signalling::WebViewer> {
    Some(specdeck_signalling::WebViewer::embedded())
}

#[cfg(not(feature = "web-viewer"))]
fn web_viewer() -> Option<specdeck_signalling::WebViewer> {
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
