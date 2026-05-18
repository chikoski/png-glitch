use crate::WebpGlitch;
use rand_chacha::ChaCha8Rng;

/// WebP グリッチフィルターのトレイト。
/// `WebpGlitchContext::add_filter()` に渡して使用する。
pub trait WebpGlitchFilter: Send + Sync {
    fn apply(&self, webp: &mut WebpGlitch, rng: &mut ChaCha8Rng);
}
