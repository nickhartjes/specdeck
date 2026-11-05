//! Captures diagnostic events in memory for one test: everything emitted on the test's thread,
//! and in tasks and peer connections started from it.
#![allow(dead_code)]

use std::collections::BTreeMap;
use std::fmt;
use std::sync::{Arc, Mutex, OnceLock};

use tracing::Dispatch;
use tracing::field::{Field, Visit};
use tracing::subscriber::{DefaultGuard, NoSubscriber};
pub use tracing_subscriber::filter::LevelFilter;
use tracing_subscriber::layer::{Context, SubscriberExt};

#[derive(Clone, Debug)]
pub struct Captured {
    pub name: String,
    pub level: tracing::Level,
    pub fields: BTreeMap<String, String>,
}

impl Captured {
    pub fn field(&self, name: &str) -> Option<&str> {
        self.fields.get(name).map(String::as_str)
    }
}

#[derive(Clone, Default)]
pub struct Capture(Arc<Mutex<Vec<Captured>>>);

impl Capture {
    /// Starts capturing every event, at every level, until the guard is dropped.
    pub fn start() -> (Self, DefaultGuard) {
        Self::start_at(LevelFilter::TRACE)
    }

    /// Starts capturing events at `level` and above, as the app's log does.
    pub fn start_at(level: LevelFilter) -> (Self, DefaultGuard) {
        // With a single live subscriber, tracing caches whether an event is wanted from the
        // subscriber of the thread that reaches it first. A thread without a capture then turns
        // the event off for every capture until the next one starts. A second subscriber that
        // stays registered keeps tracing asking each capture.
        static KEEP: OnceLock<Dispatch> = OnceLock::new();
        KEEP.get_or_init(|| Dispatch::new(NoSubscriber::default()));
        let capture = Self::default();
        let subscriber = tracing_subscriber::registry().with(level).with(capture.clone());
        let guard = tracing::subscriber::set_default(subscriber);
        (capture, guard)
    }

    pub fn events(&self) -> Vec<Captured> {
        self.0.lock().unwrap().clone()
    }

    pub fn named(&self, name: &str) -> Vec<Captured> {
        self.events().into_iter().filter(|event| event.name == name).collect()
    }
}

impl<S: tracing::Subscriber> tracing_subscriber::Layer<S> for Capture {
    fn on_event(&self, event: &tracing::Event<'_>, _: Context<'_, S>) {
        let mut visitor = Fields::default();
        event.record(&mut visitor);
        let name = visitor.0.remove("message").unwrap_or_default();
        self.0.lock().unwrap().push(Captured {
            name,
            level: *event.metadata().level(),
            fields: visitor.0,
        });
    }
}

#[derive(Default)]
struct Fields(BTreeMap<String, String>);

impl Visit for Fields {
    fn record_str(&mut self, field: &Field, value: &str) {
        self.0.insert(field.name().to_owned(), value.to_owned());
    }

    fn record_debug(&mut self, field: &Field, value: &dyn fmt::Debug) {
        self.0.insert(field.name().to_owned(), format!("{value:?}"));
    }
}
