//! The flood limit (the `diagnostics` spec, *Log flood limit*): each event name is written at
//! most [`PER_MINUTE`] times a minute; the rest are counted and summed up in one
//! `log.suppressed` entry once the minute is over.

use std::collections::HashMap;
use std::time::{Duration, Instant};

/// Lines per event name per minute.
pub const PER_MINUTE: u32 = 30;
const MINUTE: Duration = Duration::from_secs(60);

/// One event name's current minute.
pub struct Window {
    pub start: Instant,
    written: u32,
    left_out: u64,
}

#[derive(Default)]
pub struct Limit {
    windows: HashMap<String, Window>,
}

impl Limit {
    /// Whether event `name` may be written at `now`, and how many of it were left out in its
    /// previous minute when that minute ended unnoticed (to be recorded first).
    pub fn admit(&mut self, name: &str, now: Instant) -> (bool, Option<u64>) {
        let Some(window) = self.windows.get_mut(name) else {
            self.windows.insert(name.to_owned(), Window { start: now, written: 1, left_out: 0 });
            return (true, None);
        };
        if now.saturating_duration_since(window.start) >= MINUTE {
            let left_out = std::mem::take(&mut window.left_out);
            *window = Window { start: now, written: 1, left_out: 0 };
            return (true, (left_out > 0).then_some(left_out));
        }
        if window.written < PER_MINUTE {
            window.written += 1;
            (true, None)
        } else {
            window.left_out += 1;
            (false, None)
        }
    }

    /// The event names whose minute is over with events left out, and how many; forgets every
    /// minute that is over.
    pub fn ended(&mut self, now: Instant) -> Vec<(String, u64)> {
        let mut ended = Vec::new();
        self.windows.retain(|name, window| {
            if now.saturating_duration_since(window.start) < MINUTE {
                return true;
            }
            if window.left_out > 0 {
                ended.push((name.clone(), window.left_out));
            }
            false
        });
        ended.sort();
        ended
    }

    #[cfg(test)]
    pub fn windows_mut(&mut self) -> impl Iterator<Item = &mut Window> {
        self.windows.values_mut()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thirty_a_minute_then_counted() {
        let mut limit = Limit::default();
        let start = Instant::now();
        let admitted = (0..500).filter(|_| limit.admit("peer.send_failed", start).0).count();
        assert_eq!(admitted, PER_MINUTE as usize);
        assert!(limit.ended(start + Duration::from_secs(59)).is_empty());
        assert_eq!(
            limit.ended(start + MINUTE),
            [("peer.send_failed".to_owned(), 500 - u64::from(PER_MINUTE))]
        );
        // Counted once only.
        assert!(limit.ended(start + 2 * MINUTE).is_empty());
    }

    #[test]
    fn names_are_limited_on_their_own() {
        let mut limit = Limit::default();
        let now = Instant::now();
        for _ in 0..100 {
            limit.admit("a", now);
        }
        assert_eq!(limit.admit("b", now), (true, None));
    }

    #[test]
    fn the_next_event_after_the_minute_brings_the_count() {
        let mut limit = Limit::default();
        let start = Instant::now();
        for _ in 0..40 {
            limit.admit("a", start);
        }
        assert_eq!(limit.admit("a", start + MINUTE), (true, Some(10)));
        assert!(limit.ended(start + MINUTE).is_empty(), "already recorded");
    }
}
