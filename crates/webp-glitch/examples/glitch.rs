//! Basic example demonstrating how to glitch a WebP image.
//!
//! Run from anywhere in the workspace:
//!   cargo run -p webp-glitch --example glitch
use std::fs;
use std::path::Path;
use webp_glitch::{WebpGlitch, WebpPixel};

fn main() {
    let input_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("etc/sample00.webp");
    let out_dir = std::env::temp_dir().join("png-glitch-examples");
    fs::create_dir_all(&out_dir).expect("Failed to create example output directory");
    let output_path = out_dir.join("sample00-glitched.webp");

    let mut glitch = WebpGlitch::open(&input_path).expect("Failed to open etc/sample00.webp");

    let width = glitch.width();
    let height = glitch.height();
    println!("Input: {}x{} WebP", width, height);

    // --- Effect 1: Invert colors on top 1/3 ---
    let invert_end = height / 3;
    for y in 0..invert_end {
        if let Some(mut line) = glitch.scan_line_mut(y) {
            line.process_pixels(|_, p| match p {
                WebpPixel::RGB(r, g, b) => WebpPixel::RGB(255 - r, 255 - g, 255 - b),
                WebpPixel::RGBA(r, g, b, a) => WebpPixel::RGBA(255 - r, 255 - g, 255 - b, a),
            });
        }
    }

    // --- Effect 2: Channel shift (R->G->B->R) on middle 1/3 ---
    let shift_start = height / 3;
    let shift_end = height / 3 * 2;
    for y in shift_start..shift_end {
        if let Some(mut line) = glitch.scan_line_mut(y) {
            line.process_pixels(|_, p| match p {
                WebpPixel::RGB(r, g, b) => WebpPixel::RGB(b, r, g),
                WebpPixel::RGBA(r, g, b, a) => WebpPixel::RGBA(b, r, g, a),
            });
        }
    }

    // --- Effect 3: Horizontal chromatic aberration on bottom 1/3 ---
    let aberration_start = height / 3 * 2;
    {
        let mut lines = glitch.scan_lines();
        for line in lines.iter_mut().skip(aberration_start as usize) {
            let w = width as usize;
            let mut r_ch = vec![0u8; w];
            let mut g_ch = vec![0u8; w];
            let mut b_ch = vec![0u8; w];
            for x in 0..w {
                if let Some(p) = line.get_pixel(x) {
                    r_ch[x] = p.r();
                    g_ch[x] = p.g();
                    b_ch[x] = p.b();
                }
            }
            line.process_pixels(|x, p| {
                let r = r_ch[(x + 4) % w];
                let g = g_ch[x];
                let b = b_ch[(x + w - 4) % w];
                match p {
                    WebpPixel::RGB(_, _, _) => WebpPixel::RGB(r, g, b),
                    WebpPixel::RGBA(_, _, _, a) => WebpPixel::RGBA(r, g, b, a),
                }
            });
        }
    }

    glitch.save(&output_path).expect("Failed to save WebP");
    println!("Saved: {}", output_path.display());
}
