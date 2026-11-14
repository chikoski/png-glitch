//! Recipes as values: [`Recipe`] → resolve → [`ResolvedRecipe`] → execute (ADR 0004).
//!
//! A [`Recipe`] is a list of steps that is not tied to an image. *Resolving* it with a seed
//! and an [`ImageHeader`] draws every random choice and yields a [`ResolvedRecipe`], which holds
//! only concrete `png-glitch` effects. *Executing* a resolved recipe is deterministic and uses
//! no RNG.

use crate::{
    FilterConfig, FilterEntry, GlitchFilter, MaskSet, RecipeError, Result, RowRange,
    filter_from_config,
};
use png_glitch::effects as fx;
use png_glitch::effects::{Effect, Mask, Masked, ScanLinesEffect};
use png_glitch::{ColorType, PngGlitch, ScanLines};
use rand::{Rng, SeedableRng};
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

/// The mask of `entry` for the whole image, or `None` if the entry has no `rect` / `mask`.
fn entry_mask(entry: &FilterEntry, header: &ImageHeader, masks: &MaskSet) -> Result<Option<Mask>> {
    let mask = match (&entry.rect, &entry.mask) {
        (Some(_), Some(_)) => {
            return Err(RecipeError::InvalidSpec(
                "a step cannot have both `rect` and `mask`".into(),
            ));
        }
        (Some(r), None) => Mask::rect(header.width, header.height, r.x, r.y, r.w, r.h),
        (None, Some(reference)) => masks.lookup(reference)?,
        (None, None) if entry.invert_mask => {
            return Err(RecipeError::InvalidSpec(
                "`invert-mask` needs a `rect` or a `mask`".into(),
            ));
        }
        (None, None) => return Ok(None),
    };
    Ok(Some(if entry.invert_mask {
        mask.inverted()
    } else {
        mask
    }))
}

/// Resolves a built-in step with its `rows` / `rect` / `mask` for an image with header `header`.
///
/// The step is resolved for an image as tall as the `rows` range, so its random choices and row
/// numbers are relative to the range. A mask is cropped to the same rows and wraps the resolved
/// step; the pair then runs on the row sub-view.
pub(crate) fn resolve_entry(
    entry: &FilterEntry,
    header: &ImageHeader,
    rng: &mut dyn Rng,
    masks: &MaskSet,
) -> Result<ResolvedStep> {
    let range = match &entry.rows {
        Some(rows) => rows.resolve(header.height),
        None => 0..header.height,
    };
    let sub = ImageHeader {
        height: range.end - range.start,
        ..*header
    };
    let mask = entry_mask(entry, header, masks)?;
    let mut step = filter_from_config(&entry.config)?.resolve(&sub, rng);
    if let Some(mask) = mask {
        step = step.masked(
            mask.crop_rows(range.start, range.end),
            sub.line_size() + 1,
            sub.height,
        )?;
    }
    if entry.rows.is_some() {
        step.in_rows(range.start, range.end)
    } else {
        Ok(step)
    }
}

/// Like [`resolve_entry`] for a step with `config`, `rows` and `mask` (the mask for the whole
/// image, from [`entry_mask`]), but writes into `slot` and reuses the buffers already there when
/// `slot` holds the same kind of step (#207). `slot` afterwards equals what `resolve_entry`
/// returns, with the same RNG draws.
///
/// A masked step keeps two copies of the step (the `step` field and the one inside its
/// [`MaskedEffect`]). Both are resolved into in place from the same RNG state, so they are equal
/// and neither allocates once warm. The effect is reused only when it is not shared with a
/// clone of the recipe and has the same mask; otherwise it is rebuilt, as in `resolve`.
pub(crate) fn resolve_targeted_into(
    config: &FilterConfig,
    rows: Option<RowRange>,
    mask: Option<Mask>,
    header: &ImageHeader,
    rng: &mut ChaCha8Rng,
    slot: &mut ResolvedStep,
    scratch: &mut StepScratch,
) -> Result<()> {
    let range = match &rows {
        Some(rows) => rows.resolve(header.height),
        None => 0..header.height,
    };
    let sub = ImageHeader {
        height: range.end - range.start,
        ..*header
    };
    let filter = filter_from_config(config)?;
    if rows.is_some() && !matches!(slot, ResolvedStep::Rows { .. }) {
        *slot = ResolvedStep::Rows {
            start: 0,
            end: 0,
            step: Box::new(ResolvedStep::Noop),
        };
    }
    let target = match slot {
        ResolvedStep::Rows { start, end, step } if rows.is_some() => {
            (*start, *end) = (range.start, range.end);
            &mut **step
        }
        other => other,
    };
    match mask {
        None => filter.resolve_into(&sub, rng, target, scratch),
        Some(mask) => {
            let mask = mask.crop_rows(range.start, range.end);
            resolve_masked_into(&filter, &sub, rng, mask, target, scratch)?;
        }
    }
    let base = match &*target {
        ResolvedStep::Masked { step, .. } => &**step,
        other => other,
    };
    if rows.is_some() && !base.supports_rows() {
        return Err(RecipeError::InvalidSpec(format!(
            "`rows` is not supported for this step: {base:?}"
        )));
    }
    Ok(())
}

