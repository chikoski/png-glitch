//! Binding layer: maps the WIT world to the plain functions in the crate root.

wit_bindgen::generate!({
    world: "png-glitch",
    path: "wit",
});

use exports::chikoski::png_glitch::glitch as wit;
use png_glitch_recipe as meta;

struct Component;

fn to_wit_error(e: crate::GlitchError) -> wit::GlitchError {
    match e {
        crate::GlitchError::InvalidImage(m) => wit::GlitchError::InvalidImage(m),
        crate::GlitchError::UnsupportedFormat => wit::GlitchError::UnsupportedFormat,
        crate::GlitchError::InvalidRecipe(m) => wit::GlitchError::InvalidRecipe(m),
        crate::GlitchError::TooLarge => wit::GlitchError::TooLarge,
    }
}

fn kind(k: meta::ParamKind) -> wit::ParamKind {
    match k {
        meta::ParamKind::Float => wit::ParamKind::Float,
        meta::ParamKind::Int => wit::ParamKind::Int,
        meta::ParamKind::Enum => wit::ParamKind::Enum,
        _ => wit::ParamKind::Strategy,
    }
}

fn format(f: &meta::Format) -> wit::Format {
    match f {
        meta::Format::Png => wit::Format::Png,
        _ => wit::Format::Webp,
    }
}

impl wit::Guest for Component {
    fn list_filters() -> Vec<wit::FilterInfo> {
        crate::list_filters()
            .iter()
            .map(|i| wit::FilterInfo {
                name: i.name.into(),
                yaml_type: i.yaml_type.into(),
                aliases: i.aliases.iter().map(|s| (*s).into()).collect(),
                doc: i.doc.into(),
                example: i.example.into(),
                formats: i.formats.iter().map(format).collect(),
                params: i
                    .params
                    .iter()
                    .map(|p| wit::ParamInfo {
                        name: p.name.into(),
                        kind: kind(p.kind),
                        min: p.min,
                        max: p.max,
                        default: p.default.map(Into::into),
                        choices: p.choices.iter().map(|s| (*s).into()).collect(),
                        doc: p.doc.into(),
                    })
                    .collect(),
                deprecated: i.deprecated,
            })
            .collect()
    }

    fn validate_recipe(recipe: String) -> Result<(), wit::GlitchError> {
        crate::validate_recipe(&recipe).map_err(to_wit_error)
    }

    fn glitch(image: Vec<u8>, recipe: String, seed: u64) -> Result<Vec<u8>, wit::GlitchError> {
        crate::glitch(&image, &recipe, seed).map_err(to_wit_error)
    }
}

export!(Component);
