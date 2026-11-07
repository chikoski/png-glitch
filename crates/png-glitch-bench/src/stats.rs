//! Summary statistics over per-iteration timings.

use serde::{Deserialize, Serialize};

/// Frame budget at 30 fps, in milliseconds.
pub const FRAME_BUDGET_MS: f64 = 1000.0 / 30.0;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Summary {
    pub count: usize,
    pub mean_ms: f64,
    pub p50_ms: f64,
    pub p95_ms: f64,
    pub p99_ms: f64,
    pub max_ms: f64,
}

/// Nearest-rank percentile of an ascending-sorted slice (`q` in `0.0..=1.0`).
fn percentile(sorted: &[f64], q: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    let rank = (q * sorted.len() as f64).ceil() as usize;
    sorted[rank.clamp(1, sorted.len()) - 1]
}

pub fn summarize(samples_ms: &[f64]) -> Summary {
    let mut sorted = samples_ms.to_vec();
    sorted.sort_by(f64::total_cmp);
    let mean = if sorted.is_empty() {
        0.0
    } else {
        sorted.iter().sum::<f64>() / sorted.len() as f64
    };
    Summary {
        count: sorted.len(),
        mean_ms: mean,
        p50_ms: percentile(&sorted, 0.50),
        p95_ms: percentile(&sorted, 0.95),
        p99_ms: percentile(&sorted, 0.99),
        max_ms: sorted.last().copied().unwrap_or(0.0),
    }
}

/// Share of samples above the frame budget (`0.0..=1.0`).
pub fn share_over_budget(samples_ms: &[f64]) -> f64 {
    if samples_ms.is_empty() {
        return 0.0;
    }
    samples_ms.iter().filter(|&&s| s > FRAME_BUDGET_MS).count() as f64 / samples_ms.len() as f64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn percentiles_use_nearest_rank() {
        let samples: Vec<f64> = (1..=100).map(f64::from).collect();
        let s = summarize(&samples);
        assert_eq!(s.count, 100);
        assert_eq!(s.p50_ms, 50.0);
        assert_eq!(s.p95_ms, 95.0);
        assert_eq!(s.p99_ms, 99.0);
        assert_eq!(s.max_ms, 100.0);
        assert!((s.mean_ms - 50.5).abs() < 1e-9);
    }

    #[test]
    fn empty_input_is_all_zero() {
        let s = summarize(&[]);
        assert_eq!((s.count, s.p50_ms, s.max_ms), (0, 0.0, 0.0));
    }

    #[test]
    fn budget_share_counts_strictly_over() {
        assert_eq!(share_over_budget(&[10.0, 40.0, 50.0, 20.0]), 0.5);
    }
}
