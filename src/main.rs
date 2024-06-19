use std::path::Path;

use clap::Parser;

use png_glitch::PngGlitch;

use crate::cli::Cli;

mod cli;

fn main() {
    let arguments = Cli::parse();
    let input_file = arguments.png_file;
    let output_file = arguments.output_file.unwrap();

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