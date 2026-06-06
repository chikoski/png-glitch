/// WebP グリッチの基本的な使い方を示すサンプル。
///
/// 実行方法:
///   cargo run --example glitch --manifest-path crates/webp-glitch/Cargo.toml
///
/// 出力: crates/webp-glitch/etc/sample00-glitched.webp
use webp_glitch::{WebpGlitch, WebpPixel};

fn main() {
    let mut glitch =
        WebpGlitch::open("etc/sample00.webp").expect("etc/sample00.webp を開けませんでした");

    let width = glitch.width();
    let height = glitch.height();
    println!("入力: {}x{} WebP", width, height);

    // --- エフェクト 1: 上 1/3 を色反転 ---
    let invert_end = height / 3;
    for y in 0..invert_end {
        if let Some(mut line) = glitch.scan_line_mut(y) {
            line.process_pixels(|_, p| match p {
                WebpPixel::RGB(r, g, b) => WebpPixel::RGB(255 - r, 255 - g, 255 - b),
                WebpPixel::RGBA(r, g, b, a) => WebpPixel::RGBA(255 - r, 255 - g, 255 - b, a),
            });
        }
    }

    // --- エフェクト 2: 中央 1/3 をチャンネルシフト (R→G→B→R) ---
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

    // --- エフェクト 3: 下 1/3 に水平クロマティック収差 ---
    let aberration_start = height / 3 * 2;
    {
        let mut lines = glitch.scan_lines();
        for y in aberration_start as usize..lines.len() {
            let w = width as usize;
            // 各チャンネルを収集して水平オフセットして戻す
            let mut r_ch = vec![0u8; w];
            let mut g_ch = vec![0u8; w];
            let mut b_ch = vec![0u8; w];
            for x in 0..w {
                if let Some(p) = lines[y].get_pixel(x) {
                    r_ch[x] = p.r();
                    g_ch[x] = p.g();
                    b_ch[x] = p.b();
                }
            }
            lines[y].process_pixels(|x, p| {
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

    glitch
        .save("etc/sample00-glitched.webp")
        .expect("保存に失敗しました");
    println!("保存完了: etc/sample00-glitched.webp");
}
