//! Row targeting: `rows: a..b` on a step.

use crate::FilterConfig;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
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
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FilterEntry {
    /// The step.
    #[serde(flatten)]
    pub config: FilterConfig,
    /// The rows the step works on; `None` for the whole image.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rows: Option<RowRange>,
}

impl FilterEntry {
    /// An entry that targets `rows`.
    pub fn with_rows(config: FilterConfig, rows: RowRange) -> Self {
        Self {
            config,
            rows: Some(rows),
        }
    }
}

impl From<FilterConfig> for FilterEntry {
    fn from(config: FilterConfig) -> Self {
        Self { config, rows: None }
    }
}
