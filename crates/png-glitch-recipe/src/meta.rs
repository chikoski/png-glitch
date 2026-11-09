//! Filter metadata as data: names, aliases, parameters, ranges, defaults and the formats each
//! filter supports.
//!
//! This is the one source of truth for what `--list-filters`, the `-f` parser, `--surprise`
//! and the WebAssembly component (`list-filters`) say about a filter. [`info_for`] is an
//! exhaustive `match` over [`FilterConfig`], so adding a variant does not compile until it has
//! metadata here.
//!
//! The step-level targeting keys (`rows`, `rect`, `mask`, `invert-mask`) apply to every step and
//! are not listed in each filter's [`FilterInfo::params`].

use crate::{FilterConfig, MAX_RANDOM_COPY_TIMES};
use serde::Serialize;

/// Image format a filter works on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[non_exhaustive]
#[serde(rename_all = "lowercase")]
pub enum Format {
    /// PNG.
    Png,
    /// WebP.
    Webp,
}

/// Type of a filter parameter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[non_exhaustive]
#[serde(rename_all = "lowercase")]
pub enum ParamKind {
    /// A floating point number.
    Float,
    /// An integer.
    Int,
    /// One of [`ParamInfo::choices`].
    Enum,
    /// The strategy of `regenerate`: a filter type, `adaptive`, or a comma-separated pattern.
    Strategy,
}

/// One parameter of a filter.
#[derive(Debug, Clone, Copy, Serialize)]
#[non_exhaustive]
pub struct ParamInfo {
    /// Name as written in YAML and `-f name:key=value`.
    pub name: &'static str,
    /// Type of the value.
    pub kind: ParamKind,
    /// Smallest valid value, if bounded.
    pub min: Option<f64>,
    /// Largest valid value, if bounded.
    pub max: Option<f64>,
    /// Value used when the parameter is omitted; `None` means the parameter is required.
    pub default: Option<&'static str>,
    /// Valid values of an [`ParamKind::Enum`] parameter; empty otherwise.
    pub choices: &'static [&'static str],
    /// One-line description.
    pub doc: &'static str,
}

/// Metadata of one filter.
#[derive(Debug, Clone, Copy, Serialize)]
#[non_exhaustive]
pub struct FilterInfo {
    /// Kebab-case name, as in `-f`.
    pub name: &'static str,
    /// PascalCase `type:` value in YAML.
    pub yaml_type: &'static str,
    /// Other accepted spellings of the name.
    pub aliases: &'static [&'static str],
    /// One-line description.
    pub doc: &'static str,
    /// An example `-f` argument.
    pub example: &'static str,
    /// Formats the filter works on.
    pub formats: &'static [Format],
    /// Parameters, in the order they are documented.
    pub params: &'static [ParamInfo],
    /// `true` for filters kept only for compatibility.
    pub deprecated: bool,
}

impl FilterInfo {
    /// `true` if the filter works on `format`.
    pub fn supports(&self, format: Format) -> bool {
        self.formats.contains(&format)
    }

    /// Looks a parameter up by name.
    pub fn param(&self, name: &str) -> Option<&'static ParamInfo> {
        self.params.iter().find(|p| p.name == name)
    }

    fn matches(&self, name: &str) -> bool {
        let norm = |s: &str| s.to_ascii_lowercase().replace('_', "-");
        let wanted = norm(name);
        norm(self.name) == wanted
            || norm(self.yaml_type) == wanted
            || self.aliases.iter().any(|a| norm(a) == wanted)
    }
}

const BOTH: &[Format] = &[Format::Png, Format::Webp];
const PNG: &[Format] = &[Format::Png];
const WEBP: &[Format] = &[Format::Webp];

const fn p(
    name: &'static str,
    kind: ParamKind,
    min: Option<f64>,
    max: Option<f64>,
    default: Option<&'static str>,
    choices: &'static [&'static str],
    doc: &'static str,
) -> ParamInfo {
    ParamInfo {
        name,
        kind,
        min,
        max,
        default,
        choices,
        doc,
    }
}

const MAGNITUDE: ParamInfo = p(
    "magnitude",
    ParamKind::Float,
    Some(0.0),
    Some(1.0),
    None,
    &[],
    "Probability of touching each scanline",
);
const FILTER_TYPES: &[&str] = &["none", "sub", "up", "average", "paeth"];

