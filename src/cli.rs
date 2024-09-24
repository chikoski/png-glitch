use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
pub struct Cli {
    #[arg(short, default_value = "glitched.png")]
    pub output_file: String,
    pub png_file: String,

    #[command(subcommand)]
    pub sub_command: Option<GlitchStrategy>,
}
#[derive(Subcommand, Debug)]
pub enum GlitchStrategy {
    Substitute {
        #[clap(short, default_value = "0")]
        index: usize,
        #[clap(short, default_value = "0")]
        value: u8,
    },
    RandomCopy {
        #[clap(short, default_value = "1")]
        times: u32,
    }
}