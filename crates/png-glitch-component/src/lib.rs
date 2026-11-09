//! png-glitch as a WebAssembly component.
//!
//! The logic is plain Rust ([`list_filters`], [`validate_recipe`], [`glitch`]) and is tested
//! natively. On `wasm32` the `component` module binds it to the WIT world in `wit/world.wit`.
//! Nothing here reads the clock, the file system or OS randomness: the seed is a parameter.
#![warn(missing_docs)]
#![deny(unsafe_code)]
#![cfg_attr(
    not(test),
    deny(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::indexing_slicing
    )
)]

use png_glitch_recipe::{
    FilterInfo, GlitchContext, MaskSet, ParseOptions, PngError, RecipeError, RecipeFile,
};

mod session;
pub use png_glitch::{ColorType, Encoding, FilterStrategy, FilterType};
pub use png_glitch_recipe::SeedMode;
pub use session::Session;

#[cfg(target_arch = "wasm32")]
#[allow(unsafe_code, missing_docs, clippy::all)]
mod component;

/// Largest recipe text accepted, in bytes.
pub const MAX_RECIPE_BYTES: usize = 1 << 20;

/// Why a call failed (the WIT `glitch-error`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GlitchError {
    /// A PNG signature, but not a decodable PNG.
    InvalidImage(String),
    /// Not a PNG (WebP or an unknown signature).
    UnsupportedFormat,
    /// The recipe does not parse or fails validation.
    InvalidRecipe(String),
    /// The image exceeds the decode limits.
    TooLarge,
    /// `apply` without a recipe.
    NoRecipe,
    /// An encoding other than 8-bit gray, gray-alpha, rgb or rgba.
    UnsupportedEncoding,
}

impl std::fmt::Display for GlitchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidImage(m) => write!(f, "invalid image: {m}"),
            Self::UnsupportedFormat => f.write_str("unsupported format: only PNG is supported"),
            Self::InvalidRecipe(m) => write!(f, "invalid recipe: {m}"),
            Self::TooLarge => f.write_str("the image exceeds the decode limits"),
            Self::NoRecipe => f.write_str("no recipe is set (call set-recipe first)"),
            Self::UnsupportedEncoding => {
                f.write_str("unsupported encoding: only 8-bit gray, gray-alpha, rgb and rgba")
            }
        }
    }
}

impl std::error::Error for GlitchError {}

/// Every built-in filter (the same table as `png-glitch --list-filters --json`).
pub fn list_filters() -> &'static [FilterInfo] {
    png_glitch_recipe::filters()
}

pub(crate) fn parse_recipe(recipe: &str) -> Result<RecipeFile, GlitchError> {
    if recipe.len() > MAX_RECIPE_BYTES {
        return Err(GlitchError::InvalidRecipe(format!(
            "the recipe is {} bytes; the limit is {MAX_RECIPE_BYTES}",
            recipe.len()
        )));
    }
    let file = RecipeFile::from_yaml(recipe).map_err(recipe_error)?;
    if file.filters.iter().any(|f| f.mask.is_some()) {
        return Err(GlitchError::InvalidRecipe(
            "mask steps are not supported in the component".into(),
        ));
    }
    Ok(file)
}

pub(crate) fn recipe_error(e: RecipeError) -> GlitchError {
    match e {
        RecipeError::Parse(p) => image_error(p),
        other => GlitchError::InvalidRecipe(other.to_string()),
    }
}

pub(crate) fn image_error(e: PngError) -> GlitchError {
    match e {
        PngError::InvalidSignature => GlitchError::UnsupportedFormat,
        PngError::ImageTooLarge { .. } => GlitchError::TooLarge,
        PngError::UnsupportedEncoding { .. } | PngError::EmptyFilterPattern => {
            GlitchError::UnsupportedEncoding
        }
        other => GlitchError::InvalidImage(other.to_string()),
    }
}

/// Checks that `recipe` parses and every step is valid, without an image.
pub fn validate_recipe(recipe: &str) -> Result<(), GlitchError> {
    let file = parse_recipe(recipe)?;
    // Build every step against a throwaway context to run the parameter validation.
    let mut ctx = GlitchContext::new(PROBE_PNG, Some(0)).map_err(recipe_error)?;
    for entry in file.filters {
        ctx.add_from_entry(entry, &MaskSet::new())
            .map_err(recipe_error)?;
    }
    Ok(())
}

/// Applies `recipe` to the PNG `image` with `seed`; the same inputs give the same bytes.
pub fn glitch(image: &[u8], recipe: &str, seed: u64) -> Result<Vec<u8>, GlitchError> {
    let file = parse_recipe(recipe)?;
    // Parsing enforces the default `Limits` before any pixel buffer is allocated.
    let mut ctx = GlitchContext::new_with_options(image, Some(seed), ParseOptions::default())
        .map_err(|e| match e {
            RecipeError::Parse(p) => image_error(p),
            other => recipe_error(other),
        })?;
    for entry in file.filters {
        ctx.add_from_entry(entry, &MaskSet::new())
            .map_err(recipe_error)?;
    }
    ctx.execute();
    ctx.buffer()
        .map_err(|e| GlitchError::InvalidImage(e.to_string()))
}

/// A 2x2 RGBA PNG used to validate recipes without an image.
const PROBE_PNG: &[u8] = include_bytes!("probe.png");
