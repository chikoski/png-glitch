//! WebP glitch context: queue, seed and execute glitch filters on a [`WebpGlitch`] image.
//!
//! This crate is the WebP counterpart of [`png_glitch_recipe::GlitchContext`]. It was split out of
//! `png-glitch-recipe` so that the PNG recipe crate carries no dependency on `webp-glitch` or
//! `libwebp-sys2`. It reuses the shared configuration types ([`FilterConfig`], [`SortCriterion`],
//! [`BitOp`], [`SwapTarget`], [`AlphaGlitchStrategy`], [`PixelFilter`]) from `png-glitch-recipe`.
//!
//! The effects that exist for both formats ([`PixelSort`], [`Bitwise`], ...) are defined in
//! `png-glitch-recipe`. Because of Rust's orphan rule they are applied to WebP through the local
//! wrapper [`ForWebp`], e.g. `ctx.add_filter(ForWebp(PixelSort::new(0.5)))`.
#![warn(missing_docs)]
#![forbid(unsafe_code)]
#![cfg_attr(
    not(test),
    deny(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::indexing_slicing
    )
)]

use anyhow::{Context, bail};
use png_glitch_recipe::rand::{Rng, RngExt, SeedableRng};
pub use png_glitch_recipe::{
    AlphaGlitchStrategy, BitOp, Bitwise, Brighten, ChannelSwap, ChromaticAberration,
    ColorDistortion, Darken, FilterConfig, HorizontalShift, Invert, PixelFilter,
    PixelFilterAdapter, PixelSort, ShiftChannels, SortCriterion, SwapTarget,
};
pub use png_glitch_recipe::{ChaCha8Rng, rand, rand_chacha};
pub use png_glitch_recipe::{FilterInfo, Format, ParamInfo, ParamKind, filter_info, filters};
pub use webp_glitch::{WebpError, WebpGlitch, WebpGlitchFilter, WebpPixel};

fn clamp_magnitude(m: f64) -> f64 {
    if m.is_nan() { 0.0 } else { m.clamp(0.0, 1.0) }
}

/// Applies a `png-glitch-recipe` effect (or a [`PixelFilterAdapter`]) to WebP images.
///
/// `WebpGlitchFilter` and the effect types both live in other crates, so the orphan rule forbids
/// implementing one for the other here. This local wrapper carries the WebP implementation.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct ForWebp<F>(pub F);

/// Glitch context for WebP images.
pub struct WebpGlitchContext {
    webp: WebpGlitch,
    filters: Vec<Box<dyn WebpGlitchFilter>>,
    rng: ChaCha8Rng,
}

impl WebpGlitchContext {
    /// Creates a new [`WebpGlitchContext`] from a file path.
    pub fn open(path: impl AsRef<std::path::Path>, seed: Option<u64>) -> anyhow::Result<Self> {
        let path = path.as_ref();
        let webp = WebpGlitch::open(path)
            .with_context(|| format!("failed to open '{}'", path.display()))?;
        let rng = match seed {
            Some(s) => ChaCha8Rng::seed_from_u64(s),
            None => rand::make_rng(),
        };
        Ok(Self {
            webp,
            filters: Vec::new(),
            rng,
        })
    }

    /// Creates a new [`WebpGlitchContext`] from a byte slice.
    pub fn new(data: &[u8], seed: Option<u64>) -> anyhow::Result<Self> {
        let webp = WebpGlitch::new(data.to_vec())?;
        let rng = match seed {
            Some(s) => ChaCha8Rng::seed_from_u64(s),
            None => rand::make_rng(),
        };
        Ok(Self {
            webp,
            filters: Vec::new(),
            rng,
        })
    }

    /// Resets the internal RNG with the given `seed` (or a fresh OS seed if `None`).
    pub fn set_seed(&mut self, seed: Option<u64>) {
        self.rng = match seed {
            Some(s) => ChaCha8Rng::seed_from_u64(s),
            None => rand::make_rng(),
        };
    }

    /// Adds a filter to the execution queue.
    pub fn add_filter(&mut self, filter: impl WebpGlitchFilter + 'static) {
        self.filters.push(Box::new(filter));
    }

    /// Clears all queued filters without executing them.
    pub fn clear_filters(&mut self) {
        self.filters.clear();
    }

