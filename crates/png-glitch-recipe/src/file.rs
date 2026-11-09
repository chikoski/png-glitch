//! The recipe file format read by `--config`.

use crate::{FilterEntry, RecipeError, Result};
use serde::Deserialize;

/// The contents of a recipe file: the `--config` format of the CLI.
///
/// A file is a YAML mapping with a `filters` list; JSON objects of the same shape parse too
/// (JSON is read as YAML flow syntax). Unknown keys are rejected.
///
/// # Examples
/// ```
/// use png_glitch_recipe::RecipeFile;
///
/// let file = RecipeFile::from_yaml("filters:\n  - type: Replace\n    magnitude: 0.1\n").unwrap();
/// assert_eq!(file.filters.len(), 1);
/// let json = RecipeFile::from_yaml(r#"{"filters": [{"type": "invert"}]}"#).unwrap();
/// assert_eq!(json.filters.len(), 1);
/// assert!(RecipeFile::from_yaml("filters: [{type: Replace, magnitude: 2}]").is_err());
/// ```
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecipeFile {
    /// The steps, applied in order.
    pub filters: Vec<FilterEntry>,
}

impl RecipeFile {
    /// Parses a recipe from YAML (or JSON) text.
    ///
    /// # Errors
    /// [`RecipeError::InvalidRecipe`] with the parser's message.
    pub fn from_yaml(text: &str) -> Result<Self> {
        serde_saphyr::from_str(text).map_err(|e| RecipeError::InvalidRecipe(e.to_string()))
    }
}
