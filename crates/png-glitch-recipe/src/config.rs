//! Declarative filter configuration ([`FilterConfig`]) and its parameter types.

use crate::*;

macro_rules! bail_spec {
    ($($arg:tt)*) => {
        return Err(RecipeError::InvalidSpec(format!($($arg)*)))
    };
}

/// Maximum number of scanline copies allowed in a single [`RandomCopy`] invocation (`100_000`).
pub const MAX_RANDOM_COPY_TIMES: u32 = 100_000;

/// Builds the filter a [`FilterConfig`] step describes, validating its parameters.
pub(crate) fn filter_from_config(config: FilterConfig) -> Result<Box<dyn BuiltinFilter>> {
    Ok(match config {
        FilterConfig::ChangeFilterType { magnitude } => Box::new(ChangeFilterType::new(magnitude)),
        FilterConfig::Replace { magnitude } => Box::new(Replace::new(magnitude)),
        FilterConfig::Transpose { magnitude } => Box::new(Transpose::new(magnitude)),
        FilterConfig::SetZero { magnitude } => Box::new(SetZero::new(magnitude)),
        FilterConfig::RandomCopy { times } => {
            if times > MAX_RANDOM_COPY_TIMES {
                bail_spec!("random-copy `times` must be <= {MAX_RANDOM_COPY_TIMES}, got {times}");
            }
            Box::new(RandomCopy::new(times))
        }
        FilterConfig::Substitute { index, value } => Box::new(Substitute::new(index, value)),
        FilterConfig::PixelSort {
            magnitude,
            criterion,
        } => {
            let mut f = PixelSort::new(magnitude);
            if let Some(c) = criterion {
                f.criterion = c;
            }
            Box::new(f)
        }
        FilterConfig::Bitwise {
            magnitude,
            op,
            value,
        } => {
            let mut f = Bitwise::new(magnitude);
            if let Some(o) = op {
                f.op = o;
            }
            if let Some(v) = value {
                f.value = v;
            }
            Box::new(f)
        }
        FilterConfig::ChannelSwap { magnitude, target } => {
            let mut f = ChannelSwap::new(magnitude);
            if let Some(t) = target {
                f.target = t;
            }
            Box::new(f)
        }
        FilterConfig::HorizontalShift { magnitude } => Box::new(HorizontalShift::new(magnitude)),
        FilterConfig::BlockScramble {
            magnitude,
            block_size,
        } => {
            let mut f = BlockScramble::new(magnitude);
            if let Some(bs) = block_size {
                f.block_size = bs;
            }
            Box::new(f)
        }
        FilterConfig::ColorDistortion {
            magnitude,
            strength,
        } => {
            let mut f = ColorDistortion::new(magnitude);
            if let Some(s) = strength {
                if s < 0 {
                    bail_spec!("color-distortion `strength` must be >= 0, got {s}");
                }
                f.strength = s;
            }
            Box::new(f)
        }
        FilterConfig::ColorSpaceGlitch {
            magnitude,
            hue_shift,
            saturation_mult,
            lightness_mult,
        } => {
            let mut f = ColorSpaceGlitch::new(magnitude);
            if let Some(h) = hue_shift {
                if !h.is_finite() {
                    bail_spec!("color-space-glitch `hue_shift` must be a finite number, got {h}");
                }
                f.hue_shift = h;
            }
            if let Some(s) = saturation_mult {
                if !s.is_finite() || s < 0.0 {
                    bail_spec!(
                        "color-space-glitch `saturation_mult` must be a finite number >= 0.0, got {s}"
                    );
                }
                f.saturation_mult = s;
            }
            if let Some(l) = lightness_mult {
                if !l.is_finite() || l < 0.0 {
                    bail_spec!(
                        "color-space-glitch `lightness_mult` must be a finite number >= 0.0, got {l}"
                    );
                }
                f.lightness_mult = l;
            }
            Box::new(f)
        }
        FilterConfig::ChromaticAberration {
            magnitude,
            r_offset,
            g_offset,
            b_offset,
        } => {
            let mut f = ChromaticAberration::new(magnitude);
            if let Some(r) = r_offset {
                f.r_offset = r;
            }
            if let Some(g) = g_offset {
                f.g_offset = g;
            }
            if let Some(b) = b_offset {
                f.b_offset = b;
            }
            Box::new(f)
        }
        FilterConfig::Invert => Box::new(Invert),
        FilterConfig::Brighten { strength } => Box::new(Brighten::new(strength)),
        FilterConfig::Darken { strength } => Box::new(Darken::new(strength)),
        FilterConfig::ShiftChannels { r, g, b } => Box::new(ShiftChannels::new(r, g, b)),
        FilterConfig::RemoveFilter => Box::new(RemoveFilter),
        FilterConfig::SubFilter => Box::new(SubFilter),
        FilterConfig::UpFilter => Box::new(UpFilter),
        FilterConfig::AverageFilter => Box::new(AverageFilter),
        FilterConfig::PaethFilter => Box::new(PaethFilter),
        FilterConfig::MacroblockGlitch { .. }
        | FilterConfig::AlphaGlitch { .. }
        | FilterConfig::LossyArtifact { .. } => {
            return Err(RecipeError::UnsupportedEffect {
                effect: format!("{config:?}"),
            });
        }
    })
}

