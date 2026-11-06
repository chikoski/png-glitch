//! Recipes as values: [`Recipe`] → resolve → [`ResolvedRecipe`] → execute (ADR 0004).
//!
//! A [`Recipe`] is a list of steps that is not tied to an image. *Resolving* it with a seed
//! and an [`ImageHeader`] draws every random choice and yields a [`ResolvedRecipe`], which holds
//! only concrete `png-glitch` effects. *Executing* a resolved recipe is deterministic and uses
//! no RNG.

use crate::{FilterConfig, GlitchFilter, RecipeError, Result, filter_from_config};
use png_glitch::effects as fx;
use png_glitch::effects::Effect;
use png_glitch::{ColorType, PngGlitch};
use rand::{RngCore, SeedableRng};
use rand_chacha::ChaCha8Rng;

/// The parts of a PNG header that resolving a recipe depends on (no pixel data).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ImageHeader {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// Color type.
    pub color_type: ColorType,
    /// Bit depth per channel.
    pub bit_depth: u8,
}

impl ImageHeader {
    /// Reads the header of a loaded image.
    pub fn of(png: &PngGlitch) -> Self {
        Self {
            width: png.width(),
            height: png.height(),
            color_type: png.color_type(),
            bit_depth: png.bit_depth(),
        }
    }

    /// Byte size of one scan line's filtered data (without the filter-type byte).
    pub fn line_size(&self) -> usize {
        let bits = self.width as usize * self.color_type.bit_per_pixel(self.bit_depth);
        bits.div_ceil(8)
    }
}

/// A step whose parameters are all concrete: one deterministic `png-glitch` effect.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
#[allow(missing_docs)]
pub enum ResolvedStep {
    /// Does nothing (the step had nothing to do for this image).
    Noop,
    Refilter(fx::Refilter),
    SetFilterTypes(fx::SetFilterTypes),
    SetBytes(fx::SetBytes),
    Substitute(fx::Substitute),
    SwapLines(fx::SwapLines),
    CopyLines(fx::CopyLines),
    ShiftLines(fx::ShiftLines),
    PixelSort(fx::PixelSort),
    Bitwise(fx::Bitwise),
    SwapChannels(fx::SwapChannels),
    OffsetColors(fx::OffsetColors),
    HslShift(fx::HslShift),
    ChannelShift(fx::ChannelShift),
    SwapBlocks(fx::SwapBlocks),
    Invert(fx::Invert),
    Brighten(fx::Brighten),
    Darken(fx::Darken),
    ShiftChannels(fx::ShiftChannels),
}

impl ResolvedStep {
    /// Applies the step to the image. Deterministic; uses no RNG.
    pub fn execute(&self, png: &mut PngGlitch) {
        match self {
            Self::Noop => {}
            Self::Refilter(e) => e.apply_to(png),
            Self::SetFilterTypes(e) => e.apply_to(png),
            Self::SetBytes(e) => e.apply_to(png),
            Self::Substitute(e) => e.apply_to(png),
            Self::SwapLines(e) => e.apply_to(png),
            Self::CopyLines(e) => e.apply_to(png),
            Self::ShiftLines(e) => e.apply_to(png),
            Self::PixelSort(e) => e.apply_to(png),
            Self::Bitwise(e) => e.apply_to(png),
            Self::SwapChannels(e) => e.apply_to(png),
            Self::OffsetColors(e) => e.apply_to(png),
            Self::HslShift(e) => e.apply_to(png),
            Self::ChannelShift(e) => e.apply_to(png),
            Self::SwapBlocks(e) => e.apply_to(png),
            Self::Invert(e) => e.apply_to(png),
            Self::Brighten(e) => e.apply_to(png),
            Self::Darken(e) => e.apply_to(png),
            Self::ShiftChannels(e) => e.apply_to(png),
        }
    }
}

/// Turns a built-in filter's parameters and random specification into a concrete step.
pub(crate) trait Resolve {
    fn resolve(&self, header: &ImageHeader, rng: &mut dyn RngCore) -> ResolvedStep;
}

