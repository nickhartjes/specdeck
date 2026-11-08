//! Sliding-window counters for rate limiting join attempts, and token buckets for relayed
//! messages.

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

/// Allows `burst` events at once, then `per_second`.
pub struct TokenBucket {
    burst: f64,
    per_second: f64,
    tokens: f64,
    at: Instant,
}

impl TokenBucket {
    pub fn new(burst: f64, per_second: f64, now: Instant) -> Self {
        Self { burst, per_second, tokens: burst, at: now }
    }

    /// Takes one token; false when there is none left.
    pub fn take(&mut self, now: Instant) -> bool {
        let elapsed = now.saturating_duration_since(self.at).as_secs_f64();
        self.tokens = (self.tokens + elapsed * self.per_second).min(self.burst);
        self.at = now;
        if self.tokens < 1.0 {
            return false;
        }
        self.tokens -= 1.0;
        true
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

    #[test]
    fn a_bucket_allows_its_burst_then_its_rate() {
        let start = Instant::now();
        let mut bucket = TokenBucket::new(3.0, 2.0, start);
        assert!((0..3).all(|_| bucket.take(start)));
        assert!(!bucket.take(start));
        // Two tokens per second: one back after half a second.
        assert!(bucket.take(start + Duration::from_millis(500)));
        assert!(!bucket.take(start + Duration::from_millis(500)));
        // Never more than the burst, however long it waited.
        let later = start + Duration::from_secs(60);
        assert!((0..3).all(|_| bucket.take(later)));
        assert!(!bucket.take(later));
    }
}
