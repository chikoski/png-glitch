use crate::cli::{Cli, GlitchStrategy};
use crate::command::{Command, RandomCopy, Substitute};
use png_glitch::PngGlitch;

pub struct Context {
    command: Box<dyn Command>,
    png_glitch: PngGlitch,
    output_file: String,
}

impl Context {
    pub fn start(&mut self) -> anyhow::Result<()> {
        self.command.run(&mut self.png_glitch);
        self.png_glitch.save(&self.output_file)?;
        Ok(())
    }
}

impl TryFrom<Cli> for Context {
    type Error = anyhow::Error;

    fn try_from(cli: Cli) -> Result<Self, Self::Error> {
        let png_glitch = PngGlitch::open(cli.png_file)?;
        let command = create_command(cli.sub_command);
        let output_file = cli.output_file;

        let context = Context {
            png_glitch,
            command,
            output_file,
        };
        Ok(context)
    }
}

fn create_command(glitch_strategy: Option<GlitchStrategy>) -> Box<dyn Command> {
    match glitch_strategy {
        Some(strategy) => create_command_from_glitch_strategy(strategy),
        None => Box::new(Substitute::default()),
    }
}

fn create_command_from_glitch_strategy(glitch_strategy: GlitchStrategy) -> Box<dyn Command> {
    match glitch_strategy {
        GlitchStrategy::Substitute { index, value} => {
            Box::new(Substitute::new(index, value))
        }
        GlitchStrategy::RandomCopy { times } => {
            Box::new(RandomCopy::new(times))
        }
    }
}