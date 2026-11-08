//! The web viewer, served by the service itself (`web-viewer` design decision 5), so every
//! sharing service, the public one or a self-hosted one, serves its own copy and nothing else is
//! deployed. Built with the `web-viewer` feature, the page (`app/ui/dist-web`) is embedded in the
//! binary; without it the service serves no page, as before.
//!
//! Routes: `/` and `/join` give the page, `/assets/*` its files, and `/web-viewer.json` tells the
//! host's app that the page is here and which session protocol it speaks, and operators which
//! release the service runs.

use std::collections::HashMap;
use std::sync::Arc;

use axum::Router;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use specdeck_protocol::control::PROTOCOL_VERSION;

/// The page's file at the root of the build.
const PAGE: &str = "web.html";

/// The web viewer's files, by path relative to the build's root (`web.html`, `assets/…`).
#[derive(Clone, Default)]
pub struct WebViewer {
    files: Arc<HashMap<String, Vec<u8>>>,
}

impl WebViewer {
    pub fn from_files(files: impl IntoIterator<Item = (String, Vec<u8>)>) -> Self {
        Self { files: Arc::new(files.into_iter().collect()) }
    }

    /// The page built into this binary.
    #[cfg(feature = "web-viewer")]
    pub fn embedded() -> Self {
        #[derive(rust_embed::RustEmbed)]
        #[folder = "$CARGO_MANIFEST_DIR/../../app/ui/dist-web"]
        struct Build;
        Self::from_files(
            Build::iter()
                .filter_map(|path| Some((path.to_string(), Build::get(&path)?.data.into_owned()))),
        )
    }

    fn file(&self, path: &str) -> Option<&[u8]> {
        self.files.get(path).map(Vec::as_slice)
    }
}

pub(crate) fn routes<S>(web: WebViewer) -> Router<S> {
    Router::new()
        .route("/", get(page))
        .route("/join", get(page))
        .route("/assets/{*path}", get(asset))
        .route("/web-viewer.json", get(version))
        .with_state(web)
}

async fn page(State(web): State<WebViewer>, headers: HeaderMap) -> Response {
    let Some(html) = web.file(PAGE) else { return StatusCode::NOT_FOUND.into_response() };
    let mut response = (
        [(header::CONTENT_TYPE, "text/html; charset=utf-8"), (header::CACHE_CONTROL, "no-cache")],
        html.to_vec(),
    )
        .into_response();
    let headers_out = response.headers_mut();
    if let Ok(policy) = HeaderValue::from_str(&content_security_policy(headers.get(header::HOST))) {
        headers_out.insert(header::CONTENT_SECURITY_POLICY, policy);
    }
    headers_out.insert("cross-origin-opener-policy", HeaderValue::from_static("same-origin"));
    protect(headers_out);
    response
}

async fn asset(State(web): State<WebViewer>, Path(path): Path<String>) -> Response {
    let path = format!("assets/{path}");
    let Some(bytes) = web.file(&path) else { return StatusCode::NOT_FOUND.into_response() };
    let mut response = (
        [
            (header::CONTENT_TYPE, content_type(&path)),
            // Built file names carry a hash of their content.
            (header::CACHE_CONTROL, "public, max-age=31536000, immutable"),
        ],
        bytes.to_vec(),
    )
        .into_response();
    protect(response.headers_mut());
    response
}

/// Lets the host's app see that this service serves the web viewer, and which protocol it speaks.
async fn version() -> Response {
    let mut response = (
        [(header::CONTENT_TYPE, "application/json"), (header::CACHE_CONTROL, "no-cache")],
        // The release this service was built from (the workspace version, which the release
        // workflow checks against the tag), so one request tells which release runs.
        format!(
            "{{\"protocol\":{PROTOCOL_VERSION},\"version\":\"{}\"}}",
            env!("CARGO_PKG_VERSION")
        ),
    )
        .into_response();
    protect(response.headers_mut());
    response
}

fn protect(headers: &mut HeaderMap) {
    headers.insert(header::X_CONTENT_TYPE_OPTIONS, HeaderValue::from_static("nosniff"));
    headers.insert(header::REFERRER_POLICY, HeaderValue::from_static("no-referrer"));
}

/// Only this service's own code, styles and WebSocket; no other address, no framing, no forms.
/// The page's own WebSocket address is named too, for browsers where `'self'` does not cover
/// WebSockets. It comes from the request's `Host`, which only ever names the address the page was
/// loaded from; a `Host` that is not a plain host name is left out.
fn content_security_policy(host: Option<&HeaderValue>) -> String {
    let own = host
        .and_then(|host| host.to_str().ok())
        .filter(|host| {
            !host.is_empty()
                && host.chars().all(|c| c.is_ascii_alphanumeric() || ".-:[]".contains(c))
        })
        .map(|host| format!(" wss://{host} ws://{host}"))
        .unwrap_or_default();
    format!(
        "default-src 'none'; script-src 'self' 'wasm-unsafe-eval'; style-src 'self' 'unsafe-inline'; \
         img-src 'self' data:; font-src 'self'; connect-src 'self'{own}; base-uri 'none'; \
         form-action 'none'; frame-ancestors 'none'"
    )
}

fn content_type(path: &str) -> &'static str {
    match path.rsplit_once('.').map(|(_, extension)| extension) {
        Some("js") => "text/javascript",
        Some("css") => "text/css",
        // Browsers compile WebAssembly while it downloads only with this type.
        Some("wasm") => "application/wasm",
        Some("svg") => "image/svg+xml",
        Some("png") => "image/png",
        Some("woff2") => "font/woff2",
        Some("json") => "application/json",
        _ => "application/octet-stream",
    }
}
