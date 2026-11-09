//! [`GlitchContext`]: an image, a queue of filters and an RNG.

use crate::*;

/// A trait for glitch filters (effects) operating on PNG images.
pub trait GlitchFilter {
    /// Applies the filter to the [`PngGlitch`] context using the provided RNG.
    fn apply(&self, png: &mut PngGlitch, rng: &mut dyn RngCore);
}

/// A pixel-level filter that operates on normalized RGBA `u8` values.
///
/// Implement this trait to create a filter that works on pixel values independent of the image
/// format. Use [`PixelFilterAdapter`] to apply it to a PNG image; the `webp-glitch-context`
/// crate applies the same adapter to WebP images.
pub trait PixelFilter: Send + Sync {
    /// Transforms a single RGBA pixel `(r, g, b, a)` in `0..=255` range.
    fn apply_pixel(&self, r: u8, g: u8, b: u8, a: u8) -> (u8, u8, u8, u8);
}

/// Wraps a [`PixelFilter`] so it can be used as a [`GlitchFilter`] (PNG).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PixelFilterAdapter<F: PixelFilter>(pub F);

/// A struct that manages the glitch context for PNG images.
pub struct GlitchContext {
    png: PngGlitch,
    filters: Vec<Box<dyn GlitchFilter>>,
    rng: ChaCha8Rng,
}

/// Seeds an RNG, drawing from the OS when `seed` is `None` (needs the `os-rng` feature).
pub(crate) fn rng_from_seed(seed: Option<u64>) -> Result<ChaCha8Rng> {
    match seed {
        Some(s) => Ok(ChaCha8Rng::seed_from_u64(s)),
        #[cfg(feature = "os-rng")]
        None => Ok(ChaCha8Rng::from_os_rng()),
        #[cfg(not(feature = "os-rng"))]
        None => Err(RecipeError::SeedRequired),
    }
}

impl GlitchContext {
    /// Creates a new [`GlitchContext`] from a file path.
    pub fn open(path: impl AsRef<Path>, seed: Option<u64>) -> Result<Self> {
        Self::open_with_options(path, seed, ParseOptions::default())
    }

    /// Creates a new [`GlitchContext`] from a file path using custom PNG [`ParseOptions`].
    pub fn open_with_options(
        path: impl AsRef<Path>,
        seed: Option<u64>,
        options: ParseOptions,
    ) -> Result<Self> {
        let path = path.as_ref();
        let png =
            PngGlitch::open_with_options(path, options).map_err(|source| RecipeError::Open {
                path: path.to_path_buf(),
                source,
            })?;
        let rng = rng_from_seed(seed)?;
        Ok(Self {
            png,
            filters: Vec::new(),
            rng,
        })
    }

    /// Creates a new [`GlitchContext`] from a byte slice.
    pub fn new(data: &[u8], seed: Option<u64>) -> Result<Self> {
        Self::new_with_options(data, seed, ParseOptions::default())
    }

    /// Creates a new [`GlitchContext`] from a byte slice using custom PNG [`ParseOptions`].
    pub fn new_with_options(data: &[u8], seed: Option<u64>, options: ParseOptions) -> Result<Self> {
        let png = PngGlitch::from_bytes_with_options(data, options).map_err(RecipeError::Parse)?;
        let rng = rng_from_seed(seed)?;
        Ok(Self {
            png,
            filters: Vec::new(),
            rng,
        })
    }

    /// Returns any non-fatal PNG integrity warnings recorded when parsing in lenient mode.
    pub fn warnings(&self) -> &[PngWarning] {
        self.png.warnings()
    }

    /// Returns the wrapped image, for reading.
    pub fn png(&self) -> &PngGlitch {
        &self.png
    }

    /// Returns the wrapped image, so low-level [`PngGlitch`] operations can be mixed with filters.
    ///
    /// Queued filters have not run yet; call [`execute`](Self::execute) first to see their effect.
    pub fn png_mut(&mut self) -> &mut PngGlitch {
        &mut self.png
    }

    /// Consumes the context and returns the wrapped image. Queued filters are discarded.
    pub fn into_inner(self) -> PngGlitch {
        self.png
    }

    /// Queues a closure as a filter, for quick experiments.
    ///
    /// The closure receives the image and the context's RNG, like [`GlitchFilter::apply`].
    pub fn add_fn(&mut self, f: impl Fn(&mut PngGlitch, &mut dyn RngCore) + 'static) {
        struct FnFilter<F>(F);
        impl<F: Fn(&mut PngGlitch, &mut dyn RngCore)> GlitchFilter for FnFilter<F> {
            fn apply(&self, png: &mut PngGlitch, rng: &mut dyn RngCore) {
                (self.0)(png, rng)
            }
        }
        self.add_filter(FnFilter(f));
    }

    /// Resets the internal RNG with the given `seed` (or a fresh OS seed if `None`).
    ///
    /// Without the `os-rng` feature there is no OS seed: `None` leaves the RNG unchanged.
    pub fn set_seed(&mut self, seed: Option<u64>) {
        match seed {
            Some(s) => self.rng = ChaCha8Rng::seed_from_u64(s),
            #[cfg(feature = "os-rng")]
            None => self.rng = ChaCha8Rng::from_os_rng(),
            #[cfg(not(feature = "os-rng"))]
            None => {}
        }
    }

