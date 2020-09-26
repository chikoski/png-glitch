mod error;
pub mod png;

pub use error::ErrorKind;
pub use png::parse;
pub use png::PNG;

use std::fs::File;
use std::io::{Read, Write};

type ScanLineOperatorFunction = fn(&mut [u8]) -> ();

enum Command {
  ScanLine(ScanLineOperatorFunction),
}

pub struct Glitcher {
  png: PNG,
  commands: Vec<Command>,
}

impl Glitcher {
  pub fn new(png: PNG) -> Glitcher {
    Glitcher {
      png: png,
      commands: Vec::new(),
    }
  }
  pub fn scan_line(&mut self, f: ScanLineOperatorFunction) {
    self.commands.push(Command::ScanLine(f))
  }
  pub fn serialize(&mut self, dest: &mut dyn Write) -> std::io::Result<()> {
    self.execute();
    png::serialize(&self.png, dest)
  }
  fn execute(&mut self) {
    for command in self.commands.iter() {
      match command {
        Command::ScanLine(f) => {
          for line in self.png.scan_lines_mut() {
            f(line);
          }
        }
      }
    }
  }
}

pub fn open(path: &str) -> Result<Glitcher, ErrorKind> {
  let mut buffer = Vec::new();
  load(path, &mut buffer).map_err(|_| ErrorKind::IOError)?;
  let parsed = parse(&buffer)?;
  Ok(Glitcher::new(parsed))
}

fn load(file_name: &str, buffer: &mut Vec<u8>) -> std::io::Result<()> {
  let mut file = File::open(file_name)?;
  file.read_to_end(buffer)?;
  std::io::Result::Ok(())
}
