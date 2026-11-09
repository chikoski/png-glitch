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
    /// Number of timed iterations in which anything was allocated. Reports written before this
    /// field existed read as 0.
    #[serde(default)]
    pub alloc_iters: usize,
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

/// Stages that must not allocate in steady state: the live path (`Encoding`/`reload_pixels`,
/// `Renderer`, `LivePipeline`, `LiveRecipe`) and the effects that run on scan lines in place,
/// which `crates/png-glitch/tests/*_alloc.rs` already cover. `parse`, `serialize` and
/// `full-frame` (the file workflow) and `from_pixels/*` (builds a model) are report-only, and so
/// is `effect/pixel-sort`, which still allocates a pixel list per row (#208).
pub fn is_live_stage(name: &str) -> bool {
    const LIVE_EFFECTS: [&str; 11] = [
        "invert",
        "shift-channels",
        "brighten",
        "darken",
        "refilter-paeth",
        "bitwise-xor",
        "swap-channels",
        "hsl-shift",
        "channel-shift",
        "swap-blocks",
        "masked-invert",
    ];
    name.starts_with("live/")
        || name.starts_with("reload_pixels/")
        || name.starts_with("render_into/")
        || name
            .strip_prefix("effect/")
            .is_some_and(|e| LIVE_EFFECTS.contains(&e))
}

impl Report {
    /// Why this run fails the zero-allocation check, one message per live stage that breaks it.
    ///
    /// A live stage fails when it allocates in more than `max(1, frames / 10)` timed iterations
    /// (one stray iteration is tolerated: the counter is global, so it also sees other threads,
    /// e.g. a rayon worker waking up), or when its live heap at the end of the loop is larger
    /// than at the start.
    pub fn live_violations(&self) -> Vec<String> {
        let allowed = (self.meta.frames / 10).max(1);
        let mut out = Vec::new();
        for s in self.stages.iter().filter(|s| is_live_stage(&s.name)) {
            if s.alloc_iters > allowed {
                out.push(format!(
                    "{}: allocated in {} of {} iterations ({} bytes per iteration on average); a live stage must not allocate after warm-up",
                    s.name, s.alloc_iters, self.meta.frames, s.alloc_bytes_per_iter
                ));
            }
            if s.live_bytes_end > s.live_bytes_start {
                out.push(format!(
                    "{}: live heap grew by {} bytes over the run ({} -> {})",
                    s.name,
                    s.live_bytes_end - s.live_bytes_start,
                    s.live_bytes_start,
                    s.live_bytes_end
                ));
            }
        }
        out
    }

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stats::Summary;

    fn stage(name: &str, alloc_iters: usize, start: u64, end: u64) -> StageReport {
        StageReport {
            name: name.into(),
            summary: Summary {
                count: 1,
                mean_ms: 1.0,
                p50_ms: 1.0,
                p95_ms: 1.0,
                p99_ms: 1.0,
                max_ms: 1.0,
            },
            alloc_bytes_per_iter: alloc_iters as u64 * 100,
            alloc_iters,
            live_bytes_start: start,
            live_bytes_end: end,
            over_33ms_share: None,
        }
    }

    fn report(frames: usize, stages: Vec<StageReport>) -> Report {
        Report {
            meta: Meta {
                target: "t".into(),
                simd128: false,
                rayon_num_threads: None,
                git_commit: None,
                size: "360p".into(),
                width: 640,
                height: 360,
                frames,
                warmup: 0,
                pool: 1,
            },
            stages,
        }
    }

    #[test]
    fn live_stages_are_the_live_path_and_the_in_place_effects() {
        for live in [
            "live/encode",
            "live/frame/static/swap",
            "reload_pixels/rgba-paeth",
            "render_into/sub",
            "effect/invert",
            "effect/masked-invert",
        ] {
            assert!(is_live_stage(live), "{live}");
        }
        for report_only in [
            "parse",
            "serialize",
            "full-frame",
            "from_pixels/rgba-paeth",
            "effect/pixel-sort",
            "tag-only/set_filter_type",
            "unfilter/sub",
        ] {
            assert!(!is_live_stage(report_only), "{report_only}");
        }
    }

    #[test]
    fn a_live_stage_that_allocates_every_frame_or_grows_the_heap_fails() {
        let r = report(
            30,
            vec![
                stage("live/encode", 30, 100, 100),
                stage("live/decode", 1, 100, 100), // one stray iteration is tolerated
                stage("effect/invert", 0, 100, 164), // heap grew
                stage("parse", 30, 100, 900),      // report-only
            ],
        );
        let v = r.live_violations();
        assert_eq!(v.len(), 2, "{v:?}");
        assert!(v[0].starts_with("live/encode: allocated in 30 of 30"));
        assert!(v[1].starts_with("effect/invert: live heap grew by 64 bytes"));
    }

    #[test]
    fn a_report_without_alloc_iters_still_reads() {
        let json = r#"{"name":"a","count":1,"mean_ms":1.0,"p50_ms":1.0,"p95_ms":1.0,"p99_ms":1.0,"max_ms":1.0,"alloc_bytes_per_iter":5,"live_bytes_start":1,"live_bytes_end":1}"#;
        let s: StageReport = serde_json::from_str(json).unwrap();
        assert_eq!(s.alloc_iters, 0);
    }
}