/// The masked part of [`resolve_targeted_into`]: `mask` is already cropped to the rows of `sub`.
fn resolve_masked_into(
    filter: &Builtin,
    sub: &ImageHeader,
    rng: &mut ChaCha8Rng,
    mask: Mask,
    target: &mut ResolvedStep,
    scratch: &mut StepScratch,
) -> Result<()> {
    if let ResolvedStep::Masked {
        step,
        mask: old,
        effect,
    } = target
        && *old == mask
        && let Some(masked) = Arc::get_mut(&mut effect.0)
    {
        // The effect's copy first, from a copy of the RNG; then the `step` field, from the RNG
        // itself, which leaves it where `resolve` would.
        let inner = scratch.masked.get_or_insert_with(Box::default);
        filter.resolve_into(sub, &mut rng.clone(), &mut masked.effect_mut().0, inner);
        filter.resolve_into(sub, rng, step, scratch);
        if !step.supports_mask() {
            return Err(step.mask_unsupported());
        }
        return Ok(());
    }
    *target = filter
        .resolve(sub, rng)
        .masked(mask, sub.line_size() + 1, sub.height)?;
    Ok(())
}

/// A [`GlitchFilter`] that runs a built-in step with a `rows` / `rect` / `mask`.
pub(crate) struct EntryFilter {
    entry: FilterEntry,
    masks: MaskSet,
}

impl GlitchFilter for EntryFilter {
    fn apply(&self, png: &mut PngGlitch, rng: &mut dyn Rng) {
        let header = ImageHeader::of(png);
        // The entry was checked when the filter was made; `Err` cannot happen here.
        if let Ok(step) = resolve_entry(&self.entry, &header, rng, &self.masks) {
            step.execute(png);
        }
    }
}

/// Checks that `entry` can run, and returns a filter that does it. `masks` must hold the mask
/// the entry refers to.
pub(crate) fn entry_filter(entry: FilterEntry, masks: &MaskSet) -> Result<EntryFilter> {
    // Any header will do: only the kind of step and the mask lookup are checked.
    let probe = ImageHeader {
        width: 8,
        height: 8,
        color_type: ColorType::TrueColorAlpha,
        bit_depth: 8,
    };
    let mut rng = ChaCha8Rng::seed_from_u64(0);
    resolve_entry(&entry, &probe, &mut rng, masks)?;
    let mut own = MaskSet::new();
    if let Some(reference) = &entry.mask
        && let Some(mask) = masks.get(&reference.path)
    {
        own.insert(reference.path.clone(), mask.clone(), None);
    }
    Ok(EntryFilter { entry, masks: own })
}

/// A user-defined recipe step: draws its random choices from the RNG and returns a
/// deterministic `png-glitch` effect.
///
/// Closures `Fn(&ImageHeader, &mut dyn Rng) -> ResolvedEffect` implement this trait, which is
/// handy for quick experiments. Add a step with [`Recipe::then_step`].
pub trait RecipeStep: Send + Sync {
    /// Draws the choices for an image with the given header and returns the effect to run.
    fn resolve(&self, header: &ImageHeader, rng: &mut dyn Rng) -> ResolvedEffect;
}

impl<F> RecipeStep for F
where
    F: Fn(&ImageHeader, &mut dyn Rng) -> ResolvedEffect + Send + Sync,
{
    fn resolve(&self, header: &ImageHeader, rng: &mut dyn Rng) -> ResolvedEffect {
        self(header, rng)
    }
}

/// A step that always runs the same effect.
struct FixedStep(ResolvedEffect);

impl RecipeStep for FixedStep {
    fn resolve(&self, _header: &ImageHeader, _rng: &mut dyn Rng) -> ResolvedEffect {
        self.0.clone()
    }
}

