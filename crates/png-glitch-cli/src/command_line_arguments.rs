use clap::Parser;

#[derive(Parser, Debug)]
pub struct CommandLineArguments {
    pub files: Vec<String>
}