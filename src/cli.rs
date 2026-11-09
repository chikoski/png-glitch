use clap::Parser;
use png_glitch_recipe::{
    AlphaGlitchStrategy, BitOp, FilterConfig, FilterEntry, MAX_RANDOM_COPY_TIMES, MaskRef,
    PreProcess, RectSpec, RegenerateStrategy, RowRange, SortCriterion, SwapTarget,
};
use serde::Deserialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SubstituteArg {
    pub index: usize,
    pub value: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShiftChannelsArg {
    pub r: i16,
    pub g: i16,
    pub b: i16,
}

pub fn parse_magnitude(s: &str) -> Result<f64, String> {
    let val: f64 = s
        .parse()
        .map_err(|_| format!("`{s}` is not a valid floating-point number"))?;
    if (0.0..=1.0).contains(&val) {
        Ok(val)
    } else {
        Err(format!(
            "magnitude must be between 0.0 and 1.0 (inclusive), got {val}"
        ))
    }
}

pub fn parse_quality(s: &str) -> Result<f32, String> {
    let val: f32 = s
        .parse()
        .map_err(|_| format!("`{s}` is not a valid floating-point number"))?;
    if (0.0..=100.0).contains(&val) {
        Ok(val)
    } else {
        Err(format!(
            "quality must be between 0.0 and 100.0 (inclusive), got {val}"
        ))
    }
}

pub fn parse_random_copy_times(s: &str) -> Result<u32, String> {
    let val: u32 = s
        .parse()
        .map_err(|e| format!("invalid `times` `{s}`: {e}"))?;
    if val <= MAX_RANDOM_COPY_TIMES {
        Ok(val)
    } else {
        Err(format!(
            "random-copy `times` must be <= {MAX_RANDOM_COPY_TIMES}, got {val}"
        ))
    }
}

pub fn parse_non_negative_strength(s: &str) -> Result<i16, String> {
    let val: i16 = s
        .parse()
        .map_err(|e| format!("invalid `strength` `{s}`: {e}"))?;
    if val >= 0 {
        Ok(val)
    } else {
        Err(format!(
            "color-distortion `strength` must be >= 0, got {val}"
        ))
    }
}

pub fn parse_finite_f64(s: &str) -> Result<f64, String> {
    let val: f64 = s
        .parse()
        .map_err(|_| format!("`{s}` is not a valid floating-point number"))?;
    if val.is_finite() {
        Ok(val)
    } else {
        Err(format!("expected a finite number, got {val}"))
    }
}

pub fn parse_non_negative_f64(s: &str) -> Result<f64, String> {
    let val: f64 = s
        .parse()
        .map_err(|_| format!("`{s}` is not a valid floating-point number"))?;
    if val.is_finite() && val >= 0.0 {
        Ok(val)
    } else {
        Err(format!(
            "multiplier must be a finite number >= 0.0, got {val}"
        ))
    }
}

pub fn parse_substitute(s: &str) -> Result<SubstituteArg, String> {
    let (idx_str, val_str) = s
        .split_once(':')
        .ok_or_else(|| format!("invalid format `{s}`, expected INDEX:VALUE (e.g. 10:255)"))?;
    let index = idx_str
        .parse::<usize>()
        .map_err(|e| format!("invalid index `{idx_str}`: {e}"))?;
    let value = val_str
        .parse::<u8>()
        .map_err(|e| format!("invalid byte value `{val_str}` (must be 0..=255): {e}"))?;
    Ok(SubstituteArg { index, value })
}

pub fn parse_shift_channels(s: &str) -> Result<ShiftChannelsArg, String> {
    let parts: Vec<&str> = s.split(',').collect();
    if parts.len() != 3 {
        return Err(format!(
            "invalid format `{s}`, expected 3 comma-separated integers R,G,B (e.g. 10,0,-10)"
        ));
    }
    let r = parts[0]
        .trim()
        .parse::<i16>()
        .map_err(|e| format!("invalid R shift `{}`: {e}", parts[0]))?;
    let g = parts[1]
        .trim()
        .parse::<i16>()
        .map_err(|e| format!("invalid G shift `{}`: {e}", parts[1]))?;
    let b = parts[2]
        .trim()
        .parse::<i16>()
        .map_err(|e| format!("invalid B shift `{}`: {e}", parts[2]))?;
    Ok(ShiftChannelsArg { r, g, b })
}

fn parse_kv_pairs<'a>(name: &str, rest: &'a str) -> Result<Vec<(&'a str, &'a str)>, String> {
    let mut pairs = Vec::new();
    for item in rest.split(',') {
        let item = item.trim();
        if item.is_empty() {
            continue;
        }
        let (k, v) = item.split_once('=').ok_or_else(|| {
            format!(
                "invalid parameter `{item}` for filter `{name}`: expected `key=value` (e.g. `-f '{name}:magnitude=0.2'`)"
            )
        })?;
        pairs.push((k.trim(), v.trim()));
    }
    Ok(pairs)
}

/// Parses a PNG filter type name (`none`, `sub`, `up`, `average`, `paeth`; the spellings of
/// `--pre-process`, case-insensitive) as used by `set-filter-type` and `regenerate`.
fn parse_filter_type(value: &str) -> Result<PreProcess, String> {
    let norm = value.trim().to_ascii_lowercase().replace('_', "-");
    match norm.as_str() {
        "none" | "remove-filter" | "removefilter" => Ok(PreProcess::RemoveFilter),
        "sub" | "sub-filter" | "subfilter" => Ok(PreProcess::SubFilter),
        "up" | "up-filter" | "upfilter" => Ok(PreProcess::UpFilter),
        "average" | "average-filter" | "averagefilter" => Ok(PreProcess::AverageFilter),
        "paeth" | "paeth-filter" | "paethfilter" => Ok(PreProcess::PaethFilter),
        other => Err(format!(
            "unknown PNG filter type `{other}` (expected none, sub, up, average or paeth)"
        )),
    }
}

/// Parses the value of `regenerate=`: `adaptive`, one filter type (`Fixed`), or a comma-separated
/// list (`Pattern`; a trailing comma makes a one-element pattern).
fn parse_regenerate_strategy(value: &str) -> Result<RegenerateStrategy, String> {
    let value = value.trim();
    if value.is_empty() {
        return Err("`regenerate` requires a strategy (e.g. `-f regenerate=paeth`)".into());
    }
    if value.eq_ignore_ascii_case("adaptive") {
        return Ok(RegenerateStrategy::Adaptive);
    }
    if !value.contains(',') {
        return Ok(RegenerateStrategy::Fixed(parse_filter_type(value)?));
    }
    let items: Vec<&str> = value.split(',').collect();
    // Allow one trailing comma (`paeth,`); other empty items are mistakes.
    let items = match items.split_last() {
        Some((&"", rest)) => rest,
        _ => &items[..],
    };
    let pattern = items
        .iter()
        .map(|t| parse_filter_type(t))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(RegenerateStrategy::Pattern(pattern))
}