pub(crate) fn deserialize_magnitude<'de, D>(deserializer: D) -> std::result::Result<f64, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let val = f64::deserialize(deserializer)?;
    if (0.0..=1.0).contains(&val) {
        Ok(val)
    } else {
        Err(serde::de::Error::custom(format!(
            "magnitude must be between 0.0 and 1.0, got {val}"
        )))
    }
}

/// Pre-process options for re-encoding PNG scanlines with a uniform prediction filter.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[cfg_attr(feature = "clap", derive(ValueEnum))]
#[serde(rename_all = "kebab-case")]
pub enum PreProcess {
    /// Re-encodes all scanlines with [`FilterType::None`].
    #[default]
    #[serde(alias = "RemoveFilter", alias = "removefilter", alias = "none")]
    RemoveFilter,
    /// Re-encodes all scanlines with [`FilterType::Sub`].
    #[serde(alias = "SubFilter", alias = "subfilter", alias = "sub")]
    SubFilter,
    /// Re-encodes all scanlines with [`FilterType::Up`].
    #[serde(alias = "UpFilter", alias = "upfilter", alias = "up")]
    UpFilter,
    /// Re-encodes all scanlines with [`FilterType::Average`].
    #[serde(alias = "AverageFilter", alias = "averagefilter", alias = "average")]
    AverageFilter,
    /// Re-encodes all scanlines with [`FilterType::Paeth`].
    #[serde(alias = "PaethFilter", alias = "paethfilter", alias = "paeth")]
    PaethFilter,
}

/// Strategy for corrupting the alpha channel in `AlphaGlitch` (WebP only, in `webp-glitch-context`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize)]
#[cfg_attr(feature = "clap", derive(ValueEnum))]
#[serde(rename_all = "kebab-case")]
pub enum AlphaGlitchStrategy {
    /// Inverts each alpha byte (`255 - a`).
    #[default]
    #[serde(alias = "Invert")]
    Invert,
    /// Replaces each alpha byte with a random `u8`.
    #[serde(alias = "Randomize")]
    Randomize,
    /// Sets each alpha byte to `0` (fully transparent).
    #[serde(alias = "Zero")]
    Zero,
    /// Sets each alpha byte to `255` (fully opaque).
    #[serde(alias = "One")]
    One,
}

