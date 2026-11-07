//! The pre-`ScanLinesEffect` implementations of the effects, kept as the oracle for the
//! property tests in `targeted.rs`: a ported effect must give the same bytes as these.

use super::*;
use crate::{PngGlitch, ScanLine};

/// Runs `f` on each listed row, in the given order. Rows outside the image are ignored.
fn for_rows(png: &mut PngGlitch, rows: &[u32], mut f: impl FnMut(&mut ScanLine)) {
    let mut lines = png.scan_lines();
    for &row in rows {
        if let Some(line) = lines.get_mut(row as usize) {
            f(line);
        }
    }
}

pub(super) fn set_filter_types(this: &SetFilterTypes, png: &mut PngGlitch) {
    let mut lines = png.scan_lines();
    for (line, filter_type) in lines.iter_mut().zip(&this.filter_types) {
        if let Some(filter_type) = filter_type {
            line.set_filter_type(*filter_type);
        }
    }
}

pub(super) fn set_bytes(this: &SetBytes, png: &mut PngGlitch) {
    let mut lines = png.scan_lines();
    for edit in &this.edits {
        if let Some(line) = lines.get_mut(edit.line as usize) {
            line.update(edit.index, edit.value);
        }
    }
}

pub(super) fn substitute(this: &Substitute, png: &mut PngGlitch) {
    png.foreach_scanline(|line| {
        if this.index < line.size() {
            line.update(this.index, this.value);
        }
    });
}

pub(super) fn swap_lines(this: &SwapLines, png: &mut PngGlitch) {
    let height = png.height();
    for &(a, b) in &this.swaps {
        if a != b && a < height && b < height {
            png.transpose(a, b, 1);
        }
    }
}

pub(super) fn copy_lines(this: &CopyLines, png: &mut PngGlitch) {
    let mut lines = png.scan_lines();
    for &(src, dst) in &this.copies {
        let (src, dst) = (src as usize, dst as usize);
        if src == dst || src >= lines.len() || dst >= lines.len() {
            continue;
        }
        let Some(source) = lines.get(src) else {
            continue;
        };
        let filter_type = source.filter_type();
        let buffer: Vec<u8> = (0..source.size())
            .map(|i| source.index(i).unwrap_or(0))
            .collect();
        let Some(dest) = lines.get_mut(dst) else {
            continue;
        };
        for (i, &byte) in buffer.iter().enumerate() {
            dest.update(i, byte);
        }
        dest.set_filter_type(filter_type);
    }
}

pub(super) fn shift_lines(this: &ShiftLines, png: &mut PngGlitch) {
    let mut lines = png.scan_lines();
    for s in &this.shifts {
        let Some(line) = lines.get_mut(s.line as usize) else {
            continue;
        };
        let size = line.size();
        if size <= 1 {
            continue;
        }
        let shift = s.shift % size;
        let buffer: Vec<u8> = (0..size).map(|i| line.index(i).unwrap_or(0)).collect();
        for (i, &byte) in buffer.iter().enumerate() {
            line.update((i + shift) % size, byte);
        }
    }
}