/// A step of a [`Recipe`]: a built-in step described by a [`FilterConfig`], or a user-defined one.
#[derive(Clone)]
pub enum Step {
    /// A built-in step.
    Config(FilterConfig),
    /// A built-in step restricted to a range of rows (`rows: a..b` in a recipe file).
    Rows(FilterConfig, RowRange),
    /// A built-in step with a `rect` / `mask` (and maybe `rows`): see [`FilterEntry`].
    Targeted(Box<FilterEntry>),
    /// A user-defined step.
    Custom(Arc<dyn RecipeStep>),
}

impl fmt::Debug for Step {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Config(config) => f.debug_tuple("Config").field(config).finish(),
            Self::Rows(config, rows) => f.debug_tuple("Rows").field(config).field(rows).finish(),
            Self::Targeted(entry) => f.debug_tuple("Targeted").field(entry).finish(),
            Self::Custom(_) => f.write_str("Custom(..)"),
        }
    }
}

impl PartialEq for Step {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Config(a), Self::Config(b)) => a == b,
            (Self::Rows(a, ra), Self::Rows(b, rb)) => a == b && ra == rb,
            (Self::Targeted(a), Self::Targeted(b)) => a == b,
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
    /// Recomputes the filtered values with a strategy (`PngGlitch::regenerate`). An empty
    /// `Pattern` does nothing when executed.
    Regenerate(png_glitch::FilterStrategy),
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
    /// A step restricted to the rows `start..end` of the image. The inner step was resolved for
    /// an image as tall as the range, so its row numbers are relative to `start`.
    Rows {
        /// First row.
        start: u32,
        /// One past the last row.
        end: u32,
        /// The step, with row numbers relative to `start`.
        step: Box<ResolvedStep>,
    },
    /// A step whose result is kept only where a mask allows it (ADR 0006).
    Masked {
        /// The step, resolved as for an unmasked run.
        step: Box<ResolvedStep>,
        /// The mask, for the rows the step covers.
        mask: Mask,
        /// The wrapper that runs `step` through `mask`; it owns the snapshot buffer.
        effect: MaskedEffect,
    },
    /// A user-defined effect.
    Custom(ResolvedEffect),
}

/// The effect that runs a [`ResolvedStep::Masked`] step. Two values compare equal: the step and
/// the mask next to it already say everything.
#[derive(Clone)]
pub struct MaskedEffect(Arc<Masked<StepEffect>>);

impl fmt::Debug for MaskedEffect {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("MaskedEffect(..)")
    }
}

impl PartialEq for MaskedEffect {
    fn eq(&self, _: &Self) -> bool {
        true
    }
}

/// A resolved step viewed as a [`ScanLinesEffect`], so it can be wrapped in [`Masked`].
#[derive(Debug)]
pub struct StepEffect(ResolvedStep);

impl ScanLinesEffect for StepEffect {
    fn apply_to_scan_lines(&self, lines: &mut ScanLines<'_>) {
        // `masked` only wraps steps that run on a scan-line view.
        let _ = self.0.execute_live(lines);
    }
}

impl ResolvedStep {
    /// Applies the step to the image. Deterministic; uses no RNG.
    pub fn execute(&self, png: &mut PngGlitch) {
        match self {
            Self::Noop => {}
            Self::Refilter(e) => e.apply_to(png),
            Self::Regenerate(strategy) => {
                // Fails only for an empty pattern, and then changes nothing.
                let _ = png.regenerate(strategy);
            }
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
            Self::Rows { start, end, step } => match &**step {
                Self::Refilter(r) => fx::Refilter::new(r.filter_type)
                    .within_rows(*start..*end)
                    .apply_to(png),
                inner => {
                    // `in_rows` only builds steps that run on a scan-line view.
                    let mut lines = png.scan_lines_mut();
                    let _ = inner.execute_live(&mut lines.rows(*start..*end));
                }
            },
            Self::Masked { effect, .. } => effect.0.apply_to(png),
            Self::Custom(e) => e.apply_to(png),
        }
    }

    /// Keeps the result of a step only where `mask` allows it. The step was resolved for an image
    /// with `height` rows of `stride` bytes (the filter-type tag plus the filtered bytes), which
    /// are the rows `mask` covers.
    ///
    /// # Errors
    /// [`RecipeError::InvalidSpec`] for `regenerate` and the re-encodes (`sub-filter`, ...),
    /// which recompute filtered values and cannot be masked, and for a user-defined effect that
    /// is not a `ScanLinesEffect`.
    pub fn masked(self, mask: Mask, stride: usize, height: u32) -> Result<ResolvedStep> {
        if !self.supports_mask() {
            return Err(self.mask_unsupported());
        }
        let effect = MaskedEffect(Arc::new(Masked::new(
            StepEffect(self.clone()),
            mask.clone(),
            stride,
            height,
        )));
        Ok(Self::Masked {
            step: Box::new(self),
            mask,
            effect,
        })
    }

