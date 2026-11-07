//! The browser on macOS: a hidden, incognito WebKit window per session, driven through WebKit's own
//! calls, since Tauri's `eval` returns nothing and Tauri takes no snapshots (`claude-browser`
//! design decision 5). The spike on branch `spike-browser` measured these calls in every window
//! state.

use std::cell::Cell;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use block2::RcBlock;
use objc2::MainThreadMarker;
use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2_app_kit::{NSBitmapImageFileType, NSBitmapImageRep, NSImage, NSImageCompressionFactor};
use objc2_foundation::{NSDictionary, NSError, NSNumber, NSString};
use objc2_web_kit::{WKSnapshotConfiguration, WKWebView};
use serde::Deserialize;
use specdeck_core::browser::{BrowserAction, BrowserHandler, BrowserResult};
use tauri::webview::PageLoadEvent;
use tauri::{AppHandle, Manager, Url, WebviewUrl, WebviewWindow, WebviewWindowBuilder};
use tokio::sync::{Notify, oneshot};

use crate::sessions::SessionId;

/// What runs inside the page.
const SCRIPT: &str = include_str!("browser.js");

/// How long a page may take to load.
const PAGE_LOAD: Duration = Duration::from_secs(30);
/// How long a click or key may take to start a page load that is then waited for.
const SETTLE: Duration = Duration::from_millis(300);
/// The largest screenshot sent; small enough to travel in the timeline later as well.
const MAX_SCREENSHOT_BYTES: usize = 150 * 1024;
/// Snapshot width in points and JPEG quality, tried in order until one fits.
const SCREENSHOT_STEPS: [(f64, f64); 3] = [(640.0, 0.7), (640.0, 0.5), (480.0, 0.5)];

/// One session's browser.
struct Browser {
    window: WebviewWindow,
    /// The private name under which `browser.js` keeps its state on the page's window.
    key: String,
    loads: Arc<Loads>,
}

/// Counts page loads, so an action can wait for the one it started.
#[derive(Default)]
struct Loads {
    counts: Mutex<(u64, u64)>, // started, finished
    changed: Notify,
}

impl Loads {
    fn seen(&self, event: PageLoadEvent) {
        let mut counts = self.counts.lock().unwrap();
        match event {
            PageLoadEvent::Started => counts.0 += 1,
            PageLoadEvent::Finished => counts.1 += 1,
        }
        drop(counts);
        self.changed.notify_waiters();
    }

    fn counts(&self) -> (u64, u64) {
        *self.counts.lock().unwrap()
    }

    /// Waits until more than `finished` loads have finished.
    async fn finished_after(&self, finished: u64, limit: Duration) -> bool {
        tokio::time::timeout(limit, async {
            loop {
                let changed = self.changed.notified();
                if self.counts().1 > finished {
                    return;
                }
                changed.await;
            }
        })
        .await
        .is_ok()
    }

    /// Waits until more than `started` loads have started.
    async fn started_after(&self, started: u64, limit: Duration) -> bool {
        tokio::time::timeout(limit, async {
            loop {
                let changed = self.changed.notified();
                if self.counts().0 > started {
                    return;
                }
                changed.await;
            }
        })
        .await
        .is_ok()
    }
}

fn browsers() -> &'static Mutex<HashMap<SessionId, Arc<Browser>>> {
    static BROWSERS: OnceLock<Mutex<HashMap<SessionId, Arc<Browser>>>> = OnceLock::new();
    BROWSERS.get_or_init(Mutex::default)
}

pub fn runner(app: AppHandle, id: SessionId) -> BrowserHandler {
    Arc::new(move |action| {
        let app = app.clone();
        Box::pin(async move {
            match browser(&app, id) {
                Ok(browser) => browser.run(action).await,
                Err(error) => {
                    BrowserResult::error(format!("The browser could not be opened: {error}"))
                }
            }
        })
    })
}

pub fn close(_app: &AppHandle, id: SessionId) {
    if let Some(browser) = browsers().lock().unwrap().remove(&id) {
        let _ = browser.window.destroy();
    }
}

/// Session `id`'s browser, made now if it has none.
fn browser(app: &AppHandle, id: SessionId) -> tauri::Result<Arc<Browser>> {
    let mut all = browsers().lock().unwrap();
    if let Some(browser) = all.get(&id) {
        return Ok(browser.clone());
    }
    let label = super::label(id);
    // A window of that name left behind would refuse the new one.
    if let Some(stale) = app.get_webview_window(&label) {
        let _ = stale.destroy();
    }
    let key = format!("__specdeck_{}", random_hex());
    let loads = Arc::new(Loads::default());
    let seen = loads.clone();
    let window = WebviewWindowBuilder::new(
        app,
        &label,
        WebviewUrl::External(Url::parse("about:blank").unwrap()),
    )
    .title("Claude's browser")
    .inner_size(1280.0, 800.0)
    .visible(false)
    .focused(false)
    .skip_taskbar(true)
    .incognito(true)
    .initialization_script(format!("{}.installConsole();", call_on(&key)))
    .on_page_load(move |_, payload| seen.seen(payload.event()))
    .build()?;
    let browser = Arc::new(Browser { window, key, loads });
    all.insert(id, browser.clone());
    tracing::info!(session = id, "browser.opened");
    Ok(browser)
}

