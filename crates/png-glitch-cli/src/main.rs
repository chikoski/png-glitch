mod command_line_arguments;

use std::path::Path;
use clap::Parser;
use png_glitch::PngGlitch;
use crate::command_line_arguments::CommandLineArguments;

fn main() {
    let arguments = CommandLineArguments::parse();
    for file in arguments.files {
        if let Err(e) = run(file) {
            println!("{:?}", e);
        }
    }
}

fn run(path: impl AsRef<Path>) -> anyhow::Result<()>{
    let glitch = PngGlitch::open(path)?;
    glitch.save("aaa.png")
}