    /// Executes all queued filters on the image in registration order and drains the filter queue.
    ///
    /// Calling `execute()` a second time without adding new filters is a no-op.
    pub fn execute(&mut self) {
        for filter in self.filters.drain(..) {
            filter.apply(&mut self.webp, &mut self.rng);
        }
    }

    /// Returns the encoded glitched WebP image as a byte vector.
    pub fn buffer(&self) -> anyhow::Result<Vec<u8>> {
        Ok(self.webp.encode()?)
    }

    /// Saves the glitched WebP image to a file.
    pub fn save(&self, path: impl AsRef<std::path::Path>) -> anyhow::Result<()> {
        let path = path.as_ref();
        let data = self.buffer()?;
        std::fs::write(path, data)
            .with_context(|| format!("failed to save '{}'", path.display()))?;
        Ok(())
    }

    /// Returns the width of the image in pixels.
    pub fn width(&self) -> u32 {
        self.webp.width()
    }

    /// Returns the height of the image in pixels.
    pub fn height(&self) -> u32 {
        self.webp.height()
    }

    /// Adds a filter from a [`FilterConfig`] definition.
    ///
    /// Returns an error if the filter configuration is not supported for WebP images
    /// (for example, PNG scanline filter manipulations).
    pub fn add_from_config(&mut self, config: FilterConfig) -> anyhow::Result<()> {
        match config {
            FilterConfig::Invert => self.add_filter(ForWebp(PixelFilterAdapter(Invert))),
            FilterConfig::Brighten { strength } => {
                self.add_filter(ForWebp(PixelFilterAdapter(Brighten::new(strength))));
            }
            FilterConfig::Darken { strength } => {
                self.add_filter(ForWebp(PixelFilterAdapter(Darken::new(strength))));
            }
            FilterConfig::ShiftChannels { r, g, b } => {
                self.add_filter(ForWebp(PixelFilterAdapter(ShiftChannels::new(r, g, b))));
            }
            FilterConfig::PixelSort {
                magnitude,
                criterion,
            } => {
                let mut f = PixelSort::new(magnitude);
                if let Some(c) = criterion {
                    f.criterion = c;
                }
                self.add_filter(ForWebp(f));
            }
            FilterConfig::Bitwise {
                magnitude,
                op,
                value,
            } => {
                let mut f = Bitwise::new(magnitude);
                if let Some(o) = op {
                    f.op = o;
                }
                if let Some(v) = value {
                    f.value = v;
                }
                self.add_filter(ForWebp(f));
            }
            FilterConfig::ChannelSwap { magnitude, target } => {
                let mut f = ChannelSwap::new(magnitude);
                if let Some(t) = target {
                    f.target = t;
                }
                self.add_filter(ForWebp(f));
            }
            FilterConfig::HorizontalShift { magnitude } => {
                self.add_filter(ForWebp(HorizontalShift::new(magnitude)));
            }
            FilterConfig::ColorDistortion {
                magnitude,
                strength,
            } => {
                let mut f = ColorDistortion::new(magnitude);
                if let Some(s) = strength {
                    if s < 0 {
                        bail!("color-distortion `strength` must be >= 0, got {s}");
                    }
                    f.strength = s;
                }
                self.add_filter(ForWebp(f));
            }
            FilterConfig::ChromaticAberration {
                magnitude,
                r_offset,
                g_offset,
                b_offset,
            } => {
                let mut f = ChromaticAberration::new(magnitude);
                if let Some(r) = r_offset {
                    f.r_offset = r;
                }
                if let Some(g) = g_offset {
                    f.g_offset = g;
                }
                if let Some(b) = b_offset {
                    f.b_offset = b;
                }
                self.add_filter(ForWebp(f));
            }
            FilterConfig::MacroblockGlitch { magnitude } => {
                self.add_filter(MacroblockGlitch::new(magnitude));
            }
            FilterConfig::AlphaGlitch {
                magnitude,
                strategy,
            } => {
                let mut f = AlphaGlitch::new(magnitude);
                if let Some(s) = strategy {
                    f.strategy = s;
                }
                self.add_filter(f);
            }
            FilterConfig::LossyArtifact { quality } => {
                self.add_filter(LossyArtifact::new(quality));
            }
            other => {
                bail!(
                    "Filter `{:?}` is a PNG-only filter and is not supported for WebP images",
                    other
                );
            }
        }
        Ok(())
    }
}