/// `browser.js` applied to `key`; append `.action(...)`.
fn call_on(key: &str) -> String {
    format!("(\n{SCRIPT}\n)({})", serde_json::to_string(key).unwrap())
}

fn random_hex() -> String {
    let mut bytes = [0u8; 8];
    let _ = getrandom::fill(&mut bytes);
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// What every `browser.js` action answers.
#[derive(Deserialize)]
struct Answer {
    ok: bool,
    text: String,
}

impl Browser {
    async fn run(&self, action: BrowserAction) -> BrowserResult {
        match action {
            BrowserAction::Navigate { url } => self.navigate(&url).await,
            BrowserAction::Snapshot => self.script("read()").await,
            BrowserAction::Click { element } => self.settled(&format!("click({element})")).await,
            BrowserAction::Type { element, text, submit } => {
                let text = serde_json::to_string(&text).unwrap();
                let call = format!("type({element}, {text}, {submit})");
                if submit { self.settled(&call).await } else { self.script(&call).await }
            }
            BrowserAction::Press { key } => {
                self.settled(&format!("press({})", serde_json::to_string(&key).unwrap())).await
            }
            BrowserAction::Screenshot => self.screenshot().await,
            BrowserAction::Back => self.back().await,
            BrowserAction::Console => self.script("consoleEntries()").await,
        }
    }

    /// Runs a `browser.js` action.
    async fn script(&self, call: &str) -> BrowserResult {
        let js = format!("JSON.stringify({}.{call})", call_on(&self.key));
        match self.evaluate(js).await {
            Ok(json) => match serde_json::from_str::<Answer>(&json) {
                Ok(Answer { ok: true, text }) => BrowserResult::text(text),
                Ok(Answer { ok: false, text }) => BrowserResult::error(text),
                Err(_) => BrowserResult::error("The page gave an answer that could not be read."),
            },
            Err(error) => BrowserResult::error(error),
        }
    }

    /// Runs an action that may load another page, waits for that page, and says where it is.
    async fn settled(&self, call: &str) -> BrowserResult {
        let (started, _) = self.loads.counts();
        let result = self.script(call).await;
        if result.is_error {
            return result;
        }
        if self.loads.started_after(started, SETTLE).await {
            let (_, finished) = self.loads.counts();
            // The load may already be done; then this returns at once.
            let pending = self.loads.counts().0 > finished;
            if pending {
                self.loads.finished_after(finished, PAGE_LOAD).await;
            }
        }
        let page = self.script("page()").await;
        BrowserResult::text(format!("{}\nNow on: {}", result.text, page.text))
    }

    async fn navigate(&self, address: &str) -> BrowserResult {
        let url = match Url::parse(address) {
            Ok(url) if matches!(url.scheme(), "http" | "https") => url,
            Ok(_) => return BrowserResult::error("Only http and https addresses can be opened."),
            Err(error) => {
                return BrowserResult::error(format!("`{address}` is not an address: {error}"));
            }
        };
        // WebKit reports no load for a server that is not there; ask first, so the answer says why.
        if let Err(problem) = reachable(&url).await {
            return BrowserResult::error(format!("The page could not be loaded: {problem}."));
        }
        let (_, finished) = self.loads.counts();
        if let Err(error) = self.window.navigate(url) {
            return BrowserResult::error(format!("The page could not be opened: {error}"));
        }
        if !self.loads.finished_after(finished, PAGE_LOAD).await {
            let page = self.script("page()").await;
            return BrowserResult::error(format!(
                "The page did not finish loading within {} seconds. Now on: {}",
                PAGE_LOAD.as_secs(),
                page.text
            ));
        }
        let page = self.script("page()").await;
        BrowserResult::text(format!("Opened {}", page.text))
    }

    async fn back(&self) -> BrowserResult {
        let (started, finished) = self.loads.counts();
        let _ = self.window.eval("history.back()");
        if !self.loads.started_after(started, Duration::from_secs(2)).await {
            return BrowserResult::error("There is no earlier page to go back to.");
        }
        self.loads.finished_after(finished, PAGE_LOAD).await;
        let page = self.script("page()").await;
        BrowserResult::text(format!("Back on {}", page.text))
    }

    async fn screenshot(&self) -> BrowserResult {
        for (width, quality) in SCREENSHOT_STEPS {
            match self.snapshot(width, quality).await {
                Ok(jpeg) if jpeg.len() <= MAX_SCREENSHOT_BYTES => {
                    let page = self.script("page()").await;
                    return BrowserResult::image(page.text, STANDARD.encode(jpeg));
                }
                Ok(_) => continue,
                Err(error) => return BrowserResult::error(error),
            }
        }
        BrowserResult::error("The page is too busy to fit in a screenshot of 150 KB.")
    }

    /// Runs `js` in the page and gives its result, which must be a string.
    async fn evaluate(&self, js: String) -> Result<String, String> {
        let (answer, answered) = oneshot::channel();
        let sent = self.window.with_webview(move |webview| {
            // SAFETY: on macOS `inner` is the WKWebView, and this runs on the main thread.
            let webview = unsafe { &*(webview.inner() as *const WKWebView) };
            let answer = Cell::new(Some(answer));
            let handler = RcBlock::new(move |result: *mut AnyObject, error: *mut NSError| {
                let Some(answer) = answer.take() else { return };
                let outcome = match unsafe { error.as_ref() } {
                    Some(error) => {
                        Err(format!("The page's script failed: {}", error.localizedDescription()))
                    }
                    None => unsafe { result.as_ref() }
                        .and_then(|r| r.downcast_ref::<NSString>())
                        .map(|s| s.to_string())
                        .ok_or_else(|| "The page gave no answer.".to_owned()),
                };
                let _ = answer.send(outcome);
            });
            unsafe {
                webview
                    .evaluateJavaScript_completionHandler(&NSString::from_str(&js), Some(&handler))
            };
        });
        if sent.is_err() {
            return Err("The browser window is gone.".into());
        }
        answered.await.unwrap_or_else(|_| Err("The browser did not answer.".into()))
    }

    /// A snapshot of the visible page, `width` points wide, as JPEG.
    async fn snapshot(&self, width: f64, quality: f64) -> Result<Vec<u8>, String> {
        let (answer, answered) = oneshot::channel();
        let sent = self.window.with_webview(move |webview| {
            // SAFETY: as in `evaluate`.
            let webview = unsafe { &*(webview.inner() as *const WKWebView) };
            let Some(main) = MainThreadMarker::new() else {
                let _ =
                    answer.send(Err("The screenshot was not taken on the main thread.".to_owned()));
                return;
            };
            let config = unsafe { WKSnapshotConfiguration::new(main) };
            unsafe { config.setSnapshotWidth(Some(&NSNumber::new_f64(width))) };
            let answer = Cell::new(Some(answer));
            let handler = RcBlock::new(move |image: *mut NSImage, error: *mut NSError| {
                let Some(answer) = answer.take() else { return };
                let outcome = match (unsafe { image.as_ref() }, unsafe { error.as_ref() }) {
                    (_, Some(error)) => {
                        Err(format!("The screenshot failed: {}", error.localizedDescription()))
                    }
                    (Some(image), None) => jpeg(image, quality)
                        .ok_or_else(|| "The screenshot could not be encoded.".to_owned()),
                    (None, None) => Err("The screenshot came back empty.".to_owned()),
                };
                let _ = answer.send(outcome);
            });
            unsafe {
                webview.takeSnapshotWithConfiguration_completionHandler(Some(&config), &handler)
            };
        });
        if sent.is_err() {
            return Err("The browser window is gone.".into());
        }
        answered.await.unwrap_or_else(|_| Err("The browser did not answer.".into()))
    }
}

/// macOS encodes: image, TIFF, bitmap, JPEG. No image crate.
fn jpeg(image: &NSImage, quality: f64) -> Option<Vec<u8>> {
    let tiff = image.TIFFRepresentation()?;
    let bitmap = NSBitmapImageRep::imageRepWithData(&tiff)?;
    let factor = NSNumber::new_f64(quality);
    let properties: Retained<NSDictionary<NSString, AnyObject>> = NSDictionary::from_slices(
        &[unsafe { NSImageCompressionFactor }],
        &[&*factor as &AnyObject],
    );
    let data = unsafe {
        bitmap.representationUsingType_properties(NSBitmapImageFileType::JPEG, &properties)
    }?;
    Some(data.to_vec())
}

/// Whether something answers at the address's host and port.
async fn reachable(url: &Url) -> Result<(), String> {
    let host = url.host_str().ok_or("the address has no host")?;
    let port = url.port_or_known_default().ok_or("the address has no port")?;
    match tokio::time::timeout(Duration::from_secs(5), tokio::net::TcpStream::connect((host, port)))
        .await
    {
        Ok(Ok(_)) => Ok(()),
        Ok(Err(error)) => Err(format!("nothing answers at {host}:{port} ({error})")),
        Err(_) => Err(format!("{host}:{port} did not answer within 5 seconds")),
    }
}
