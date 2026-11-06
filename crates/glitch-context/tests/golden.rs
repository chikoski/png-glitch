//! Golden tests for seed reproducibility.
//!
//! Each PNG filter is applied with a fixed seed to a small committed fixture and the BLAKE3 hash
//! of the encoded output is snapshotted. A change here means seeded output changed; see the
//! "Output & Reproducibility" section of the README and CONTRIBUTING.md before updating snapshots.

use glitch_context::{FilterConfig, GlitchContext};
use insta::assert_snapshot;

const SEED: u64 = 42;
const FIXTURE: &[u8] = include_bytes!("fixtures/small.png");

fn golden_hash(yaml: &str) -> String {
    let config: FilterConfig = serde_saphyr::from_str(yaml).expect("valid filter config");
    let mut ctx = GlitchContext::new(FIXTURE, Some(SEED)).expect("fixture parses");
    ctx.add_from_config(config)
        .expect("filter is supported for PNG");
    ctx.execute();
    blake3::hash(&ctx.buffer().expect("encodes"))
        .to_hex()
        .to_string()
}

macro_rules! golden {
    ($name:ident, $yaml:expr) => {
        #[test]
        fn $name() {
            assert_snapshot!(golden_hash($yaml));
        }
    };
}

golden!(
    change_filter_type,
    "type: change-filter-type\nmagnitude: 0.3"
);
golden!(replace, "type: replace\nmagnitude: 0.1");
golden!(transpose, "type: transpose\nmagnitude: 0.3");
golden!(set_zero, "type: set-zero\nmagnitude: 0.5");
golden!(random_copy, "type: random-copy\ntimes: 5");
golden!(substitute, "type: substitute\nindex: 3\nvalue: 255");
golden!(
    pixel_sort,
    "type: pixel-sort\nmagnitude: 1.0\ncriterion: brightness"
);
golden!(
    bitwise,
    "type: bitwise\nmagnitude: 0.2\nop: xor\nvalue: 255"
);
golden!(
    channel_swap,
    "type: channel-swap\nmagnitude: 0.5\ntarget: br"
);
golden!(horizontal_shift, "type: horizontal-shift\nmagnitude: 0.3");
golden!(
    block_scramble,
    "type: block-scramble\nmagnitude: 0.3\nblock_size: 8"
);
golden!(
    color_distortion,
    "type: color-distortion\nmagnitude: 0.3\nstrength: 40"
);
golden!(
    color_space_glitch,
    "type: color-space-glitch\nmagnitude: 0.5\nhue_shift: 90.0"
);
golden!(
    chromatic_aberration,
    "type: chromatic-aberration\nmagnitude: 0.5\nr_offset: 4\nb_offset: -4"
);
golden!(invert, "type: invert");
golden!(brighten, "type: brighten\nstrength: 30");
golden!(darken, "type: darken\nstrength: 30");
golden!(shift_channels, "type: shift-channels\nr: 15\ng: 0\nb: -15");
golden!(remove_filter, "type: remove-filter");
golden!(sub_filter, "type: sub-filter");
golden!(up_filter, "type: up-filter");
golden!(average_filter, "type: average-filter");
golden!(paeth_filter, "type: paeth-filter");
