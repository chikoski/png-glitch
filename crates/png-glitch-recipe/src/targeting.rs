//! Row targeting: `rows: a..b` on a step.

use crate::FilterConfig;
use png_glitch::effects::Mask;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::collections::HashMap;
use std::fmt;
use std::ops::Range;
use std::str::FromStr;

/// A range of image rows written `a..b`, `a..` or `..b` (end exclusive, 0-based).
///
/// A missing start means the first row and a missing end means the last row. When a step is
/// resolved the range is clamped to the image: a start at or past the last row, or an end not
/// after the start, selects no rows (the step does nothing).
///
/// # Examples
/// ```
/// use png_glitch_recipe::RowRange;
///
/// let r: RowRange = "300..452".parse().unwrap();
/// assert_eq!(r.to_string(), "300..452");
/// assert_eq!(r.resolve(400), 300..400);
/// assert_eq!("..10".parse::<RowRange>().unwrap().resolve(100), 0..10);
/// assert_eq!("90..".parse::<RowRange>().unwrap().resolve(100), 90..100);
/// assert!("5".parse::<RowRange>().is_err());
/// ```
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct RowRange {
    /// First row, or `None` for row 0.
    pub start: Option<u32>,
    /// One past the last row, or `None` for the end of the image.
    pub end: Option<u32>,
}

impl RowRange {
    /// A range with both ends given.
    pub fn new(start: u32, end: u32) -> Self {
        Self {
            start: Some(start),
            end: Some(end),
        }
    }

    /// The rows this range selects in an image `height` rows tall (empty if none).
    pub fn resolve(&self, height: u32) -> Range<u32> {
        let start = self.start.unwrap_or(0).min(height);
        let end = self.end.unwrap_or(height).min(height).max(start);
        start..end
    }
}

impl fmt::Display for RowRange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(s) = self.start {
            write!(f, "{s}")?;
        }
        f.write_str("..")?;
        if let Some(e) = self.end {
            write!(f, "{e}")?;
        }
        Ok(())
    }
}

impl FromStr for RowRange {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, String> {
        let bad = || format!("invalid row range `{s}`: expected `a..b`, `a..` or `..b`");
        let (a, b) = s.trim().split_once("..").ok_or_else(bad)?;
        let num = |t: &str| -> Result<Option<u32>, String> {
            let t = t.trim();
            if t.is_empty() {
                Ok(None)
            } else {
                t.parse().map(Some).map_err(|_| bad())
            }
        };
        Ok(Self {
            start: num(a)?,
            end: num(b)?,
        })
    }
}

impl Serialize for RowRange {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for RowRange {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let text = String::deserialize(d)?;
        text.parse().map_err(serde::de::Error::custom)
    }
}

/// A rectangle `x,y,w,h` in pixels, written `rect: 10,20,100,50` in a recipe.
///
/// The rectangle is clipped to the image. Inside it a step has full effect and outside none
/// (see [`FilterEntry`]).
///
/// # Examples
/// ```
/// use png_glitch_recipe::RectSpec;
///
/// let r: RectSpec = "10,20,100,50".parse().unwrap();
/// assert_eq!((r.x, r.y, r.w, r.h), (10, 20, 100, 50));
/// assert_eq!(r.to_string(), "10,20,100,50");
/// assert!("10,20,100".parse::<RectSpec>().is_err());
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct RectSpec {
    /// Left edge.
    pub x: u32,
    /// Top edge.
    pub y: u32,
    /// Width.
    pub w: u32,
    /// Height.
    pub h: u32,
}

impl fmt::Display for RectSpec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{},{},{},{}", self.x, self.y, self.w, self.h)
    }
}

impl FromStr for RectSpec {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, String> {
        let bad = || format!("invalid rectangle `{s}`: expected `x,y,w,h` (four whole numbers)");
        let parts: Vec<u32> = s
            .split(',')
            .map(|t| t.trim().parse().map_err(|_| bad()))
            .collect::<Result<_, _>>()?;
        match parts[..] {
            [x, y, w, h] => Ok(Self { x, y, w, h }),
            _ => Err(bad()),
        }
    }
}

impl Serialize for RectSpec {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for RectSpec {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let text = String::deserialize(d)?;
        text.parse().map_err(serde::de::Error::custom)
    }
}

