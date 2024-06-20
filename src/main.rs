use std::path::Path;

use clap::Parser;

use png_glitch::PngGlitch;

use crate::cli::{Cli, Config};

mod cli;

fn main() {
    let config = Cli::parse().into();

    if let Err(e) = start(&config) {
        println!("{:?}", e);
    }
}

fn start(config: &Config) -> anyhow::Result<()> {
    let mut glitch = PngGlitch::open(&config.input_file)?;
    run(&mut glitch);
    glitch.save(&config.output_file)?;
    Ok(())
}

fn run(glitch: &mut PngGlitch)  {
    glitch.glitch(|context| {
        context.data()[1] = 0;
    });
    glitch.foreach_scanline(|scanline| {
        scanline[2] = 0;
    });
}