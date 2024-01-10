use clap::Parser;

#[derive(Parser, Debug)]
pub struct CommandLineArguments {
    pub file: String,
    #[arg(short)]
    pub output_file: Option<String>,
}