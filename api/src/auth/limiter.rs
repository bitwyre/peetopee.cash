use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

pub struct RateLimiter {
    max: u32,
    window: Duration,
    hits: Mutex<HashMap<String, Vec<Instant>>>,
}

impl RateLimiter {
    pub fn new(max: u32, window: Duration) -> Self {
        Self { max, window, hits: Mutex::new(HashMap::new()) }
    }

    /// Returns true if the request is allowed, and records it.
    pub fn check(&self, key: &str) -> bool {
        let mut hits = self.hits.lock().unwrap();
        let now = Instant::now();
        // Prune stale hits across all keys first, dropping entries left empty so that
        // attacker-controlled keys (e.g. arbitrary emails or IPs) can't grow the map
        // unboundedly over time.
        hits.retain(|_, v| {
            v.retain(|t| now.duration_since(*t) < self.window);
            !v.is_empty()
        });
        let entry = hits.entry(key.to_string()).or_default();
        if entry.len() as u32 >= self.max {
            return false;
        }
        entry.push(now);
        true
    }
}