// PixelFilterAdapter: applies a PixelFilter to WebP images

impl<F: PixelFilter> WebpGlitchFilter for ForWebp<PixelFilterAdapter<F>> {
    fn apply(&self, webp: &mut WebpGlitch, _rng: &mut dyn Rng) {
        webp.foreach_scanline(|line| {
            line.process_pixels(|_, p| match p {
                WebpPixel::RGB(r, g, b) => {
                    let (nr, ng, nb, _) = self.0.0.apply_pixel(r, g, b, 255);
                    WebpPixel::RGB(nr, ng, nb)
                }
                WebpPixel::RGBA(r, g, b, a) => {
                    let (nr, ng, nb, na) = self.0.0.apply_pixel(r, g, b, a);
                    WebpPixel::RGBA(nr, ng, nb, na)
                }
            });
        });
    }
}

/// WebP version of the [`Invert`] filter.
#[deprecated(note = "Use `PixelFilterAdapter(Invert)` instead, which works on both PNG and WebP.")]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct WebpInvert;

#[allow(deprecated)]
impl WebpGlitchFilter for WebpInvert {
    fn apply(&self, webp: &mut WebpGlitch, _rng: &mut dyn Rng) {
        webp.foreach_scanline(|line| {
            line.process_pixels(|_, p| match p {
                WebpPixel::RGB(r, g, b) => WebpPixel::RGB(255 - r, 255 - g, 255 - b),
                WebpPixel::RGBA(r, g, b, a) => WebpPixel::RGBA(255 - r, 255 - g, 255 - b, a),
            });
        });
    }
}

/// WebP version of the [`Brighten`] filter.
#[deprecated(
    note = "Use `PixelFilterAdapter(Brighten { strength })` instead, which works on both PNG and WebP."
)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct WebpBrighten {
    /// Brightness increment added to each channel (saturating at `255`).
    pub strength: u8,
}

#[allow(deprecated)]
impl WebpGlitchFilter for WebpBrighten {
    fn apply(&self, webp: &mut WebpGlitch, _rng: &mut dyn Rng) {
        let s = self.strength;
        webp.foreach_scanline(|line| {
            line.process_pixels(|_, p| match p {
                WebpPixel::RGB(r, g, b) => WebpPixel::RGB(
                    r.saturating_add(s),
                    g.saturating_add(s),
                    b.saturating_add(s),
                ),
                WebpPixel::RGBA(r, g, b, a) => WebpPixel::RGBA(
                    r.saturating_add(s),
                    g.saturating_add(s),
                    b.saturating_add(s),
                    a,
                ),
            });
        });
    }
}

/// WebP version of the [`ShiftChannels`] filter.
#[deprecated(
    note = "Use `PixelFilterAdapter(ShiftChannels { r, g, b })` instead, which works on both PNG and WebP."
)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct WebpShiftChannels {
    /// Signed offset added to the red channel.
    pub r: i16,
    /// Signed offset added to the green channel.
    pub g: i16,
    /// Signed offset added to the blue channel.
    pub b: i16,
}

#[allow(deprecated)]
impl WebpGlitchFilter for WebpShiftChannels {
    fn apply(&self, webp: &mut WebpGlitch, _rng: &mut dyn Rng) {
        let r_off = self.r;
        let g_off = self.g;
        let b_off = self.b;
        webp.foreach_scanline(|line| {
            line.process_pixels(|_, p| {
                let shift = |v: u8, off: i16| (v as i16 + off).clamp(0, 255) as u8;
                match p {
                    WebpPixel::RGB(r, g, b) => {
                        WebpPixel::RGB(shift(r, r_off), shift(g, g_off), shift(b, b_off))
                    }
                    WebpPixel::RGBA(r, g, b, a) => {
                        WebpPixel::RGBA(shift(r, r_off), shift(g, g_off), shift(b, b_off), a)
                    }
                }
            });
        });
    }
}

