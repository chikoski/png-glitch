use super::chunk::chunk_type;
use super::crc;
use super::PNG;

use std::io::{Read, Write};

use flate2::read::ZlibEncoder;

pub const IEND: [u8; 12] = [
  0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4e, 0x44, 0xae, 0x42, 0x60, 0x82,
];

const SEED_IDAT: u32 = 900662814;

pub fn serialize(png: &PNG, dest: &mut dyn Write) -> std::io::Result<()> {
  dest.write(&PNG::MAGIC)?;
  dest.write(&png.header)?;
  serialize_data(png, dest)?;
  dest.write(&IEND)?;
  Ok(())
}

fn serialize_data(png: &PNG, dest: &mut dyn Write) -> std::io::Result<()> {
  let mut encoder = ZlibEncoder::new(&png.data[..], flate2::Compression::best());
  let mut deflated: Vec<u8> = Vec::new();
  encoder.read_to_end(&mut deflated)?;

  let idat_crc = crc::update(SEED_IDAT, &deflated);
  dest.write(&(deflated.len() as u32).to_be_bytes())?;
  dest.write(&chunk_type::IDAT)?;
  dest.write(&deflated)?;
  dest.write(&idat_crc.to_be_bytes())?;
  Ok(())
}
