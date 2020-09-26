pub fn parse_length(buffer: &[u8]) -> usize {
  u32::from_be_bytes([buffer[0], buffer[1], buffer[2], buffer[3]]) as usize
}

pub fn parse_chunk_type(buffer: &[u8]) -> [u8; 4] {
  [buffer[4], buffer[5], buffer[6], buffer[7]]
}

pub fn parse_data(buffer: &[u8]) -> &[u8] {
  let size = parse_length(buffer);
  &buffer[8..(8 + size)]
}

pub mod chunk_type {
  pub const IHDR: [u8; 4] = [0x49, 0x48, 0x44, 0x52];
  pub const PLTE: [u8; 4] = [0x50, 0x4C, 0x54, 0x45];
  pub const IDAT: [u8; 4] = [0x49, 0x44, 0x41, 0x54];
  pub const IEND: [u8; 4] = [0x49, 0x45, 0x4E, 0x44];
}