pub(super) fn pixel_sort(this: &PixelSort, png: &mut PngGlitch) {
    for_rows(png, &this.rows, |line| {
        let mut pixels = Vec::with_capacity(line.pixels_count() as usize);
        for i in 0..line.pixels_count() as usize {
            if let Some(p) = line.get_pixel(i) {
                pixels.push(p);
            }
        }
        pixels.sort_by(|a, b| {
            this.value(a)
                .partial_cmp(&this.value(b))
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        for (i, p) in pixels.into_iter().enumerate() {
            line.set_pixel(i, p);
        }
    });
}

pub(super) fn bitwise(this: &Bitwise, png: &mut PngGlitch) {
    for_rows(png, &this.rows, |line| {
        for i in 0..line.size() {
            if let Some(b) = line.index(i) {
                let new_b = match this.op {
                    BitOp::And => b & this.value,
                    BitOp::Or => b | this.value,
                    BitOp::Xor => b ^ this.value,
                };
                line.update(i, new_b);
            }
        }
    });
}

pub(super) fn swap_channels(this: &SwapChannels, png: &mut PngGlitch) {
    for_rows(png, &this.rows, |line| {
        line.process_pixels(|_, p| match p {
            Pixel::RGB(r, g, b) => match this.target {
                SwapTarget::Rg => Pixel::RGB(g, r, b),
                SwapTarget::Gb => Pixel::RGB(r, b, g),
                SwapTarget::Br => Pixel::RGB(b, g, r),
            },
            Pixel::RGBA(r, g, b, a) => match this.target {
                SwapTarget::Rg => Pixel::RGBA(g, r, b, a),
                SwapTarget::Gb => Pixel::RGBA(r, b, g, a),
                SwapTarget::Br => Pixel::RGBA(b, g, r, a),
            },
            _ => p,
        });
    });
}

pub(super) fn offset_colors(this: &OffsetColors, png: &mut PngGlitch) {
    let mut lines = png.scan_lines();
    for entry in &this.lines {
        let Some(line) = lines.get_mut(entry.line as usize) else {
            continue;
        };
        line.process_pixels(|x, p| {
            let [r_off, g_off, b_off] = entry.offsets.get(x).copied().unwrap_or([0; 3]);
            match p {
                Pixel::RGB(r, g, b) => Pixel::RGB(
                    r.wrapping_add_signed(r_off),
                    g.wrapping_add_signed(g_off),
                    b.wrapping_add_signed(b_off),
                ),
                Pixel::RGBA(r, g, b, a) => Pixel::RGBA(
                    r.wrapping_add_signed(r_off),
                    g.wrapping_add_signed(g_off),
                    b.wrapping_add_signed(b_off),
                    a,
                ),
                Pixel::Gray(v) => Pixel::Gray(v.wrapping_add_signed(r_off)),
                Pixel::GrayAlpha(v, a) => Pixel::GrayAlpha(v.wrapping_add_signed(r_off), a),
                _ => p,
            }
        });
    }
}

pub(super) fn hsl_shift(this: &HslShift, png: &mut PngGlitch) {
    let hue_shift = if this.hue_shift.is_finite() {
        this.hue_shift
    } else {
        0.0
    };
    let sat_mult = if this.saturation_mult.is_finite() && this.saturation_mult >= 0.0 {
        this.saturation_mult
    } else {
        1.0
    };
    let light_mult = if this.lightness_mult.is_finite() && this.lightness_mult >= 0.0 {
        this.lightness_mult
    } else {
        1.0
    };
    let transform = |r, g, b| {
        let (h, s, l) = rgb_to_hsl(r, g, b);
        let h = (h + hue_shift).rem_euclid(360.0);
        let s = (s * sat_mult).clamp(0.0, 1.0);
        let l = (l * light_mult).clamp(0.0, 1.0);
        hsl_to_rgb(h, s, l)
    };
    for_rows(png, &this.rows, |line| {
        line.process_pixels(|_, p| match p {
            Pixel::RGB(r, g, b) => {
                let (r, g, b) = transform(r, g, b);
                Pixel::RGB(r, g, b)
            }
            Pixel::RGBA(r, g, b, a) => {
                let (r, g, b) = transform(r, g, b);
                Pixel::RGBA(r, g, b, a)
            }
            _ => p,
        });
    });
}

pub(super) fn channel_shift(this: &ChannelShift, png: &mut PngGlitch) {
    let width = png.width() as usize;
    if width == 0 {
        return;
    }
    for_rows(png, &this.rows, |line| {
        let mut r_channel = Vec::with_capacity(width);
        let mut g_channel = Vec::with_capacity(width);
        let mut b_channel = Vec::with_capacity(width);
        let mut alpha = Vec::with_capacity(width);

        for x in 0..width {
            if let Some(p) = line.get_pixel(x) {
                r_channel.push(p.r());
                g_channel.push(p.g());
                b_channel.push(p.b());
                alpha.push(p.a());
            }
        }

        line.process_pixels(|x, p| {
            let get_val = |chan: &[u16], offset: i32| {
                let nx = (x as i64 + i64::from(offset)).rem_euclid(width as i64) as usize;
                chan.get(nx).copied().unwrap_or(0)
            };
            match p {
                Pixel::RGB(_, _, _) => Pixel::RGB(
                    get_val(&r_channel, this.r_offset),
                    get_val(&g_channel, this.g_offset),
                    get_val(&b_channel, this.b_offset),
                ),
                Pixel::RGBA(_, _, _, _) => Pixel::RGBA(
                    get_val(&r_channel, this.r_offset),
                    get_val(&g_channel, this.g_offset),
                    get_val(&b_channel, this.b_offset),
                    alpha.get(x).copied().unwrap_or(0),
                ),
                _ => p,
            }
        });
    });
}

pub(super) fn swap_blocks(this: &SwapBlocks, png: &mut PngGlitch) {
    let width = png.width();
    let height = png.height();
    let block_size = this.block_size;
    if width == 0 || height == 0 || block_size == 0 {
        return;
    }
    let num_blocks_x = width / block_size;
    let num_blocks_y = height / block_size;
    if num_blocks_x == 0 || num_blocks_y == 0 {
        return;
    }

    let mut scan_lines = png.scan_lines();

    for swap in &this.swaps {
        let (src_bx, src_by) = swap.src;
        let (dst_bx, dst_by) = swap.dst;
        if src_bx >= num_blocks_x
            || dst_bx >= num_blocks_x
            || src_by >= num_blocks_y
            || dst_by >= num_blocks_y
            || (src_bx == dst_bx && src_by == dst_by)
        {
            continue;
        }

        for y_off in 0..block_size {
            let src_y = src_by * block_size + y_off;
            let dst_y = dst_by * block_size + y_off;
            if src_y >= height || dst_y >= height {
                continue;
            }

            if src_y == dst_y {
                let Some(line) = scan_lines.get_mut(src_y as usize) else {
                    continue;
                };
                for x_off in 0..block_size {
                    let src_x = src_bx * block_size + x_off;
                    let dst_x = dst_bx * block_size + x_off;
                    if src_x < width && dst_x < width {
                        let p_src = line.get_pixel(src_x as usize);
                        let p_dst = line.get_pixel(dst_x as usize);
                        if let (Some(ps), Some(pd)) = (p_src, p_dst) {
                            line.set_pixel(src_x as usize, pd);
                            line.set_pixel(dst_x as usize, ps);
                        }
                    }
                }
            } else {
                let (low, high) = (src_y.min(dst_y) as usize, src_y.max(dst_y) as usize);
                let (left, right) = scan_lines.split_at_mut(high);
                let (Some(low_line), Some(high_line)) = (left.get_mut(low), right.first_mut())
                else {
                    continue;
                };
                let (src_line, dst_line) = if src_y < dst_y {
                    (low_line, high_line)
                } else {
                    (high_line, low_line)
                };

                for x_off in 0..block_size {
                    let src_x = src_bx * block_size + x_off;
                    let dst_x = dst_bx * block_size + x_off;
                    if src_x < width && dst_x < width {
                        let p_src = src_line.get_pixel(src_x as usize);
                        let p_dst = dst_line.get_pixel(dst_x as usize);
                        if let (Some(ps), Some(pd)) = (p_src, p_dst) {
                            src_line.set_pixel(src_x as usize, pd);
                            dst_line.set_pixel(dst_x as usize, ps);
                        }
                    }
                }
            }
        }
    }
}
