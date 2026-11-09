//! [`Session`]: the live workflow behind the WIT `session` resource.

use crate::{GlitchError, image_error, parse_recipe, recipe_error};
use png_glitch::{Encoding, LivePipeline, PixelSource, PngError, PngGlitch};
use png_glitch_recipe::{ImageHeader, LiveRecipe, Recipe, SeedMode};

/// A resident PNG model for a live loop.
///
/// `reload_pixels`, `apply`, `reset` and `render_into` make no heap allocation in steady state
/// (after the recipe is set, and one warm-up frame). `set_recipe`, `render_rgba` and `snapshot`
/// allocate by design: parsing happens once, and compression happens off the real-time path.
///
/// Each frame is a PNG: `render_into` shows exactly what a decoder reads from `snapshot`.
pub struct Session {
    live: LivePipeline,
    /// The model as it was after the last load; `reset` copies it back.
    clean: PngGlitch,
    recipe: Option<LiveRecipe>,
    from_pixels: bool,
}

fn map(e: PngError) -> GlitchError {
    image_error(e)
}

impl Session {
    /// From the bytes of a PNG file. The default decode limits apply.
    pub fn from_png(png: &[u8]) -> Result<Self, GlitchError> {
        let model = PngGlitch::from_bytes_with_options(png, Default::default()).map_err(map)?;
        let clean = PngGlitch::from_bytes_with_options(png, Default::default()).map_err(map)?;
        Ok(Self {
            live: LivePipeline::from_png(model),
            clean,
            recipe: None,
            from_pixels: false,
        })
    }

    /// From RGBA8 pixels (`width * height * 4` bytes), running only the encoder's filter step.
    pub fn from_pixels(
        width: u32,
        height: u32,
        rgba: &[u8],
        encoding: Encoding,
    ) -> Result<Self, GlitchError> {
        let live = LivePipeline::new(PixelSource::rgba8(width, height, rgba), encoding.clone())
            .map_err(map)?;
        let clean = PngGlitch::from_pixels(PixelSource::rgba8(width, height, rgba), encoding)
            .map_err(map)?;
        Ok(Self {
            live,
            clean,
            recipe: None,
            from_pixels: true,
        })
    }

    /// Refills the model with the next frame (RGBA8, same size) and discards earlier glitches.
    /// No heap allocation.
    pub fn reload_pixels(&mut self, rgba: &[u8]) -> Result<(), GlitchError> {
        if !self.from_pixels {
            return Err(GlitchError::InvalidImage(
                "this session was created from a PNG file; reload-pixels needs from-pixels".into(),
            ));
        }
        self.live.encode(rgba).map_err(map)?;
        self.clean.copy_data_from(self.live.model()).map_err(map)
    }

    /// Parses `recipe` once and keeps it for [`apply`](Self::apply). The frame counter of
    /// [`SeedMode::PerFrame`] starts at 0.
    pub fn set_recipe(
        &mut self,
        recipe: &str,
        seed: u64,
        mode: SeedMode,
    ) -> Result<(), GlitchError> {
        let file = parse_recipe(recipe)?;
        let header = ImageHeader::of(self.live.model());
        let runner = LiveRecipe::new(Recipe::from_entries(file.filters), header, Some(seed), mode)
            .map_err(recipe_error)?;
        self.recipe = Some(runner);
        Ok(())
    }

    /// Runs the recipe on the current scan lines. Parses nothing and, with built-in steps,
    /// makes no heap allocation.
    pub fn apply(&mut self) -> Result<(), GlitchError> {
        match self.recipe.as_mut() {
            Some(runner) => runner.run(&mut self.live).map_err(recipe_error),
            None => Err(GlitchError::NoRecipe),
        }
    }

    /// Restores the model as it was after the last load. No heap allocation. The recipe and
    /// its frame counter are kept.
    pub fn reset(&mut self) -> Result<(), GlitchError> {
        self.live.restore_from(&self.clean).map_err(map)
    }

    /// Image width in pixels.
    pub fn width(&self) -> u32 {
        ImageHeader::of(self.live.model()).width
    }

    /// Image height in pixels.
    pub fn height(&self) -> u32 {
        ImageHeader::of(self.live.model()).height
    }

    /// Writes the current frame as RGBA8 into `out` (`width * height * 4` bytes). No heap
    /// allocation.
    pub fn render_into(&mut self, out: &mut [u8]) -> Result<(), GlitchError> {
        self.live.decode_into(out).map_err(map)
    }

    /// The current frame as a new RGBA8 buffer.
    pub fn render_rgba(&mut self) -> Result<Vec<u8>, GlitchError> {
        let len = (self.width() as usize)
            .checked_mul(self.height() as usize)
            .and_then(|n| n.checked_mul(4))
            .ok_or(GlitchError::TooLarge)?;
        let mut out = vec![0u8; len];
        self.render_into(&mut out)?;
        Ok(out)
    }

    /// The current frame as a PNG file; compression runs here.
    pub fn snapshot(&self) -> Result<Vec<u8>, GlitchError> {
        self.live.snapshot().map_err(map)
    }
}