const fn f(
    name: &'static str,
    yaml_type: &'static str,
    aliases: &'static [&'static str],
    doc: &'static str,
    example: &'static str,
    formats: &'static [Format],
    params: &'static [ParamInfo],
) -> FilterInfo {
    FilterInfo {
        name,
        yaml_type,
        aliases,
        doc,
        example,
        formats,
        params,
        deprecated: false,
    }
}

const fn deprecated(mut info: FilterInfo) -> FilterInfo {
    info.deprecated = true;
    info
}

const INVERT: FilterInfo = f(
    "invert",
    "Invert",
    &[],
    "Inverts all color channels.",
    "-f invert",
    BOTH,
    &[],
);

const BRIGHTEN: FilterInfo = f(
    "brighten",
    "Brighten",
    &[],
    "Increases brightness (saturating at bit-depth maximum).",
    "-f brighten=30",
    BOTH,
    &[p(
        "strength",
        ParamKind::Int,
        Some(0.0),
        Some(65535.0),
        None,
        &[],
        "Amount added to each channel",
    )],
);

const DARKEN: FilterInfo = f(
    "darken",
    "Darken",
    &[],
    "Decreases brightness (saturating at 0).",
    "-f darken=30",
    BOTH,
    &[p(
        "strength",
        ParamKind::Int,
        Some(0.0),
        Some(65535.0),
        None,
        &[],
        "Amount subtracted from each channel",
    )],
);

const SHIFT_CHANNELS: FilterInfo = f(
    "shift-channels",
    "ShiftChannels",
    &[],
    "Shifts color channels by signed offsets.",
    "-f shift-channels=15,0,-15",
    BOTH,
    &[
        p(
            "r",
            ParamKind::Int,
            Some(-32768.0),
            Some(32767.0),
            None,
            &[],
            "Red offset",
        ),
        p(
            "g",
            ParamKind::Int,
            Some(-32768.0),
            Some(32767.0),
            None,
            &[],
            "Green offset",
        ),
        p(
            "b",
            ParamKind::Int,
            Some(-32768.0),
            Some(32767.0),
            None,
            &[],
            "Blue offset",
        ),
    ],
);

const PIXEL_SORT: FilterInfo = f(
    "pixel-sort",
    "PixelSort",
    &[],
    "Sorts pixels within selected scanlines.",
    "-f 'pixel-sort:magnitude=0.2,criterion=hue'",
    BOTH,
    &[
        MAGNITUDE,
        p(
            "criterion",
            ParamKind::Enum,
            None,
            None,
            Some("brightness"),
            &["brightness", "hue"],
            "What the pixels are sorted by",
        ),
    ],
);

const BITWISE: FilterInfo = f(
    "bitwise",
    "Bitwise",
    &[],
    "Applies a bitwise operation to scanline bytes.",
    "-f 'bitwise:magnitude=0.05,op=xor,value=255'",
    BOTH,
    &[
        MAGNITUDE,
        p(
            "op",
            ParamKind::Enum,
            None,
            None,
            Some("xor"),
            &["and", "or", "xor"],
            "Bitwise operation",
        ),
        p(
            "value",
            ParamKind::Int,
            Some(0.0),
            Some(255.0),
            Some("0"),
            &[],
            "Operand",
        ),
    ],
);

const CHANNEL_SWAP: FilterInfo = f(
    "channel-swap",
    "ChannelSwap",
    &[],
    "Swaps two color channels in selected scanlines.",
    "-f 'channel-swap:magnitude=0.2,target=br'",
    BOTH,
    &[
        MAGNITUDE,
        p(
            "target",
            ParamKind::Enum,
            None,
            None,
            Some("rg"),
            &["rg", "gb", "br"],
            "Channels to swap",
        ),
    ],
);

const HORIZONTAL_SHIFT: FilterInfo = f(
    "horizontal-shift",
    "HorizontalShift",
    &[],
    "Rotates scanline bytes horizontally by a random offset.",
    "-f horizontal-shift=0.1",
    BOTH,
    &[MAGNITUDE],
);

