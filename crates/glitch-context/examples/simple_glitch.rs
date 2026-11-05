use glitch_context::{ChangeFilterType, GlitchContext, Replace, SetZero, Transpose};
use std::fs;
use std::path::Path;

fn main() -> anyhow::Result<()> {
    let input_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../png-glitch/etc/sample00.png");
    let out_dir = std::env::temp_dir().join("png-glitch-examples");
    fs::create_dir_all(&out_dir)?;
    let output_path = out_dir.join("glitched_output.png");

    println!("Opening {}", input_path.display());
    let mut context = GlitchContext::open(&input_path, None)?;

    println!("Applying filters...");
    context.add_filter(ChangeFilterType { magnitude: 0.1 });
    context.add_filter(Transpose { magnitude: 0.05 });
    context.add_filter(Replace { magnitude: 0.01 });
    context.add_filter(SetZero { magnitude: 0.01 });

    context.execute();

    println!("Saving to {}", output_path.display());
    context.save(&output_path)?;

    println!("Done!");
    Ok(())
}
