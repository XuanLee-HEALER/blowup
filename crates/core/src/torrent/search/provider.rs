//! Shared provider trait and per-task rate-limit / retry helpers.
//!
//! See spec §2.4-2.5.

use crate::torrent::search::types::{ProviderError, RawTorrent, SearchContext};
use async_trait::async_trait;
use std::future::Future;
use std::time::{Duration, Instant};

#[async_trait]
pub trait SearchProvider: Send + Sync {
    /// Stable short name used in logs and as `RawTorrent::source`.
    fn name(&self) -> &'static str;

    /// Minimum time between outgoing requests to this provider.
    /// Enforced within a single search task (see `CallPacer`).
    fn min_interval(&self) -> Duration;

    /// How many retry attempts on retryable errors (Timeout / Connect /
    /// 5xx / 429). Default 2.
    fn max_retries(&self) -> u32 {
        2
    }

    async fn search(&self, ctx: &SearchContext<'_>) -> Result<Vec<RawTorrent>, ProviderError>;
}

// ── CallPacer ──────────────────────────────────────────────────────

/// Per-task rate limiter. One instance per `search()` call — not
/// shared across tasks. Tracks the time of the last outgoing request
/// and, on the next call, sleeps just long enough to maintain
/// `min_interval` spacing.
///
/// Safe because it lives inside a single async task and is accessed
/// via `&mut self`; no Mutex needed.
pub struct CallPacer {
    min_interval: Duration,
    last: Option<Instant>,
}

impl CallPacer {
    pub fn new(min_interval: Duration) -> Self {
        Self {
            min_interval,
            last: None,
        }
    }

    pub async fn wait(&mut self) {
        if let Some(prev) = self.last {
            let elapsed = prev.elapsed();
            if elapsed < self.min_interval {
                tokio::time::sleep(self.min_interval - elapsed).await;
            }
        }
        self.last = Some(Instant::now());
    }
}

// ── with_retry helper ──────────────────────────────────────────────

/// Run `op` up to `max_retries + 1` times, with per-attempt pacing and
/// exponential backoff between retries. Only retryable errors trigger
/// another attempt; non-retryable (parse / 4xx) short-circuit out.
pub async fn with_retry<F, Fut, T>(
    pacer: &mut CallPacer,
    max_retries: u32,
    mut op: F,
) -> Result<T, ProviderError>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<T, ProviderError>>,
{
    let mut attempt: u32 = 0;
    loop {
        pacer.wait().await;
        match op().await {
            Ok(v) => return Ok(v),
            Err(e) if !e.is_retryable() => return Err(e),
            Err(e) if attempt >= max_retries => return Err(e),
            Err(e) => {
                tracing::warn!(attempt, error = %e, "provider call failed, retrying");
                let backoff = Duration::from_secs(2u64.pow(attempt).min(30));
                tokio::time::sleep(backoff).await;
                attempt += 1;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn pacer_first_call_no_wait() {
        let mut p = CallPacer::new(Duration::from_millis(100));
        let t = Instant::now();
        p.wait().await;
        assert!(t.elapsed() < Duration::from_millis(20));
    }

    #[tokio::test]
    async fn pacer_enforces_min_interval() {
        let mut p = CallPacer::new(Duration::from_millis(100));
        p.wait().await;
        let t = Instant::now();
        p.wait().await;
        assert!(t.elapsed() >= Duration::from_millis(95));
        assert!(t.elapsed() < Duration::from_millis(250));
    }

    #[tokio::test]
    async fn retry_returns_ok_on_first_success() {
        let mut pacer = CallPacer::new(Duration::from_millis(1));
        let result: Result<i32, _> = with_retry(&mut pacer, 2, || async { Ok(42) }).await;
        assert_eq!(result.unwrap(), 42);
    }

    #[tokio::test]
    async fn retry_short_circuits_on_non_retryable() {
        let mut pacer = CallPacer::new(Duration::from_millis(1));
        let mut calls = 0u32;
        let result: Result<i32, _> = with_retry(&mut pacer, 5, || {
            calls += 1;
            async { Err::<i32, _>(ProviderError::Http4xx(400)) }
        })
        .await;
        assert!(result.is_err());
        assert_eq!(calls, 1, "should not retry on 4xx");
    }

    #[tokio::test]
    async fn retry_exhausts_budget_on_retryable() {
        let mut pacer = CallPacer::new(Duration::from_millis(1));
        let mut calls = 0u32;
        let result: Result<i32, _> = with_retry(&mut pacer, 2, || {
            calls += 1;
            async { Err::<i32, _>(ProviderError::Timeout) }
        })
        .await;
        assert!(result.is_err());
        assert_eq!(calls, 3, "1 initial + 2 retries = 3 total attempts");
    }

    #[tokio::test]
    async fn retry_recovers_after_failure() {
        let mut pacer = CallPacer::new(Duration::from_millis(1));
        let mut calls = 0u32;
        let result: Result<i32, _> = with_retry(&mut pacer, 3, || {
            calls += 1;
            let should_succeed = calls >= 2;
            async move {
                if should_succeed {
                    Ok(99)
                } else {
                    Err(ProviderError::Timeout)
                }
            }
        })
        .await;
        assert_eq!(result.unwrap(), 99);
        assert_eq!(calls, 2);
    }
}
