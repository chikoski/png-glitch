//! Recipes as values: [`Recipe`] → resolve → [`ResolvedRecipe`] → execute (ADR 0004).
//!
//! A [`Recipe`] is a list of steps that is not tied to an image. *Resolving* it with a seed
//! and an [`ImageHeader`] draws every random choice and yields a [`ResolvedRecipe`], which holds
//! only concrete `png-glitch` effects. *Executing* a resolved recipe is deterministic and uses
//! no RNG.

use crate::{FilterConfig, GlitchFilter, RecipeError, Result, filter_from_config};
use png_glitch::effects as fx;
use png_glitch::effects::{Effect, ScanLinesEffect};
use png_glitch::{ColorType, PngGlitch, ScanLines};
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

trait ErasedLiveEffect: ScanLinesEffect + fmt::Debug + Send + Sync {}
impl<T: ScanLinesEffect + fmt::Debug + Send + Sync> ErasedLiveEffect for T {}

#[derive(Clone)]
enum EffectKind {
    /// Any [`Effect`]: file workflow only.
    File(Arc<dyn ErasedEffect>),
    /// A [`ScanLinesEffect`]: file workflow and live loop.
    Live(Arc<dyn ErasedLiveEffect>),
}

/// A user-defined `png-glitch` effect with concrete parameters, as produced by a
/// [`RecipeStep`]. Cheap to clone; two values are equal only if they are the same instance.
///
/// It wraps either a plain [`Effect`] ([`new`](Self::new)), which runs on files only, or a
/// [`ScanLinesEffect`] ([`new_live`](Self::new_live)), which also runs in a
/// [`LiveRecipe`](crate::LiveRecipe). Both give the same bytes in the file workflow.
#[derive(Clone)]
pub struct ResolvedEffect(EffectKind);

impl ResolvedEffect {
    /// Wraps an effect so a [`ResolvedRecipe`] can hold it. The effect cannot run in a live loop;
    /// use [`new_live`](Self::new_live) for that.
    pub fn new(effect: impl Effect + fmt::Debug + Send + Sync + 'static) -> Self {
        Self(EffectKind::File(Arc::new(effect)))
    }

    /// Wraps a [`ScanLinesEffect`], which can run both on a file and in a
    /// [`LiveRecipe`](crate::LiveRecipe).
    pub fn new_live(effect: impl ScanLinesEffect + fmt::Debug + Send + Sync + 'static) -> Self {
        Self(EffectKind::Live(Arc::new(effect)))
    }

    /// Whether this effect can run in a live loop (it was made with
    /// [`new_live`](Self::new_live)).
    pub fn is_live(&self) -> bool {
        matches!(self.0, EffectKind::Live(_))
    }

    fn apply_to(&self, png: &mut PngGlitch) {
        match &self.0 {
            EffectKind::File(e) => e.apply_to(png),
            EffectKind::Live(e) => e.apply_to_scan_lines(&mut png.scan_lines_mut()),
        }
    }
}

impl fmt::Debug for ResolvedEffect {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.0 {
            EffectKind::File(e) => e.fmt(f),
            EffectKind::Live(e) => e.fmt(f),
        }
    }
}

impl PartialEq for ResolvedEffect {
    fn eq(&self, other: &Self) -> bool {
        match (&self.0, &other.0) {
            (EffectKind::File(a), EffectKind::File(b)) => Arc::ptr_eq(a, b),
            (EffectKind::Live(a), EffectKind::Live(b)) => Arc::ptr_eq(a, b),
            _ => false,
        }
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
            Self::Custom(e) => e.apply_to(png),
        }
    }

    /// Applies the step to the scan lines of a live frame. Deterministic; uses no RNG.
    ///
    /// Fails with [`RecipeError::NotLive`] for a step that recomputes filtered values
    /// ([`Refilter`](fx::Refilter); [`LiveRecipe`](crate::LiveRecipe) runs it as a generation
    /// boundary instead) and for a [`Custom`](Self::Custom) step made with
    /// [`ResolvedEffect::new`]. Gives the same bytes as [`execute`](Self::execute).
    pub(crate) fn execute_live(&self, lines: &mut ScanLines<'_>) -> Result<()> {
        match self {
            Self::Noop => {}
            Self::Refilter(_) => return Err(self.not_live()),
            Self::SetFilterTypes(e) => e.apply_to_scan_lines(lines),
            Self::SetBytes(e) => e.apply_to_scan_lines(lines),
            Self::Substitute(e) => e.apply_to_scan_lines(lines),
            Self::SwapLines(e) => e.apply_to_scan_lines(lines),
            Self::CopyLines(e) => e.apply_to_scan_lines(lines),
            Self::ShiftLines(e) => e.apply_to_scan_lines(lines),
            Self::PixelSort(e) => e.apply_to_scan_lines(lines),
            Self::Bitwise(e) => e.apply_to_scan_lines(lines),
            Self::SwapChannels(e) => e.apply_to_scan_lines(lines),
            Self::OffsetColors(e) => e.apply_to_scan_lines(lines),
            Self::HslShift(e) => e.apply_to_scan_lines(lines),
            Self::ChannelShift(e) => e.apply_to_scan_lines(lines),
            Self::SwapBlocks(e) => e.apply_to_scan_lines(lines),
            Self::Invert(e) => e.apply_to_scan_lines(lines),
            Self::Brighten(e) => e.apply_to_scan_lines(lines),
            Self::Darken(e) => e.apply_to_scan_lines(lines),
            Self::ShiftChannels(e) => e.apply_to_scan_lines(lines),
            Self::Custom(e) => match &e.0 {
                EffectKind::Live(e) => e.apply_to_scan_lines(lines),
                EffectKind::File(_) => return Err(self.not_live()),
            },
        }
        Ok(())
    }

    /// The filter type if this step is a generation boundary (`Refilter`).
    pub(crate) fn refilter_type(&self) -> Option<png_glitch::FilterType> {
        match self {
            Self::Refilter(r) => Some(r.filter_type),
            _ => None,
        }
    }

    /// Whether the live runner can run this step (as a scan-line effect or a boundary).
    pub(crate) fn is_live(&self) -> bool {
        match self {
            Self::Custom(e) => e.is_live(),
            _ => true,
        }
    }

    pub(crate) fn not_live(&self) -> RecipeError {
        RecipeError::NotLive {
            step: format!("{self:?}"),
        }
    }
}