impl WebpGlitchFilter for ForWebp<PixelSort> {
    fn apply(&self, webp: &mut WebpGlitch, rng: &mut dyn Rng) {
        let this = &self.0;
        let magnitude = clamp_magnitude(this.magnitude);
        webp.foreach_scanline(|line| {
            if rng.random_bool(magnitude) {
                let count = line.pixels_count() as usize;
                let mut pixels: Vec<WebpPixel> =
                    (0..count).filter_map(|x| line.get_pixel(x)).collect();
                pixels.sort_by(|a, b| {
                    let val = |p: &WebpPixel| match this.criterion {
                        SortCriterion::Brightness => {
                            (p.r() as f64 + p.g() as f64 + p.b() as f64) / 3.0
                        }
                        SortCriterion::Hue => {
                            let r = p.r() as f64 / 255.0;
                            let g = p.g() as f64 / 255.0;
                            let b = p.b() as f64 / 255.0;
                            let max = r.max(g).max(b);
                            let min = r.min(g).min(b);
                            if max == min {
                                0.0
                            } else if max == r {
                                (60.0 * ((g - b) / (max - min)) + 360.0) % 360.0
                            } else if max == g {
                                60.0 * ((b - r) / (max - min)) + 120.0
                            } else {
                                60.0 * ((r - g) / (max - min)) + 240.0
                            }
                        }
                    };
                    val(a)
                        .partial_cmp(&val(b))
                        .unwrap_or(std::cmp::Ordering::Equal)
                });
                for (x, p) in pixels.into_iter().enumerate() {
                    line.set_pixel(x, p);
                }
            }
        });
    }
}

impl WebpGlitchFilter for ForWebp<Bitwise> {
    fn apply(&self, webp: &mut WebpGlitch, rng: &mut dyn Rng) {
        let this = &self.0;
        let magnitude = clamp_magnitude(this.magnitude);
        webp.foreach_scanline(|line| {
            if rng.random_bool(magnitude) {
                for i in 0..line.size() {
                    if let Some(b) = line.index(i) {
                        let new_b = match this.op {
                            BitOp::And => b & this.value,
                            BitOp::Or => b | this.value,
                            BitOp::Xor => b ^ this.value,
                        };
                        line.update(i, new_b);
                    }
                }
            }
        });
    }
}

impl WebpGlitchFilter for ForWebp<ChannelSwap> {
    fn apply(&self, webp: &mut WebpGlitch, rng: &mut dyn Rng) {
        let this = &self.0;
        let magnitude = clamp_magnitude(this.magnitude);
        webp.foreach_scanline(|line| {
            if rng.random_bool(magnitude) {
                line.process_pixels(|_, p| match p {
                    WebpPixel::RGB(r, g, b) => match this.target {
                        SwapTarget::Rg => WebpPixel::RGB(g, r, b),
                        SwapTarget::Gb => WebpPixel::RGB(r, b, g),
                        SwapTarget::Br => WebpPixel::RGB(b, g, r),
                    },
                    WebpPixel::RGBA(r, g, b, a) => match this.target {
                        SwapTarget::Rg => WebpPixel::RGBA(g, r, b, a),
                        SwapTarget::Gb => WebpPixel::RGBA(r, b, g, a),
                        SwapTarget::Br => WebpPixel::RGBA(b, g, r, a),
                    },
                });
            }
        });
    }
}

impl WebpGlitchFilter for ForWebp<HorizontalShift> {
    fn apply(&self, webp: &mut WebpGlitch, rng: &mut dyn Rng) {
        let this = &self.0;
        let magnitude = clamp_magnitude(this.magnitude);
        webp.foreach_scanline(|line| {
            let size = line.size();
            if size <= 1 {
                return;
            }
            if rng.random_bool(magnitude) {
                let shift = rng.random_range(0..size);
                let mut buf: Vec<u8> = (0..size).filter_map(|i| line.index(i)).collect();
                buf.rotate_left(shift);
                for (i, b) in buf.into_iter().enumerate() {
                    line.update(i, b);
                }
            }
        });
    }
}