/// Parses a `-f / --filter <SPEC>` argument: a [`parse_filter_spec`] spec with optional targeting
/// keys in its `key=value` list:
///
/// - `rows=a..b` (`a..`, `..b`): only those rows;
/// - `rect=x,y,w,h`: only that rectangle;
/// - `mask=FILE`: only where the PNG `FILE` is bright (its size must equal the image's);
/// - `invert-mask`: select everything outside the `rect` / `mask` instead.
///
/// For example `-f 'replace:magnitude=0.2,rows=300..452'` or `-f 'invert:rect=10,20,100,50'`. The
/// keys go in the `<filter>:<k>=<v>` form, so a spec without parameters, such as `invert`, is
/// written `invert:rect=...`.
pub fn parse_filter_entry(spec: &str) -> Result<FilterEntry, String> {
    let s = spec.trim();
    // The targeting keys are only recognised after the `:` that starts the key=value list.
    let Some((name, list)) = s.split_once(':').filter(|(n, _)| !n.contains('=')) else {
        return parse_filter_spec(s).map(Into::into);
    };
    let mut entry = FilterEntry::from(FilterConfig::Invert);
    let mut found = false;
    let mut rest: Vec<&str> = Vec::new();
    let mut items = list.split(',').peekable();
    while let Some(item) = items.next() {
        let item = item.trim();
        let once = |taken: bool, key: &str| {
            if taken {
                Err(format!("`{key}` is given twice"))
            } else {
                Ok(())
            }
        };
        if let Some(value) = item.strip_prefix("rows=") {
            once(entry.rows.is_some(), "rows")?;
            entry.rows = Some(value.parse::<RowRange>()?);
            found = true;
        } else if let Some(first) = item.strip_prefix("rect=") {
            once(entry.rect.is_some(), "rect")?;
            // A rectangle is four comma-separated numbers: take the next three items too.
            let mut parts = vec![first.trim()];
            for _ in 0..3 {
                match items.next() {
                    Some(p) => parts.push(p.trim()),
                    None => break,
                }
            }
            entry.rect = Some(parts.join(",").parse::<RectSpec>()?);
            found = true;
        } else if let Some(path) = item.strip_prefix("mask=") {
            once(entry.mask.is_some(), "mask")?;
            if path.trim().is_empty() {
                return Err("`mask=` needs a file name".into());
            }
            entry.mask = Some(MaskRef {
                path: path.trim().to_string(),
                crc32: None,
            });
            found = true;
        } else if item == "invert-mask" {
            entry.invert_mask = true;
            found = true;
        } else {
            rest.push(item);
        }
    }
    if !found {
        return parse_filter_spec(s).map(Into::into);
    }
    if entry.rect.is_some() && entry.mask.is_some() {
        return Err("`rect` and `mask` cannot be combined on one step".into());
    }
    if entry.invert_mask && entry.rect.is_none() && entry.mask.is_none() {
        return Err("`invert-mask` needs a `rect=` or a `mask=`".into());
    }
    let rest = rest.join(",");
    entry.config = if rest.trim().is_empty() {
        parse_filter_spec(name)?
    } else {
        parse_filter_spec(&format!("{name}:{rest}"))?
    };
    Ok(entry)
}

