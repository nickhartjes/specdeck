//! The web viewer's routes, with a small page in place of the real build.

use std::sync::Arc;

use axum::body::{Body, to_bytes};
use axum::http::{Request, Response, StatusCode, header};
use specdeck_protocol::control::PROTOCOL_VERSION;
use specdeck_signalling::{Config, Service, WebViewer, router};
use tower::ServiceExt;

fn page() -> WebViewer {
    WebViewer::from_files([
        ("web.html".to_owned(), b"<!doctype html><title>SpecDeck</title>".to_vec()),
        ("assets/web-abc.js".to_owned(), b"console.log(1)".to_vec()),
        ("assets/specdeck_web_bg-abc.wasm".to_owned(), b"\0asm".to_vec()),
    ])
}

async fn get(web: Option<WebViewer>, path: &str) -> Response<Body> {
    let service = Arc::new(Service::new(Config { web, ..Config::default() }));
    let request = Request::get(path)
        .header(header::HOST, "specdeck.example.com")
        .body(Body::empty())
        .unwrap();
    router(service).oneshot(request).await.unwrap()
}

fn header<'a>(response: &'a Response<Body>, name: &str) -> &'a str {
    response.headers().get(name).and_then(|value| value.to_str().ok()).unwrap_or("")
}

#[tokio::test]
async fn the_page_is_served_at_the_root_and_at_join_with_its_protections() {
    for path in ["/", "/join"] {
        let response = get(Some(page()), path).await;
        assert_eq!(response.status(), StatusCode::OK, "{path}");
        assert_eq!(header(&response, "content-type"), "text/html; charset=utf-8");
        assert_eq!(header(&response, "cache-control"), "no-cache");
        let policy = header(&response, "content-security-policy");
        assert!(
            policy.starts_with("default-src 'none'; script-src 'self' 'wasm-unsafe-eval';"),
            "{policy}"
        );
        assert!(
            policy.contains(
                "connect-src 'self' wss://specdeck.example.com ws://specdeck.example.com;"
            ),
            "{policy}"
        );
        assert!(policy.contains("frame-ancestors 'none'"));
        assert_eq!(header(&response, "referrer-policy"), "no-referrer");
        assert_eq!(header(&response, "x-content-type-options"), "nosniff");
        assert_eq!(header(&response, "cross-origin-opener-policy"), "same-origin");
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        assert!(body.starts_with(b"<!doctype html>"));
    }
}

#[tokio::test]
async fn a_host_header_that_is_not_a_host_name_is_left_out_of_the_policy() {
    let service = Arc::new(Service::new(Config { web: Some(page()), ..Config::default() }));
    let request = Request::get("/")
        .header(header::HOST, "evil.example.com; script-src *")
        .body(Body::empty())
        .unwrap();
    let response = router(service).oneshot(request).await.unwrap();
    let policy = header(&response, "content-security-policy");
    assert!(policy.contains("connect-src 'self';"), "{policy}");
    assert!(!policy.contains("evil"));
}

#[tokio::test]
async fn assets_have_their_types_and_are_cached_for_good() {
    let script = get(Some(page()), "/assets/web-abc.js").await;
    assert_eq!(script.status(), StatusCode::OK);
    assert_eq!(header(&script, "content-type"), "text/javascript");
    assert_eq!(header(&script, "cache-control"), "public, max-age=31536000, immutable");
    let wasm = get(Some(page()), "/assets/specdeck_web_bg-abc.wasm").await;
    assert_eq!(header(&wasm, "content-type"), "application/wasm");
    assert_eq!(get(Some(page()), "/assets/missing.js").await.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn the_app_can_tell_the_page_is_there_and_its_protocol() {
    let response = get(Some(page()), "/web-viewer.json").await;
    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    assert_eq!(body, format!("{{\"protocol\":{PROTOCOL_VERSION}}}").as_bytes());
}

#[tokio::test]
async fn other_paths_are_not_found() {
    for path in ["/web.html", "/assets", "/other", "/join/x"] {
        assert_eq!(get(Some(page()), path).await.status(), StatusCode::NOT_FOUND, "{path}");
    }
}

#[tokio::test]
async fn without_the_web_viewer_the_service_serves_no_page() {
    for path in ["/", "/join", "/web-viewer.json", "/assets/web-abc.js"] {
        assert_eq!(get(None, path).await.status(), StatusCode::NOT_FOUND, "{path}");
    }
    let health = get(None, "/health").await;
    assert_eq!(health.status(), StatusCode::OK);
    assert_eq!(get(Some(page()), "/health").await.status(), StatusCode::OK);
}
