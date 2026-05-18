# webp-glitch: Overview

`webp-glitch` は WebP 画像を意図的に破壊してグリッチアートを生成するための Rust ライブラリです。既存の `png-glitch` / `glitch-context` エコシステムと並列に位置し、同じグリッチフィルターを WebP フォーマットに適用できます。

## 高レベルゴール

1. **フォーマット認識グリッチ**: WebP の RIFF チャンク構造を理解した上で、各チャンクを選択的に操作する。
2. **ピクセルレベル操作**: デコード → ピクセル操作 → 再エンコードのパイプラインで、既存 `glitch-context` フィルターを再利用する。
3. **ビットストリーム破壊**: 圧縮されたビットストリームを直接操作して、デコーダに視覚的アーティファクトを生成させる（VP8 / VP8L 双方をサポート）。
4. **シンプルな API**: `PngGlitch` に倣ったフルエント API を提供する。

## WebP フォーマットの概要

WebP は RIFF コンテナフォーマットを使用する。主要なチャンク:

| チャンク | 説明 |
| :--- | :--- |
| `VP8 ` | 非可逆圧縮 (VP8 ビットストリーム、DCT ベース) |
| `VP8L` | 可逆圧縮 (VP8L ビットストリーム、LZ77 + Huffman) |
| `VP8X` | 拡張フォーマット (アルファ、アニメーション、メタデータ) |
| `ALPH` | アルファチャンネル (拡張フォーマット時) |
| `ANIM` | アニメーションパラメーター |
| `ANMF` | アニメーションフレーム |
| `ICCP` | ICC カラープロファイル |
| `EXIF` | EXIF メタデータ |

## コアアーキテクチャ

```
crates/webp-glitch/
├── src/
│   ├── lib.rs          # WebpGlitch 公開エントリポイント
│   ├── riff.rs         # RIFF コンテナ パーサ / エンコーダ
│   ├── vp8.rs          # VP8 ビットストリーム操作（非可逆）
│   ├── vp8l.rs         # VP8L ビットストリーム操作（可逆）
│   ├── pixel.rs        # WebP ピクセル表現と操作
│   └── scan_line.rs    # スキャンライン抽象化
├── specs/
│   ├── OVERVIEW.md     # このファイル
│   ├── RIFF_STRUCTURE.md
│   ├── PIXEL_PIPELINE.md
│   └── GLITCH_OPERATIONS.md
└── Cargo.toml
```

### 主要コンポーネント

#### `WebpGlitch`

`PngGlitch` に対応するメイン構造体。WebP ファイルの読み込み、ピクセルデータへのアクセス、グリッチ操作の適用、再エンコードを担当する。

```rust
pub struct WebpGlitch {
    riff: RiffContainer,       // RIFF チャンクツリー
    pixels: PixelBuffer,       // デコード済みピクセルデータ
    config: WebpConfig,        // 画像メタデータ（幅、高さ、カラータイプ）
}
```

#### `RiffContainer`

RIFF チャンクのツリー構造。チャンクレベルのグリッチ（サイズ改ざん、チャンク入れ替え、データ破壊）を提供する。

#### `PixelBuffer` / `ScanLine`

デコードされたピクセルデータへの行単位アクセスを提供する。`png-glitch` の `ScanLine` と同等のインターフェースを持ち、`glitch-context` のフィルター群をそのまま適用できるよう設計する。

## 依存関係方針

| クレート | 用途 | 理由 |
| :--- | :--- | :--- |
| `webp` | WebP デコード / エンコード | libwebp バインディング |
| `anyhow` | エラーハンドリング | 既存クレートとの一貫性 |
| `thiserror` | カスタムエラー型 | 既存クレートとの一貫性 |
| `rayon` | 並列処理 | スキャンライン並列操作 |

`glitch-context` クレートへの依存は **持たない**。代わりに `glitch-context` 側が `webp-glitch` に対応するアダプターを追加する（詳細は GLITCH_OPERATIONS.md 参照）。