/// Declarative configuration for a single glitch filter step (used in YAML config files and `-f/--filter`).
#[derive(Debug, Deserialize, Serialize, Clone, PartialEq)]
#[serde(tag = "type", deny_unknown_fields)]
pub enum FilterConfig {
    /// Randomly overwrites the PNG filter type byte of scanlines.
    #[serde(alias = "change-filter-type", alias = "change_filter_type")]
    ChangeFilterType {
        /// Probability (`0.0..=1.0`) of changing each scanline's filter type byte.
        #[serde(deserialize_with = "deserialize_magnitude")]
        magnitude: f64,
    },
    /// Replaces random scanline bytes with random values.
    #[serde(alias = "replace")]
    Replace {
        /// Probability (`0.0..=1.0`) of replacing a byte in each scanline.
        #[serde(deserialize_with = "deserialize_magnitude")]
        magnitude: f64,
    },
    /// Swaps random scanlines.
    #[serde(
        alias = "transpose",
        alias = "random-transpose",
        alias = "RandomTranspose"
    )]
    Transpose {
        /// Probability (`0.0..=1.0`) of swapping each scanline with another random scanline.
        #[serde(deserialize_with = "deserialize_magnitude")]
        magnitude: f64,
    },
    /// Sets random scanline bytes to `0`.
    #[serde(alias = "set-zero", alias = "set_zero")]
    SetZero {
        /// Probability (`0.0..=1.0`) of zeroing a byte in each scanline.
        #[serde(deserialize_with = "deserialize_magnitude")]
        magnitude: f64,
    },
    /// Copies random scanlines onto other scanlines `times` times.
    #[serde(alias = "random-copy", alias = "random_copy")]
    RandomCopy {
        /// Number of random scanline copy operations to perform.
        times: u32,
    },
    /// Sets the byte at `index` to `value` on every scanline.
    #[serde(alias = "substitute")]
    Substitute {
        /// Byte offset within each scanline.
        index: usize,
        /// Replacement byte value.
        value: u8,
    },
    /// Sorts pixels within randomly selected scanlines.
    #[serde(alias = "pixel-sort", alias = "pixel_sort")]
    PixelSort {
        /// Probability (`0.0..=1.0`) of sorting each scanline.
        #[serde(deserialize_with = "deserialize_magnitude")]
        magnitude: f64,
        /// Optional sorting criterion (defaults to [`SortCriterion::Brightness`]).
        #[serde(skip_serializing_if = "Option::is_none")]
        criterion: Option<SortCriterion>,
    },
    /// Applies a bitwise operation to bytes in randomly selected scanlines.
    #[serde(alias = "bitwise")]
    Bitwise {
        /// Probability (`0.0..=1.0`) of modifying each scanline.
        #[serde(deserialize_with = "deserialize_magnitude")]
        magnitude: f64,
        /// Optional bitwise operator (defaults to [`BitOp::Xor`]).
        #[serde(skip_serializing_if = "Option::is_none")]
        op: Option<BitOp>,
        /// Optional byte operand (defaults to `0`).
        #[serde(skip_serializing_if = "Option::is_none")]
        value: Option<u8>,
    },
    /// Swaps two color channels in randomly selected scanlines.
    #[serde(alias = "channel-swap", alias = "channel_swap")]
    ChannelSwap {
        /// Probability (`0.0..=1.0`) of swapping channels in each scanline.
        #[serde(deserialize_with = "deserialize_magnitude")]
        magnitude: f64,
        /// Optional channel pair to swap (defaults to [`SwapTarget::Rg`]).
        #[serde(skip_serializing_if = "Option::is_none")]
        target: Option<SwapTarget>,
    },
    /// Horizontally rotates scanline bytes by a random offset.
    #[serde(alias = "horizontal-shift", alias = "horizontal_shift")]
    HorizontalShift {
        /// Probability (`0.0..=1.0`) of shifting each scanline.
        #[serde(deserialize_with = "deserialize_magnitude")]
        magnitude: f64,
    },
    /// Scrambles square blocks of pixels across the image.
    #[serde(alias = "block-scramble", alias = "block_scramble")]
    BlockScramble {
        /// Fraction (`0.0..=1.0`) of blocks to swap.
        #[serde(deserialize_with = "deserialize_magnitude")]
        magnitude: f64,
        /// Optional block edge length in pixels (defaults to `16`).
        #[serde(skip_serializing_if = "Option::is_none")]
        block_size: Option<u32>,
    },
    /// Adds random per-pixel color noise on randomly selected scanlines.
    #[serde(alias = "color-distortion", alias = "color_distortion")]
    ColorDistortion {
        /// Probability (`0.0..=1.0`) of distorting each scanline.
        #[serde(deserialize_with = "deserialize_magnitude")]
        magnitude: f64,
        /// Optional maximum noise amplitude (defaults to `20`).
        #[serde(skip_serializing_if = "Option::is_none")]
        strength: Option<i16>,
    },
    /// Shifts hue and scales saturation/lightness in HSL color space.
    #[serde(alias = "color-space-glitch", alias = "color_space_glitch")]
    ColorSpaceGlitch {
        /// Probability (`0.0..=1.0`) of transforming each scanline.
        #[serde(deserialize_with = "deserialize_magnitude")]
        magnitude: f64,
        /// Optional hue shift in degrees (defaults to `0.0`).
        #[serde(skip_serializing_if = "Option::is_none")]
        hue_shift: Option<f64>,
        /// Optional saturation multiplier (defaults to `1.0`).
        #[serde(skip_serializing_if = "Option::is_none")]
        saturation_mult: Option<f64>,
        /// Optional lightness multiplier (defaults to `1.0`).
        #[serde(skip_serializing_if = "Option::is_none")]
        lightness_mult: Option<f64>,
    },
    /// Horizontally offsets R, G, and B channels on randomly selected scanlines.
    #[serde(alias = "chromatic-aberration", alias = "chromatic_aberration")]
    ChromaticAberration {
        /// Probability (`0.0..=1.0`) of applying chromatic aberration to each scanline.
        #[serde(deserialize_with = "deserialize_magnitude")]
        magnitude: f64,
        /// Optional red channel horizontal offset in pixels (defaults to `2`).
        #[serde(skip_serializing_if = "Option::is_none")]
        r_offset: Option<i32>,
        /// Optional green channel horizontal offset in pixels (defaults to `0`).
        #[serde(skip_serializing_if = "Option::is_none")]
        g_offset: Option<i32>,
        /// Optional blue channel horizontal offset in pixels (defaults to `-2`).
        #[serde(skip_serializing_if = "Option::is_none")]
        b_offset: Option<i32>,
    },
    /// Inverts all color channels.
    #[serde(alias = "invert")]
    Invert,
    /// Increases brightness by `strength` (saturating at the bit-depth maximum).
    #[serde(alias = "brighten")]
    Brighten {
        /// Amount added to each color channel.
        strength: u16,
    },
    /// Decreases brightness by `strength` (saturating at `0`).
    #[serde(alias = "darken")]
    Darken {
        /// Amount subtracted from each color channel.
        strength: u16,
    },
    /// Adds fixed signed offsets `(r, g, b)` to color channels.
    #[serde(alias = "shift-channels", alias = "shift_channels")]
    ShiftChannels {
        /// Red channel offset.
        r: i16,
        /// Green channel offset.
        g: i16,
        /// Blue channel offset.
        b: i16,
    },
    /// Re-encodes all scanlines with [`FilterType::None`].
    #[serde(alias = "remove-filter", alias = "remove_filter", alias = "none")]
    RemoveFilter,
    /// Re-encodes all scanlines with [`FilterType::Sub`].
    #[serde(alias = "sub-filter", alias = "sub_filter", alias = "sub")]
    SubFilter,
    /// Re-encodes all scanlines with [`FilterType::Up`].
    #[serde(alias = "up-filter", alias = "up_filter", alias = "up")]
    UpFilter,
    /// Re-encodes all scanlines with [`FilterType::Average`].
    #[serde(alias = "average-filter", alias = "average_filter", alias = "average")]
    AverageFilter,
    /// Re-encodes all scanlines with [`FilterType::Paeth`].
    #[serde(alias = "paeth-filter", alias = "paeth_filter", alias = "paeth")]
    PaethFilter,
    /// Swaps 16×16 macroblocks along VP8 block boundaries (WebP only).
    #[serde(alias = "macroblock-glitch", alias = "macroblock_glitch")]
    MacroblockGlitch {
        /// Fraction (`0.0..=1.0`) of macroblocks to swap.
        #[serde(deserialize_with = "deserialize_magnitude")]
        magnitude: f64,
    },
    /// Selectively corrupts the alpha channel (WebP only).
    #[serde(alias = "alpha-glitch", alias = "alpha_glitch")]
    AlphaGlitch {
        /// Probability (`0.0..=1.0`) of corrupting each scanline's alpha channel (defaults to `1.0`).
        #[serde(
            default = "default_alpha_magnitude",
            deserialize_with = "deserialize_magnitude"
        )]
        magnitude: f64,
        /// Optional alpha corruption strategy (defaults to [`AlphaGlitchStrategy::Invert`]).
        #[serde(skip_serializing_if = "Option::is_none")]
        strategy: Option<AlphaGlitchStrategy>,
    },
    /// Lowers WebP re-encoding quality (`0.0..=100.0`) to introduce lossy compression artifacts (WebP only).
    #[serde(
        alias = "lossy-artifact",
        alias = "lossy_artifact",
        alias = "lossy-quality",
        alias = "lossy_quality"
    )]
    LossyArtifact {
        /// Re-encoding quality in `0.0..=100.0`.
        quality: f32,
    },
}

