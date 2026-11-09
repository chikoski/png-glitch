//! The filter metadata table agrees with `FilterConfig`, the filter `Default`s and validation.

use png_glitch_recipe::*;

const FIXTURE: &[u8] = include_bytes!("fixtures/small.png");

fn sample_value(p: &ParamInfo) -> String {
    match p.kind {
        ParamKind::Float => p
            .min
            .unwrap_or(0.5)
            .max(0.5)
            .min(p.max.unwrap_or(f64::MAX))
            .to_string(),
        ParamKind::Int => "1".to_string(),
        ParamKind::Enum => p.choices[0].to_string(),
        ParamKind::Strategy => "adaptive".to_string(),
        _ => unreachable!("new ParamKind needs a sample value"),
    }
}

fn yaml(type_name: &str, params: &[(&str, String)]) -> String {
    let mut s = format!("type: {type_name}\n");
    for (k, v) in params {
        s.push_str(&format!("{k}: {v}\n"));
    }
    s
}

fn parse(text: &str) -> std::result::Result<FilterConfig, serde_saphyr::Error> {
    serde_saphyr::from_str::<FilterConfig>(text)
}

#[test]
fn names_are_unique_and_cover_every_variant() {
    let mut seen = std::collections::HashSet::new();
    for info in filters() {
        assert!(seen.insert(info.name), "duplicate name {}", info.name);
        assert!(!info.formats.is_empty(), "{} supports no format", info.name);
    }
    assert_eq!(filters().len(), 28);
}

#[test]
fn yaml_type_and_every_alias_resolve_to_their_entry() {
    for info in filters() {
        let required: Vec<(&str, String)> = info
            .params
            .iter()
            .filter(|p| p.default.is_none())
            .map(|p| (p.name, sample_value(p)))
            .collect();
        let mut spellings = vec![info.yaml_type, info.name];
        spellings.extend(info.aliases);
        for spelling in spellings {
            let config = parse(&yaml(spelling, &required))
                .unwrap_or_else(|e| panic!("`{spelling}` of {} did not parse: {e}", info.name));
            assert_eq!(info_for(&config).name, info.name, "spelling `{spelling}`");
            assert_eq!(filter_info(spelling).map(|i| i.name), Some(info.name));
        }
    }
}

#[test]
fn every_parameter_name_is_accepted_by_serde() {
    // FilterConfig denies unknown fields, so a misspelt parameter name fails here.
    for info in filters() {
        let all: Vec<(&str, String)> = info
            .params
            .iter()
            .map(|p| (p.name, sample_value(p)))
            .collect();
        parse(&yaml(info.yaml_type, &all))
            .unwrap_or_else(|e| panic!("{} with all parameters: {e}", info.name));
    }
}

fn default_of(filter: &str, param: &str) -> &'static str {
    filter_info(filter)
        .and_then(|i| i.param(param))
        .and_then(|p| p.default)
        .unwrap_or_else(|| panic!("{filter}.{param} has no default"))
}

#[test]
fn defaults_match_the_filter_defaults() {
    let b = BlockScramble::default();
    assert_eq!(
        default_of("block-scramble", "block_size"),
        b.block_size.to_string()
    );
    let c = ColorDistortion::default();
    assert_eq!(
        default_of("color-distortion", "strength"),
        c.strength.to_string()
    );
    let a = ChromaticAberration::default();
    assert_eq!(
        default_of("chromatic-aberration", "r_offset"),
        a.r_offset.to_string()
    );
    assert_eq!(
        default_of("chromatic-aberration", "g_offset"),
        a.g_offset.to_string()
    );
    assert_eq!(
        default_of("chromatic-aberration", "b_offset"),
        a.b_offset.to_string()
    );
    let s = ColorSpaceGlitch::default();
    assert_eq!(
        default_of("color-space-glitch", "hue_shift"),
        format!("{:?}", s.hue_shift)
    );
    assert_eq!(
        default_of("color-space-glitch", "saturation_mult"),
        format!("{:?}", s.saturation_mult)
    );
    assert_eq!(
        default_of("color-space-glitch", "lightness_mult"),
        format!("{:?}", s.lightness_mult)
    );
    let w = Bitwise::default();
    assert_eq!(default_of("bitwise", "value"), w.value.to_string());
    assert_eq!(w.op, BitOp::Xor);
    assert_eq!(default_of("bitwise", "op"), "xor");
    assert_eq!(PixelSort::default().criterion, SortCriterion::Brightness);
    assert_eq!(default_of("pixel-sort", "criterion"), "brightness");
    assert_eq!(ChannelSwap::default().target, SwapTarget::Rg);
    assert_eq!(default_of("channel-swap", "target"), "rg");
    assert_eq!(default_of("alpha-glitch", "strategy"), "invert");
    assert_eq!(default_of("alpha-glitch", "magnitude"), "1.0");
}

#[test]
fn enum_choices_deserialize() {
    for info in filters() {
        for p in info.params.iter().filter(|p| p.kind == ParamKind::Enum) {
            for choice in p.choices {
                let mut params: Vec<(&str, String)> = info
                    .params
                    .iter()
                    .filter(|q| q.default.is_none() && q.name != p.name)
                    .map(|q| (q.name, sample_value(q)))
                    .collect();
                params.push((p.name, choice.to_string()));
                parse(&yaml(info.yaml_type, &params))
                    .unwrap_or_else(|e| panic!("{}.{}={choice}: {e}", info.name, p.name));
            }
        }
    }
}

fn magnitude_filters() -> impl Iterator<Item = &'static FilterInfo> {
    filters().iter().filter(|i| {
        i.param("magnitude")
            .is_some_and(|p| p.min == Some(0.0) && p.max == Some(1.0))
    })
}

#[test]
fn magnitude_range_matches_validation() {
    for info in magnitude_filters() {
        let others: Vec<(&str, String)> = info
            .params
            .iter()
            .filter(|p| p.default.is_none() && p.name != "magnitude")
            .map(|p| (p.name, sample_value(p)))
            .collect();
        for ok in ["0.0", "1.0"] {
            let mut params = others.clone();
            params.push(("magnitude", ok.into()));
            parse(&yaml(info.yaml_type, &params))
                .unwrap_or_else(|e| panic!("{} magnitude {ok}: {e}", info.name));
        }
        for bad in ["-0.01", "1.01"] {
            let mut params = others.clone();
            params.push(("magnitude", bad.into()));
            assert!(
                parse(&yaml(info.yaml_type, &params)).is_err(),
                "{} accepted magnitude {bad}",
                info.name
            );
        }
    }
}

#[test]
fn random_copy_times_range_matches_validation() {
    let times = filter_info("random-copy").unwrap().param("times").unwrap();
    let max = times.max.unwrap() as u32;
    assert_eq!(max, MAX_RANDOM_COPY_TIMES);
    let add = |t: u32| {
        GlitchContext::new(FIXTURE, Some(1))
            .unwrap()
            .add_from_config(FilterConfig::RandomCopy { times: t })
    };
    assert!(add(times.min.unwrap() as u32).is_ok());
    assert!(add(max).is_ok());
    assert!(add(max + 1).is_err());
}