/// Parses a repeatable `-f / --filter <SPEC>` argument into a [`FilterConfig`].
///
/// Supported forms:
/// - `<filter>` (for unit filters such as `invert`, `remove-filter`, `sub-filter`, etc.)
/// - `<filter>=<value>` (shorthand for single-value filters, e.g. `replace=0.1`, `brighten=20`, `random-copy=5`, `shift-channels=10,0,-10`, `substitute=4:255`)
/// - `<filter>:<k1>=<v1>,<k2>=<v2>` (key-value parameters, e.g. `chromatic-aberration:magnitude=0.2,r=4,b=-4`)
pub fn parse_filter_spec(spec: &str) -> Result<FilterConfig, String> {
    let s = spec.trim();
    if s.is_empty() {
        return Err("filter specification cannot be empty".into());
    }

    // Separate filter name from parameters (`:` or `=`)
    let colon_pos = s.find(':');
    let eq_pos = s.find('=');

    let (raw_name, mode) = match (colon_pos, eq_pos) {
        (Some(c), Some(e)) if c < e => (&s[..c], Some((':', &s[c + 1..]))),
        (Some(c), Some(e)) => {
            // `=` appears before `:` (e.g. `substitute=10:255`)
            let _ = c;
            (&s[..e], Some(('=', &s[e + 1..])))
        }
        (Some(c), None) => (&s[..c], Some((':', &s[c + 1..]))),
        (None, Some(e)) => (&s[..e], Some(('=', &s[e + 1..]))),
        (None, None) => (s, None),
    };

    let norm = raw_name.trim().to_ascii_lowercase().replace('_', "-");

    match norm.as_str() {
        "invert" => {
            if mode.is_some() {
                return Err("`invert` does not take any parameters".into());
            }
            Ok(FilterConfig::Invert)
        }
        "remove-filter" | "removefilter" | "none" => {
            if mode.is_some() {
                return Err("`remove-filter` does not take any parameters".into());
            }
            Ok(FilterConfig::RemoveFilter)
        }
        "sub-filter" | "subfilter" | "sub" => {
            if mode.is_some() {
                return Err("`sub-filter` does not take any parameters".into());
            }
            Ok(FilterConfig::SubFilter)
        }
        "up-filter" | "upfilter" | "up" => {
            if mode.is_some() {
                return Err("`up-filter` does not take any parameters".into());
            }
            Ok(FilterConfig::UpFilter)
        }
        "average-filter" | "averagefilter" | "average" => {
            if mode.is_some() {
                return Err("`average-filter` does not take any parameters".into());
            }
            Ok(FilterConfig::AverageFilter)
        }
        "paeth-filter" | "paethfilter" | "paeth" => {
            if mode.is_some() {
                return Err("`paeth-filter` does not take any parameters".into());
            }
            Ok(FilterConfig::PaethFilter)
        }
        "set-filter-type" | "setfiltertype" => {
            let value = match mode {
                Some(('=', v)) => v,
                Some((':', rest)) => {
                    let mut filter = None;
                    for (k, v) in parse_kv_pairs(&norm, rest)? {
                        match k {
                            "filter" => filter = Some(v),
                            other => {
                                return Err(format!(
                                    "unknown parameter `{other}` for `set-filter-type` (expected `filter`)"
                                ));
                            }
                        }
                    }
                    filter.ok_or_else(|| "missing `filter` for `set-filter-type`".to_string())?
                }
                _ => {
                    return Err(
                        "`set-filter-type` requires a filter type (e.g. `-f set-filter-type=paeth`)"
                            .into(),
                    );
                }
            };
            Ok(FilterConfig::SetFilterType {
                filter: parse_filter_type(value)?,
            })
        }
        "regenerate" => {
            let value = match mode {
                Some(('=', v)) => v,
                Some((':', rest)) => {
                    let mut strategy = None;
                    for (k, v) in parse_kv_pairs(&norm, rest)? {
                        match k {
                            "strategy" => strategy = Some(v),
                            other => {
                                return Err(format!(
                                    "unknown parameter `{other}` for `regenerate` (expected `strategy`)"
                                ));
                            }
                        }
                    }
                    strategy.ok_or_else(|| "missing `strategy` for `regenerate`".to_string())?
                }
                _ => {
                    return Err(
                        "`regenerate` requires a strategy (e.g. `-f regenerate=paeth`)".into(),
                    );
                }
            };
            Ok(FilterConfig::Regenerate {
                strategy: parse_regenerate_strategy(value)?,
            })
        }
        "change-filter-type" | "changefiltertype" => {
            let magnitude = extract_single_magnitude(&norm, mode)?;
            Ok(FilterConfig::ChangeFilterType { magnitude })
        }
        "replace" => {
            let magnitude = extract_single_magnitude(&norm, mode)?;
            Ok(FilterConfig::Replace { magnitude })
        }
        "transpose" | "random-transpose" | "randomtranspose" => {
            let magnitude = extract_single_magnitude(&norm, mode)?;
            Ok(FilterConfig::Transpose { magnitude })
        }
        "set-zero" | "setzero" => {
            let magnitude = extract_single_magnitude(&norm, mode)?;
            Ok(FilterConfig::SetZero { magnitude })
        }
        "horizontal-shift" | "horizontalshift" => {
            let magnitude = extract_single_magnitude(&norm, mode)?;
            Ok(FilterConfig::HorizontalShift { magnitude })
        }
        "macroblock-glitch" | "macroblockglitch" => {
            let magnitude = extract_single_magnitude(&norm, mode)?;
            Ok(FilterConfig::MacroblockGlitch { magnitude })
        }
        "random-copy" | "randomcopy" => {
            let times = match mode {
                Some(('=', val)) => parse_random_copy_times(val)?,
                Some((':', rest)) => {
                    let mut times = None;
                    for (k, v) in parse_kv_pairs(&norm, rest)? {
                        match k {
                            "times" | "n" => {
                                times = Some(parse_random_copy_times(v)?);
                            }
                            other => {
                                return Err(format!(
                                    "unknown parameter `{other}` for `random-copy` (expected `times`)"
                                ));
                            }
                        }
                    }
                    times.ok_or_else(|| "missing `times` for `random-copy`".to_string())?
                }
                _ => return Err("`random-copy` requires a count (e.g. `-f random-copy=5`)".into()),
            };
            Ok(FilterConfig::RandomCopy { times })
        }
        "brighten" | "darken" => {
            let strength = match mode {
                Some(('=', val)) => val
                    .parse::<u16>()
                    .map_err(|e| format!("invalid `strength` for `{norm}`: {e}"))?,
                Some((':', rest)) => {
                    let mut strength = None;
                    for (k, v) in parse_kv_pairs(&norm, rest)? {
                        match k {
                            "strength" => {
                                strength = Some(
                                    v.parse::<u16>()
                                        .map_err(|e| format!("invalid `strength`: {e}"))?,
                                );
                            }
                            other => {
                                return Err(format!(
                                    "unknown parameter `{other}` for `{norm}` (expected `strength`)"
                                ));
                            }
                        }
                    }
                    strength.ok_or_else(|| format!("missing `strength` for `{norm}`"))?
                }
                _ => {
                    return Err(format!(
                        "`{norm}` requires a strength (e.g. `-f {norm}=20`)"
                    ));
                }
            };
            if norm == "brighten" {
                Ok(FilterConfig::Brighten { strength })
            } else {
                Ok(FilterConfig::Darken { strength })
            }
        }
        "substitute" => {
            let (index, value) = match mode {
                Some(('=', val)) => {
                    let arg = parse_substitute(val)?;
                    (arg.index, arg.value)
                }
                Some((':', rest)) => {
                    if !rest.contains('=') {
                        let arg = parse_substitute(rest)?;
                        (arg.index, arg.value)
                    } else {
                        let mut index = None;
                        let mut value = None;
                        for (k, v) in parse_kv_pairs(&norm, rest)? {
                            match k {
                                "index" | "idx" => {
                                    index = Some(
                                        v.parse::<usize>()
                                            .map_err(|e| format!("invalid `index`: {e}"))?,
                                    );
                                }
                                "value" | "val" => {
                                    value = Some(
                                        v.parse::<u8>()
                                            .map_err(|e| format!("invalid `value`: {e}"))?,
                                    );
                                }
                                other => {
                                    return Err(format!(
                                        "unknown parameter `{other}` for `substitute` (expected `index`, `value`)"
                                    ));
                                }
                            }
                        }
                        (
                            index.ok_or_else(|| "missing `index` for `substitute`".to_string())?,
                            value.ok_or_else(|| "missing `value` for `substitute`".to_string())?,
                        )
                    }
                }
                _ => {
                    return Err(
                        "`substitute` requires `INDEX:VALUE` (e.g. `-f substitute=10:255`)".into(),
                    );
                }
            };
            Ok(FilterConfig::Substitute { index, value })
        }
        "shift-channels" | "shiftchannels" => {
            let (r, g, b) = match mode {
                Some(('=', val)) => {
                    let arg = parse_shift_channels(val)?;
                    (arg.r, arg.g, arg.b)
                }
                Some((':', rest)) => {
                    if !rest.contains('=') {
                        let arg = parse_shift_channels(rest)?;
                        (arg.r, arg.g, arg.b)
                    } else {
                        let mut r = 0i16;
                        let mut g = 0i16;
                        let mut b = 0i16;
                        for (k, v) in parse_kv_pairs(&norm, rest)? {
                            match k {
                                "r" => {
                                    r = v.parse::<i16>().map_err(|e| format!("invalid `r`: {e}"))?
                                }
                                "g" => {
                                    g = v.parse::<i16>().map_err(|e| format!("invalid `g`: {e}"))?
                                }
                                "b" => {
                                    b = v.parse::<i16>().map_err(|e| format!("invalid `b`: {e}"))?
                                }
                                other => {
                                    return Err(format!(
                                        "unknown parameter `{other}` for `shift-channels` (expected `r`, `g`, `b`)"
                                    ));
                                }
                            }
                        }
                        (r, g, b)
                    }
                }
                _ => {
                    return Err(
                        "`shift-channels` requires `R,G,B` (e.g. `-f shift-channels=10,0,-10`)"
                            .into(),
                    );
                }
            };
            Ok(FilterConfig::ShiftChannels { r, g, b })
        }
        "pixel-sort" | "pixelsort" => {
            let mut magnitude = None;
            let mut criterion = None;
            match mode {
                Some(('=', val)) => magnitude = Some(parse_magnitude(val)?),
                Some((':', rest)) => {
                    for (k, v) in parse_kv_pairs(&norm, rest)? {
                        match k {
                            "magnitude" | "mag" => magnitude = Some(parse_magnitude(v)?),
                            "criterion" => {
                                criterion = Some(match v.to_ascii_lowercase().as_str() {
                                    "brightness" => SortCriterion::Brightness,
                                    "hue" => SortCriterion::Hue,
                                    _ => {
                                        return Err(format!(
                                            "invalid criterion `{v}` (expected `brightness` or `hue`)"
                                        ));
                                    }
                                });
                            }
                            other => {
                                return Err(format!(
                                    "unknown parameter `{other}` for `pixel-sort` (expected `magnitude`, `criterion`)"
                                ));
                            }
                        }
                    }
                }
                _ => {}
            }
            let magnitude = magnitude.ok_or_else(|| {
                "`pixel-sort` requires `magnitude` (e.g. `-f pixel-sort=0.2` or `-f 'pixel-sort:magnitude=0.2,criterion=hue'`)".to_string()
            })?;
            Ok(FilterConfig::PixelSort {
                magnitude,
                criterion,
            })
        }
        "bitwise" => {
            let mut magnitude = None;
            let mut op = None;
            let mut value = None;
            match mode {
                Some(('=', val)) => magnitude = Some(parse_magnitude(val)?),
                Some((':', rest)) => {
                    for (k, v) in parse_kv_pairs(&norm, rest)? {
                        match k {
                            "magnitude" | "mag" => magnitude = Some(parse_magnitude(v)?),
                            "op" => {
                                op = Some(match v.to_ascii_lowercase().as_str() {
                                    "and" => BitOp::And,
                                    "or" => BitOp::Or,
                                    "xor" => BitOp::Xor,
                                    _ => {
                                        return Err(format!(
                                            "invalid bitwise op `{v}` (expected `and`, `or`, `xor`)"
                                        ));
                                    }
                                });
                            }
                            "value" | "val" => {
                                value = Some(
                                    v.parse::<u8>()
                                        .map_err(|e| format!("invalid `value`: {e}"))?,
                                );
                            }
                            other => {
                                return Err(format!(
                                    "unknown parameter `{other}` for `bitwise` (expected `magnitude`, `op`, `value`)"
                                ));
                            }
                        }
                    }
                }
                _ => {}
            }
            let magnitude = magnitude.ok_or_else(|| {
                "`bitwise` requires `magnitude` (e.g. `-f 'bitwise:magnitude=0.1,op=xor,value=255'`)".to_string()
            })?;
            Ok(FilterConfig::Bitwise {
                magnitude,
                op,
                value,
            })
        }
        "channel-swap" | "channelswap" => {
            let mut magnitude = None;
            let mut target = None;
            match mode {
                Some(('=', val)) => magnitude = Some(parse_magnitude(val)?),
                Some((':', rest)) => {
                    for (k, v) in parse_kv_pairs(&norm, rest)? {
                        match k {
                            "magnitude" | "mag" => magnitude = Some(parse_magnitude(v)?),
                            "target" => {
                                target = Some(match v.to_ascii_lowercase().as_str() {
                                    "rg" => SwapTarget::Rg,
                                    "gb" => SwapTarget::Gb,
                                    "br" => SwapTarget::Br,
                                    _ => {
                                        return Err(format!(
                                            "invalid target `{v}` (expected `rg`, `gb`, `br`)"
                                        ));
                                    }
                                });
                            }
                            other => {
                                return Err(format!(
                                    "unknown parameter `{other}` for `channel-swap` (expected `magnitude`, `target`)"
                                ));
                            }
                        }
                    }
                }
                _ => {}
            }
            let magnitude = magnitude.ok_or_else(|| {
                "`channel-swap` requires `magnitude` (e.g. `-f 'channel-swap:magnitude=0.2,target=br'`)".to_string()
            })?;
            Ok(FilterConfig::ChannelSwap { magnitude, target })
        }
        "block-scramble" | "blockscramble" => {
            let mut magnitude = None;
            let mut block_size = None;
            match mode {
                Some(('=', val)) => magnitude = Some(parse_magnitude(val)?),
                Some((':', rest)) => {
                    for (k, v) in parse_kv_pairs(&norm, rest)? {
                        match k {
                            "magnitude" | "mag" => magnitude = Some(parse_magnitude(v)?),
                            "block_size" | "block-size" | "size" => {
                                block_size = Some(
                                    v.parse::<u32>()
                                        .map_err(|e| format!("invalid `block_size`: {e}"))?,
                                );
                            }
                            other => {
                                return Err(format!(
                                    "unknown parameter `{other}` for `block-scramble` (expected `magnitude`, `block_size`)"
                                ));
                            }
                        }
                    }
                }
                _ => {}
            }
            let magnitude = magnitude.ok_or_else(|| {
                "`block-scramble` requires `magnitude` (e.g. `-f 'block-scramble:magnitude=0.1,block_size=32'`)".to_string()
            })?;
            Ok(FilterConfig::BlockScramble {
                magnitude,
                block_size,
            })
        }
        "color-distortion" | "colordistortion" => {
            let mut magnitude = None;
            let mut strength = None;
            match mode {
                Some(('=', val)) => magnitude = Some(parse_magnitude(val)?),
                Some((':', rest)) => {
                    for (k, v) in parse_kv_pairs(&norm, rest)? {
                        match k {
                            "magnitude" | "mag" => magnitude = Some(parse_magnitude(v)?),
                            "strength" => {
                                strength = Some(parse_non_negative_strength(v)?);
                            }
                            other => {
                                return Err(format!(
                                    "unknown parameter `{other}` for `color-distortion` (expected `magnitude`, `strength`)"
                                ));
                            }
                        }
                    }
                }
                _ => {}
            }
            let magnitude = magnitude.ok_or_else(|| {
                "`color-distortion` requires `magnitude` (e.g. `-f 'color-distortion:magnitude=0.2,strength=30'`)".to_string()
            })?;
            Ok(FilterConfig::ColorDistortion {
                magnitude,
                strength,
            })
        }
        "color-space-glitch" | "colorspaceglitch" => {
            let mut magnitude = None;
            let mut hue_shift = None;
            let mut saturation_mult = None;
            let mut lightness_mult = None;
            match mode {
                Some(('=', val)) => magnitude = Some(parse_magnitude(val)?),
                Some((':', rest)) => {
                    for (k, v) in parse_kv_pairs(&norm, rest)? {
                        match k {
                            "magnitude" | "mag" => magnitude = Some(parse_magnitude(v)?),
                            "hue_shift" | "hue-shift" | "hue" => {
                                hue_shift = Some(parse_finite_f64(v)?);
                            }
                            "saturation_mult" | "saturation-mult" | "saturation" | "sat" => {
                                saturation_mult = Some(parse_non_negative_f64(v)?);
                            }
                            "lightness_mult" | "lightness-mult" | "lightness" | "light" => {
                                lightness_mult = Some(parse_non_negative_f64(v)?);
                            }
                            other => {
                                return Err(format!(
                                    "unknown parameter `{other}` for `color-space-glitch` (expected `magnitude`, `hue_shift`, `saturation_mult`, `lightness_mult`)"
                                ));
                            }
                        }
                    }
                }
                _ => {}
            }
            let magnitude = magnitude.ok_or_else(|| {
                "`color-space-glitch` requires `magnitude` (e.g. `-f 'color-space-glitch:magnitude=0.2,hue_shift=90'`)".to_string()
            })?;
            Ok(FilterConfig::ColorSpaceGlitch {
                magnitude,
                hue_shift,
                saturation_mult,
                lightness_mult,
            })
        }
        "chromatic-aberration" | "chromaticaberration" => {
            let mut magnitude = None;
            let mut r_offset = None;
            let mut g_offset = None;
            let mut b_offset = None;
            match mode {
                Some(('=', val)) => magnitude = Some(parse_magnitude(val)?),
                Some((':', rest)) => {
                    for (k, v) in parse_kv_pairs(&norm, rest)? {
                        match k {
                            "magnitude" | "mag" => magnitude = Some(parse_magnitude(v)?),
                            "r_offset" | "r-offset" | "r" => {
                                r_offset = Some(
                                    v.parse::<i32>()
                                        .map_err(|e| format!("invalid `r_offset`: {e}"))?,
                                );
                            }
                            "g_offset" | "g-offset" | "g" => {
                                g_offset = Some(
                                    v.parse::<i32>()
                                        .map_err(|e| format!("invalid `g_offset`: {e}"))?,
                                );
                            }
                            "b_offset" | "b-offset" | "b" => {
                                b_offset = Some(
                                    v.parse::<i32>()
                                        .map_err(|e| format!("invalid `b_offset`: {e}"))?,
                                );
                            }
                            other => {
                                return Err(format!(
                                    "unknown parameter `{other}` for `chromatic-aberration` (expected `magnitude`, `r_offset`/`r`, `g_offset`/`g`, `b_offset`/`b`)"
                                ));
                            }
                        }
                    }
                }
                _ => {}
            }
            let magnitude = magnitude.ok_or_else(|| {
                "`chromatic-aberration` requires `magnitude` (e.g. `-f 'chromatic-aberration:magnitude=0.2,r=3,b=-3'`)".to_string()
            })?;
            Ok(FilterConfig::ChromaticAberration {
                magnitude,
                r_offset,
                g_offset,
                b_offset,
            })
        }
        "alpha-glitch" | "alphaglitch" => {
            let mut magnitude = 1.0;
            let mut strategy = None;
            let parse_strat = |v: &str| -> Result<AlphaGlitchStrategy, String> {
                match v.to_ascii_lowercase().as_str() {
                    "invert" => Ok(AlphaGlitchStrategy::Invert),
                    "randomize" => Ok(AlphaGlitchStrategy::Randomize),
                    "zero" => Ok(AlphaGlitchStrategy::Zero),
                    "one" => Ok(AlphaGlitchStrategy::One),
                    _ => Err(format!(
                        "invalid alpha-glitch strategy `{v}` (expected `invert`, `randomize`, `zero`, `one`)"
                    )),
                }
            };
            match mode {
                Some(('=', val)) => strategy = Some(parse_strat(val)?),
                Some((':', rest)) => {
                    for (k, v) in parse_kv_pairs(&norm, rest)? {
                        match k {
                            "magnitude" | "mag" => magnitude = parse_magnitude(v)?,
                            "strategy" => strategy = Some(parse_strat(v)?),
                            other => {
                                return Err(format!(
                                    "unknown parameter `{other}` for `alpha-glitch` (expected `magnitude`, `strategy`)"
                                ));
                            }
                        }
                    }
                }
                _ => {
                    return Err(
                        "`alpha-glitch` requires a strategy (e.g. `-f alpha-glitch=invert`)".into(),
                    );
                }
            }
            Ok(FilterConfig::AlphaGlitch {
                magnitude,
                strategy,
            })
        }
        "lossy-artifact" | "lossyartifact" | "lossy-quality" | "lossyquality" => {
            let quality = match mode {
                Some(('=', val)) => parse_quality(val)?,
                Some((':', rest)) => {
                    let mut quality = None;
                    for (k, v) in parse_kv_pairs(&norm, rest)? {
                        match k {
                            "quality" | "q" => quality = Some(parse_quality(v)?),
                            other => {
                                return Err(format!(
                                    "unknown parameter `{other}` for `lossy-artifact` (expected `quality`)"
                                ))
                            }
                        }
                    }
                    quality.ok_or_else(|| "missing `quality` for `lossy-artifact`".to_string())?
                }
                _ => {
                    return Err(
                        "`lossy-artifact` requires `quality` in 0.0..=100.0 (e.g. `-f lossy-artifact=15`)"
                            .into(),
                    )
                }
            };
            Ok(FilterConfig::LossyArtifact { quality })
        }
        other => Err(format!(
            "unknown filter `{other}` (run `png-glitch --list-filters` to see all available filters)"
        )),
    }
}