    /// Restricts a step (resolved for an image as tall as the range) to the rows `start..end`.
    ///
    /// # Errors
    /// [`RecipeError::InvalidSpec`] for `regenerate`, which recomputes the whole image, and
    /// for a user-defined effect that is not a `ScanLinesEffect`.
    pub fn in_rows(self, start: u32, end: u32) -> Result<ResolvedStep> {
        if !self.supports_rows() {
            return Err(RecipeError::InvalidSpec(format!(
                "`rows` is not supported for this step: {self:?}"
            )));
        }
        Ok(Self::Rows {
            start,
            end,
            step: Box::new(self),
        })
    }

    /// Whether [`masked`](Self::masked) accepts this step.
    fn supports_mask(&self) -> bool {
        match self {
            Self::Regenerate(_) | Self::Refilter(_) => false,
            Self::Custom(e) => e.is_live(),
            _ => true,
        }
    }

    fn mask_unsupported(&self) -> RecipeError {
        RecipeError::InvalidSpec(format!(
            "`rect` / `mask` is not supported for this step (it recomputes filtered values): {self:?}"
        ))
    }

    /// Whether [`in_rows`](Self::in_rows) accepts this step.
    fn supports_rows(&self) -> bool {
        match self {
            Self::Regenerate(_) => false,
            Self::Custom(e) => e.is_live(),
            _ => true,
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
            Self::Refilter(_) | Self::Regenerate(_) => return Err(self.not_live()),
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
            Self::Rows { start, end, step } => {
                if matches!(**step, Self::Refilter(_)) {
                    return Err(self.not_live());
                }
                step.execute_live(&mut lines.rows(*start..*end))?;
            }
            Self::Masked { effect, .. } => effect.0.apply_to_scan_lines(lines),
            Self::Custom(e) => match &e.0 {
                EffectKind::Live(e) => e.apply_to_scan_lines(lines),
                EffectKind::File(_) => return Err(self.not_live()),
            },
        }
        Ok(())
    }

    /// Runs the step as a generation boundary if it is one (`Refilter`, `Regenerate`), else
    /// returns `None`.
    pub(crate) fn run_boundary(&self, live: &mut png_glitch::LivePipeline) -> Option<Result<()>> {
        let result = match self {
            Self::Refilter(r) => live.regenerate(&png_glitch::FilterStrategy::Fixed(r.filter_type)),
            Self::Regenerate(strategy) => live.regenerate(strategy),
            _ => return None,
        };
        Some(result.map_err(RecipeError::Encode))
    }