    /// Adds a filter to the execution queue.
    pub fn add_filter(&mut self, filter: impl GlitchFilter + 'static) {
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
            filter.apply(&mut self.png, &mut self.rng);
        }
    }

    /// Returns the encoded glitched image as a byte vector.
    pub fn buffer(&self) -> Result<Vec<u8>> {
        self.png.to_bytes().map_err(RecipeError::Encode)
    }

    /// Saves the glitched image to a file.
    pub fn save(&self, path: impl AsRef<Path>) -> Result<()> {
        let path = path.as_ref();
        self.png.save(path).map_err(|source| RecipeError::Save {
            path: path.to_path_buf(),
            source,
        })
    }

    /// Returns the width of the image in pixels.
    pub fn width(&self) -> u32 {
        self.png.width()
    }

    /// Returns the height of the image in pixels.
    pub fn height(&self) -> u32 {
        self.png.height()
    }

    /// Re-encodes all scanlines using the specified PNG prediction filter type.
    ///
    /// This recomputes every filtered value; to overwrite only the tag use
    /// `png_mut().set_filter_type(..)`.
    #[deprecated(
        since = "0.3.0",
        note = "use `png_mut().reencode_with(..)` or `png_mut().set_filter_type(..)`; removed before 1.0"
    )]
    pub fn change_filter_type(&mut self, filter_type: FilterType) {
        self.png.reencode_with(filter_type);
    }

    /// Queues a pre-process filter that re-encodes all scanlines with a uniform PNG filter type.
    pub fn pre_process(&mut self, pre_process: PreProcess) {
        self.add_filter(Regenerate::new(RegenerateStrategy::Fixed(pre_process)));
    }

    /// Adds a step that works on the rows `rows` only. See [`FilterEntry`].
    ///
    /// # Errors
    /// As [`add_from_config`](Self::add_from_config), and [`RecipeError::InvalidSpec`] for
    /// `regenerate`, which recomputes the whole image.
    pub fn add_from_config_in_rows(&mut self, config: FilterConfig, rows: RowRange) -> Result<()> {
        self.add_filter(crate::resolve::rows_filter(config, rows)?);
        Ok(())
    }

    /// Adds a filter from a [`FilterConfig`] definition.
    ///
    /// Returns an error if the filter configuration is not supported for PNG images
    /// (for example, WebP-only filters such as `MacroblockGlitch`, `AlphaGlitch`, or `LossyArtifact`).
    pub fn add_from_config(&mut self, config: FilterConfig) -> Result<()> {
        self.add_filter(filter_from_config(&config)?);
        Ok(())
    }
}

impl PixelFilter for Invert {
    fn apply_pixel(&self, r: u8, g: u8, b: u8, a: u8) -> (u8, u8, u8, u8) {
        (255 - r, 255 - g, 255 - b, a)
    }
}

impl PixelFilter for Brighten {
    fn apply_pixel(&self, r: u8, g: u8, b: u8, a: u8) -> (u8, u8, u8, u8) {
        let s = self.strength.min(255) as u8;
        (
            r.saturating_add(s),
            g.saturating_add(s),
            b.saturating_add(s),
            a,
        )
    }
}

impl PixelFilter for Darken {
    fn apply_pixel(&self, r: u8, g: u8, b: u8, a: u8) -> (u8, u8, u8, u8) {
        let s = self.strength.min(255) as u8;
        (
            r.saturating_sub(s),
            g.saturating_sub(s),
            b.saturating_sub(s),
            a,
        )
    }
}

impl PixelFilter for ShiftChannels {
    fn apply_pixel(&self, r: u8, g: u8, b: u8, a: u8) -> (u8, u8, u8, u8) {
        let shift = |v: u8, off: i16| (v as i16 + off).clamp(0, 255) as u8;
        (shift(r, self.r), shift(g, self.g), shift(b, self.b), a)
    }
}

impl<F: PixelFilter> GlitchFilter for PixelFilterAdapter<F> {
    fn apply(&self, png: &mut PngGlitch, _rng: &mut dyn RngCore) {
        png.foreach_scanline(|line| {
            let max_val = ((1u32 << line.bit_depth() as u32) - 1) as u16;
            line.process_pixels(|_, pixel| {
                let scale_down = |v: u16| -> u8 {
                    if max_val == 255 {
                        v as u8
                    } else {
                        (v as u32 * 255 / max_val as u32) as u8
                    }
                };
                let scale_up = |v: u8| -> u16 {
                    if max_val == 255 {
                        v as u16
                    } else {
                        (v as u32 * max_val as u32 / 255) as u16
                    }
                };
                match pixel {
                    Pixel::RGB(r, g, b) => {
                        let (nr, ng, nb, _) =
                            self.0
                                .apply_pixel(scale_down(r), scale_down(g), scale_down(b), 255);
                        Pixel::RGB(scale_up(nr), scale_up(ng), scale_up(nb))
                    }
                    Pixel::RGBA(r, g, b, a) => {
                        let (nr, ng, nb, na) = self.0.apply_pixel(
                            scale_down(r),
                            scale_down(g),
                            scale_down(b),
                            scale_down(a),
                        );
                        Pixel::RGBA(scale_up(nr), scale_up(ng), scale_up(nb), scale_up(na))
                    }
                    Pixel::Gray(v) => {
                        let gv = scale_down(v);
                        let (nr, _, _, _) = self.0.apply_pixel(gv, gv, gv, 255);
                        Pixel::Gray(scale_up(nr))
                    }
                    Pixel::GrayAlpha(v, a) => {
                        let gv = scale_down(v);
                        let (nr, _, _, na) = self.0.apply_pixel(gv, gv, gv, scale_down(a));
                        Pixel::GrayAlpha(scale_up(nr), scale_up(na))
                    }
                    other => other,
                }
            });
        });
    }
}
