use glitch_context::{ChangeFilterType, GlitchContext, Replace, SetZero, Transpose};

fn main() -> anyhow::Result<()> {
    // Relative path to crates/png-glitch/etc/sample00.png from crates/glitch-context

    let input_path = "../png-glitch/etc/sample00.png";
    let output_path = "glitched_output.png";

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
