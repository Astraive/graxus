//! Request rate limiting using an interval-based approach.
//!
//! The [`RateLimiter`] ensures that API calls are spaced apart by at least
//! `60 / rpm` seconds, preventing 429 errors from provider rate limits.

use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Rate limiter that enforces a minimum interval between requests.
///
/// Uses a fixed-interval strategy: each request must wait at least
/// `60 / requests_per_minute` seconds after the previous one.
///
/// # Thread Safety
///
/// This type is safe to share across async tasks via `Arc<RateLimiter>`.
pub struct RateLimiter {
    requests_per_minute: u32,
    last_request: Mutex<Option<Instant>>,
}

impl RateLimiter {
    /// Create a new rate limiter with the given requests-per-minute limit.
    pub fn new(rpm: u32) -> Self {
        Self {
            requests_per_minute: rpm,
            last_request: Mutex::new(None),
        }
    }

    /// Wait if needed to respect the rate limit.
    ///
    /// If the previous request was less than `60/rpm` seconds ago, sleeps
    /// for the remaining duration. Otherwise returns immediately.
    pub async fn wait(&self) {
        let min_interval = Duration::from_secs_f64(60.0 / self.requests_per_minute as f64);

        // Determine if we need to wait, and for how long. The lock guard is
        // dropped at the end of this block, before any async work.
        let wait_time = {
            let last = self.last_request.lock().unwrap_or_else(|e| e.into_inner());
            match *last {
                Some(prev) => {
                    let elapsed = prev.elapsed();
                    if elapsed < min_interval {
                        Some(min_interval - elapsed)
                    } else {
                        None
                    }
                }
                None => None,
            }
        };

        if let Some(duration) = wait_time {
            tokio::time::sleep(duration).await;
        }

        *self.last_request.lock().unwrap_or_else(|e| e.into_inner()) = Some(Instant::now());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn new_limiter_has_no_last_request() {
        let limiter = RateLimiter::new(60);
        let last = limiter.last_request.lock().unwrap();
        assert!(last.is_none());
    }

    #[tokio::test]
    async fn first_request_does_not_wait() {
        let limiter = RateLimiter::new(60);
        let start = Instant::now();
        limiter.wait().await;
        // First request should return almost immediately (< 50ms)
        assert!(start.elapsed() < Duration::from_millis(50));
    }

    #[tokio::test]
    async fn rapid_requests_are_throttled() {
        let limiter = Arc::new(RateLimiter::new(600)); // 10/sec = 100ms interval
        let start = Instant::now();

        limiter.wait().await;
        limiter.wait().await;

        // Second request should have waited ~100ms
        let elapsed = start.elapsed();
        assert!(elapsed >= Duration::from_millis(80)); // some tolerance
    }

    #[tokio::test]
    async fn high_rpm_allows_rapid_requests() {
        let limiter = Arc::new(RateLimiter::new(6000)); // 100/sec = 10ms interval
        let start = Instant::now();

        for _ in 0..5 {
            limiter.wait().await;
        }

        // 5 requests at 10ms intervals should complete in ~50ms
        assert!(start.elapsed() < Duration::from_millis(200));
    }
}
