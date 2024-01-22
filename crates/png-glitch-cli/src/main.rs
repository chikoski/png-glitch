use std::path::Path;

use clap::Parser;

use png_glitch::PngGlitch;

use crate::command_line_arguments::CommandLineArguments;

mod command_line_arguments;

fn main() {
    let arguments = CommandLineArguments::parse();
    let input_file = arguments.file;
    let output_file = arguments.output_file.unwrap_or("output.png".to_string());

    if let Err(e) = run(input_file, output_file) {
        println!("{:?}", e);
    }
}

fn run(input: impl AsRef<Path>, output: impl AsRef<Path>) -> anyhow::Result<()> {
    let mut glitch = PngGlitch::open(input)?;

    glitch.glitch(|context| {
        context.data()[1] = 0;
    });
    glitch.foreach_scanline(|scanline| {
        scanline[2] = 0;
    });

    glitch.save(output)
}

#[cfg(target_arch = "wasm32")]
mod wasm32 {}
