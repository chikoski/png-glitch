use crate::pixel::WebpPixel;

/// 1行分のピクセルデータへの可変参照。
pub struct ScanLine<'a> {
    data: &'a mut [u8],
    width: u32,
    has_alpha: bool,
}

impl<'a> ScanLine<'a> {
    pub(crate) fn new(data: &'a mut [u8], width: u32, has_alpha: bool) -> Self {
        Self {
            data,
            width,
            has_alpha,
        }
    }

    pub fn pixels_count(&self) -> u32 {
        self.width
    }

    pub fn size(&self) -> usize {
        self.data.len()
    }

    fn channels(&self) -> usize {
        if self.has_alpha { 4 } else { 3 }
    }

    /// The bytes of pixel `x`, or `None` past the width or past the end of the data.
    fn pixel_bytes(&self, x: usize) -> Option<&[u8]> {
        if x >= self.width as usize {
            return None;
        }
        let ch = self.channels();
        self.data.get(x * ch..(x + 1) * ch)
    }

    /// ピクセル `x`。幅の外、またはデータが足りないときは `None`。
    pub fn get_pixel(&self, x: usize) -> Option<WebpPixel> {
        decode(self.pixel_bytes(x)?)
    }

    /// ピクセル `x` を書き換える。幅の外、またはデータが足りないときは何もしない。
    pub fn set_pixel(&mut self, x: usize, pixel: WebpPixel) {
        if x >= self.width as usize {
            return;
        }
        let ch = self.channels();
        if let Some(bytes) = self.data.get_mut(x * ch..(x + 1) * ch) {
            encode(bytes, pixel);
        }
    }

    pub fn index(&self, i: usize) -> Option<u8> {
        self.data.get(i).copied()
    }

    pub fn update(&mut self, i: usize, value: u8) {
        if let Some(b) = self.data.get_mut(i) {
            *b = value;
        }
    }

    /// 全ピクセルにクロージャを適用して書き戻す。
    pub fn process_pixels<F>(&mut self, mut f: F)
    where
        F: FnMut(usize, WebpPixel) -> WebpPixel,
    {
        let width = self.width as usize;
        let ch = self.channels();
        // A row shorter than `width` pixels stops at its last whole pixel.
        for (x, bytes) in self.data.chunks_exact_mut(ch).take(width).enumerate() {
            if let Some(pixel) = decode(bytes) {
                encode(bytes, f(x, pixel));
            }
        }
    }
}

/// A pixel from its 3 (RGB) or 4 (RGBA) bytes.
fn decode(bytes: &[u8]) -> Option<WebpPixel> {
    match *bytes {
        [r, g, b] => Some(WebpPixel::RGB(r, g, b)),
        [r, g, b, a] => Some(WebpPixel::RGBA(r, g, b, a)),
        _ => None,
    }
}

/// Writes `pixel` into 3 (RGB: alpha is dropped) or 4 (RGBA) bytes.
fn encode(bytes: &mut [u8], pixel: WebpPixel) {
    match bytes {
        [r, g, b] => (*r, *g, *b) = (pixel.r(), pixel.g(), pixel.b()),
        [r, g, b, a] => (*r, *g, *b, *a) = (pixel.r(), pixel.g(), pixel.b(), pixel.a()),
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Rows shorter or longer than `width` pixels, and indices past the end (#140): nothing
    /// panics, and a pixel exists exactly where its bytes do.
    #[test]
    fn out_of_range_and_short_rows_do_not_panic() {
        for has_alpha in [false, true] {
            let ch = if has_alpha { 4 } else { 3 };
            for width in 0..6u32 {
                for len in 0..30usize {
                    let mut data: Vec<u8> = (0..len as u8).collect();
                    let mut line = ScanLine::new(&mut data, width, has_alpha);
                    for x in 0..10 {
                        let fits = x < width as usize && (x + 1) * ch <= len;
                        assert_eq!(line.get_pixel(x).is_some(), fits, "{width} {len} {x}");
                        line.set_pixel(x, WebpPixel::RGBA(1, 2, 3, 4));
                        let _ = line.index(x);
                        line.update(x, 9);
                    }
                    let before = line.data.to_vec();
                    line.process_pixels(|_, p| p);
                    assert_eq!(line.data, &before[..]);
                    line.process_pixels(|_, _| WebpPixel::RGB(5, 6, 7));
                }
            }
        }
    }
}
