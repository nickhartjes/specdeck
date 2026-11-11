//! Which pages the app's windows may load (`confine-window-navigation` design decision 1). Session
//! windows and the settings window show only the app's own page, and Claude's browser only web
//! pages. Each window refuses everything else in its `on_navigation` handler; links leave the app
//! through `open_content_link` instead, so there is one way to the system browser.

use tauri::{AppHandle, Manager, Runtime, Url, WebviewUrl, WebviewWindowBuilder};

/// A builder for a window that shows the app's own page, `url`: a session window or the settings
/// window. It refuses every other page. Every such window is built here; a test checks that no
/// other code builds one.
pub fn app_window<'a, R: Runtime, M: Manager<R>>(
    app: &'a M,
    label: &str,
    url: WebviewUrl,
) -> WebviewWindowBuilder<'a, R, M> {
    let origin = app_origin(app.app_handle());
    WebviewWindowBuilder::new(app, label, url).on_navigation(move |url| own_page(url, &origin))
}

/// A builder for a window of Claude's browser, on its empty start page. It loads only web pages,
/// however it is sent elsewhere: a redirect, a link, a script or going back.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
pub fn browser_window<'a, R: Runtime, M: Manager<R>>(
    app: &'a M,
    label: &str,
) -> WebviewWindowBuilder<'a, R, M> {
    let start = WebviewUrl::External(Url::parse("about:blank").unwrap());
    WebviewWindowBuilder::new(app, label, start).on_navigation(web_page)
}

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

/// Whether Claude's browser may load `url`: a web page, the empty page it starts on, or a frame a
/// page writes itself (`<iframe srcdoc>`), which has that page's origin. The rule also sees
/// frames, and cannot tell them from the page, so `about:srcdoc` is allowed at the top too, where
/// it is empty. Only the macOS browser makes windows; the tests check the rule everywhere.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
pub fn web_page(url: &Url) -> bool {
    matches!(url.scheme(), "http" | "https")
        || matches!(url.as_str(), "about:blank" | "about:srcdoc")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn url(text: &str) -> Url {
        Url::parse(text).unwrap()
    }

    /// The app's own source files, without their tests: `path` and text.
    fn sources() -> Vec<(String, String)> {
        let mut files = Vec::new();
        let mut dirs = vec![std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src")];
        while let Some(dir) = dirs.pop() {
            for entry in std::fs::read_dir(dir).unwrap().flatten() {
                let path = entry.path();
                if path.is_dir() {
                    dirs.push(path);
                } else if path.extension().is_some_and(|ext| ext == "rs") {
                    let text = std::fs::read_to_string(&path).unwrap();
                    let code = text.split("#[cfg(test)]").next().unwrap().to_owned();
                    files.push((path.display().to_string(), code));
                }
            }
        }
        files
    }

    /// A window built without a navigation rule could be led to another page, which would then
    /// receive the session. So windows are built only by `app_window` and `browser_window`, and
    /// each of those sets one.
    #[test]
    fn every_window_gets_a_navigation_rule() {
        let builders: Vec<(String, String)> = sources()
            .into_iter()
            .flat_map(|(path, code)| {
                code.match_indices("WebviewWindowBuilder::new(")
                    .map(|(at, _)| {
                        // The builder's statement: up to the end of the expression it starts.
                        let rest = &code[at..];
                        let end = rest.find("\n}").unwrap_or(rest.len());
                        (path.clone(), rest[..end].to_owned())
                    })
                    .collect::<Vec<_>>()
            })
            .collect();
        assert!(!builders.is_empty());
        for (path, builder) in builders {
            assert!(path.ends_with("navigation.rs"), "{path} builds a window: use app_window");
            assert!(builder.contains(".on_navigation("), "{path}: a window without a rule");
        }
        let navigation = sources().into_iter().find(|(path, _)| path.ends_with("navigation.rs"));
        let builds = navigation.unwrap().1.matches("WebviewWindowBuilder::new(").count();
        assert_eq!(builds, 2, "app_window and browser_window");
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
    fn web_page_is_http_https_the_empty_page_and_srcdoc_frames() {
        let allowed =
            ["http://localhost:5173/", "https://example.com/", "about:blank", "about:srcdoc"];
        for allowed in allowed {
            assert!(web_page(&url(allowed)), "{allowed}");
        }
        for refused in [
            "tauri://localhost/",
            "file:///etc/passwd",
            "data:text/html,x",
            "blob:https://example.com/0b1c2d3e-0000-4000-8000-000000000000",
            "javascript:alert(1)",
            "about:config",
        ] {
            assert!(!web_page(&url(refused)), "{refused}");
        }
    }
}
