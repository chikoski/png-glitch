use crate::WebpGlitch;
use rand::Rng;

/// Trait for WebP glitch filters (effects).
///
/// Pass implementations to `WebpGlitchContext::add_filter()` or `AnyGlitchContext`.
pub trait WebpGlitchFilter: Send + Sync {
    /// Applies the filter to the [`WebpGlitch`] instance using the provided RNG.
    fn apply(&self, webp: &mut WebpGlitch, rng: &mut dyn Rng);
}
