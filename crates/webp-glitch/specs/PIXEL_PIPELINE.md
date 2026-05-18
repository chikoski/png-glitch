# ピクセルパイプラインと ScanLine API

WebP グリッチの中心となる「デコード → 操作 → 再エンコード」パイプラインと、`glitch-context` との統合方法を定義する。

## パイプライン概要

```
WebP バイト列
    │
    ▼ RiffContainer::parse()
RIFF チャンク列
    │
    ▼ PixelBuffer::decode_from()  (libwebp 経由)
ピクセルバッファ (RGBA u8 × 4)
    │
    ▼ グリッチフィルター適用
操作済みピクセルバッファ
    │
    ▼ PixelBuffer::encode_to()   (libwebp 経由)
VP8 / VP8L チャンクデータ
    │
    ▼ RiffContainer::encode()
WebP バイト列
```

非可逆・可逆どちらのフォーマットも同一パイプラインを使用する。エンコード時のフォーマット選択は `WebpConfig` で制御する。

## ピクセル表現

### `WebpPixel`

```rust
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum WebpPixel {
    RGB(u8, u8, u8),
    RGBA(u8, u8, u8, u8),
}

impl WebpPixel {
    pub fn r(&self) -> u8;
    pub fn g(&self) -> u8;
    pub fn b(&self) -> u8;
    pub fn a(&self) -> u8;  // RGB の場合は 255 を返す
}
```

`png-glitch` の `Pixel` は `u16` チャンネルを使用するが、WebP は `u8` ネイティブのため `WebpPixel` は `u8` を使用する。グリッチフィルターの再利用時はスケール変換アダプターを挟む（後述）。

### `PixelBuffer`

```rust
pub struct PixelBuffer {
    width: u32,
    height: u32,
    has_alpha: bool,
    data: Vec<u8>,   // RGBA 順、行優先
}

impl PixelBuffer {
    pub fn width(&self) -> u32;
    pub fn height(&self) -> u32;
    pub fn scan_lines(&mut self) -> Vec<ScanLine<'_>>;
    pub fn scan_line(&mut self, y: u32) -> Option<ScanLine<'_>>;
}
```

## `ScanLine` API

`png-glitch` の `ScanLine` と類似した API を提供し、フィルター実装の移植コストを最小化する。

```rust
pub struct ScanLine<'a> {
    data: &'a mut [u8],   // この行の RGBA データ
    width: u32,
    has_alpha: bool,
}

impl<'a> ScanLine<'a> {
    /// ピクセル数を返す。
    pub fn pixels_count(&self) -> u32;

    /// バイト数を返す (pixels_count * チャンネル数)。
    pub fn size(&self) -> usize;

    /// インデックス位置のピクセルを取得する。
    pub fn get_pixel(&self, x: usize) -> Option<WebpPixel>;

    /// インデックス位置にピクセルを書き込む。
    pub fn set_pixel(&mut self, x: usize, pixel: WebpPixel);

    /// インデックス位置の生バイトを取得する。
    pub fn index(&self, i: usize) -> Option<u8>;

    /// インデックス位置の生バイトを更新する。
    pub fn update(&mut self, i: usize, value: u8);

    /// 全ピクセルにクロージャを適用する。
    /// クロージャ引数: (x: usize, pixel: WebpPixel) -> WebpPixel
    pub fn process_pixels<F>(&mut self, f: F)
    where
        F: FnMut(usize, WebpPixel) -> WebpPixel;
}
```

## `WebpGlitch` のメイン API

```rust
pub struct WebpGlitch {
    riff: RiffContainer,
    pixels: PixelBuffer,
    config: WebpConfig,
}

pub struct WebpConfig {
    pub quality: f32,        // 再エンコード品質 (0〜100)、非可逆時のみ有効
    pub lossless: bool,      // true: 可逆、false: 非可逆
}

impl WebpGlitch {
    /// ファイルから WebP を読み込む。
    pub fn open(path: impl AsRef<Path>) -> Result<Self, WebpError>;

    /// バイト列から WebP を読み込む。
    pub fn new(data: Vec<u8>) -> Result<Self, WebpError>;

    /// 画像の幅を返す。
    pub fn width(&self) -> u32;

    /// 画像の高さを返す。
    pub fn height(&self) -> u32;

    /// 全スキャンラインのリストを返す。
    pub fn scan_lines(&mut self) -> Vec<ScanLine<'_>>;

    /// 各スキャンラインにクロージャを順次適用する。
    pub fn foreach_scanline<F>(&mut self, f: F) -> &mut Self
    where
        F: FnMut(&mut ScanLine<'_>);

    /// 各スキャンラインにクロージャを並列適用する (rayon 使用)。
    pub fn par_foreach_scanline<F>(&mut self, f: F) -> &mut Self
    where
        F: Fn(&mut ScanLine<'_>) + Send + Sync;

    /// グリッチフィルターを適用する。
    pub fn apply<T: WebpGlitchPreset>(&mut self, preset: T) -> &mut Self;

    /// WebP としてファイルに保存する。
    pub fn save(self, path: impl AsRef<Path>) -> Result<(), WebpError>;

    /// WebP としてバイト列にエンコードして返す。
    pub fn encode(&self) -> Result<Vec<u8>, WebpError>;
}
```

## `glitch-context` フィルターとの統合

`glitch-context` の `GlitchFilter` トレイトは `PngGlitch` を受け取るため、`WebpGlitch` には直接適用できない。統合のために 2 つのアプローチを検討する:

### アプローチ A: `WebpGlitchFilter` トレイト (推奨)

`webp-glitch` 側に新しいトレイトを定義し、`glitch-context` に `impl WebpGlitchFilter for <各フィルター>` を追加する。

```rust
// webp-glitch/src/lib.rs
pub trait WebpGlitchFilter {
    fn apply(&self, webp: &mut WebpGlitch, rng: &mut ChaCha8Rng);
}
```

`glitch-context` で実装例:

```rust
// glitch-context/src/lib.rs
impl WebpGlitchFilter for PixelSort {
    fn apply(&self, webp: &mut WebpGlitch, rng: &mut ChaCha8Rng) {
        webp.foreach_scanline(|scan_line| {
            // WebpPixel で同様のソートロジックを実装
        });
    }
}
```

### アプローチ B: 共通ピクセルトレイト (将来検討)

将来的に `glitch-core` のような共通クレートを新設し、フォーマット非依存のピクセルトレイトを定義することで、フィルター実装の重複を排除できる。現時点では過度な抽象化となるため、アプローチ A を採用する。

## `WebpGlitchContext`

`GlitchContext` に相当するコンテキスト管理構造体。`glitch-context` クレートに追加する。

```rust
pub struct WebpGlitchContext {
    webp: WebpGlitch,
    filters: Vec<Box<dyn WebpGlitchFilter>>,
    rng: ChaCha8Rng,
}

impl WebpGlitchContext {
    pub fn open(path: impl AsRef<Path>, seed: Option<u64>) -> Result<Self>;
    pub fn new(data: &[u8], seed: Option<u64>) -> Result<Self>;
    pub fn add_filter(&mut self, filter: impl WebpGlitchFilter + 'static);
    pub fn execute(&mut self);
    pub fn buffer(&self) -> Result<Vec<u8>>;
    pub fn save(&self, path: impl AsRef<Path>) -> Result<()>;
    pub fn width(&self) -> u32;
    pub fn height(&self) -> u32;
}
```
