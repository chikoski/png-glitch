//! The stages that exist today. Each stage times one call on a fresh model per iteration;
//! building the input (encoding, parsing) happens outside the timed region.

use crate::alloc::{allocated_bytes, live_bytes};
use crate::fixture::{encode_png, frame_rgba};
use crate::report::{Meta, Report, StageReport};
use crate::stats::{share_over_budget, summarize};
use png_glitch::effects::{
    BitOp, Bitwise, BlockSwap, ChannelShift, Effect, HslShift, Invert, PixelSort, Refilter,
    ShiftChannels, SortCriterion, SwapBlocks, SwapChannels, SwapTarget,
};
use png_glitch::effects::{Brighten, Darken};
use png_glitch::{FilterType, PngGlitch};
use std::hint::black_box;
use std::time::Instant;

pub struct Config {
    pub size_name: String,
    pub width: u32,
    pub height: u32,
    /// Measured iterations per stage.
    pub frames: usize,
    pub warmup: usize,
    /// Number of distinct frames cycled through (keeps setup memory and time bounded).
    pub pool: usize,
    /// Run only stages whose name contains this text.
    pub only: Option<String>,
}

type EffectFn = Box<dyn Fn(&mut PngGlitch)>;

const FILTERS: [(&str, FilterType, png::Filter); 5] = [
    ("none", FilterType::None, png::Filter::NoFilter),
    ("sub", FilterType::Sub, png::Filter::Sub),
    ("up", FilterType::Up, png::Filter::Up),
    ("average", FilterType::Average, png::Filter::Avg),
    ("paeth", FilterType::Paeth, png::Filter::Paeth),
];

/// Times `run` over `warmup + frames` iterations. `setup(i)` builds the input and is not timed.
/// Allocation counters wrap only the timed call; the heap is sampled around the whole loop.
fn run_stage<S, R>(
    name: &str,
    cfg: &Config,
    over_budget: bool,
    mut setup: impl FnMut(usize) -> S,
    mut run: impl FnMut(S) -> R,
) -> StageReport {
    eprintln!("  {name}");
    for i in 0..cfg.warmup {
        drop(black_box(run(setup(i % cfg.pool))));
    }
    let live_start = live_bytes();
    let mut samples = Vec::with_capacity(cfg.frames);
    let mut allocated = 0u64;
    for i in 0..cfg.frames {
        let input = setup(i % cfg.pool);
        let a0 = allocated_bytes();
        let t0 = Instant::now();
        let out = black_box(run(input));
        let elapsed = t0.elapsed();
        allocated += allocated_bytes() - a0;
        samples.push(elapsed.as_secs_f64() * 1000.0);
        drop(out);
    }
    let live_end = live_bytes();
    StageReport {
        name: name.to_string(),
        summary: summarize(&samples),
        alloc_bytes_per_iter: allocated / cfg.frames.max(1) as u64,
        live_bytes_start: live_start,
        live_bytes_end: live_end,
        over_33ms_share: over_budget.then(|| share_over_budget(&samples)),
    }
}

fn parse(bytes: &[u8]) -> PngGlitch {
    PngGlitch::from_bytes(bytes).expect("fixture PNG parses")
}

fn unfiltered(bytes: &[u8]) -> PngGlitch {
    let mut png = parse(bytes);
    png.remove_filter();
    png
}

/// Effects with fixed parameters. Row-based effects touch every fourth row.
fn effects(width: u32, height: u32) -> Vec<(&'static str, EffectFn)> {
    fn boxed<E: Effect + 'static>(e: E) -> EffectFn {
        Box::new(move |png| e.apply_to(png))
    }
    let rows: Vec<u32> = (0..height).step_by(4).collect();
    let blocks_x = (width / 16).max(2);
    let blocks_y = (height / 16).max(2);
    let swaps = (0..16)
        .map(|i| BlockSwap {
            src: (i % blocks_x, (i / 2) % blocks_y),
            dst: ((i * 7 + 3) % blocks_x, (i * 5 + 1) % blocks_y),
        })
        .collect();
    vec![
        ("invert", boxed(Invert::new())),
        ("shift-channels", boxed(ShiftChannels::new(8, -8, 16))),
        ("brighten", boxed(Brighten::new(20))),
        ("darken", boxed(Darken::new(20))),
        ("refilter-paeth", boxed(Refilter::new(FilterType::Paeth))),
        (
            "pixel-sort",
            boxed(PixelSort::new(SortCriterion::Brightness, rows.clone())),
        ),
        (
            "bitwise-xor",
            boxed(Bitwise::new(BitOp::Xor, 0x55, rows.clone())),
        ),
        (
            "swap-channels",
            boxed(SwapChannels::new(SwapTarget::Rg, rows.clone())),
        ),
        (
            "hsl-shift",
            boxed(HslShift::new(30.0, 1.1, 1.0, rows.clone())),
        ),
        ("channel-shift", boxed(ChannelShift::new(4, 0, -4, rows))),
        ("swap-blocks", boxed(SwapBlocks::new(16, swaps))),
    ]
}

