use crate::pixel::WebpPixel;

/// 1行分のピクセルデータへの可変参照。
pub struct ScanLine<'a> {
    data: &'a mut [u8],
    width: u32,
    has_alpha: bool,
}

impl<'a> ScanLine<'a> {
    pub(crate) fn new(data: &'a mut [u8], width: u32, has_alpha: bool) -> Self {
        Self { data, width, has_alpha }
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

    pub fn get_pixel(&self, x: usize) -> Option<WebpPixel> {
        if x >= self.width as usize {
            return None;
        }
        let off = x * self.channels();
        if self.has_alpha {
            Some(WebpPixel::RGBA(
                self.data[off],
                self.data[off + 1],
                self.data[off + 2],
                self.data[off + 3],
            ))
        } else {
            Some(WebpPixel::RGB(
                self.data[off],
                self.data[off + 1],
                self.data[off + 2],
            ))
        }
    }

    pub fn set_pixel(&mut self, x: usize, pixel: WebpPixel) {
        if x >= self.width as usize {
            return;
        }
        let off = x * self.channels();
        if self.has_alpha {
            self.data[off] = pixel.r();
            self.data[off + 1] = pixel.g();
            self.data[off + 2] = pixel.b();
            self.data[off + 3] = pixel.a();
        } else {
            self.data[off] = pixel.r();
            self.data[off + 1] = pixel.g();
            self.data[off + 2] = pixel.b();
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
        for x in 0..width {
            let off = x * ch;
            let pixel = if self.has_alpha {
                WebpPixel::RGBA(
                    self.data[off],
                    self.data[off + 1],
                    self.data[off + 2],
                    self.data[off + 3],
                )
            } else {
                WebpPixel::RGB(self.data[off], self.data[off + 1], self.data[off + 2])
            };
            let out = f(x, pixel);
            self.data[off] = out.r();
            self.data[off + 1] = out.g();
            self.data[off + 2] = out.b();
            if self.has_alpha {
                self.data[off + 3] = out.a();
            }
        }
    }
}
