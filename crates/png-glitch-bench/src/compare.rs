//! `compare`: fail when a stage's p50 regresses by more than a threshold.

use crate::report::Report;
use std::collections::BTreeMap;

/// Stages whose baseline p50 is below this are noise-dominated on shared runners.
pub const MIN_BASE_MS: f64 = 1.0;

#[derive(Debug, PartialEq)]
pub struct Row {
    pub stage: String,
    pub base_ms: f64,
    pub head_ms: f64,
    /// `head / base - 1`.
    pub change: f64,
    pub ignored: bool,
    pub regressed: bool,
}

/// Best (lowest) p50 per stage over several runs of the same build; repeating runs and taking
/// the minimum filters out noise from a shared runner.
fn best_p50(reports: &[Report]) -> BTreeMap<String, f64> {
    let mut best = BTreeMap::new();
    for report in reports {
        for stage in &report.stages {
            let entry = best.entry(stage.name.clone()).or_insert(f64::INFINITY);
            *entry = entry.min(stage.summary.p50_ms);
        }
    }
    best
}

/// The threshold for `stage`: its own from `--stage-threshold`, else the global one.
fn threshold_for(stage: &str, threshold: f64, per_stage: &[(String, f64)]) -> f64 {
    per_stage
        .iter()
        .find(|(name, _)| name == stage)
        .map_or(threshold, |&(_, t)| t)
}

pub fn compare(
    base: &[Report],
    head: &[Report],
    threshold: f64,
    per_stage: &[(String, f64)],
) -> Vec<Row> {
    let base = best_p50(base);
    let head = best_p50(head);
    base.iter()
        .filter_map(|(stage, &base_ms)| {
            let head_ms = *head.get(stage)?;
            let change = if base_ms > 0.0 {
                head_ms / base_ms - 1.0
            } else {
                0.0
            };
            let ignored = base_ms < MIN_BASE_MS;
            Some(Row {
                stage: stage.clone(),
                base_ms,
                head_ms,
                change,
                ignored,
                regressed: !ignored && change > threshold_for(stage, threshold, per_stage),
            })
        })
        .collect()
}

pub fn render(rows: &[Row], threshold: f64, per_stage: &[(String, f64)]) -> String {
    let mut out = format!(
        "{:<26} {:>10} {:>10} {:>9}  (p50, threshold +{:.0}%, stages under {MIN_BASE_MS} ms ignored)\n",
        "stage",
        "base ms",
        "head ms",
        "change",
        threshold * 100.0
    );
    for r in rows {
        let own = per_stage.iter().find(|(name, _)| *name == r.stage);
        let flag = if r.regressed {
            "  REGRESSION"
        } else if r.ignored {
            "  (ignored)"
        } else {
            ""
        };
        out.push_str(&format!(
            "{:<26} {:>10.3} {:>10.3} {:>+8.1}%{flag}\n",
            r.stage,
            r.base_ms,
            r.head_ms,
            r.change * 100.0
        ));
        if let Some((_, t)) = own {
            out.pop();
            out.push_str(&format!("  [threshold +{:.0}%]\n", t * 100.0));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::report::{Meta, StageReport};
    use crate::stats::Summary;

    fn report(stages: &[(&str, f64)]) -> Report {
        Report {
            meta: Meta {
                target: "t".into(),
                simd128: false,
                rayon_num_threads: None,
                git_commit: None,
                size: "360p".into(),
                width: 640,
                height: 360,
                frames: 1,
                warmup: 0,
                pool: 1,
            },
            stages: stages
                .iter()
                .map(|&(name, p50)| StageReport {
                    name: name.into(),
                    summary: Summary {
                        count: 1,
                        mean_ms: p50,
                        p50_ms: p50,
                        p95_ms: p50,
                        p99_ms: p50,
                        max_ms: p50,
                    },
                    alloc_bytes_per_iter: 0,
                    alloc_iters: 0,
                    live_bytes_start: 0,
                    live_bytes_end: 0,
                    over_33ms_share: None,
                })
                .collect(),
        }
    }

    #[test]
    fn flags_only_regressions_over_threshold() {
        let base = [report(&[("a", 10.0), ("b", 10.0), ("c", 0.5)])];
        let head = [report(&[("a", 13.0), ("b", 11.0), ("c", 5.0)])];
        let rows = compare(&base, &head, 0.25, &[]);
        let flags: Vec<_> = rows
            .iter()
            .map(|r| (r.stage.as_str(), r.regressed))
            .collect();
        // c is under 1 ms in the baseline, so it is ignored even though it got 10x slower.
        assert_eq!(flags, [("a", true), ("b", false), ("c", false)]);
    }

    #[test]
    fn a_stage_threshold_replaces_the_global_one_for_that_stage_only() {
        let base = [report(&[("a", 10.0), ("b", 10.0)])];
        let head = [report(&[("a", 13.0), ("b", 13.0)])];
        let per_stage = [("a".to_string(), 0.40)];
        let flags: Vec<_> = compare(&base, &head, 0.25, &per_stage)
            .iter()
            .map(|r| (r.stage.clone(), r.regressed))
            .collect();
        assert_eq!(flags, [("a".to_string(), false), ("b".to_string(), true)]);
        assert!(
            render(&compare(&base, &head, 0.25, &per_stage), 0.25, &per_stage)
                .contains("[threshold +40%]")
        );
    }

    #[test]
    fn takes_the_best_of_repeated_runs() {
        let base = [report(&[("a", 10.0)]), report(&[("a", 20.0)])];
        let head = [report(&[("a", 30.0)]), report(&[("a", 11.0)])];
        let rows = compare(&base, &head, 0.25, &[]);
        assert_eq!((rows[0].base_ms, rows[0].head_ms), (10.0, 11.0));
        assert!(!rows[0].regressed);
    }

    #[test]
    fn stages_missing_on_one_side_are_skipped() {
        let rows = compare(
            &[report(&[("a", 10.0)])],
            &[report(&[("b", 10.0)])],
            0.25,
            &[],
        );
        assert!(rows.is_empty());
    }
}