/// Turns a built-in filter's parameters and random specification into a concrete step.
pub(crate) trait Resolve {
    fn resolve(&self, header: &ImageHeader, rng: &mut dyn RngCore) -> ResolvedStep;

    /// Writes into `out` exactly what [`resolve`](Self::resolve) returns, with the same RNG
    /// draws in the same order, but reuses the buffers already in `out` (and `scratch`) when
    /// `out` holds the same kind of step. Capacity grows to the largest size seen, then stays.
    ///
    /// The default allocates like `resolve`; it is enough for steps without lists.
    fn resolve_into(
        &self,
        header: &ImageHeader,
        rng: &mut dyn RngCore,
        out: &mut ResolvedStep,
        _scratch: &mut StepScratch,
    ) {
        *out = self.resolve(header, rng);
    }
}

/// Buffers a step keeps between [`Recipe::resolve_into`] calls that its [`ResolvedStep`] cannot
/// hold: spare per-row lists of `ColorDistortion`, so that the number of distorted rows can
/// change from frame to frame without allocating.
#[derive(Default)]
pub(crate) struct StepScratch {
    pub(crate) offsets: Vec<Vec<[i16; 3]>>,
}

/// Takes the list that `field` finds in `out` (empty, with its capacity), or an empty `Vec`.
pub(crate) fn take_buffer<T>(
    out: &mut ResolvedStep,
    field: impl FnOnce(&mut ResolvedStep) -> Option<&mut Vec<T>>,
) -> Vec<T> {
    let mut buf = field(out).map(std::mem::take).unwrap_or_default();
    buf.clear();
    buf
}

macro_rules! builtin_filters {
    ($($name:ident),* $(,)?) => {
        /// A built-in filter, built from a [`FilterConfig`] without a heap allocation.
        #[derive(Clone, Copy, Debug)]
        pub(crate) enum Builtin {
            $($name($name),)*
        }

        $(
            impl From<$name> for Builtin {
                fn from(filter: $name) -> Self {
                    Self::$name(filter)
                }
            }
        )*

        impl Resolve for Builtin {
            fn resolve(&self, header: &ImageHeader, rng: &mut dyn RngCore) -> ResolvedStep {
                match self {
                    $(Self::$name(f) => f.resolve(header, rng),)*
                }
            }

            fn resolve_into(
                &self,
                header: &ImageHeader,
                rng: &mut dyn RngCore,
                out: &mut ResolvedStep,
                scratch: &mut StepScratch,
            ) {
                match self {
                    $(Self::$name(f) => f.resolve_into(header, rng, out, scratch),)*
                }
            }
        }
    };
}

use crate::{
    AverageFilter, Bitwise, BlockScramble, ChangeFilterType, ChannelSwap, ChromaticAberration,
    ColorDistortion, ColorSpaceGlitch, HorizontalShift, PaethFilter, PixelSort, RandomCopy,
    RemoveFilter, Replace, SetZero, SubFilter, Substitute, Transpose, UpFilter,
};
use png_glitch::effects::{Brighten, Darken, Invert, ShiftChannels};

