//! Running a [`Recipe`] in a live loop (ADR 0005, decisions 3 and 4).
//!
//! [`LiveRecipe`] drives a [`LivePipeline`]: after `live.encode(frame)` (`E`) and before
//! `live.decode_into(out)` (`D`), `run` executes the recipe's steps on the frame's scan lines
//! (`G`). A step that resolves to [`Refilter`](png_glitch::effects::Refilter) (`RemoveFilter`,
//! `SubFilter`, ...) runs as `regenerate(Fixed(t))`, a generation boundary, so a recipe gives
//! the same bytes as in the file workflow.

use crate::{ImageHeader, Recipe, RecipeError, ResolvedRecipe, Result};
use png_glitch::{FilterStrategy, LivePipeline};
use rand::RngCore;

/// The `frame`-th output (0-based) of SplitMix64 started at `seed`: the seed of one frame in
/// [`SeedMode::PerFrame`].
///
/// This function is part of the reproducibility guarantee (#58): the same `(seed, frame)`
/// always gives the same value, on every platform and version. Changing it is a breaking change
/// of the output and needs a new ADR. Hosts outside Rust can recompute it.
///
/// ```
/// use png_glitch_recipe::frame_seed;
/// assert_eq!(frame_seed(0, 0), 0xE220A8397B1DCDAF);
/// assert_eq!(frame_seed(0, 1), 0x6E789E6AA1B965F4);
/// assert_eq!(frame_seed(42, 0), 0xBDD732262FEB6E95);
/// assert_eq!(frame_seed(42, 29), 0xB1B59F1B53585E43);
/// ```
pub const fn frame_seed(seed: u64, frame: u64) -> u64 {
    let mut z = seed.wrapping_add(frame.wrapping_add(1).wrapping_mul(0x9E37_79B9_7F4A_7C15));
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// How a [`LiveRecipe`] seeds each frame. Record it with any snapshot or recording.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum SeedMode {
    /// Resolve once with the seed and execute the same [`ResolvedRecipe`] on every frame.
    /// A frame makes no heap allocation.
    Static,
    /// Before frame `n`, resolve with [`frame_seed`]`(seed, n)`. Resolving allocates.
    PerFrame,
}

/// A [`Recipe`] running in a live loop. See the module documentation.
///
/// The frame counter starts at 0 and counts the frames given to [`run`](Self::run), not the
/// frames captured.
///
/// ```
/// use png_glitch::{Encoding, FilterStrategy, FilterType, LivePipeline, PixelSource};
/// use png_glitch_recipe::{FilterConfig, ImageHeader, LiveRecipe, Recipe, SeedMode};
///
/// let (w, h) = (16u32, 16u32);
/// let first = vec![128u8; (w * h * 4) as usize];
/// let mut live = LivePipeline::new(
///     PixelSource::rgba8(w, h, &first),
///     Encoding::rgba8(FilterStrategy::Fixed(FilterType::Paeth)),
/// )?;
/// let recipe = Recipe::new()
///     .then(FilterConfig::Transpose { magnitude: 0.3 })
///     .then(FilterConfig::Invert);
/// let header = ImageHeader::of(live.model());
/// let mut runner = LiveRecipe::new(recipe, header, Some(42), SeedMode::PerFrame)?;
///
/// let mut out = vec![0u8; first.len()];
/// for frame in [&first, &first] {
///     live.encode(frame)?; // E
///     runner.run(&mut live)?; // G
///     live.decode_into(&mut out)?; // D
/// }
/// assert_eq!(runner.frame(), 2);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Debug)]
pub struct LiveRecipe {
    recipe: Recipe,
    header: ImageHeader,
    seed: u64,
    mode: SeedMode,
    frame: u64,
    current: ResolvedRecipe,
    /// The frame `current` was resolved for.
    current_frame: u64,
}

