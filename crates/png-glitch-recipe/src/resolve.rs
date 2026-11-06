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
use std::fmt;
use std::sync::Arc;

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

trait ErasedEffect: Effect + fmt::Debug + Send + Sync {}
impl<T: Effect + fmt::Debug + Send + Sync> ErasedEffect for T {}

/// A user-defined `png-glitch` [`Effect`] with concrete parameters, as produced by a
/// [`RecipeStep`]. Cheap to clone; two values are equal only if they are the same instance.
#[derive(Clone)]
pub struct ResolvedEffect(Arc<dyn ErasedEffect>);

impl ResolvedEffect {
    /// Wraps an effect so a [`ResolvedRecipe`] can hold it.
    pub fn new(effect: impl Effect + fmt::Debug + Send + Sync + 'static) -> Self {
        Self(Arc::new(effect))
    }
}

impl fmt::Debug for ResolvedEffect {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl PartialEq for ResolvedEffect {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

/// A user-defined recipe step: draws its random choices from the RNG and returns a
/// deterministic `png-glitch` effect.
///
/// Closures `Fn(&ImageHeader, &mut dyn RngCore) -> ResolvedEffect` implement this trait, which is
/// handy for quick experiments. Add a step with [`Recipe::then_step`].
pub trait RecipeStep: Send + Sync {
    /// Draws the choices for an image with the given header and returns the effect to run.
    fn resolve(&self, header: &ImageHeader, rng: &mut dyn RngCore) -> ResolvedEffect;
}

impl<F> RecipeStep for F
where
    F: Fn(&ImageHeader, &mut dyn RngCore) -> ResolvedEffect + Send + Sync,
{
    fn resolve(&self, header: &ImageHeader, rng: &mut dyn RngCore) -> ResolvedEffect {
        self(header, rng)
    }
}

/// A step that always runs the same effect.
struct FixedStep(ResolvedEffect);

impl RecipeStep for FixedStep {
    fn resolve(&self, _header: &ImageHeader, _rng: &mut dyn RngCore) -> ResolvedEffect {
        self.0.clone()
    }
}

/// A step of a [`Recipe`]: a built-in step described by a [`FilterConfig`], or a user-defined one.
#[derive(Clone)]
pub enum Step {
    /// A built-in step.
    Config(FilterConfig),
    /// A user-defined step.
    Custom(Arc<dyn RecipeStep>),
}

impl fmt::Debug for Step {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Config(config) => f.debug_tuple("Config").field(config).finish(),
            Self::Custom(_) => f.write_str("Custom(..)"),
        }
    }
}

impl PartialEq for Step {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Config(a), Self::Config(b)) => a == b,
            (Self::Custom(a), Self::Custom(b)) => Arc::ptr_eq(a, b),
            _ => false,
        }
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
    /// A user-defined effect.
    Custom(ResolvedEffect),
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
            Self::Custom(e) => e.0.apply_to(png),
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
    steps: Vec<Step>,
}

impl Recipe {
    /// Creates an empty recipe.
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a recipe from a list of steps.
    pub fn from_configs(steps: impl IntoIterator<Item = FilterConfig>) -> Self {
        Self {
            steps: steps.into_iter().map(Step::Config).collect(),
        }
    }

    /// Appends a built-in step.
    pub fn then(mut self, step: FilterConfig) -> Self {
        self.steps.push(Step::Config(step));
        self
    }

    /// Appends a user-defined step that draws its choices from the RNG (see [`RecipeStep`]).
    pub fn then_step(mut self, step: impl RecipeStep + 'static) -> Self {
        self.steps.push(Step::Custom(Arc::new(step)));
        self
    }

    /// Appends a user-defined deterministic `png-glitch` effect.
    pub fn then_effect(self, effect: impl Effect + fmt::Debug + Send + Sync + 'static) -> Self {
        self.then_step(FixedStep(ResolvedEffect::new(effect)))
    }

    /// The built-in steps as [`FilterConfig`]s, or `None` if the recipe has a user-defined step
    /// (code cannot be written out as data).
    pub fn configs(&self) -> Option<Vec<FilterConfig>> {
        self.steps
            .iter()
            .map(|step| match step {
                Step::Config(config) => Some(config.clone()),
                Step::Custom(_) => None,
            })
            .collect()
    }

    /// The steps, in execution order.
    pub fn steps(&self) -> &[Step] {
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
            .map(|step| match step {
                Step::Config(config) => {
                    let filter = filter_from_config(config.clone())?;
                    Ok(filter.resolve(header, &mut rng))
                }
                Step::Custom(step) => Ok(ResolvedStep::Custom(step.resolve(header, &mut rng))),
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

/// On-disk form of a [`Recipe`]: `filters: [...]`, the same shape as the CLI's `--config` file.
#[derive(serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct RecipeData {
    filters: Vec<FilterConfig>,
}

impl serde::Serialize for Recipe {
    /// Serializes as `filters: [...]`. Fails if the recipe has a user-defined step.
    fn serialize<S: serde::Serializer>(
        &self,
        serializer: S,
    ) -> std::result::Result<S::Ok, S::Error> {
        let filters = self.configs().ok_or_else(|| {
            serde::ser::Error::custom("a recipe with user-defined steps cannot be serialized")
        })?;
        RecipeData { filters }.serialize(serializer)
    }
}

impl<'de> serde::Deserialize<'de> for Recipe {
    fn deserialize<D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> std::result::Result<Self, D::Error> {
        let data = RecipeData::deserialize(deserializer)?;
        Ok(Recipe::from_configs(data.filters))
    }
}