    /// Whether the live runner can run this step (as a scan-line effect or a boundary).
    pub(crate) fn is_live(&self) -> bool {
        match self {
            Self::Rows { step, .. } => !matches!(**step, Self::Refilter(_)) && step.is_live(),
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
    fn resolve(&self, header: &ImageHeader, rng: &mut dyn Rng) -> ResolvedStep;

    /// Writes into `out` exactly what [`resolve`](Self::resolve) returns, with the same RNG
    /// draws in the same order, but reuses the buffers already in `out` (and `scratch`) when
    /// `out` holds the same kind of step. Capacity grows to the largest size seen, then stays.
    ///
    /// The default allocates like `resolve`; it is enough for steps without lists.
    fn resolve_into(
        &self,
        header: &ImageHeader,
        rng: &mut dyn Rng,
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
    /// The scratch of the second copy of a masked step (see [`resolve_targeted_into`]).
    masked: Option<Box<StepScratch>>,
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
        #[derive(Clone, Debug)]
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
            fn resolve(&self, header: &ImageHeader, rng: &mut dyn Rng) -> ResolvedStep {
                match self {
                    $(Self::$name(f) => f.resolve(header, rng),)*
                }
            }

            fn resolve_into(
                &self,
                header: &ImageHeader,
                rng: &mut dyn Rng,
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
    Bitwise, BlockScramble, ChangeFilterType, ChannelSwap, ChromaticAberration, ColorDistortion,
    ColorSpaceGlitch, HorizontalShift, PixelSort, RandomCopy, Regenerate, Replace, SetFilterType,
    SetZero, Substitute, Transpose, filters::Reencode,
};
use png_glitch::effects::{Brighten, Darken, Invert, ShiftChannels};

builtin_filters!(
    ChangeFilterType,
    SetFilterType,
    Regenerate,
    Reencode,
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
    fn apply(&self, png: &mut PngGlitch, rng: &mut dyn Rng) {
        let header = ImageHeader::of(png);
        self.resolve(&header, rng).execute(png);
    }
}

macro_rules! impl_glitch_filter_via_resolve {
    ($($t:ty),* $(,)?) => {
        $(
            impl GlitchFilter for $t {
                fn apply(&self, png: &mut PngGlitch, rng: &mut dyn Rng) {
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
    masks: MaskSet,
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
            masks: MaskSet::new(),
        }
    }

    /// Builds a recipe from entries, each with an optional row range.
    pub fn from_entries(entries: impl IntoIterator<Item = FilterEntry>) -> Self {
        Self {
            steps: entries
                .into_iter()
                .map(|e| {
                    if e.is_plain() {
                        Step::Config(e.config)
                    } else if e.rows.is_some() && !e.has_region() && !e.invert_mask {
                        Step::Rows(e.config, e.rows.unwrap_or_default())
                    } else {
                        Step::Targeted(Box::new(e))
                    }
                })
                .collect(),
            masks: MaskSet::new(),
        }
    }

    /// Supplies the masks that steps with a `mask` refer to, by `path`. The recipe crate reads
    /// no files; the host loads them. A step whose mask is missing fails to resolve with
    /// [`RecipeError::MissingMask`], and one whose CRC-32 differs with
    /// [`RecipeError::MaskChecksum`].
    pub fn with_masks(mut self, masks: MaskSet) -> Self {
        self.masks = masks;
        self
    }

    /// Appends a built-in step with a `rows` / `rect` / `mask`. See [`FilterEntry`].
    pub fn then_entry(mut self, entry: FilterEntry) -> Self {
        self.steps.push(Step::Targeted(Box::new(entry)));
        self
    }

    /// Appends a built-in step that works on the rows `rows` only.
    ///
    /// The step is resolved for an image as tall as the range, so its random choices and row
    /// numbers are relative to the range. [`resolve`](Self::resolve) fails for `regenerate`,
    /// which recomputes the whole image.
    pub fn then_rows(mut self, step: FilterConfig, rows: RowRange) -> Self {
        self.steps.push(Step::Rows(step, rows));
        self
    }

    /// The steps as [`FilterEntry`] values, or `None` if the recipe has a user-defined step.
    pub fn entries(&self) -> Option<Vec<FilterEntry>> {
        self.steps
            .iter()
            .map(|step| match step {
                Step::Config(config) => Some(config.clone().into()),
                Step::Rows(config, rows) => Some(FilterEntry::with_rows(config.clone(), *rows)),
                Step::Targeted(entry) => Some((**entry).clone()),
                Step::Custom(_) => None,
            })
            .collect()
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
                Step::Rows(..) | Step::Targeted(_) | Step::Custom(_) => None,
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
                Step::Rows(config, rows) => resolve_entry(
                    &FilterEntry::with_rows(config.clone(), *rows),
                    header,
                    &mut rng,
                    &self.masks,
                ),
                Step::Targeted(entry) => resolve_entry(entry, header, &mut rng, &self.masks),
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
    /// Steps targeted with `rows` and `rect` (and `invert-mask`) reuse their buffers too (#207);
    /// a masked step reuses its snapshot buffer unless `out` was cloned, which shares it. A step
    /// with a `mask:` image copies the mask on every call.
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
                Step::Rows(config, rows) => resolve_targeted_into(
                    config,
                    Some(*rows),
                    None,
                    header,
                    &mut rng,
                    slot,
                    scratch,
                )?,
                // `entry_mask` allocates for a `mask:` image (it copies the mask); a `rect:` mask
                // is a few numbers.
                Step::Targeted(entry) => resolve_targeted_into(
                    &entry.config,
                    entry.rows,
                    entry_mask(entry, header, &self.masks)?,
                    header,
                    &mut rng,
                    slot,
                    scratch,
                )?,
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
    filters: Vec<FilterEntry>,
}

impl serde::Serialize for Recipe {
    /// Serializes as `filters: [...]`, with `rows`, `rect`, `mask` and `invert-mask` where a
    /// step has them (the supplied [`MaskSet`] is not part of it). Fails if the recipe has a
    /// user-defined step.
    fn serialize<S: serde::Serializer>(
        &self,
        serializer: S,
    ) -> std::result::Result<S::Ok, S::Error> {
        let filters = self.entries().ok_or_else(|| {
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
        Ok(Recipe::from_entries(data.filters))
    }
}
