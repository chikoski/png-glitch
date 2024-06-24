use clap::Parser;

#[derive(Parser, Debug)]
pub struct Cli {
    #[arg(short, default_value = "glitched.png")]
    pub output_file: String,
    pub png_file: String,
}