/// A built-in filter: usable directly ([`GlitchFilter`]) and resolvable ([`Resolve`]).
pub(crate) trait BuiltinFilter: GlitchFilter + Resolve {}
impl<T: GlitchFilter + Resolve> BuiltinFilter for T {}

impl GlitchFilter for Box<dyn BuiltinFilter> {
    fn apply(&self, png: &mut PngGlitch, rng: &mut dyn RngCore) {
        (**self).apply(png, rng)
    }
}

macro_rules! impl_glitch_filter_via_resolve {
    ($($t:ty),* $(,)?) => {
        $(
            impl GlitchFilter for $t {
                fn apply(&self, png: &mut PngGlitch, rng: &mut dyn RngCore) {
                    let header = ImageHeader::of(png);
                    $crate::Resolve::resolve(self, &header, rng).execute(png);
                }
            }
        )*
    };
}

/// An ordered list of glitch steps, independent of any image.
///
/// A recipe can be cloned, compared and reused for any number of images and seeds.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Recipe {
    steps: Vec<FilterConfig>,
}

impl Recipe {
    /// Creates an empty recipe.
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a recipe from a list of steps.
    pub fn from_configs(steps: impl IntoIterator<Item = FilterConfig>) -> Self {
        Self {
            steps: steps.into_iter().collect(),
        }
    }

    /// Appends a step.
    pub fn then(mut self, step: FilterConfig) -> Self {
        self.steps.push(step);
        self
    }

    /// The steps, in execution order.
    pub fn steps(&self) -> &[FilterConfig] {
        &self.steps
    }

    /// Draws every random choice and returns a recipe with concrete parameters.
    ///
    /// `seed` seeds the RNG (`None` picks an OS seed, which is recorded in the result).
    /// The same recipe, header and seed always give the same result, and applying it matches
    /// what `GlitchContext` does for the same seed.
    pub fn resolve(&self, header: &ImageHeader, seed: Option<u64>) -> Result<ResolvedRecipe> {
        let (mut rng, seed) = match seed {
            Some(seed) => (ChaCha8Rng::seed_from_u64(seed), Some(seed)),
            None => (ChaCha8Rng::from_os_rng(), None),
        };
        let steps = self
            .steps
            .iter()
            .map(|config| {
                let filter = filter_from_config(config.clone())?;
                Ok(filter.resolve(header, &mut rng))
            })
            .collect::<Result<Vec<_>>>()?;
        Ok(ResolvedRecipe {
            header: *header,
            seed,
            steps,
        })
    }

    /// Resolves the recipe for `png` and executes it on it.
    pub fn run(&self, png: &mut PngGlitch, seed: Option<u64>) -> Result<ResolvedRecipe> {
        let resolved = self.resolve(&ImageHeader::of(png), seed)?;
        resolved.execute(png)?;
        Ok(resolved)
    }
}

/// A recipe whose parameters are all concrete, tied to the header it was resolved for.
#[derive(Clone, Debug, PartialEq)]
pub struct ResolvedRecipe {
    header: ImageHeader,
    seed: Option<u64>,
    steps: Vec<ResolvedStep>,
}

impl ResolvedRecipe {
    /// The header this recipe was resolved for.
    pub fn header(&self) -> &ImageHeader {
        &self.header
    }

    /// The seed it was resolved with (`None` if an OS seed was used).
    pub fn seed(&self) -> Option<u64> {
        self.seed
    }

    /// The concrete steps, in execution order.
    pub fn steps(&self) -> &[ResolvedStep] {
        &self.steps
    }

    /// Executes the steps on `png`. Deterministic; uses no RNG.
    ///
    /// Returns [`RecipeError::HeaderMismatch`] if `png`'s header differs from the one the
    /// recipe was resolved for.
    pub fn execute(&self, png: &mut PngGlitch) -> Result<()> {
        let actual = ImageHeader::of(png);
        if actual != self.header {
            return Err(RecipeError::HeaderMismatch {
                expected: self.header,
                actual,
            });
        }
        for step in &self.steps {
            step.execute(png);
        }
        Ok(())
    }
}
