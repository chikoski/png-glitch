use glitch_context::{ChangeFilterType, GlitchContext, Replace, SetZero, Transpose};

fn main() -> anyhow::Result<()> {
    // CARGO_MANIFEST_DIR is this example's crate directory; the sample lives in
    // the png-glitch crate beside it, so the input works from any directory.
    let input_path = concat!(env!("CARGO_MANIFEST_DIR"), "/../png-glitch/etc/sample00.png");
    // Output goes to target/examples-output/ instead of polluting the cwd.
    let output_dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../target/examples-output");
    std::fs::create_dir_all(output_dir)?;
    let output_path = format!("{output_dir}/glitched_output.png");

    println!("Opening {}", input_path);
    let mut context = GlitchContext::open(input_path, None)?;

    println!("Applying filters...");
    context.add_filter(ChangeFilterType { magnitude: 0.1 });
    context.add_filter(Transpose { magnitude: 0.05 });
    context.add_filter(Replace { magnitude: 0.01 });
    context.add_filter(SetZero { magnitude: 0.01 });

    context.execute();

    println!("Saving to {}", output_path);
    context.save(output_path)?;

    println!("Done!");
    Ok(())
}
