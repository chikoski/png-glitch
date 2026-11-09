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
        crate::GlitchError::NoRecipe => wit::GlitchError::NoRecipe,
        crate::GlitchError::UnsupportedEncoding => wit::GlitchError::UnsupportedEncoding,
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

mod live_binding {
    use super::exports::chikoski::png_glitch::live as lv;
    use super::{Component, to_wit_error, wit};
    use crate::{ColorType, Encoding, FilterStrategy, FilterType, SeedMode, Session};
    use std::cell::RefCell;

    pub struct SessionImpl(RefCell<Session>);

    fn filter_type(t: lv::FilterType) -> FilterType {
        match t {
            lv::FilterType::None => FilterType::None,
            lv::FilterType::Sub => FilterType::Sub,
            lv::FilterType::Up => FilterType::Up,
            lv::FilterType::Average => FilterType::Average,
            lv::FilterType::Paeth => FilterType::Paeth,
        }
    }

    fn encoding(e: lv::Encoding) -> Encoding {
        Encoding {
            color_type: match e.color_type {
                lv::ColorType::Gray => ColorType::GrayScale,
                lv::ColorType::GrayAlpha => ColorType::GrayScaleAlpha,
                lv::ColorType::Rgb => ColorType::TrueColor,
                lv::ColorType::Rgba => ColorType::TrueColorAlpha,
            },
            bit_depth: e.bit_depth,
            filter: match e.filter {
                lv::FilterStrategy::Fixed(t) => FilterStrategy::Fixed(filter_type(t)),
                lv::FilterStrategy::Pattern(p) => {
                    FilterStrategy::Pattern(p.into_iter().map(filter_type).collect())
                }
                lv::FilterStrategy::Adaptive => FilterStrategy::Adaptive,
            },
        }
    }

    fn open(s: Result<Session, crate::GlitchError>) -> Result<lv::Session, wit::GlitchError> {
        s.map(|s| lv::Session::new(SessionImpl(RefCell::new(s))))
            .map_err(to_wit_error)
    }

    impl lv::Guest for Component {
        type Session = SessionImpl;
    }

    impl lv::GuestSession for SessionImpl {
        fn from_png(png: Vec<u8>) -> Result<lv::Session, wit::GlitchError> {
            open(Session::from_png(&png))
        }

        fn from_pixels(
            width: u32,
            height: u32,
            rgba: Vec<u8>,
            enc: lv::Encoding,
        ) -> Result<lv::Session, wit::GlitchError> {
            open(Session::from_pixels(width, height, &rgba, encoding(enc)))
        }

        fn reload_pixels(&self, rgba: Vec<u8>) -> Result<(), wit::GlitchError> {
            self.0
                .borrow_mut()
                .reload_pixels(&rgba)
                .map_err(to_wit_error)
        }

        fn set_recipe(
            &self,
            recipe: String,
            seed: u64,
            mode: lv::SeedMode,
        ) -> Result<(), wit::GlitchError> {
            let mode = match mode {
                lv::SeedMode::Fixed => SeedMode::Static,
                lv::SeedMode::PerFrame => SeedMode::PerFrame,
            };
            self.0
                .borrow_mut()
                .set_recipe(&recipe, seed, mode)
                .map_err(to_wit_error)
        }

        fn apply(&self) -> Result<(), wit::GlitchError> {
            self.0.borrow_mut().apply().map_err(to_wit_error)
        }

        fn reset(&self) -> Result<(), wit::GlitchError> {
            self.0.borrow_mut().reset().map_err(to_wit_error)
        }

        fn render_rgba(&self) -> Result<Vec<u8>, wit::GlitchError> {
            self.0.borrow_mut().render_rgba().map_err(to_wit_error)
        }

        fn snapshot(&self) -> Result<Vec<u8>, wit::GlitchError> {
            self.0.borrow().snapshot().map_err(to_wit_error)
        }
    }
}

export!(Component);
