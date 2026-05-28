use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Simple rate limiter using a token bucket.
pub struct RateLimiter {
    requests_per_minute: u32,
    last_request: Mutex<Option<Instant>>,
}

impl RateLimiter {
    pub fn new(rpm: u32) -> Self {
        Self { requests_per_minute: rpm, last_request: Mutex::new(None) }
    }

    /// Wait if needed to respect rate limit.
    pub async fn wait(&self) {
        let min_interval = Duration::from_secs_f64(60.0 / self.requests_per_minute as f64);
        let mut last = self.last_request.lock().unwrap();
        if let Some(prev) = *last {
            let elapsed = prev.elapsed();
            if elapsed < min_interval {
                let wait_time = min_interval - elapsed;
                drop(last);
                tokio::time::sleep(wait_time).await;
                *self.last_request.lock().unwrap() = Some(Instant::now());
                return;
            }
        }
        *last = Some(Instant::now());
    }
}
