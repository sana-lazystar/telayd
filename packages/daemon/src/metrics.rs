//! Latency and observability metrics for the daemon.
//!
//! Lightweight in-memory metrics (no external push in L0).
//! K3 target: p95 round-trip (inquiry-push sent → tmux inject complete) < 500ms.

use std::collections::VecDeque;
use std::sync::Mutex;
use std::time::Instant;

use tracing::info;

/// Maximum number of latency samples to keep in the rolling window.
const MAX_SAMPLES: usize = 200;

/// Collects per-inquiry latency samples.
pub struct MetricsCollector {
    /// Rolling window of latency measurements in milliseconds.
    latency_samples: Mutex<VecDeque<u64>>,
}

impl Default for MetricsCollector {
    fn default() -> Self {
        Self::new()
    }
}

impl MetricsCollector {
    pub fn new() -> Self {
        Self {
            latency_samples: Mutex::new(VecDeque::with_capacity(MAX_SAMPLES)),
        }
    }

    /// Records a latency sample (milliseconds).
    pub fn record_latency(&self, latency_ms: u64) {
        let mut samples = self.latency_samples.lock().expect("lock latency_samples");
        if samples.len() >= MAX_SAMPLES {
            samples.pop_front();
        }
        samples.push_back(latency_ms);
    }

    /// Computes an approximate p95 latency from the rolling window.
    ///
    /// Returns `None` if there are no samples yet.
    pub fn p95_latency_ms(&self) -> Option<u64> {
        let samples = self.latency_samples.lock().expect("lock");
        if samples.is_empty() {
            return None;
        }
        let mut sorted: Vec<u64> = samples.iter().copied().collect();
        sorted.sort_unstable();
        let idx = ((sorted.len() as f64) * 0.95) as usize;
        Some(sorted[idx.min(sorted.len() - 1)])
    }

    /// Logs a summary of current metrics.
    pub fn log_summary(&self) {
        let p95 = self.p95_latency_ms();
        let race_total = crate::tmux_controller::INJECT_RACE_TOTAL
            .load(std::sync::atomic::Ordering::Relaxed);
        info!(
            target: "metrics",
            p95_latency_ms = ?p95,
            inject_race_total = race_total,
            "metrics summary"
        );
    }
}

/// A stopwatch for measuring per-inquiry round-trip latency.
pub struct LatencyTimer {
    start: Instant,
}

impl LatencyTimer {
    pub fn start() -> Self {
        Self {
            start: Instant::now(),
        }
    }

    /// Returns elapsed milliseconds since `start()`.
    pub fn elapsed_ms(&self) -> u64 {
        self.start.elapsed().as_millis() as u64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn p95_computed_correctly() {
        let m = MetricsCollector::new();
        for i in 1..=100u64 {
            m.record_latency(i);
        }
        let p95 = m.p95_latency_ms().unwrap();
        // Sorted 1..100; index 95 (0-based) = value 96.
        assert!(p95 >= 94 && p95 <= 100, "unexpected p95: {p95}");
    }

    #[test]
    fn no_samples_returns_none() {
        let m = MetricsCollector::new();
        assert!(m.p95_latency_ms().is_none());
    }

    #[test]
    fn window_rolls_over_max() {
        let m = MetricsCollector::new();
        for i in 0..300u64 {
            m.record_latency(i);
        }
        let samples = m.latency_samples.lock().unwrap();
        assert_eq!(samples.len(), MAX_SAMPLES);
    }
}
