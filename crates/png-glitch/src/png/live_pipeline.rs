//! `LivePipeline`: the per-frame loop `D(G(E(frame)))` as one owner (ADR 0005, decision 1).

use super::{Encoding, FilterStrategy, PixelSource, PngError, Renderer, ScanLines};
use crate::PngGlitch;

/// A live frame loop over one model: encode pixels into scan lines (`E`), glitch the scan lines
/// (`G`), decode them back to pixels (`D`), and, off the real-time path, serialize a snapshot.
///
/// It is a thin owner of what already exists: a model made by [`PngGlitch::from_pixels`], a
/// [`Renderer`], the [`Encoding`] and a scratch buffer. [`new`](LivePipeline::new) allocates
/// once; [`encode`](LivePipeline::encode), taking the view and
/// [`decode_into`](LivePipeline::decode_into) make **no heap allocation** in steady state.
///
/// The model is not handed out mutably, so `G` can only use the [`ScanLines`] view and cannot
/// recompute filtered values.
///
/// # The laws
///
/// 1. `decode_into(encode(x)) == x`, after the colour conversion documented on [`Encoding`]
///    (alpha 255 for `TrueColor` and `GrayScale`, the gray formula for the gray types).
/// 2. `decode_into` equals [`Renderer::render_into`] on the same model, hence a PNG decoder
///    reading [`snapshot`](LivePipeline::snapshot).
/// 3. `snapshot()` is a valid PNG and equals `model().to_bytes()`.
///
/// # Examples
/// The per-frame loop:
/// ```
/// use png_glitch::{Encoding, FilterStrategy, FilterType, LivePipeline, PixelSource};
///
/// let (w, h) = (16u32, 8u32);
/// let first = vec![0u8; (w * h * 4) as usize];
/// let mut live = LivePipeline::new(
///     PixelSource::rgba8(w, h, &first),
///     Encoding::rgba8(FilterStrategy::Fixed(FilterType::Paeth)),
/// )?; // allocates once
///
/// let mut out = vec![0u8; first.len()];
/// for frame in [&first, &first] {
///     let mut lines = live.encode(frame)?; // E: pixels -> scan lines
///     lines.swap(0, 1); // G: only scan-line operations are possible here
///     live.decode_into(&mut out)?; // D: scan lines -> pixels
/// }
/// let png_bytes = live.snapshot()?; // serialize; allocates, keep it off the real-time path
/// assert!(png_bytes.starts_with(b"\x89PNG"));
/// # Ok::<(), png_glitch::PngError>(())
/// ```
pub struct LivePipeline {
    model: PngGlitch,
    renderer: Renderer,
    encoding: Option<Encoding>,
}

impl LivePipeline {
    /// Builds the model from the first frame and allocates everything the loop needs.
    ///
    /// # Errors
    /// The errors of [`PngGlitch::from_pixels`].
    pub fn new(src: PixelSource<'_>, encoding: Encoding) -> Result<LivePipeline, PngError> {
        let model = PngGlitch::from_pixels(src, encoding.clone())?;
        let renderer = Renderer::new(&model);
        Ok(LivePipeline {
            model,
            renderer,
            encoding: Some(encoding),
        })
    }

    /// Wraps a model opened from a PNG file or bytes, so the live loop (`regenerate`,
    /// `decode_into`, `snapshot`, and `LiveRecipe::run` in `png-glitch-recipe`) works on a real
    /// file too. Allocates once, for the [`Renderer`]. [`encode`](Self::encode) is an error
    /// ([`PngError::NotCreatedFromPixels`]) on such a pipeline; the model has no [`Encoding`].
    pub fn from_png(model: PngGlitch) -> LivePipeline {
        let renderer = Renderer::new(&model);
        LivePipeline {
            model,
            renderer,
            encoding: None,
        }
    }

    /// Restores the scan lines from `clean` without allocating; see
    /// [`PngGlitch::copy_data_from`]. The frame counter of any `LiveRecipe` is not affected.
    ///
    /// # Errors
    /// [`PngError::HeaderMismatch`].
    pub fn restore_from(&mut self, clean: &PngGlitch) -> Result<(), PngError> {
        self.model.copy_data_from(clean)
    }

    /// `E`: refills the scan lines from the next RGBA8 frame and lends them for glitching.
    ///
    /// It calls [`PngGlitch::reload_pixels`], so **any glitch applied to the previous frame is
    /// overwritten**. It makes no heap allocation.
    ///
    /// # Errors
    /// [`PngError::PixelDataLength`] if `rgba` is not `width * height * 4` bytes.
    pub fn encode(&mut self, rgba: &[u8]) -> Result<ScanLines<'_>, PngError> {
        self.model.reload_pixels(rgba)?;
        Ok(self.model.scan_lines_mut())
    }

    /// The scan lines of the current frame **without reloading** them, so a glitch can be
    /// applied in several steps, or again after [`decode_into`](Self::decode_into).
    pub fn scan_lines(&mut self) -> ScanLines<'_> {
        self.model.scan_lines_mut()
    }

    /// `D`: reconstructs the current scan lines into `out` as RGBA8 with
    /// [`Renderer::render_into`]. It makes no heap allocation and leaves the scan lines
    /// unchanged.
    ///
    /// # Errors
    /// The errors of [`Renderer::render_into`].
    pub fn decode_into(&mut self, out: &mut [u8]) -> Result<(), PngError> {
        self.renderer.render_into(&self.model, out)
    }

    /// An explicit generation boundary: [`PngGlitch::regenerate`] on the model. Removes the
    /// filters (`D`) and applies `strategy` (`E`), so glitches so far are baked into the pixels.
    /// It does not count as a frame, and the pipeline's [`encoding`](Self::encoding) is
    /// unchanged: the next [`encode`](Self::encode) still uses it.
    ///
    /// # Errors
    /// [`PngError::EmptyFilterPattern`] for an empty `Pattern`.
    pub fn regenerate(&mut self, strategy: &FilterStrategy) -> Result<(), PngError> {
        self.model.regenerate(strategy).map(|_| ())
    }

    /// Serializes the current scan lines as a PNG (compression and checksums run here; it
    /// allocates). Equal to `model().to_bytes()`.
    pub fn snapshot(&self) -> Result<Vec<u8>, PngError> {
        self.model.to_bytes()
    }

    /// The model, read only.
    pub fn model(&self) -> &PngGlitch {
        &self.model
    }

    /// The encoding the pipeline was created with; `None` for one made by
    /// [`from_png`](Self::from_png).
    pub fn encoding(&self) -> Option<&Encoding> {
        self.encoding.as_ref()
    }
}
