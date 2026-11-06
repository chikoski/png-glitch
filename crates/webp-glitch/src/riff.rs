use crate::error::WebpError;
use rand::Rng;

const RIFF_SIGNATURE: &[u8; 4] = b"RIFF";
const WEBP_FORM_TYPE: &[u8; 4] = b"WEBP";

#[derive(Debug, Clone)]
pub struct RiffChunk {
    pub id: [u8; 4],
    pub data: Vec<u8>,
    /// サイズフィールドを意図的にずらす場合の補正値（グリッチ用）
    pub size_delta: i32,
}

impl RiffChunk {
    pub fn new(id: [u8; 4], data: Vec<u8>) -> Self {
        Self {
            id,
            data,
            size_delta: 0,
        }
    }

    pub fn id_str(&self) -> &str {
        std::str::from_utf8(&self.id).unwrap_or("????")
    }
}

#[derive(Debug, Clone)]
pub struct RiffContainer {
    pub chunks: Vec<RiffChunk>,
}

impl RiffContainer {
    pub fn parse(data: &[u8]) -> Result<Self, WebpError> {
        if data.len() < 12 {
            return Err(WebpError::InvalidSignature);
        }
        if data.get(0..4) != Some(RIFF_SIGNATURE.as_slice()) {
            return Err(WebpError::InvalidSignature);
        }
        if data.get(8..12) != Some(WEBP_FORM_TYPE.as_slice()) {
            return Err(WebpError::InvalidFormType);
        }

        let mut chunks = Vec::new();
        let mut pos = 12usize;

        while let Some(&[i0, i1, i2, i3, s0, s1, s2, s3]) =
            data.get(pos..).and_then(|rest| rest.first_chunk::<8>())
        {
            let id = [i0, i1, i2, i3];
            let size = u32::from_le_bytes([s0, s1, s2, s3]) as usize;
            pos += 8;

            let end = pos.saturating_add(size).min(data.len());
            let chunk_data = data.get(pos..end).unwrap_or_default().to_vec();
            chunks.push(RiffChunk::new(id, chunk_data));

            pos = pos.saturating_add(size);
            // パディング（奇数バイト時）
            if !size.is_multiple_of(2) {
                pos = pos.saturating_add(1);
            }
        }

        Ok(Self { chunks })
    }

    pub fn encode(&self) -> Vec<u8> {
        let mut payload: Vec<u8> = Vec::new();
        payload.extend_from_slice(WEBP_FORM_TYPE);

        for chunk in &self.chunks {
            payload.extend_from_slice(&chunk.id);
            let reported_size = (chunk.data.len() as i64 + chunk.size_delta as i64).max(0) as u32;
            payload.extend_from_slice(&reported_size.to_le_bytes());
            payload.extend_from_slice(&chunk.data);
            if chunk.data.len() % 2 != 0 {
                payload.push(0x00);
            }
        }

        let file_size = payload.len() as u32;
        let mut out = Vec::with_capacity(8 + payload.len());
        out.extend_from_slice(RIFF_SIGNATURE);
        out.extend_from_slice(&file_size.to_le_bytes());
        out.extend_from_slice(&payload);
        out
    }

    pub fn find_chunk(&self, id: &[u8; 4]) -> Option<&RiffChunk> {
        self.chunks.iter().find(|c| &c.id == id)
    }

    pub fn find_chunk_mut(&mut self, id: &[u8; 4]) -> Option<&mut RiffChunk> {
        self.chunks.iter_mut().find(|c| &c.id == id)
    }

    pub fn swap_chunks(&mut self, id_a: &[u8; 4], id_b: &[u8; 4]) -> Result<(), WebpError> {
        let pos_a = self
            .chunks
            .iter()
            .position(|c| &c.id == id_a)
            .ok_or(WebpError::ChunkNotFound(*id_a))?;
        let pos_b = self
            .chunks
            .iter()
            .position(|c| &c.id == id_b)
            .ok_or(WebpError::ChunkNotFound(*id_b))?;
        if pos_a != pos_b {
            let (low, high) = (pos_a.min(pos_b), pos_a.max(pos_b));
            let (left, right) = self.chunks.split_at_mut(high);
            if let (Some(a), Some(b)) = (left.get_mut(low), right.first_mut()) {
                std::mem::swap(&mut a.data, &mut b.data);
            }
        }
        Ok(())
    }

    pub fn corrupt_chunk(&mut self, id: &[u8; 4], magnitude: f64, rng: &mut impl Rng) {
        if let Some(chunk) = self.find_chunk_mut(id) {
            for byte in chunk.data.iter_mut() {
                if rng.random_bool(magnitude) {
                    *byte = rng.random();
                }
            }
        }
    }

    pub fn tamper_chunk_size(&mut self, id: &[u8; 4], delta: i32) -> Result<(), WebpError> {
        let chunk = self
            .find_chunk_mut(id)
            .ok_or(WebpError::ChunkNotFound(*id))?;
        chunk.size_delta = delta;
        Ok(())
    }

    pub fn remove_chunk(&mut self, id: &[u8; 4]) {
        self.chunks.retain(|c| &c.id != id);
    }

    pub fn duplicate_chunk(&mut self, id: &[u8; 4]) -> Result<(), WebpError> {
        let chunk = self
            .find_chunk(id)
            .ok_or(WebpError::ChunkNotFound(*id))?
            .clone();
        self.chunks.push(chunk);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncated_and_corrupt_riff_never_panics() {
        let sample = include_bytes!("../etc/sample00.webp");
        for len in 0..sample.len().min(512) {
            let _ = RiffContainer::parse(&sample[..len]);
        }
        // A chunk that claims to be larger than the file (and larger than usize on 32-bit).
        let mut data = b"RIFF\x00\x00\x00\x00WEBPVP8 \xff\xff\xff\xff".to_vec();
        data.extend_from_slice(&[0; 8]);
        let parsed = RiffContainer::parse(&data).expect("header is valid");
        assert_eq!(parsed.chunks.len(), 1);
    }
}