fn extract_single_magnitude(name: &str, mode: Option<(char, &str)>) -> Result<f64, String> {
    match mode {
        Some(('=', val)) => parse_magnitude(val),
        Some((':', rest)) => {
            let mut magnitude = None;
            for (k, v) in parse_kv_pairs(name, rest)? {
                match k {
                    "magnitude" | "mag" => magnitude = Some(parse_magnitude(v)?),
                    other => {
                        return Err(format!(
                            "unknown parameter `{other}` for `{name}` (expected `magnitude`)"
                        ));
                    }
                }
            }
            magnitude.ok_or_else(|| format!("missing `magnitude` for `{name}`"))
        }
        _ => Err(format!(
            "`{name}` requires a magnitude in 0.0..=1.0 (e.g. `-f {name}=0.1`)"
        )),
    }
}

pub fn format_filter_list() -> &'static str {
    "\
Available Glitch Filters (use with `-f / --filter <SPEC>` or in YAML `--config`):

  Shared Filters (PNG & WebP):
    invert
      Inverts all color channels.
      Example: -f invert

    brighten=<STRENGTH> | brighten:strength=<N>
      Increases brightness (saturating at bit-depth maximum). [default strength: 20]
      Example: -f brighten=30

    darken=<STRENGTH> | darken:strength=<N>
      Decreases brightness (saturating at 0). [default strength: 20]
      Example: -f darken=30

    shift-channels=<R,G,B> | shift-channels:r=<R>,g=<G>,b=<B>
      Shifts color channels by signed offsets. [default: r=0, g=0, b=0]
      Example: -f shift-channels=15,0,-15

    pixel-sort=<MAG> | pixel-sort:magnitude=<MAG>[,criterion=brightness|hue]
      Sorts pixels within selected scanlines. [MAG: 0.0..=1.0, default criterion: brightness]
      Example: -f 'pixel-sort:magnitude=0.2,criterion=hue'

    bitwise=<MAG> | bitwise:magnitude=<MAG>[,op=and|or|xor][,value=<0..255>]
      Applies a bitwise operation to scanline bytes. [MAG: 0.0..=1.0, default: op=xor, value=0]
      Example: -f 'bitwise:magnitude=0.05,op=xor,value=255'

    channel-swap=<MAG> | channel-swap:magnitude=<MAG>[,target=rg|gb|br]
      Swaps two color channels in selected scanlines. [MAG: 0.0..=1.0, default target: rg]
      Example: -f 'channel-swap:magnitude=0.2,target=br'

    horizontal-shift=<MAG>
      Rotates scanline bytes horizontally by a random offset. [MAG: 0.0..=1.0]
      Example: -f horizontal-shift=0.1

    color-distortion=<MAG> | color-distortion:magnitude=<MAG>[,strength=<N>]
      Adds random per-pixel color noise. [MAG: 0.0..=1.0, default strength: 20]
      Example: -f 'color-distortion:magnitude=0.2,strength=40'

    chromatic-aberration=<MAG> | chromatic-aberration:magnitude=<MAG>[,r=<N>][,g=<N>][,b=<N>]
      Horizontally shifts R, G, B channels. [MAG: 0.0..=1.0, default: r=2, g=0, b=-2]
      Example: -f 'chromatic-aberration:magnitude=0.2,r=4,b=-4'

  PNG-only Filters:
    block-scramble=<MAG> | block-scramble:magnitude=<MAG>[,block_size=<PX>]
      Swaps square blocks of pixels. [MAG: 0.0..=1.0, default block_size: 16]
      Example: -f 'block-scramble:magnitude=0.1,block_size=32'

    color-space-glitch=<MAG> | color-space-glitch:magnitude=<MAG>[,hue_shift=<DEG>][,saturation_mult=<F>][,lightness_mult=<F>]
      Transforms colors in HSL space. [MAG: 0.0..=1.0, default: hue_shift=0.0, saturation_mult=1.0, lightness_mult=1.0]
      Example: -f 'color-space-glitch:magnitude=0.2,hue_shift=90,saturation_mult=1.5'

    change-filter-type=<MAG>
      Randomly overwrites PNG scanline filter type bytes without re-filtering. [MAG: 0.0..=1.0]
      Example: -f change-filter-type=0.1

    replace=<MAG>
      Replaces random scanline bytes with random values. [MAG: 0.0..=1.0]
      Example: -f replace=0.05

    transpose=<MAG>
      Randomly swaps scanlines. [MAG: 0.0..=1.0]
      Example: -f transpose=0.1

    set-zero=<MAG>
      Sets random scanline bytes to 0. [MAG: 0.0..=1.0]
      Example: -f set-zero=0.05

    random-copy=<TIMES>
      Copies random scanlines onto other scanlines N times.
      Example: -f random-copy=10

    substitute=<INDEX:VALUE> | substitute:index=<IDX>,value=<0..255>
      Sets byte at INDEX to VALUE on every scanline.
      Example: -f substitute=0:255

    set-filter-type=<TYPE>
      Overwrites the filter type tag of every PNG scanline and leaves the filtered bytes as they
      are (tag only; no value is recomputed). [TYPE: none|sub|up|average|paeth]
      Example: -f set-filter-type=paeth

    regenerate=<STRATEGY>
      Starts a new generation: recomputes every filtered value (PNG only). Glitches so far are
      baked into the pixels. [STRATEGY: a TYPE (none|sub|up|average|paeth), `adaptive`, or a
      comma-separated pattern such as `sub,up,paeth`]
      Example: -f regenerate=paeth

    remove-filter | sub-filter | up-filter | average-filter | paeth-filter  [deprecated]
      Re-encode all PNG scanlines with a fixed PNG prediction filter (recompute the values).
      Use `regenerate=<TYPE>` instead; the output is the same.
      Example: -f sub-filter

  WebP-only Filters:
    macroblock-glitch=<MAG>
      Swaps 16x16 macroblocks along VP8 block boundaries. [MAG: 0.0..=1.0]
      Example: -f macroblock-glitch=0.2

    alpha-glitch=<STRATEGY> | alpha-glitch:magnitude=<MAG>,strategy=invert|randomize|zero|one
      Corrupts the alpha channel. [MAG: 0.0..=1.0 (default 1.0), default strategy: invert]
      Example: -f alpha-glitch=randomize

    lossy-artifact=<QUALITY> | lossy-artifact:quality=<0.0..100.0>
      Lowers WebP re-encoding quality to introduce compression artifacts. [QUALITY: 0.0..=100.0]
      Example: -f lossy-artifact=10