const COLOR_DISTORTION: FilterInfo = f(
    "color-distortion",
    "ColorDistortion",
    &[],
    "Adds random per-pixel color noise.",
    "-f 'color-distortion:magnitude=0.2,strength=40'",
    BOTH,
    &[
        MAGNITUDE,
        p(
            "strength",
            ParamKind::Int,
            Some(0.0),
            Some(32767.0),
            Some("20"),
            &[],
            "Largest noise amplitude",
        ),
    ],
);

const CHROMATIC_ABERRATION: FilterInfo = f(
    "chromatic-aberration",
    "ChromaticAberration",
    &[],
    "Horizontally shifts R, G, B channels.",
    "-f 'chromatic-aberration:magnitude=0.2,r=4,b=-4'",
    BOTH,
    &[
        MAGNITUDE,
        p(
            "r_offset",
            ParamKind::Int,
            None,
            None,
            Some("2"),
            &[],
            "Red offset in pixels",
        ),
        p(
            "g_offset",
            ParamKind::Int,
            None,
            None,
            Some("0"),
            &[],
            "Green offset in pixels",
        ),
        p(
            "b_offset",
            ParamKind::Int,
            None,
            None,
            Some("-2"),
            &[],
            "Blue offset in pixels",
        ),
    ],
);

const BLOCK_SCRAMBLE: FilterInfo = f(
    "block-scramble",
    "BlockScramble",
    &[],
    "Swaps square blocks of pixels.",
    "-f 'block-scramble:magnitude=0.1,block_size=32'",
    PNG,
    &[
        MAGNITUDE,
        p(
            "block_size",
            ParamKind::Int,
            Some(1.0),
            None,
            Some("16"),
            &[],
            "Block edge length in pixels",
        ),
    ],
);

const COLOR_SPACE_GLITCH: FilterInfo = f(
    "color-space-glitch",
    "ColorSpaceGlitch",
    &[],
    "Transforms colors in HSL space.",
    "-f 'color-space-glitch:magnitude=0.2,hue_shift=90,saturation_mult=1.5'",
    PNG,
    &[
        MAGNITUDE,
        p(
            "hue_shift",
            ParamKind::Float,
            None,
            None,
            Some("0.0"),
            &[],
            "Hue rotation in degrees",
        ),
        p(
            "saturation_mult",
            ParamKind::Float,
            Some(0.0),
            None,
            Some("1.0"),
            &[],
            "Saturation multiplier",
        ),
        p(
            "lightness_mult",
            ParamKind::Float,
            Some(0.0),
            None,
            Some("1.0"),
            &[],
            "Lightness multiplier",
        ),
    ],
);

const CHANGE_FILTER_TYPE: FilterInfo = f(
    "change-filter-type",
    "ChangeFilterType",
    &[],
    "Randomly overwrites PNG scanline filter type bytes without re-filtering.",
    "-f change-filter-type=0.1",
    PNG,
    &[MAGNITUDE],
);

const REPLACE: FilterInfo = f(
    "replace",
    "Replace",
    &[],
    "Replaces random scanline bytes with random values.",
    "-f replace=0.05",
    PNG,
    &[MAGNITUDE],
);

const TRANSPOSE: FilterInfo = f(
    "transpose",
    "Transpose",
    &["random-transpose", "RandomTranspose"],
    "Randomly swaps scanlines.",
    "-f transpose=0.1",
    PNG,
    &[MAGNITUDE],
);

const SET_ZERO: FilterInfo = f(
    "set-zero",
    "SetZero",
    &[],
    "Sets random scanline bytes to 0.",
    "-f set-zero=0.05",
    PNG,
    &[MAGNITUDE],
);

const RANDOM_COPY: FilterInfo = f(
    "random-copy",
    "RandomCopy",
    &[],
    "Copies random scanlines onto other scanlines N times.",
    "-f random-copy=10",
    PNG,
    &[p(
        "times",
        ParamKind::Int,
        Some(0.0),
        Some(MAX_RANDOM_COPY_TIMES as f64),
        None,
        &[],
        "Number of copies",
    )],
);

const SUBSTITUTE: FilterInfo = f(
    "substitute",
    "Substitute",
    &[],
    "Sets byte at INDEX to VALUE on every scanline.",
    "-f substitute=0:255",
    PNG,
    &[
        p(
            "index",
            ParamKind::Int,
            Some(0.0),
            None,
            None,
            &[],
            "Byte index within the scanline",
        ),
        p(
            "value",
            ParamKind::Int,
            Some(0.0),
            Some(255.0),
            None,
            &[],
            "New byte value",
        ),
    ],
);

