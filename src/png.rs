mod chunk;
mod crc;
mod parser;
mod serializer;

pub use super::ErrorKind;
pub use parser::parse;
pub use serializer::serialize;

pub enum ColorType {
  Grayscale,
  RGB,
  Palette,
  GrayscaleAlpha,
  RGBA,
  Unknown,
}

impl ColorType {
  pub fn new(value: u8) -> ColorType {
    match value {
      0 => ColorType::Grayscale,
      2 => ColorType::RGB,
      3 => ColorType::Palette,
      4 => ColorType::GrayscaleAlpha,
      6 => ColorType::RGBA,
      _ => ColorType::Unknown,
    }
  }
}

pub struct PNG {
  header: [u8; 25],
  data: Vec<u8>,
}

impl<'a> PNG {
  const MAGIC: [u8; 8] = [0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a];

  pub fn width(&self) -> u32 {
    u32::from_be_bytes([
      self.header[8],
      self.header[9],
      self.header[10],
      self.header[11],
    ])
  }

  pub fn height(&self) -> u32 {
    u32::from_be_bytes([
      self.header[12],
      self.header[13],
      self.header[14],
      self.header[15],
    ])
  }

  pub fn bit_depth(&self) -> u8 {
    self.header[16]
  }

  pub fn color_type(&self) -> ColorType {
    ColorType::new(self.header[17])
  }

  pub fn scan_lines_mut(&mut self) -> std::slice::ChunksMut<u8> {
    let bytes = self.bytes_per_scan_line();
    if bytes == 0 {
      let size = self.data.len();
      self.data.chunks_mut(size)
    } else {
      self.data.chunks_mut(bytes + 1)
    }
  }

  fn bytes_per_scan_line(&self) -> usize {
    let unit = match self.color_type() {
      ColorType::Grayscale => 1,
      ColorType::RGB => 3,
      ColorType::Palette => 0,
      ColorType::GrayscaleAlpha => 2,
      ColorType::RGBA => 4,
      _ => 0,
    };
    (self.bit_depth() as u32 * unit * self.width() / 8) as usize
  }
}
