//! Format detection and dispatch between PNG and WebP.
//!
//! The CLI is the only place that needs both formats, so the format-agnostic wrapper lives here
//! rather than in `png-glitch-recipe` or `webp-glitch-context`.

use anyhow::{Context, Result};
use png_glitch_recipe::{FilterConfig, GlitchContext, ParseOptions, PngWarning};
use std::path::Path;
use webp_glitch_context::WebpGlitchContext;

/// Image format detected from magic bytes or file extension.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageFormat {
    /// Portable Network Graphics (`.png`).
    Png,
    /// WebP image format (`.webp`).
    Webp,
}

/// Detects the [`ImageFormat`] of an in-memory image buffer from its magic bytes.
///
/// - PNG: starts with `\x89PNG\r\n\x1a\n`
/// - WebP: starts with `RIFF` (bytes `0..4`) and `WEBP` (bytes `8..12`)
pub fn detect_format(data: &[u8]) -> Option<ImageFormat> {
    if data.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some(ImageFormat::Png)
    } else if data.len() >= 12 && &data[0..4] == b"RIFF" && &data[8..12] == b"WEBP" {
        Some(ImageFormat::Webp)
    } else {
        None
    }
}

/// Format-agnostic glitch context that dispatches to either [`GlitchContext`] (PNG)
/// or [`WebpGlitchContext`] (WebP).
pub enum AnyGlitchContext {
    /// PNG glitch context.
    Png(GlitchContext),
    /// WebP glitch context.
    Webp(WebpGlitchContext),
}

impl AnyGlitchContext {
    /// Opens an image file with custom PNG [`ParseOptions`].
    pub fn open_with_options(
        path: impl AsRef<Path>,
        seed: Option<u64>,
        options: ParseOptions,
    ) -> Result<Self> {
        let path = path.as_ref();
        let data =
            std::fs::read(path).with_context(|| format!("failed to open '{}'", path.display()))?;
        let format = detect_format(&data).or_else(|| {
            let ext = path.extension()?.to_str()?;
            if ext.eq_ignore_ascii_case("webp") {
                Some(ImageFormat::Webp)
            } else if ext.eq_ignore_ascii_case("png") {
                Some(ImageFormat::Png)
            } else {
                None
            }
        });
        match format {
            Some(ImageFormat::Png) | None => {
                let ctx = GlitchContext::new_with_options(&data, seed, options)
                    .with_context(|| format!("failed to open '{}'", path.display()))?;
                Ok(Self::Png(ctx))
            }
            Some(ImageFormat::Webp) => {
                let ctx = WebpGlitchContext::new(&data, seed)
                    .with_context(|| format!("failed to open '{}'", path.display()))?;
                Ok(Self::Webp(ctx))
            }
        }
    }

    /// Creates an [`AnyGlitchContext`] from an in-memory image buffer by detecting its magic bytes.
    #[cfg(test)]
    pub fn from_bytes(data: &[u8], seed: Option<u64>) -> Result<Self> {
        Self::from_bytes_with_options(data, seed, ParseOptions::default())
    }

    /// Creates an [`AnyGlitchContext`] from an in-memory image buffer with custom PNG [`ParseOptions`].
    #[cfg(test)]
    pub fn from_bytes_with_options(
        data: &[u8],
        seed: Option<u64>,
        options: ParseOptions,
    ) -> Result<Self> {
        match detect_format(data) {
            Some(ImageFormat::Png) => Ok(Self::Png(GlitchContext::new_with_options(
                data, seed, options,
            )?)),
            Some(ImageFormat::Webp) => Ok(Self::Webp(WebpGlitchContext::new(data, seed)?)),
            None => anyhow::bail!("unrecognized image format (expected PNG or WebP magic bytes)"),
        }
    }

    /// Returns any non-fatal PNG integrity warnings recorded when parsing in lenient mode.
    pub fn warnings(&self) -> &[PngWarning] {
        match self {
            Self::Png(ctx) => ctx.warnings(),
            Self::Webp(_) => &[],
        }
    }

    /// Returns the detected [`ImageFormat`] of this context.
    pub fn format(&self) -> ImageFormat {
        match self {
            Self::Png(_) => ImageFormat::Png,
            Self::Webp(_) => ImageFormat::Webp,
        }
    }

    /// Returns `true` if the underlying image is a WebP image.
    pub fn is_webp(&self) -> bool {
        self.format() == ImageFormat::Webp
    }

    /// Resets the internal RNG with the given `seed` (or a fresh OS seed if `None`).
    pub fn set_seed(&mut self, seed: Option<u64>) {
        match self {
            Self::Png(ctx) => ctx.set_seed(seed),
            Self::Webp(ctx) => ctx.set_seed(seed),
        }
    }

    /// Adds a filter from a [`FilterConfig`] definition, returning an error if the filter is not
    /// supported for the underlying image format.
    pub fn add_from_config(&mut self, config: FilterConfig) -> Result<()> {
        match self {
            Self::Png(ctx) => ctx.add_from_config(config).map_err(Into::into),
            Self::Webp(ctx) => ctx.add_from_config(config),
        }
    }

    /// Executes all queued filters and drains the filter queue.
    pub fn execute(&mut self) {
        match self {
            Self::Png(ctx) => ctx.execute(),
            Self::Webp(ctx) => ctx.execute(),
        }
    }

    /// Encodes the glitched image into a byte vector.
    pub fn buffer(&self) -> Result<Vec<u8>> {
        match self {
            Self::Png(ctx) => ctx.buffer().map_err(Into::into),
            Self::Webp(ctx) => ctx.buffer(),
        }
    }

    /// Saves the glitched image to `path`.
    pub fn save(&self, path: impl AsRef<Path>) -> Result<()> {
        match self {
            Self::Png(ctx) => ctx.save(path).map_err(Into::into),
            Self::Webp(ctx) => ctx.save(path),
        }
    }
}