pub fn run_all(cfg: &Config) -> Report {
    let pool = cfg.pool.max(1);
    let cfg = &Config {
        pool,
        size_name: cfg.size_name.clone(),
        only: cfg.only.clone(),
        ..*cfg
    };
    let wanted = |name: &str| cfg.only.as_deref().is_none_or(|o| name.contains(o));
    let (w, h) = (cfg.width, cfg.height);

    eprintln!("preparing {pool} fixture frame(s) at {w}x{h}...");
    let raw: Vec<Vec<u8>> = (0..pool as u32).map(|i| frame_rgba(w, h, i)).collect();
    let encode_all = |filter: Option<png::Filter>| -> Vec<Vec<u8>> {
        raw.iter().map(|f| encode_png(w, h, f, filter)).collect()
    };
    let mut stages = Vec::new();

    eprintln!("running stages:");
    // Frames as a webcam pipeline would hand them over: adaptive filtering.
    let adaptive = encode_all(None);
    if wanted("parse") {
        stages.push(run_stage(
            "parse",
            cfg,
            false,
            |i| &adaptive[i],
            |b| parse(b),
        ));
    }
    if wanted("serialize") {
        stages.push(run_stage(
            "serialize",
            cfg,
            false,
            |i| parse(&adaptive[i]),
            |png| png.to_bytes().expect("serialize"),
        ));
    }
    if wanted("full-frame") {
        stages.push(run_stage(
            "full-frame",
            cfg,
            true,
            |i| &adaptive[i],
            |b| {
                let mut png = parse(b);
                ShiftChannels::new(8, -8, 16).apply_to(&mut png);
                png.to_bytes().expect("serialize")
            },
        ));
    }
    drop(adaptive);

    {
        for (name, _, encoder_filter) in FILTERS {
            let stage = format!("unfilter/{name}");
            if !wanted(&stage) {
                continue;
            }
            let pngs = encode_all(Some(encoder_filter));
            stages.push(run_stage(
                &stage,
                cfg,
                false,
                |i| parse(&pngs[i]),
                |mut png| {
                    png.remove_filter();
                    png
                },
            ));
        }
    }

    // The rest start from an unfiltered model, built from frames stored with filter type None.
    let effect_list = effects(w, h);
    let needs_plain = FILTERS.iter().any(|(n, ..)| wanted(&format!("filter/{n}")))
        || wanted("tag-only/set_filter_type")
        || effect_list
            .iter()
            .any(|(n, _)| wanted(&format!("effect/{n}")));
    let plain = if needs_plain {
        encode_all(Some(png::Filter::NoFilter))
    } else {
        Vec::new()
    };
    for (name, filter_type, _) in FILTERS {
        let stage = format!("filter/{name}");
        if wanted(&stage) {
            stages.push(run_stage(
                &stage,
                cfg,
                false,
                |i| unfiltered(&plain[i]),
                |mut png| {
                    png.apply_filter(filter_type);
                    png
                },
            ));
        }
    }
    if wanted("tag-only") {
        stages.push(run_stage(
            "tag-only/set_filter_type",
            cfg,
            false,
            |i| unfiltered(&plain[i]),
            |mut png| {
                png.set_filter_type(FilterType::Sub);
                png
            },
        ));
    }
    for (name, effect) in effect_list {
        let stage = format!("effect/{name}");
        if wanted(&stage) {
            stages.push(run_stage(
                &stage,
                cfg,
                false,
                |i| unfiltered(&plain[i]),
                |mut png| {
                    effect(&mut png);
                    png
                },
            ));
        }
    }

    Report {
        meta: Meta {
            target: env!("BENCH_TARGET").to_string(),
            simd128: cfg!(target_feature = "simd128"),
            rayon_num_threads: std::env::var("RAYON_NUM_THREADS").ok(),
            git_commit: std::env::var("BENCH_GIT_COMMIT").ok(),
            size: cfg.size_name.clone(),
            width: w,
            height: h,
            frames: cfg.frames,
            warmup: cfg.warmup,
            pool,
        },
        stages,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tiny(only: Option<&str>) -> Config {
        Config {
            size_name: "tiny".into(),
            width: 64,
            height: 48,
            frames: 3,
            warmup: 1,
            pool: 2,
            only: only.map(str::to_string),
        }
    }

    #[test]
    fn all_stages_run_on_a_tiny_image() {
        let report = run_all(&tiny(None));
        let names: Vec<_> = report.stages.iter().map(|s| s.name.as_str()).collect();
        for expected in [
            "parse",
            "serialize",
            "full-frame",
            "unfilter/paeth",
            "filter/sub",
            "tag-only/set_filter_type",
            "effect/invert",
            "effect/swap-blocks",
        ] {
            assert!(names.contains(&expected), "missing {expected}: {names:?}");
        }
        assert!(report.stages.iter().all(|s| s.summary.count == 3));
        let full = report
            .stages
            .iter()
            .find(|s| s.name == "full-frame")
            .unwrap();
        assert!(full.over_33ms_share.is_some());
    }

    #[test]
    fn only_filters_stages() {
        let report = run_all(&tiny(Some("effect/invert")));
        assert_eq!(report.stages.len(), 1);
    }
}