const SET_FILTER_TYPE: FilterInfo = f(
    "set-filter-type",
    "SetFilterType",
    &[],
    "Overwrites the filter type tag of every PNG scanline and leaves the filtered bytes as they are.",
    "-f set-filter-type=paeth",
    PNG,
    &[p(
        "filter",
        ParamKind::Enum,
        None,
        None,
        None,
        FILTER_TYPES,
        "PNG filter type tag",
    )],
);

const REGENERATE: FilterInfo = f(
    "regenerate",
    "Regenerate",
    &[],
    "Starts a new generation: recomputes every filtered value. Glitches so far are baked into the pixels.",
    "-f regenerate=paeth",
    PNG,
    &[p(
        "strategy",
        ParamKind::Strategy,
        None,
        None,
        None,
        &["none", "sub", "up", "average", "paeth", "adaptive"],
        "A filter type, `adaptive`, or a comma-separated pattern such as `sub,up,paeth` (in YAML: `adaptive`, `{ fixed: paeth }` or `{ pattern: [sub, up] }`)",
    )],
);

const REMOVE_FILTER: FilterInfo = deprecated(f(
    "remove-filter",
    "RemoveFilter",
    &["remove_filter", "none"],
    "Re-encodes all PNG scanlines with filter type None. Use `regenerate=none` instead.",
    "-f remove-filter",
    PNG,
    &[],
));

const SUB_FILTER: FilterInfo = deprecated(f(
    "sub-filter",
    "SubFilter",
    &["sub_filter", "sub"],
    "Re-encodes all PNG scanlines with the Sub filter. Use `regenerate=sub` instead.",
    "-f sub-filter",
    PNG,
    &[],
));

const UP_FILTER: FilterInfo = deprecated(f(
    "up-filter",
    "UpFilter",
    &["up_filter", "up"],
    "Re-encodes all PNG scanlines with the Up filter. Use `regenerate=up` instead.",
    "-f up-filter",
    PNG,
    &[],
));

const AVERAGE_FILTER: FilterInfo = deprecated(f(
    "average-filter",
    "AverageFilter",
    &["average_filter", "average"],
    "Re-encodes all PNG scanlines with the Average filter. Use `regenerate=average` instead.",
    "-f average-filter",
    PNG,
    &[],
));

const PAETH_FILTER: FilterInfo = deprecated(f(
    "paeth-filter",
    "PaethFilter",
    &["paeth_filter", "paeth"],
    "Re-encodes all PNG scanlines with the Paeth filter. Use `regenerate=paeth` instead.",
    "-f paeth-filter",
    PNG,
    &[],
));

const MACROBLOCK_GLITCH: FilterInfo = f(
    "macroblock-glitch",
    "MacroblockGlitch",
    &[],
    "Swaps 16x16 macroblocks along VP8 block boundaries.",
    "-f macroblock-glitch=0.2",
    WEBP,
    &[MAGNITUDE],
);

const ALPHA_GLITCH: FilterInfo = f(
    "alpha-glitch",
    "AlphaGlitch",
    &[],
    "Corrupts the alpha channel.",
    "-f alpha-glitch=randomize",
    WEBP,
    &[
        p(
            "magnitude",
            ParamKind::Float,
            Some(0.0),
            Some(1.0),
            Some("1.0"),
            &[],
            "Probability of touching each scanline",
        ),
        p(
            "strategy",
            ParamKind::Enum,
            None,
            None,
            Some("invert"),
            &["invert", "randomize", "zero", "one"],
            "How the alpha bytes are corrupted",
        ),
    ],
);

const LOSSY_ARTIFACT: FilterInfo = f(
    "lossy-artifact",
    "LossyArtifact",
    &["lossy_artifact", "lossy-quality", "lossy_quality"],
    "Lowers WebP re-encoding quality to introduce compression artifacts.",
    "-f lossy-artifact=10",
    WEBP,
    &[p(
        "quality",
        ParamKind::Float,
        Some(0.0),
        Some(100.0),
        None,
        &[],
        "Re-encoding quality",
    )],
);

