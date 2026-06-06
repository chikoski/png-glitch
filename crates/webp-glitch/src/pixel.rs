use crate::scan_line::ScanLine;

/// 1ピクセルの色情報。WebP はネイティブで u8 チャンネルを使用する。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WebpPixel {
    RGB(u8, u8, u8),
    RGBA(u8, u8, u8, u8),
}

impl WebpPixel {
    pub fn r(self) -> u8 {
        match self {
            WebpPixel::RGB(r, _, _) | WebpPixel::RGBA(r, _, _, _) => r,
        }
    }

    pub fn g(self) -> u8 {
        match self {
            WebpPixel::RGB(_, g, _) | WebpPixel::RGBA(_, g, _, _) => g,
        }
    }

    pub fn b(self) -> u8 {
        match self {
            WebpPixel::RGB(_, _, b) | WebpPixel::RGBA(_, _, b, _) => b,
        }
    }

    pub fn a(self) -> u8 {
        match self {
            WebpPixel::RGB(_, _, _) => 255,
            WebpPixel::RGBA(_, _, _, a) => a,
        }
    }

    pub fn channels(self) -> usize {
        match self {
            WebpPixel::RGB(_, _, _) => 3,
            WebpPixel::RGBA(_, _, _, _) => 4,
        }
    }
}

/// デコード済みピクセルデータを行優先 RGBA/RGB バイト列として保持する。
pub struct PixelBuffer {
    pub width: u32,
    pub height: u32,
    pub has_alpha: bool,
    /// 行優先、各ピクセルは RGBA または RGB
    pub data: Vec<u8>,
}

impl PixelBuffer {
    pub fn new(width: u32, height: u32, has_alpha: bool, data: Vec<u8>) -> Self {
        Self { width, height, has_alpha, data }
    }

    pub fn channels(&self) -> usize {
        if self.has_alpha { 4 } else { 3 }
    }

    pub fn stride(&self) -> usize {
        self.width as usize * self.channels()
    }

    pub fn scan_lines(&mut self) -> Vec<ScanLine<'_>> {
        let stride = self.stride();
        let width = self.width;
        let has_alpha = self.has_alpha;
        self.data
            .chunks_mut(stride)
            .map(|row| ScanLine::new(row, width, has_alpha))
            .collect()
    }

    pub fn scan_line_mut(&mut self, y: u32) -> Option<ScanLine<'_>> {
        if y >= self.height {
            return None;
        }
        let stride = self.stride();
        let width = self.width;
        let has_alpha = self.has_alpha;
        let start = y as usize * stride;
        let end = start + stride;
        Some(ScanLine::new(&mut self.data[start..end], width, has_alpha))
    }
}