"
}

const AFTER_HELP_EXAMPLES: &str = "\
Examples:
  # Glitch a PNG or WebP image (writes to <input-stem>-glitched.<ext>)
  png-glitch input.png -f sub-filter -f replace=0.05
  png-glitch input.webp -f 'chromatic-aberration:magnitude=0.2,r=4,b=-4' -f lossy-artifact=15

  # List all filters, their parameters, and default values
  png-glitch --list-filters

  # Run a repeatable YAML pipeline with a fixed seed
  png-glitch input.png --config pipeline.yaml --seed 42 -o result.png --force

  # Try 12 seeds and a contact sheet; re-run a favourite with --seed <printed seed>
  png-glitch input.png --variations 12 --contact-sheet sheet.png -f replace=0.05

  # Random recipe; the recipe and seed are printed so a lucky result can be re-run
  png-glitch input.png --surprise --save-recipe lucky.yaml

  # Sweep one parameter: five outputs, same seed, only the magnitude changes
  png-glitch input.png -f replace=0.1 --sweep replace.magnitude=0.01..0.2:5 --seed 1

  # Tune a recipe with an image viewer open on the output
  png-glitch input.png --config recipe.yaml --seed 42 --watch

  # Batch process a directory of PNG and WebP images
  png-glitch ./images --batch-output ./glitched -f invert -f block-scramble=0.1";

