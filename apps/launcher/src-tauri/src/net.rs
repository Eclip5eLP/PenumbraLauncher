//! HTTP client + bandwidth limiter shared by catalog fetches and downloads.

use std::sync::Mutex;
use std::time::{Duration, Instant};

pub fn client() -> reqwest::Client {
    reqwest::Client::builder()
        .user_agent(concat!("PenumbraLauncher/", env!("CARGO_PKG_VERSION")))
        .connect_timeout(Duration::from_secs(10))
        .read_timeout(Duration::from_secs(30))
        .pool_max_idle_per_host(32)
        .build()
        .expect("HTTP client builds")
}

/// Percent-encodes characters that are special in URL paths (versions may contain `+`).
pub fn path_segment(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || b"-._~".contains(&b) {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// Global token bucket: all downloads together stay under the user's limit.
#[derive(Default)]
pub struct Limiter {
    state: Mutex<Option<(Instant, f64)>>,
}

impl Limiter {
    /// Waits until `bytes` may be consumed at `limit_kbs` KB/s (0 = unlimited).
    pub async fn take(&self, bytes: usize, limit_kbs: u32) {
        if limit_kbs == 0 {
            return;
        }
        let rate = limit_kbs as f64 * 1024.0;
        let wait = {
            let mut st = self.state.lock().unwrap();
            let now = Instant::now();
            let (last, tokens) = st.get_or_insert((now, rate));
            // Refill, capped at one second of burst.
            let refilled = (*tokens + now.duration_since(*last).as_secs_f64() * rate).min(rate);
            let after = refilled - bytes as f64;
            *last = now;
            *tokens = after;
            if after < 0.0 { Duration::from_secs_f64(-after / rate) } else { Duration::ZERO }
        };
        if !wait.is_zero() {
            tokio::time::sleep(wait).await;
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn encodes_plus() {
        assert_eq!(super::path_segment("0.3.14+1.5.1-r2"), "0.3.14%2B1.5.1-r2");
    }
}
