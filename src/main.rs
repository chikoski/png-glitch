use clap::Parser;
use png_glitch::PngGlitch;

use crate::cli::Cli;

mod cli;

fn main() {
    let config = Cli::parse();

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
    glitch.foreach_scanline(|scanline| {
        scanline.update(2, 0);
    });
}