macro_rules! display_as_spec_word {
    ($ty:ty { $($variant:ident => $word:literal),* $(,)? }) => {
        impl std::fmt::Display for $ty {
            /// Renders the value as it is written in `-f` specs and YAML (lowercase).
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str(match self { $(Self::$variant => $word),* })
            }
        }
    };
}

display_as_spec_word!(SortCriterion { Brightness => "brightness", Hue => "hue" });
display_as_spec_word!(BitOp { And => "and", Or => "or", Xor => "xor" });
display_as_spec_word!(SwapTarget { Rg => "rg", Gb => "gb", Br => "br" });
display_as_spec_word!(AlphaGlitchStrategy {
    Invert => "invert",
    Randomize => "randomize",
    Zero => "zero",
    One => "one",
});

impl std::fmt::Display for FilterConfig {
    /// Renders the step in the `-f / --filter` spec syntax, e.g. `replace:magnitude=0.1`.
    /// The output parses back to an equal [`FilterConfig`].
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        fn spec(
            f: &mut std::fmt::Formatter<'_>,
            name: &str,
            params: &[(&str, Option<String>)],
        ) -> std::fmt::Result {
            f.write_str(name)?;
            let mut sep = ':';
            for (key, value) in params {
                if let Some(value) = value {
                    write!(f, "{sep}{key}={value}")?;
                    sep = ',';
                }
            }
            Ok(())
        }
        fn v<T: std::fmt::Display>(value: T) -> Option<String> {
            Some(value.to_string())
        }
        fn o<T: std::fmt::Display>(value: &Option<T>) -> Option<String> {
            value.as_ref().map(ToString::to_string)
        }
        match self {
            Self::ChangeFilterType { magnitude } => {
                spec(f, "change-filter-type", &[("magnitude", v(magnitude))])
            }
            Self::Replace { magnitude } => spec(f, "replace", &[("magnitude", v(magnitude))]),
            Self::Transpose { magnitude } => spec(f, "transpose", &[("magnitude", v(magnitude))]),
            Self::SetZero { magnitude } => spec(f, "set-zero", &[("magnitude", v(magnitude))]),
            Self::RandomCopy { times } => spec(f, "random-copy", &[("times", v(times))]),
            Self::Substitute { index, value } => {
                spec(f, "substitute", &[("index", v(index)), ("value", v(value))])
            }
            Self::PixelSort {
                magnitude,
                criterion,
            } => spec(
                f,
                "pixel-sort",
                &[("magnitude", v(magnitude)), ("criterion", o(criterion))],
            ),
            Self::Bitwise {
                magnitude,
                op,
                value,
            } => spec(
                f,
                "bitwise",
                &[
                    ("magnitude", v(magnitude)),
                    ("op", o(op)),
                    ("value", o(value)),
                ],
            ),
            Self::ChannelSwap { magnitude, target } => spec(
                f,
                "channel-swap",
                &[("magnitude", v(magnitude)), ("target", o(target))],
            ),
            Self::HorizontalShift { magnitude } => {
                spec(f, "horizontal-shift", &[("magnitude", v(magnitude))])
            }
            Self::BlockScramble {
                magnitude,
                block_size,
            } => spec(
                f,
                "block-scramble",
                &[("magnitude", v(magnitude)), ("block_size", o(block_size))],
            ),
            Self::ColorDistortion {
                magnitude,
                strength,
            } => spec(
                f,
                "color-distortion",
                &[("magnitude", v(magnitude)), ("strength", o(strength))],
            ),
            Self::ColorSpaceGlitch {
                magnitude,
                hue_shift,
                saturation_mult,
                lightness_mult,
            } => spec(
                f,
                "color-space-glitch",
                &[
                    ("magnitude", v(magnitude)),
                    ("hue_shift", o(hue_shift)),
                    ("saturation_mult", o(saturation_mult)),
                    ("lightness_mult", o(lightness_mult)),
                ],
            ),
            Self::ChromaticAberration {
                magnitude,
                r_offset,
                g_offset,
                b_offset,
            } => spec(
                f,
                "chromatic-aberration",
                &[
                    ("magnitude", v(magnitude)),
                    ("r_offset", o(r_offset)),
                    ("g_offset", o(g_offset)),
                    ("b_offset", o(b_offset)),
                ],
            ),
            Self::Invert => spec(f, "invert", &[]),
            Self::Brighten { strength } => spec(f, "brighten", &[("strength", v(strength))]),
            Self::Darken { strength } => spec(f, "darken", &[("strength", v(strength))]),
            Self::ShiftChannels { r, g, b } => spec(
                f,
                "shift-channels",
                &[("r", v(r)), ("g", v(g)), ("b", v(b))],
            ),
            Self::RemoveFilter => spec(f, "remove-filter", &[]),
            Self::SubFilter => spec(f, "sub-filter", &[]),
            Self::UpFilter => spec(f, "up-filter", &[]),
            Self::AverageFilter => spec(f, "average-filter", &[]),
            Self::PaethFilter => spec(f, "paeth-filter", &[]),
            Self::MacroblockGlitch { magnitude } => {
                spec(f, "macroblock-glitch", &[("magnitude", v(magnitude))])
            }
            Self::AlphaGlitch {
                magnitude,
                strategy,
            } => spec(
                f,
                "alpha-glitch",
                &[("magnitude", v(magnitude)), ("strategy", o(strategy))],
            ),
            Self::LossyArtifact { quality } => {
                spec(f, "lossy-artifact", &[("quality", v(quality))])
            }
        }
    }
}

pub(crate) fn default_alpha_magnitude() -> f64 {
    1.0
}