static FILTERS: [FilterInfo; 28] = [
    INVERT,
    BRIGHTEN,
    DARKEN,
    SHIFT_CHANNELS,
    PIXEL_SORT,
    BITWISE,
    CHANNEL_SWAP,
    HORIZONTAL_SHIFT,
    COLOR_DISTORTION,
    CHROMATIC_ABERRATION,
    BLOCK_SCRAMBLE,
    COLOR_SPACE_GLITCH,
    CHANGE_FILTER_TYPE,
    REPLACE,
    TRANSPOSE,
    SET_ZERO,
    RANDOM_COPY,
    SUBSTITUTE,
    SET_FILTER_TYPE,
    REGENERATE,
    REMOVE_FILTER,
    SUB_FILTER,
    UP_FILTER,
    AVERAGE_FILTER,
    PAETH_FILTER,
    MACROBLOCK_GLITCH,
    ALPHA_GLITCH,
    LOSSY_ARTIFACT,
];

/// Metadata of every built-in filter: shared filters first, then PNG-only, then WebP-only.
///
/// # Examples
/// ```
/// let info = png_glitch_recipe::filters()
///     .iter()
///     .find(|f| f.name == "block-scramble")
///     .unwrap();
/// assert_eq!(info.param("block_size").unwrap().default, Some("16"));
/// ```
pub fn filters() -> &'static [FilterInfo] {
    &FILTERS
}

/// Looks a filter up by kebab-case name, YAML `type:` value or alias (case-insensitive,
/// `_` and `-` are equivalent).
pub fn filter_info(name_or_alias: &str) -> Option<&'static FilterInfo> {
    FILTERS.iter().find(|i| i.matches(name_or_alias.trim()))
}

/// Metadata of the filter `config` describes.
///
/// Written as an exhaustive `match` without a `_` arm, so a new [`FilterConfig`] variant does
/// not compile until it has an entry here.
pub fn info_for(config: &FilterConfig) -> &'static FilterInfo {
    match config {
        FilterConfig::ChangeFilterType { .. } => &CHANGE_FILTER_TYPE,
        FilterConfig::Replace { .. } => &REPLACE,
        FilterConfig::Transpose { .. } => &TRANSPOSE,
        FilterConfig::SetZero { .. } => &SET_ZERO,
        FilterConfig::RandomCopy { .. } => &RANDOM_COPY,
        FilterConfig::Substitute { .. } => &SUBSTITUTE,
        FilterConfig::PixelSort { .. } => &PIXEL_SORT,
        FilterConfig::Bitwise { .. } => &BITWISE,
        FilterConfig::ChannelSwap { .. } => &CHANNEL_SWAP,
        FilterConfig::HorizontalShift { .. } => &HORIZONTAL_SHIFT,
        FilterConfig::BlockScramble { .. } => &BLOCK_SCRAMBLE,
        FilterConfig::ColorDistortion { .. } => &COLOR_DISTORTION,
        FilterConfig::ColorSpaceGlitch { .. } => &COLOR_SPACE_GLITCH,
        FilterConfig::ChromaticAberration { .. } => &CHROMATIC_ABERRATION,
        FilterConfig::Invert => &INVERT,
        FilterConfig::Brighten { .. } => &BRIGHTEN,
        FilterConfig::Darken { .. } => &DARKEN,
        FilterConfig::ShiftChannels { .. } => &SHIFT_CHANNELS,
        FilterConfig::SetFilterType { .. } => &SET_FILTER_TYPE,
        FilterConfig::Regenerate { .. } => &REGENERATE,
        FilterConfig::RemoveFilter => &REMOVE_FILTER,
        FilterConfig::SubFilter => &SUB_FILTER,
        FilterConfig::UpFilter => &UP_FILTER,
        FilterConfig::AverageFilter => &AVERAGE_FILTER,
        FilterConfig::PaethFilter => &PAETH_FILTER,
        FilterConfig::MacroblockGlitch { .. } => &MACROBLOCK_GLITCH,
        FilterConfig::AlphaGlitch { .. } => &ALPHA_GLITCH,
        FilterConfig::LossyArtifact { .. } => &LOSSY_ARTIFACT,
    }
}
