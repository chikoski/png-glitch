//! The JSON report and its human-readable table.

use crate::stats::Summary;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Meta {
    pub target: String,
    pub simd128: bool,
    pub rayon_num_threads: Option<String>,
    pub git_commit: Option<String>,
    pub size: String,
    pub width: u32,
    pub height: u32,
    pub frames: usize,
    pub warmup: usize,
    pub pool: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StageReport {
    pub name: String,
    #[serde(flatten)]
    pub summary: Summary,
    /// Mean bytes allocated per timed iteration.
    pub alloc_bytes_per_iter: u64,
    /// Live heap before the first and after the last measured iteration.
    pub live_bytes_start: u64,
    pub live_bytes_end: u64,
    /// Share of iterations over 33.3 ms; only reported for the full-frame stage.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub over_33ms_share: Option<f64>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Report {
    pub meta: Meta,
    pub stages: Vec<StageReport>,
}

impl Report {
    pub fn table(&self) -> String {
        let m = &self.meta;
        let mut out = format!(
            "target={} simd128={} RAYON_NUM_THREADS={} commit={} size={} ({}x{}) frames={} warmup={} pool={}\n\n",
            m.target,
            m.simd128,
            m.rayon_num_threads.as_deref().unwrap_or("-"),
            m.git_commit.as_deref().unwrap_or("-"),
            m.size,
            m.width,
            m.height,
            m.frames,
            m.warmup,
            m.pool,
        );
        out.push_str(&format!(
            "{:<26} {:>5} {:>9} {:>9} {:>9} {:>9} {:>9} {:>12} {:>10}\n",
            "stage", "n", "mean ms", "p50 ms", "p95 ms", "p99 ms", "max ms", "alloc/iter", "heap Δ"
        ));
        for s in &self.stages {
            let delta = s.live_bytes_end as i128 - s.live_bytes_start as i128;
            out.push_str(&format!(
                "{:<26} {:>5} {:>9.3} {:>9.3} {:>9.3} {:>9.3} {:>9.3} {:>12} {:>10}",
                s.name,
                s.summary.count,
                s.summary.mean_ms,
                s.summary.p50_ms,
                s.summary.p95_ms,
                s.summary.p99_ms,
                s.summary.max_ms,
                s.alloc_bytes_per_iter,
                delta,
            ));
            if let Some(share) = s.over_33ms_share {
                out.push_str(&format!("  over 33.3 ms: {:.1}%", share * 100.0));
            }
            out.push('\n');
        }
        out
    }
}
