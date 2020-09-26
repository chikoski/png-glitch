use super::chunk;
use super::chunk::chunk_type;
use super::ErrorKind;
use super::PNG;

use flate2::read::ZlibDecoder;
use log::debug;
use std::io::{Read, Write};

fn decode(src: &[u8], dest: &mut Vec<u8>) -> std::io::Result<()> {
  let mut decoder = ZlibDecoder::new(src);
  decoder.read_to_end(dest)?;
  Ok(())
}

fn do_parse(buffer: &Vec<u8>) -> Result<PNG, ErrorKind> {
  let mut idat: Vec<u8> = Vec::new();
  let offset = PNG::MAGIC.len();
  let ihdr = &buffer[offset..offset + 25];
  if chunk::parse_chunk_type(ihdr) != chunk_type::IHDR {
    return Err(ErrorKind::ParseError);
  }

  let mut buffer = &buffer[offset + 25..];
  while buffer.len() > 0 {
    let length = chunk::parse_length(buffer);
    let chunk_type = chunk::parse_chunk_type(buffer);
    match chunk_type {
      chunk_type::PLTE => (),
      chunk_type::IDAT => match idat.write(chunk::parse_data(buffer)) {
        Err(_) => return Err(ErrorKind::IOError),
        _ => (),
      },
      chunk_type::IEND => break,
      _ => {
        let chunk_type_in_string = std::str::from_utf8(&chunk_type).unwrap_or("unknown");
        debug!("{}", chunk_type_in_string);
        ()
      }
    }
    buffer = &buffer[length + 12..];
  }
  let mut header: [u8; 25] = [0; 25];
  let mut data: Vec<u8> = Vec::new();

  header.copy_from_slice(&ihdr);
  decode(&idat, &mut data).map_err(|_| ErrorKind::IOError)?;
  Ok(PNG {
    header: header,
    data: data,
  })
}

pub fn parse(buffer: &Vec<u8>) -> Result<PNG, ErrorKind> {
  if buffer[..PNG::MAGIC.len()] != PNG::MAGIC {
    Err(ErrorKind::ParseError)
  } else {
    do_parse(buffer)
  }
}
