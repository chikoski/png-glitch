//! Error type of this crate.

use crate::*;

/// Result type used by the fallible APIs of this crate.
pub type Result<T> = std::result::Result<T, RecipeError>;

/// Errors returned while loading, configuring or applying a recipe.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum RecipeError {
    /// The image file could not be opened or parsed.
    #[error("failed to open '{}'", .path.display())]
    Open {
        /// Path of the file that failed to open.
        path: PathBuf,
        /// Underlying PNG error.
        #[source]
        source: PngError,
    },
    /// The in-memory image data could not be parsed as a PNG.
    #[error("Failed to parse PNG data")]
    Parse(#[source] PngError),
    /// The glitched image could not be encoded.
    #[error("Failed to encode PNG")]
    Encode(#[source] PngError),
    /// The glitched image could not be written to a file.
    #[error("failed to save '{}'", .path.display())]
    Save {
        /// Path of the file that failed to be written.
        path: PathBuf,
        /// Underlying PNG error.
        #[source]
        source: PngError,
    },
    /// A random specification (effect parameter) is out of range or otherwise invalid.
    #[error("{0}")]
    InvalidSpec(String),
    /// A recipe file (YAML or JSON) could not be parsed or failed validation.
    #[error("invalid recipe: {0}")]
    InvalidRecipe(String),
    /// A resolved recipe was executed on an image whose header differs from the one it was resolved for.
    #[error("resolved recipe does not match the image: expected {expected:?}, got {actual:?}")]
    HeaderMismatch {
        /// Header the recipe was resolved for.
        expected: ImageHeader,
        /// Header of the image it was executed on.
        actual: ImageHeader,
    },
    /// The effect exists but cannot be applied to PNG images.
    #[error("Filter `{effect}` is a WebP-only filter and is not supported for PNG images")]
    UnsupportedEffect {
        /// Debug representation of the rejected effect.
        effect: String,
    },
    /// No seed was given and this build has no OS random number generator.
    ///
    /// Enable the `os-rng` feature, or pass `Some(seed)`.
    #[error("a seed is required: this build has no OS random number generator (`os-rng` feature)")]
    SeedRequired,
    /// A step cannot run in a live loop: it is a user-defined effect that is not a
    /// `ScanLinesEffect` (wrap it with `ResolvedEffect::new_live`).
    #[error("step cannot run in a live loop (it is not a ScanLinesEffect): {step}")]
    NotLive {
        /// Debug representation of the rejected step.
        step: String,
    },
    /// A step names a mask file that the host did not supply.
    #[error("the mask '{path}' was not supplied (load it and add it with `MaskSet::insert`)")]
    MissingMask {
        /// The `path` the step refers to.
        path: String,
    },
    /// The mask the host supplied is not the one the recipe was written for.
    #[error("the mask '{path}' has CRC-32 {actual:08x}, but the recipe expects {expected:08x}")]
    MaskChecksum {
        /// The `path` the step refers to.
        path: String,
        /// The CRC-32 written in the recipe.
        expected: u32,
        /// The CRC-32 of the supplied file.
        actual: u32,
    },
}