impl WebpGlitchFilter for ForWebp<ColorDistortion> {
    fn apply(&self, webp: &mut WebpGlitch, rng: &mut dyn Rng) {
        let this = &self.0;
        let strength = this.strength.saturating_abs().min(255);
        let magnitude = clamp_magnitude(this.magnitude);
        webp.foreach_scanline(|line| {
            if rng.random_bool(magnitude) {
                line.process_pixels(|_, p| {
                    let r_off = rng.random_range(-strength..=strength);
                    let g_off = rng.random_range(-strength..=strength);
                    let b_off = rng.random_range(-strength..=strength);
                    let shift = |v: u8, off: i16| (v as i16 + off).clamp(0, 255) as u8;
                    match p {
                        WebpPixel::RGB(r, g, b) => {
                            WebpPixel::RGB(shift(r, r_off), shift(g, g_off), shift(b, b_off))
                        }
                        WebpPixel::RGBA(r, g, b, a) => {
                            WebpPixel::RGBA(shift(r, r_off), shift(g, g_off), shift(b, b_off), a)
                        }
                    }
                });
            }
        });
    }
}

impl WebpGlitchFilter for ForWebp<ChromaticAberration> {
    fn apply(&self, webp: &mut WebpGlitch, rng: &mut dyn Rng) {
        let this = &self.0;
        let width = webp.width() as usize;
        if width == 0 {
            return;
        }
        let magnitude = clamp_magnitude(this.magnitude);
        webp.foreach_scanline(|line| {
            if rng.random_bool(magnitude) {
                let mut r_ch = Vec::with_capacity(width);
                let mut g_ch = Vec::with_capacity(width);
                let mut b_ch = Vec::with_capacity(width);
                let mut a_ch = Vec::with_capacity(width);
                for x in 0..width {
                    if let Some(p) = line.get_pixel(x) {
                        r_ch.push(p.r());
                        g_ch.push(p.g());
                        b_ch.push(p.b());
                        a_ch.push(p.a());
                    }
                }
                let get = |ch: &[u8], off: i32, x: usize| {
                    let nx = (x as i64 + i64::from(off)).rem_euclid(width as i64) as usize;
                    ch.get(nx).copied().unwrap_or(0)
                };
                line.process_pixels(|x, p| {
                    let r = get(&r_ch, this.r_offset, x);
                    let g = get(&g_ch, this.g_offset, x);
                    let b = get(&b_ch, this.b_offset, x);
                    match p {
                        WebpPixel::RGB(_, _, _) => WebpPixel::RGB(r, g, b),
                        WebpPixel::RGBA(_, _, _, _) => {
                            WebpPixel::RGBA(r, g, b, a_ch.get(x).copied().unwrap_or(0))
                        }
                    }
                });
            }
        });
    }
}

// ============================================================
// WebP-specific filters
// ============================================================

/// Filter that swaps 16×16 macroblocks along VP8 block boundaries in WebP images.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MacroblockGlitch {
    /// Fraction (`0.0..=1.0`) of 16×16 macroblocks to swap.
    pub magnitude: f64,
}

impl Default for MacroblockGlitch {
    fn default() -> Self {
        Self { magnitude: 0.5 }
    }
}

impl MacroblockGlitch {
    /// Creates a new [`MacroblockGlitch`] filter with the given magnitude in `0.0..=1.0`.
    pub fn new(magnitude: f64) -> Self {
        Self {
            magnitude: clamp_magnitude(magnitude),
        }
    }
}