builtin_filters!(
    ChangeFilterType,
    RemoveFilter,
    SubFilter,
    UpFilter,
    AverageFilter,
    PaethFilter,
    Replace,
    Transpose,
    RandomCopy,
    Substitute,
    SetZero,
    PixelSort,
    Bitwise,
    ChannelSwap,
    HorizontalShift,
    BlockScramble,
    ColorDistortion,
    ColorSpaceGlitch,
    ChromaticAberration,
    Invert,
    Brighten,
    Darken,
    ShiftChannels,
);

impl GlitchFilter for Builtin {
    fn apply(&self, png: &mut PngGlitch, rng: &mut dyn RngCore) {
        let header = ImageHeader::of(png);
        self.resolve(&header, rng).execute(png);
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
            None => (crate::context::rng_from_seed(None)?, None),
        };
        let steps = self
            .steps
            .iter()
            .map(|step| match step {
                Step::Config(config) => Ok(filter_from_config(config)?.resolve(header, &mut rng)),
                Step::Custom(step) => Ok(ResolvedStep::Custom(step.resolve(header, &mut rng))),
            })
            .collect::<Result<Vec<_>>>()?;
        Ok(ResolvedRecipe {
            header: *header,
            seed,
            steps,
            scratch: Vec::new(),
        })
    }

    /// Like [`resolve`](Self::resolve), but writes the result into `out`, reusing its buffers.
    ///
    /// `out` afterwards equals what `resolve(header, seed)` returns: the RNG draws are the same,
    /// so seeds reproduce the same output. `out` can hold anything before, such as the result
    /// for another seed, header or recipe.
    ///
    /// # Allocation
    ///
    /// When `out` already holds a step of the same kind at the same position (as it does when
    /// the same recipe is resolved again for the same header, e.g. once per frame in a live loop),
    /// that step's lists are cleared and refilled in place. On the first such call each list
    /// reserves its upper bound (for "each row with probability `p`", the image height; for
    /// `random-copy`, `times`; for `block-scramble`, the number of swaps), so later calls make
    /// no heap allocation for built-in steps, whatever the seed. `color-distortion` also keeps
    /// one spare offset list per image row, allocated on that first call (6 bytes per pixel).
    ///
    /// A user-defined step ([`Step::Custom`]) cannot reuse buffers: it is resolved as in
    /// `resolve`, with whatever allocation its [`RecipeStep::resolve`] makes.
    ///
    /// # Errors
    ///
    /// The same as [`resolve`](Self::resolve). On error the content of `out` is unspecified
    /// (still a valid value); resolve again before using it.
    pub fn resolve_into(
        &self,
        header: &ImageHeader,
        seed: Option<u64>,
        out: &mut ResolvedRecipe,
    ) -> Result<()> {
        let (mut rng, seed) = match seed {
            Some(seed) => (ChaCha8Rng::seed_from_u64(seed), Some(seed)),
            None => (crate::context::rng_from_seed(None)?, None),
        };
        let n = self.steps.len();
        out.steps.truncate(n);
        out.steps.resize(n, ResolvedStep::Noop);
        out.scratch.resize_with(n, StepScratch::default);
        let slots = out.steps.iter_mut().zip(out.scratch.iter_mut());
        for (step, (slot, scratch)) in self.steps.iter().zip(slots) {
            match step {
                Step::Config(config) => {
                    filter_from_config(config)?.resolve_into(header, &mut rng, slot, scratch)
                }
                Step::Custom(step) => *slot = ResolvedStep::Custom(step.resolve(header, &mut rng)),
            }
        }
        out.header = *header;
        out.seed = seed;
        Ok(())
    }

    /// Resolves the recipe for `png` and executes it on it.
    pub fn run(&self, png: &mut PngGlitch, seed: Option<u64>) -> Result<ResolvedRecipe> {
        let resolved = self.resolve(&ImageHeader::of(png), seed)?;
        resolved.execute(png)?;
        Ok(resolved)
    }
}

/// A recipe whose parameters are all concrete, tied to the header it was resolved for.
pub struct ResolvedRecipe {
    header: ImageHeader,
    seed: Option<u64>,
    steps: Vec<ResolvedStep>,
    /// Spare buffers for [`Recipe::resolve_into`], one per step. Not part of the value: it is
    /// ignored by `PartialEq` and `Debug`, and not cloned.
    scratch: Vec<StepScratch>,
}

impl Clone for ResolvedRecipe {
    fn clone(&self) -> Self {
        Self {
            header: self.header,
            seed: self.seed,
            steps: self.steps.clone(),
            scratch: Vec::new(),
        }
    }
}

impl PartialEq for ResolvedRecipe {
    fn eq(&self, other: &Self) -> bool {
        self.header == other.header && self.seed == other.seed && self.steps == other.steps
    }
}

impl fmt::Debug for ResolvedRecipe {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ResolvedRecipe")
            .field("header", &self.header)
            .field("seed", &self.seed)
            .field("steps", &self.steps)
            .finish()
    }
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
