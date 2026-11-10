//! Which pages the app's windows may load (`confine-window-navigation` design decision 1). Session
//! windows and the settings window show only the app's own page, and Claude's browser only web
//! pages. Each window refuses everything else in its `on_navigation` handler; links leave the app
//! through `open_content_link` instead, so there is one way to the system browser.

use tauri::{AppHandle, Runtime, Url};

/// The origin the app's own page is served from: the `devUrl` in a development build, otherwise
/// the address Tauri serves the bundled page at on this platform.
pub fn app_origin<R: Runtime>(app: &AppHandle<R>) -> Url {
    let dev = tauri::is_dev().then(|| app.config().build.dev_url.clone()).flatten();
    dev.unwrap_or_else(|| {
        let bundled = if cfg!(windows) { "http://tauri.localhost/" } else { "tauri://localhost/" };
        Url::parse(bundled).unwrap()
    })
}

/// Whether `url` is the app's own page: the same scheme, host and port as `app_origin`.
pub fn own_page(url: &Url, app_origin: &Url) -> bool {
    url.scheme() == app_origin.scheme()
        && url.host_str() == app_origin.host_str()
        && url.port_or_known_default() == app_origin.port_or_known_default()
}

/// Whether Claude's browser may load `url`: a web page, or the empty page it starts on. Only the
/// macOS browser makes windows; the tests check the rule everywhere.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
pub fn web_page(url: &Url) -> bool {
    matches!(url.scheme(), "http" | "https") || url.as_str() == "about:blank"
}

#[cfg(test)]
mod tests {
    use super::*;

    fn url(text: &str) -> Url {
        Url::parse(text).unwrap()
    }

    #[test]
    fn own_page_is_the_app_origin_only() {
        let bundled = url("tauri://localhost");
        let windows = url("http://tauri.localhost/");
        let dev = url("http://localhost:5173");
        assert!(own_page(&url("tauri://localhost/index.html"), &bundled));
        assert!(own_page(&url("tauri://localhost/index.html?window=settings"), &bundled));
        assert!(own_page(&url("http://tauri.localhost/"), &windows));
        assert!(own_page(&url("http://tauri.localhost/index.html"), &windows));
        assert!(own_page(&url("http://localhost:5173/"), &dev));
        // Each only for its own origin.
        assert!(!own_page(&url("tauri://localhost/index.html"), &windows));
        assert!(!own_page(&url("http://tauri.localhost/"), &bundled));
        assert!(!own_page(&url("http://localhost:5173/"), &bundled));
        assert!(!own_page(&url("http://localhost:5174/"), &dev));
        assert!(!own_page(&url("https://tauri.localhost/"), &windows));
        for other in
            ["https://example.com/", "file:///etc/passwd", "tauri://localhost.evil.example/"]
        {
            for origin in [&bundled, &windows, &dev] {
                assert!(!own_page(&url(other), origin), "{other} for {origin}");
            }
        }
    }

    #[test]
    fn web_page_is_http_https_and_the_empty_page() {
        for allowed in ["http://localhost:5173/", "https://example.com/", "about:blank"] {
            assert!(web_page(&url(allowed)), "{allowed}");
        }
        for refused in [
            "tauri://localhost/",
            "file:///etc/passwd",
            "data:text/html,x",
            "blob:https://example.com/0b1c2d3e-0000-4000-8000-000000000000",
            "javascript:alert(1)",
            "about:srcdoc",
        ] {
            assert!(!web_page(&url(refused)), "{refused}");
        }
    }
}
