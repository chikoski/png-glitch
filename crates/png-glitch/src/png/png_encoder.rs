use std::io::Write;

use flate2::{Compression, Crc};
use flate2::write::ZlibEncoder;

use crate::png::chunk::{Chunk, ChunkType};
use crate::png::Png;
use crate::png::png_parser::png_header::PngHeader;
use crate::png::png_parser::SIGNATURE;

pub trait PngEncoder {
    fn encode(&self, writer: impl Write) -> anyhow::Result<impl Write>;
}

impl PngEncoder for ChunkType {
    fn encode(&self, mut writer: impl Write) -> anyhow::Result<impl Write> {
        match self {
            Self::Start => writer.write_all(&ChunkType::IHDR),
            Self::End => writer.write_all(&ChunkType::IEND),
            Self::Data => writer.write_all(&ChunkType::IDAT),
            Self::Other(t) => writer.write_all(t)
        }?;
        Ok(writer)
    }
}

impl PngEncoder for Chunk {
    fn encode(&self, mut writer: impl Write) -> anyhow::Result<impl Write> {
        writer.write_all(&(self.length() as u32).to_be_bytes())?;
        let mut writer = self.chunk_type.encode(writer)?;
        writer.write_all(&self.data)?;
        writer.write_all(&self.crc)?;
        Ok(writer)
    }
}

impl PngEncoder for PngHeader {
    fn encode(&self, writer: impl Write) -> anyhow::Result<impl Write> {
        self.inner.encode(writer)
    }
}

impl PngEncoder for Png {
    fn encode(&self, mut writer: impl Write) -> anyhow::Result<impl Write> {
        writer.write_all(&SIGNATURE)?;
        self.header.inner.encode(&mut writer)?;
        for chunk in self.misc_chunks.iter() {
            chunk.encode(&mut writer)?;
        }
        create_idat_chunk(self)?.encode(&mut writer)?;
        self.terminator.inner.encode(&mut writer)?;
        Ok(writer)
    }
}

fn create_idat_chunk(png: &Png) -> anyhow::Result<Chunk> {
    let mut encoder = ZlibEncoder::new(vec![], Compression::fast());
    encoder.write_all(&png.data)?;

    let mut crc = Crc::new();
    crc.update(encoder.get_ref());

    let chunk = Chunk::new(ChunkType::Data, encoder.get_ref().to_vec(), crc.amount().to_be_bytes());
    Ok(chunk)
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_encode_ihdr() -> anyhow::Result<()>{
        let bytes = include_bytes!("../../etc/sample00.png");
        let png = Png::parse(bytes)?;
        let mut buffer = vec![];
        png.header.encode(&mut buffer)?;
        assert_eq!(&buffer[0..4], &(png.header.inner.length() as u32).to_be_bytes());
        assert_eq!(&buffer[4..8], ChunkType::IHDR);
        assert_eq!(&buffer[8..21], &png.header.inner.data);
        assert_eq!(&buffer[21..25], &png.header.inner.crc);
        Ok(())
    }
}