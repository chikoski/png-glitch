use clap::Parser;

#[derive(Parser, Debug)]
pub struct Cli {
    #[arg(short, default_value = "glitched.png")]
    pub output_file: Option<String>,
    pub png_file: String,
}

pub struct Config {
    pub output_file: String,
    pub input_file: String,
}

impl From<Cli> for Config {
    fn from(value: Cli) -> Self {
        Config {
            output_file: value.output_file.unwrap(),
            input_file: value.png_file,
        }
    }
}
