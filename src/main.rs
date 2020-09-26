use clap::App;
use log::info;

use png_glitch::ErrorKind;

fn start(file_name: &str) -> Result<(), ErrorKind> {
  info!("file name = {}", file_name);
  let mut glitch = png_glitch::open(file_name)?;
  glitch.scan_line(|row| row[0] = if row[0] % 2 == 1 { 1 } else { 3 });
  glitch
    .serialize(&mut std::io::stdout())
    .map_err(|_| ErrorKind::IOError)?;
  Ok(())
}

fn main() {
  env_logger::init();
  let app = App::new("png-glitch")
    .version("0.1.0")
    .about("Glitch PNG image file")
    .author("chikoski")
    .args_from_usage("<FILE>         'PNG file'")
    .get_matches();
  if let Some(file) = app.value_of("FILE") {
    match start(file) {
      _ => (),
    };
  }
}
