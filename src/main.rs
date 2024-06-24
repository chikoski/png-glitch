use clap::Parser;
use png_glitch::{FilterType, PngGlitch};

use crate::cli::Cli;

mod cli;

fn main() {
    let config = Cli::parse().into();

    if let Err(e) = start(&config) {
        println!("{:?}", e);
    }
}

fn start(cli: &Cli) -> anyhow::Result<()> {
    let mut glitch = PngGlitch::open(&cli.png_file)?;
    run(&mut glitch);
    glitch.save(&cli.output_file)?;
    Ok(())
}

fn run(glitch: &mut PngGlitch) {
    glitch.glitch(|context| {
        context.data()[1] = 0;
    });

    glitch.foreach_scanline(|scanline| {
        scanline[2] = 0;
    });
}