impl LiveRecipe {
    /// Builds the runner and checks the recipe.
    ///
    /// With `seed: None`, one OS seed is drawn now and kept (see [`seed`](Self::seed)); it is
    /// never drawn per frame. That needs the `os-rng` feature; without it, `None` is
    /// [`RecipeError::SeedRequired`].
    ///
    /// The recipe is resolved once (for [`SeedMode::PerFrame`], with `frame_seed(seed, 0)`) and
    /// every step must be able to run live. Otherwise this returns [`RecipeError::NotLive`]
    /// (a user-defined effect that is not a `ScanLinesEffect`). In `PerFrame` mode a user
    /// [`RecipeStep`](crate::RecipeStep) could still return such an effect on a later frame;
    /// then [`run`](Self::run) returns the same error.
    pub fn new(
        recipe: Recipe,
        header: ImageHeader,
        seed: Option<u64>,
        mode: SeedMode,
    ) -> Result<Self> {
        let seed = match seed {
            Some(seed) => seed,
            None => crate::context::rng_from_seed(None)?.next_u64(),
        };
        let current = resolve_live(&recipe, &header, seed, mode, 0)?;
        Ok(Self {
            recipe,
            header,
            seed,
            mode,
            frame: 0,
            current,
            current_frame: 0,
        })
    }

    /// The seed in use: the given one, or the OS seed drawn when the runner was built.
    pub fn seed(&self) -> u64 {
        self.seed
    }

    /// The seed mode.
    pub fn mode(&self) -> SeedMode {
        self.mode
    }

    /// The number of frames run so far, which is also the 0-based index of the next frame.
    pub fn frame(&self) -> u64 {
        self.frame
    }

    /// The resolved recipe of the most recent [`run`](Self::run) (before the first run, of
    /// frame 0). A host records it, with [`seed`](Self::seed), [`mode`](Self::mode) and the
    /// frame number, to reproduce a snapshot.
    pub fn current(&self) -> &ResolvedRecipe {
        &self.current
    }

    /// `G`: runs the recipe on the current scan lines of `live`, then counts the frame. Call it
    /// after `live.encode(..)` and before `live.decode_into(..)`.
    ///
    /// Steps that resolve to `Refilter` run as `live.regenerate(Fixed(t))`. In
    /// [`SeedMode::Static`] this makes no heap allocation (apart from what the steps
    /// themselves need).
    ///
    /// # Errors
    /// [`RecipeError::HeaderMismatch`] if `live`'s image is not the one the runner was built for;
    /// [`RecipeError::NotLive`] as described in [`new`](Self::new). The frame counter is not
    /// advanced on error.
    pub fn run(&mut self, live: &mut LivePipeline) -> Result<()> {
        let actual = ImageHeader::of(live.model());
        if actual != self.header {
            return Err(RecipeError::HeaderMismatch {
                expected: self.header,
                actual,
            });
        }
        if self.mode == SeedMode::PerFrame && self.current_frame != self.frame {
            self.current =
                resolve_live(&self.recipe, &self.header, self.seed, self.mode, self.frame)?;
            self.current_frame = self.frame;
        }
        for step in self.current.steps() {
            match step.refilter_type() {
                Some(t) => live
                    .regenerate(&FilterStrategy::Fixed(t))
                    .map_err(RecipeError::Encode)?,
                None => step.execute_live(&mut live.scan_lines())?,
            }
        }
        self.frame += 1;
        Ok(())
    }
}

/// Resolves `recipe` for `frame` and checks that every step can run live.
fn resolve_live(
    recipe: &Recipe,
    header: &ImageHeader,
    seed: u64,
    mode: SeedMode,
    frame: u64,
) -> Result<ResolvedRecipe> {
    let frame_seed = match mode {
        SeedMode::Static => seed,
        SeedMode::PerFrame => frame_seed(seed, frame),
    };
    let resolved = recipe.resolve(header, Some(frame_seed))?;
    match resolved.steps().iter().find(|s| !s.is_live()) {
        Some(step) => Err(step.not_live()),
        None => Ok(resolved),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_seed_test_vectors() {
        assert_eq!(frame_seed(0, 0), 0xE220A8397B1DCDAF);
        assert_eq!(frame_seed(0, 1), 0x6E789E6AA1B965F4);
        assert_eq!(frame_seed(42, 0), 0xBDD732262FEB6E95);
        assert_eq!(frame_seed(42, 29), 0xB1B59F1B53585E43);
    }
}