#[derive(Parser, Debug)]
#[command(
    name = "png-glitch",
    author,
    version,
    about = "A CLI tool to create glitch art from PNG and WebP images.",
    long_about = None,
    after_help = AFTER_HELP_EXAMPLES
)]
pub struct Cli {
    /// Input PNG or WebP file (or directory when used with `--batch-output`)
    #[arg(value_name = "INPUT", required_unless_present = "list_filters")]
    pub input: Option<String>,

    /// List all available glitch filters with their parameters and defaults
    #[arg(long)]
    pub list_filters: bool,

    /// Output file path [default: `<input-stem>-glitched.<ext>`]
    #[arg(
        short,
        long = "output",
        value_name = "FILE",
        conflicts_with = "batch_output"
    )]
    pub output_file: Option<String>,

    /// Overwrite existing output file(s) without prompting
    #[arg(long)]
    pub force: bool,

    /// Enforce strict PNG integrity checks (CRC, consecutive IDAT, filter bytes, trailing data)
    #[arg(long)]
    pub strict: bool,

    /// Random seed for reproducible output (printed to stderr if omitted)
    #[arg(long, value_name = "SEED")]
    pub seed: Option<u64>,

    /// Batch process all PNG and WebP images in INPUT directory and save to DIR
    #[arg(long, value_name = "DIR")]
    pub batch_output: Option<String>,

    /// Generate N variations with different seeds, saved as `<out-stem>-s<seed>.<ext>`
    #[arg(
        long,
        value_name = "N",
        value_parser = clap::value_parser!(u32).range(1..=10_000),
        conflicts_with_all = ["batch_output", "list_filters", "show_recipe"],
        help_heading = "Exploration"
    )]
    pub variations: Option<u32>,

    /// Apply a random recipe of 2-5 steps drawn from the seed; the recipe is always printed
    /// (re-run it with `--config saved.yaml --seed S`)
    #[arg(
        long,
        conflicts_with_all = ["batch_output", "list_filters", "show_recipe", "variations", "sweep", "watch", "config"],
        help_heading = "Exploration"
    )]
    pub surprise: bool,

    /// Sweep one numeric parameter of one step over N evenly spaced values with a fixed
    /// seed, saved as `<out-stem>-<param>-<value>.<ext>`. STEP is a 0-based index or a step
    /// type name (first match)
    #[arg(
        long,
        value_name = "STEP.PARAM=FROM..TO:N",
        value_parser = crate::sweep::parse_sweep,
        conflicts_with_all = ["batch_output", "list_filters", "show_recipe", "variations"],
        help_heading = "Exploration"
    )]
    pub sweep: Option<crate::sweep::SweepSpec>,

    /// Also write a contact sheet of all variations to FILE (PNG input only)
    #[arg(
        long,
        value_name = "FILE",
        requires = "variations",
        help_heading = "Exploration"
    )]
    pub contact_sheet: Option<String>,

    /// Width in pixels of each contact-sheet cell
    #[arg(
        long,
        value_name = "PX",
        default_value_t = 256,
        value_parser = clap::value_parser!(u32).range(1..=4096),
        requires = "contact_sheet",
        help_heading = "Exploration"
    )]
    pub contact_cell_width: u32,

    /// Re-render whenever the `--config` file or the input image changes (polled every 250 ms; Ctrl-C to stop)
    #[arg(
        long,
        requires = "config",
        conflicts_with_all = ["batch_output", "variations", "sweep", "list_filters", "show_recipe"],
        help_heading = "Exploration"
    )]
    pub watch: bool,

    /// Path to YAML config file
    #[arg(long, value_name = "FILE")]
    pub config: Option<String>,

    /// Print the effective recipe (flags, `-f` and `--config` merged, in order) and the seed as YAML to stderr
    #[arg(long, help_heading = "Recipe")]
    pub print_recipe: bool,

    /// Save the effective recipe as YAML to FILE; it can be fed back with `--config FILE --seed SEED`
    #[arg(long, value_name = "FILE", help_heading = "Recipe")]
    pub save_recipe: Option<String>,

    /// Embed the effective recipe and seed in the output PNG (an `iTXt` chunk; PNG only)
    #[arg(long, help_heading = "Recipe")]
    pub embed_recipe: bool,

    /// Print the recipe embedded in INPUT (written by `--embed-recipe`) and exit
    #[arg(long, help_heading = "Recipe")]
    pub show_recipe: bool,

    /// Add a filter step to the pipeline (repeatable, applied in order; see `--list-filters`).
    /// Add `rows=a..b`, `rect=x,y,w,h` or `mask=FILE` (and `invert-mask`) to the parameters to restrict the step to a region, e.g. `replace:magnitude=0.2,rows=300..452` or `invert:rect=10,20,100,50`
    #[arg(
        short = 'f',
        long = "filter",
        value_name = "SPEC",
        value_parser = parse_filter_entry,
        help_heading = "Glitch Filters"
    )]
    pub filters: Vec<FilterEntry>,

    /// Randomly overwrite PNG scanline filter type bytes [0.0..=1.0] (PNG only)
    #[arg(long, value_name = "MAG", value_parser = parse_magnitude, help_heading = "Glitch Filters")]
    pub change_filter_type: Option<f64>,

    /// Replace random scanline bytes with noise [0.0..=1.0] (PNG only)
    #[arg(long, value_name = "MAG", value_parser = parse_magnitude, help_heading = "Glitch Filters")]
    pub replace: Option<f64>,

    /// Randomly swap scanlines [0.0..=1.0] (PNG only)
    #[arg(long, value_name = "MAG", value_parser = parse_magnitude, help_heading = "Glitch Filters")]
    pub transpose: Option<f64>,

    /// Set random scanline bytes to zero [0.0..=1.0] (PNG only)
    #[arg(long, value_name = "MAG", value_parser = parse_magnitude, help_heading = "Glitch Filters")]
    pub set_zero: Option<f64>,

    /// Copy random scanlines N times (PNG only)
    #[arg(long, value_name = "N", value_parser = parse_random_copy_times, help_heading = "Glitch Filters")]
    pub random_copy: Option<u32>,

    /// Substitute byte at INDEX with VALUE (`INDEX:VALUE`, PNG only)
    #[arg(long, value_name = "INDEX:VALUE", value_parser = parse_substitute, help_heading = "Glitch Filters")]
    pub substitute: Option<SubstituteArg>,

    /// Sort pixels within scanlines [0.0..=1.0] (configure with `--pixel-sort-criterion`)
    #[arg(long, value_name = "MAG", value_parser = parse_magnitude, help_heading = "Glitch Filters")]
    pub pixel_sort: Option<f64>,

    /// Criterion for `--pixel-sort`
    #[arg(long, value_name = "CRITERION", value_enum, default_value_t = SortCriterion::Brightness, help_heading = "Glitch Filters")]
    pub pixel_sort_criterion: SortCriterion,

    /// Apply bitwise operation to scanline bytes [0.0..=1.0] (configure with `--bitwise-op`, `--bitwise-value`)
    #[arg(long, value_name = "MAG", value_parser = parse_magnitude, help_heading = "Glitch Filters")]
    pub bitwise: Option<f64>,

    /// Operator for `--bitwise`
    #[arg(long, value_name = "OP", value_enum, default_value_t = BitOp::Xor, help_heading = "Glitch Filters")]
    pub bitwise_op: BitOp,

    /// Byte operand (0..=255) for `--bitwise`
    #[arg(
        long,
        value_name = "BYTE",
        default_value_t = 0,
        help_heading = "Glitch Filters"
    )]
    pub bitwise_value: u8,

    /// Swap two color channels [0.0..=1.0] (configure with `--channel-swap-target`)
    #[arg(long, value_name = "MAG", value_parser = parse_magnitude, help_heading = "Glitch Filters")]
    pub channel_swap: Option<f64>,

    /// Channel pair for `--channel-swap`
    #[arg(long, value_name = "TARGET", value_enum, default_value_t = SwapTarget::Rg, help_heading = "Glitch Filters")]
    pub channel_swap_target: SwapTarget,

    /// Shift scanlines horizontally [0.0..=1.0]
    #[arg(long, value_name = "MAG", value_parser = parse_magnitude, help_heading = "Glitch Filters")]
    pub horizontal_shift: Option<f64>,

    /// Scramble square pixel blocks [0.0..=1.0] (PNG only; configure with `--block-scramble-size`)
    #[arg(long, value_name = "MAG", value_parser = parse_magnitude, help_heading = "Glitch Filters")]
    pub block_scramble: Option<f64>,

    /// Block size in pixels for `--block-scramble`
    #[arg(
        long,
        value_name = "PX",
        default_value_t = 16,
        help_heading = "Glitch Filters"
    )]
    pub block_scramble_size: u32,

    /// Add random color distortion [0.0..=1.0] (configure with `--color-distortion-strength`)
    #[arg(long, value_name = "MAG", value_parser = parse_magnitude, help_heading = "Glitch Filters")]
    pub color_distortion: Option<f64>,

    /// Distortion amplitude for `--color-distortion` (>= 0)
    #[arg(long, value_name = "N", default_value_t = 20, value_parser = parse_non_negative_strength, allow_hyphen_values = true, help_heading = "Glitch Filters")]
    pub color_distortion_strength: i16,

    /// Transform colors in HSL space [0.0..=1.0] (PNG only; configure with `--hue-shift`, `--saturation-mult`, `--lightness-mult`)
    #[arg(long, value_name = "MAG", value_parser = parse_magnitude, help_heading = "Glitch Filters")]
    pub color_space_glitch: Option<f64>,

    /// Hue shift in degrees (0.0..=360.0) for `--color-space-glitch`
    #[arg(long, value_name = "DEG", default_value_t = 0.0, value_parser = parse_finite_f64, allow_hyphen_values = true, help_heading = "Glitch Filters")]
    pub hue_shift: f64,

    /// Saturation multiplier (>= 0.0) for `--color-space-glitch`
    #[arg(long, value_name = "MULT", default_value_t = 1.0, value_parser = parse_non_negative_f64, help_heading = "Glitch Filters")]
    pub saturation_mult: f64,

    /// Lightness multiplier (>= 0.0) for `--color-space-glitch`
    #[arg(long, value_name = "MULT", default_value_t = 1.0, value_parser = parse_non_negative_f64, help_heading = "Glitch Filters")]
    pub lightness_mult: f64,

    /// Horizontally offset R/G/B channels [0.0..=1.0] (configure with `--r-offset`, `--g-offset`, `--b-offset`)
    #[arg(long, value_name = "MAG", value_parser = parse_magnitude, help_heading = "Glitch Filters")]
    pub chromatic_aberration: Option<f64>,

    /// Red channel horizontal offset in pixels for `--chromatic-aberration`
    #[arg(
        long,
        value_name = "PX",
        default_value_t = 2,
        allow_hyphen_values = true,
        help_heading = "Glitch Filters"
    )]
    pub r_offset: i32,

    /// Green channel horizontal offset in pixels for `--chromatic-aberration`
    #[arg(
        long,
        value_name = "PX",
        default_value_t = 0,
        allow_hyphen_values = true,
        help_heading = "Glitch Filters"
    )]
    pub g_offset: i32,

    /// Blue channel horizontal offset in pixels for `--chromatic-aberration`
    #[arg(long, value_name = "PX", default_value_t = -2, allow_hyphen_values = true, help_heading = "Glitch Filters")]
    pub b_offset: i32,

    /// Invert all color channels
    #[arg(long, help_heading = "Glitch Filters")]
    pub invert: bool,

    /// Increase brightness by STRENGTH (saturating at bit-depth maximum)
    #[arg(long, value_name = "STRENGTH", help_heading = "Glitch Filters")]
    pub brighten: Option<u16>,

    /// Decrease brightness by STRENGTH (saturating at 0)
    #[arg(long, value_name = "STRENGTH", help_heading = "Glitch Filters")]
    pub darken: Option<u16>,

    /// Shift R, G, B channels by signed offsets (`R,G,B`)
    #[arg(long, value_name = "R,G,B", value_parser = parse_shift_channels, allow_hyphen_values = true, help_heading = "Glitch Filters")]
    pub shift_channels: Option<ShiftChannelsArg>,

    /// Re-encode all PNG scanlines with a uniform prediction filter before applying glitch filters (PNG only)
    #[arg(
        long,
        value_name = "FILTER",
        value_enum,
        help_heading = "PNG Prediction Filter Re-encoding"
    )]
    pub pre_process: Option<PreProcess>,

    /// [deprecated: use `-f regenerate=none`] Re-encode all PNG scanlines with PNG prediction filter `None` (pipeline step, PNG only)
    #[arg(long, help_heading = "PNG Prediction Filter Re-encoding")]
    pub remove_filter: bool,

    /// [deprecated: use `-f regenerate=sub`] Re-encode all PNG scanlines with PNG prediction filter `Sub` (pipeline step, PNG only)
    #[arg(
        long,
        visible_alias = "sub-filter",
        help_heading = "PNG Prediction Filter Re-encoding"
    )]
    pub sub: bool,

    /// [deprecated: use `-f regenerate=up`] Re-encode all PNG scanlines with PNG prediction filter `Up` (pipeline step, PNG only)
    #[arg(
        long,
        visible_alias = "up-filter",
        help_heading = "PNG Prediction Filter Re-encoding"
    )]
    pub up: bool,

    /// [deprecated: use `-f regenerate=average`] Re-encode all PNG scanlines with PNG prediction filter `Average` (pipeline step, PNG only)
    #[arg(
        long,
        visible_alias = "average-filter",
        help_heading = "PNG Prediction Filter Re-encoding"
    )]
    pub average: bool,

    /// [deprecated: use `-f regenerate=paeth`] Re-encode all PNG scanlines with PNG prediction filter `Paeth` (pipeline step, PNG only)
    #[arg(
        long,
        visible_alias = "paeth-filter",
        help_heading = "PNG Prediction Filter Re-encoding"
    )]
    pub paeth: bool,

    /// Swap 16x16 macroblocks along VP8 boundaries [0.0..=1.0] (WebP only)
    #[arg(long, value_name = "MAG", value_parser = parse_magnitude, help_heading = "WebP Filters")]
    pub macroblock_glitch: Option<f64>,

    /// Corrupt alpha channel using STRATEGY (WebP only)
    #[arg(
        long,
        value_name = "STRATEGY",
        value_enum,
        help_heading = "WebP Filters"
    )]
    pub alpha_glitch: Option<AlphaGlitchStrategy>,

    /// Lower WebP re-encoding quality for lossy artifacts [0.0..=100.0] (WebP only)
    #[arg(long, value_name = "QUALITY", value_parser = parse_quality, help_heading = "WebP Filters")]
    pub lossy_quality: Option<f32>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConfigFile {
    pub filters: Vec<FilterEntry>,
}