/// A reference to a mask image: `mask: { path: m.png, crc32: 1234 }` in a recipe.
///
/// The recipe crate reads no files. The host (the CLI) loads the image, turns it into a
/// [`Mask`] and hands it over in a [`MaskSet`] under `path`. `crc32` is the CRC-32 of the
/// file's bytes; when it is present and the supplied file differs, resolving fails with
/// [`RecipeError::MaskChecksum`](crate::RecipeError::MaskChecksum).
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MaskRef {
    /// The path the host loads the mask from. Also the key in the [`MaskSet`].
    pub path: String,
    /// CRC-32 of the file's bytes, if known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub crc32: Option<u32>,
}

/// Masks supplied by the host, by the `path` a step refers to them with.
///
/// Attach them to a [`Recipe`](crate::Recipe) with [`Recipe::with_masks`](crate::Recipe::with_masks)
/// or pass them to [`GlitchContext::add_from_entry`](crate::GlitchContext::add_from_entry).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MaskSet(HashMap<String, (Mask, Option<u32>)>);

impl MaskSet {
    /// An empty set.
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a mask under `path`. `crc32` is the CRC-32 of the file it was loaded from, which
    /// is checked against a step's [`MaskRef::crc32`].
    pub fn insert(&mut self, path: impl Into<String>, mask: Mask, crc32: Option<u32>) {
        self.0.insert(path.into(), (mask, crc32));
    }

    /// The mask supplied for `path`.
    pub fn get(&self, path: &str) -> Option<&Mask> {
        self.0.get(path).map(|(m, _)| m)
    }

    /// `true` if no mask was supplied.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub(crate) fn lookup(&self, reference: &MaskRef) -> crate::Result<Mask> {
        let (mask, crc) =
            self.0
                .get(&reference.path)
                .ok_or_else(|| crate::RecipeError::MissingMask {
                    path: reference.path.clone(),
                })?;
        if let (Some(expected), Some(actual)) = (reference.crc32, crc)
            && expected != *actual
        {
            return Err(crate::RecipeError::MaskChecksum {
                path: reference.path.clone(),
                expected,
                actual: *actual,
            });
        }
        Ok(mask.clone())
    }
}

fn is_false(b: &bool) -> bool {
    !*b
}

/// A recipe step as written in a recipe file: a [`FilterConfig`] and an optional row range.
///
/// In YAML the range is a `rows` key next to `type`:
///
/// ```yaml
/// filters:
///   - type: replace
///     magnitude: 0.1
///     rows: 300..452
/// ```
///
/// Without `rows` the step works on the whole image and gives the same bytes as before. With
/// `rows` the step is resolved for an image as tall as the range, so its random choices (and
/// row numbers) are relative to the range; it draws a different random stream than the same
/// step without `rows`.
///
/// `rect: x,y,w,h` or `mask: { path, crc32 }` (not both) restrict the step to a region, and
/// `invert-mask: true` selects everything outside it. The step runs on the whole image (or
/// the `rows`) and the result is kept only where the mask is non-zero
/// ([`Masked`](png_glitch::effects::Masked), ADR 0006): a mask value is a strength. A region
/// cannot be combined with a step that recomputes filtered values (`regenerate`,
/// `sub-filter`, ...). With `rows` too, the mask is cropped to the same rows.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FilterEntry {
    /// The step.
    #[serde(flatten)]
    pub config: FilterConfig,
    /// The rows the step works on; `None` for the whole image.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rows: Option<RowRange>,
    /// A rectangle the step is restricted to.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rect: Option<RectSpec>,
    /// A mask image the step is restricted to.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mask: Option<MaskRef>,
    /// Select everything outside the `rect` / `mask` instead.
    #[serde(
        default,
        rename = "invert-mask",
        alias = "invert_mask",
        skip_serializing_if = "is_false"
    )]
    pub invert_mask: bool,
}

impl FilterEntry {
    /// An entry that targets `rows`.
    pub fn with_rows(config: FilterConfig, rows: RowRange) -> Self {
        Self {
            rows: Some(rows),
            ..config.into()
        }
    }

    /// `true` if the entry has a `rect` or a `mask`.
    pub fn has_region(&self) -> bool {
        self.rect.is_some() || self.mask.is_some()
    }

    /// `true` if the entry is just a config: no `rows`, `rect`, `mask` or `invert-mask`.
    pub fn is_plain(&self) -> bool {
        self.rows.is_none() && !self.has_region() && !self.invert_mask
    }
}

impl From<FilterConfig> for FilterEntry {
    fn from(config: FilterConfig) -> Self {
        Self {
            config,
            rows: None,
            rect: None,
            mask: None,
            invert_mask: false,
        }
    }
}
