//! `--check-live` runs in `cargo test` too: the live stages must not allocate in steady state.
//! It spawns the binary because the allocation counter is global, so it would also count the
//! other tests of a unit-test process running in parallel.

use std::process::Command;

fn bench() -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_png-glitch-bench"));
    cmd.env("RAYON_NUM_THREADS", "1");
    cmd
}

#[test]
fn live_stages_do_not_allocate_in_steady_state() {
    for stages in [
        "live/frame/static/swap",
        "live/encode",
        "reload_pixels/rgba-paeth",
        "render_into/sub",
        "effect/invert",
    ] {
        let out = bench()
            .args([
                "--size", "360p", "--frames", "10", "--warmup", "2", "--pool", "2",
            ])
            .args(["--stages", stages, "--check-live"])
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{stages}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        assert!(String::from_utf8_lossy(&out.stdout).contains("stage"));
    }
}

#[test]
fn report_only_stages_do_not_fail_the_check() {
    // `parse` allocates by design.
    let out = bench()
        .args([
            "--size", "360p", "--frames", "5", "--warmup", "1", "--pool", "2",
        ])
        .args(["--stages", "parse", "--check-live"])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
}
