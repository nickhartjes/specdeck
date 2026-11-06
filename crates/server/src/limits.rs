//! Sliding-window counters for rate limiting join attempts.

use std::collections::{HashMap, VecDeque};
use std::hash::Hash;
use std::time::{Duration, Instant};

pub struct RateLimiter<K> {
    limit: usize,
    window: Duration,
    hits: HashMap<K, VecDeque<Instant>>,
}

impl<K: Eq + Hash + Clone> RateLimiter<K> {
    pub fn new(limit: usize, window: Duration) -> Self {
        Self { limit, window, hits: HashMap::new() }
    }

    /// Whether `key` has used up its attempts in the current window.
    pub fn exceeded(&mut self, key: &K, now: Instant) -> bool {
        let Some(hits) = self.hits.get_mut(key) else { return false };
        while hits.front().is_some_and(|&at| now.duration_since(at) >= self.window) {
            hits.pop_front();
        }
        if hits.is_empty() {
            self.hits.remove(key);
            return false;
        }
        hits.len() >= self.limit
    }

    pub fn record(&mut self, key: K, now: Instant) {
        self.hits.entry(key).or_default().push_back(now);
        // Keep memory bounded when many clients come and go.
        if self.hits.len() > 10_000 {
            let window = self.window;
            self.hits
                .retain(|_, hits| hits.back().is_some_and(|&at| now.duration_since(at) < window));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refuses_after_the_limit_until_the_window_passes() {
        let start = Instant::now();
        let mut limiter = RateLimiter::new(3, Duration::from_secs(60));
        for i in 0..3 {
            assert!(!limiter.exceeded(&"ip", start + Duration::from_secs(i)));
            limiter.record("ip", start + Duration::from_secs(i));
        }
        assert!(limiter.exceeded(&"ip", start + Duration::from_secs(10)));
        assert!(!limiter.exceeded(&"other", start + Duration::from_secs(10)));
        // The first attempt leaves the window after 60 s.
        assert!(!limiter.exceeded(&"ip", start + Duration::from_secs(60)));
    }
}
