# グリッチ操作一覧

`webp-glitch` が提供するグリッチ操作の全カタログ。操作は 2 レイヤーに分類される:
- **チャンクレベル**: RIFF コンテナを直接操作（デコード不要）
- **ピクセルレベル**: デコードしたピクセルデータを操作

## チャンクレベル操作

これらの操作は `WebpGlitch` のメソッドとして提供し、`glitch-context` フィルター機構の外側で使用する。デコード処理を経由しないため高速。

| 操作 | メソッド | 説明 |
| :--- | :--- | :--- |
| チャンク入れ替え | `swap_chunks(a, b)` | 2 チャンクのデータを入れ替える |
| チャンク破壊 | `corrupt_chunk(id, magnitude)` | チャンクデータをランダム値で部分上書き |
| サイズ改ざん | `tamper_chunk_size(id, delta)` | チャンクサイズフィールドをずらす |
| チャンク削除 | `remove_chunk(id)` | 指定チャンクをコンテナから除去 |
| チャンク複製 | `duplicate_chunk(id)` | 同一チャンクを末尾に追加 |

### 代表的なチャンクレベルグリッチパターン

**アルファ消去**: `remove_chunk(b"ALPH")` で透明情報を削除する。

**ビットストリーム破壊**: `corrupt_chunk(b"VP8 ", 0.01)` で VP8 DCT データの 1% を破壊する。マクロブロック単位のアーティファクトが生じる。

**可逆→非可逆ミスマッチ**: VP8L チャンクのデータを VP8 チャンクとして再配置することで、デコーダに誤ったコーデックで解釈させる。

## ピクセルレベル操作 (WebpGlitchFilter)

以下のフィルターを `glitch-context` に `WebpGlitchFilter` 実装として追加する。PNG 版と同名だがピクセル型が `WebpPixel` (u8 チャンネル) になる点に注意。

### 既存フィルターの WebP 移植

| フィルター | 操作 | PNG 版との差異 |
| :--- | :--- | :--- |
| `Invert` | 全チャンネルを反転 | なし |
| `Brighten` | 輝度加算 | `u8::saturating_add` を使用 |
| `ShiftChannels` | RGB チャンネルをシフト | なし |
| `PixelSort` | スキャンライン内ピクセルをソート | なし |
| `Bitwise` | バイト単位のビット演算 | なし |
| `ChannelSwap` | RGB チャンネルを交換 | なし |
| `HorizontalShift` | スキャンラインを水平シフト | なし |
| `BlockScramble` | ブロック単位でスクランブル | なし |
| `ColorDistortion` | ランダム色ノイズを加算 | なし |
| `ColorSpaceGlitch` | HSL 空間で色操作 | なし |
| `ChromaticAberration` | チャンネル別水平オフセット | なし |
| `Transpose` | スキャンラインを入れ替え | なし |
| `RandomCopy` | スキャンラインをランダムコピー | なし |

### WebP 固有のフィルター

#### `MacroblockGlitch`

VP8 非可逆の DCT マクロブロック境界 (16×16 px) に沿ってブロックを入れ替える。`BlockScramble` に類似しているが、マクロブロックサイズに固定されるため、デコーダのブロックアーティファクトと視覚的に整合する。

```rust
pub struct MacroblockGlitch {
    pub magnitude: f64,
}
```

#### `AlphaGlitch`

アルファチャンネルのみを選択的に破壊する。RGB データはそのままで透明度だけをノイズ化する。

```rust
pub struct AlphaGlitch {
    pub magnitude: f64,
    pub strategy: AlphaGlitchStrategy,
}

pub enum AlphaGlitchStrategy {
    Invert,      // アルファを反転 (255 - a)
    Randomize,   // ランダム値で上書き
    Zero,        // 全ピクセルを透明化
    One,         // 全ピクセルを不透明化
}
```

#### `LossyArtifact`

非可逆再エンコード時の品質パラメーターを故意に低下させ、JPEG 的なリンギングアーティファクトを追加する。`WebpConfig::quality` をスキャンライン単位で変化させるのではなく、全体の品質設定として管理する。

```rust
pub struct LossyArtifact {
    pub quality: f32,   // 0.0〜100.0、低いほどアーティファクトが強い
}
```

## プリセット (Recipes)

よく使われるグリッチ効果の組み合わせをプリセットとして提供する。

| プリセット名 | 内容 |
| :--- | :--- |
| `VhsLook` | `ChromaticAberration` + `HorizontalShift` + `ColorDistortion` |
| `DataMosh` | チャンク破壊 + `BlockScramble` |
| `GhostAlpha` | `AlphaGlitch(Invert)` + `ColorSpaceGlitch` |
| `PureNoise` | `Bitwise(Xor)` + `PixelSort` |

## `WebpGlitchFilter` トレイト

```rust
pub trait WebpGlitchFilter {
    fn apply(&self, webp: &mut WebpGlitch, rng: &mut ChaCha8Rng);
}
```

`WebpGlitchContext::add_filter()` に渡すことで、実行順序付きフィルターパイプラインを構築する。

## CLI 統合

`png-glitch-cli` の `FilterConfig` に WebP 対応エントリを追加し、同一の YAML 設定ファイルで PNG / WebP 両方のグリッチを制御できるようにする。入力ファイルの拡張子で自動判別する。

```yaml
# test_config.yaml の例
filters:
  - type: ChromaticAberration
    magnitude: 0.8
    r_offset: 4
  - type: AlphaGlitch        # WebP 専用。PNG 入力時は無視
    magnitude: 0.5
    strategy: randomize
```