impl WebpGlitchFilter for MacroblockGlitch {
    fn apply(&self, webp: &mut WebpGlitch, rng: &mut dyn Rng) {
        const MB: u32 = 16;
        let width = webp.width();
        let height = webp.height();
        let num_x = width / MB;
        let num_y = height / MB;
        if num_x == 0 || num_y == 0 {
            return;
        }
        let total = (num_x * num_y) as usize;
        let swaps = (total as f64 * clamp_magnitude(self.magnitude)) as usize;
        let mut lines = webp.scan_lines();

        for _ in 0..swaps {
            let sx = rng.random_range(0..num_x);
            let sy = rng.random_range(0..num_y);
            let dx = rng.random_range(0..num_x);
            let dy = rng.random_range(0..num_y);
            if sx == dx && sy == dy {
                continue;
            }
            for row_off in 0..MB {
                let src_y = sy * MB + row_off;
                let dst_y = dy * MB + row_off;
                if src_y >= height || dst_y >= height {
                    continue;
                }
                if src_y == dst_y {
                    let Some(line) = lines.get_mut(src_y as usize) else {
                        continue;
                    };
                    for col_off in 0..MB {
                        let src_x = (sx * MB + col_off) as usize;
                        let dst_x = (dx * MB + col_off) as usize;
                        if src_x < width as usize
                            && dst_x < width as usize
                            && let (Some(ps), Some(pd)) =
                                (line.get_pixel(src_x), line.get_pixel(dst_x))
                        {
                            line.set_pixel(src_x, pd);
                            line.set_pixel(dst_x, ps);
                        }
                    }
                } else {
                    let (low, high) = (src_y.min(dst_y) as usize, src_y.max(dst_y) as usize);
                    let (left, right) = lines.split_at_mut(high);
                    let (Some(low_line), Some(high_line)) = (left.get_mut(low), right.first_mut())
                    else {
                        continue;
                    };
                    let (src_line, dst_line) = if src_y < dst_y {
                        (low_line, high_line)
                    } else {
                        (high_line, low_line)
                    };
                    for col_off in 0..MB {
                        let src_x = (sx * MB + col_off) as usize;
                        let dst_x = (dx * MB + col_off) as usize;
                        if src_x < width as usize
                            && dst_x < width as usize
                            && let (Some(ps), Some(pd)) =
                                (src_line.get_pixel(src_x), dst_line.get_pixel(dst_x))
                        {
                            src_line.set_pixel(src_x, pd);
                            dst_line.set_pixel(dst_x, ps);
                        }
                    }
                }
            }
        }
    }
}

/// Filter that selectively corrupts the alpha channel of WebP images.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AlphaGlitch {
    /// Probability (`0.0..=1.0`) of corrupting each scanline's alpha channel.
    pub magnitude: f64,
    /// Strategy used to modify alpha channel values.
    pub strategy: AlphaGlitchStrategy,
}

impl Default for AlphaGlitch {
    fn default() -> Self {
        Self {
            magnitude: 1.0,
            strategy: AlphaGlitchStrategy::Invert,
        }
    }
}

impl AlphaGlitch {
    /// Creates a new [`AlphaGlitch`] filter with the given magnitude and default [`AlphaGlitchStrategy::Invert`].
    pub fn new(magnitude: f64) -> Self {
        Self {
            magnitude: clamp_magnitude(magnitude),
            ..Default::default()
        }
    }

    /// Sets the alpha corruption strategy.
    pub fn with_strategy(mut self, strategy: AlphaGlitchStrategy) -> Self {
        self.strategy = strategy;
        self
    }
}

impl WebpGlitchFilter for AlphaGlitch {
    fn apply(&self, webp: &mut WebpGlitch, rng: &mut dyn Rng) {
        if !webp.has_alpha() {
            return;
        }
        let magnitude = clamp_magnitude(self.magnitude);
        webp.foreach_scanline(|line| {
            if rng.random_bool(magnitude) {
                line.process_pixels(|_, p| match p {
                    WebpPixel::RGBA(r, g, b, a) => {
                        let new_a = match self.strategy {
                            AlphaGlitchStrategy::Invert => 255 - a,
                            AlphaGlitchStrategy::Randomize => rng.random(),
                            AlphaGlitchStrategy::Zero => 0,
                            AlphaGlitchStrategy::One => 255,
                        };
                        WebpPixel::RGBA(r, g, b, new_a)
                    }
                    other => other,
                });
            }
        });
    }
}

/// Filter that lowers WebP re-encoding quality (`0.0..=100.0`) to introduce lossy compression artifacts.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LossyArtifact {
    /// Target WebP re-encoding quality in `0.0..=100.0`.
    pub quality: f32,
}

impl Default for LossyArtifact {
    fn default() -> Self {
        Self { quality: 10.0 }
    }
}

impl LossyArtifact {
    /// Creates a new [`LossyArtifact`] filter with the given quality (`0.0..=100.0`).
    pub fn new(quality: f32) -> Self {
        Self { quality }
    }
}

impl WebpGlitchFilter for LossyArtifact {
    fn apply(&self, webp: &mut WebpGlitch, _rng: &mut dyn Rng) {
        webp.set_quality(self.quality);
    }
}
