# RIFF 構造と チャンクレベルグリッチ

WebP ファイルは RIFF (Resource Interchange File Format) コンテナを使用する。このドキュメントでは RIFF パーサの設計とチャンクレベルのグリッチ操作を定義する。

## RIFF バイナリレイアウト

```
Offset  Size  フィールド
0       4     FourCC "RIFF"
4       4     ファイルサイズ - 8 (リトルエンディアン)
8       4     フォーム型 "WEBP"
12      ...   チャンク列
```

各チャンクのレイアウト:

```
Offset  Size  フィールド
0       4     チャンク ID (FourCC)
4       4     チャンクデータサイズ (リトルエンディアン)
8       N     チャンクデータ
8+N     0/1   パディング (N が奇数の場合に 0x00 を 1 バイト追加)
```

## `RiffContainer` の内部表現

```rust
pub struct RiffContainer {
    pub total_size: u32,
    pub chunks: Vec<RiffChunk>,
}

pub struct RiffChunk {
    pub id: [u8; 4],
    pub data: Vec<u8>,
}
```

設計上の選択:
- チャンクデータは所有権を持つ `Vec<u8>` で保持し、インプレース変更を可能にする。
- RIFF ヘッダーと各チャンクを分離して保持し、チャンク操作をシンプルにする。
- 再エンコード時にサイズフィールドを自動再計算する（グリッチ操作で意図的に改ざんする場合を除く）。

## パーサ設計

```rust
impl RiffContainer {
    /// バイト列から RIFF コンテナを解析する。
    pub fn parse(data: &[u8]) -> Result<Self, WebpError>;

    /// RIFF コンテナをバイト列に再エンコードする。
    pub fn encode(&self) -> Vec<u8>;

    /// ID でチャンクを検索する。
    pub fn find_chunk(&self, id: &[u8; 4]) -> Option<&RiffChunk>;

    /// ID でチャンクを可変参照で取得する。
    pub fn find_chunk_mut(&mut self, id: &[u8; 4]) -> Option<&mut RiffChunk>;
}
```

パーサは不正なデータに対して寛容に設計する。グリッチ操作で破壊されたデータでも最大限パースを試みる。

## チャンクレベルのグリッチ操作

### 1. チャンク入れ替え (`swap_chunks`)

指定した 2 つのチャンクの **データ** を入れ替える。ID は変更しない。VP8 チャンクの位置に VP8L のデータを置くなど、フォーマット違反を意図的に作り出す。

```rust
impl WebpGlitch {
    pub fn swap_chunks(&mut self, id_a: &[u8; 4], id_b: &[u8; 4]) -> &mut Self;
}
```

### 2. チャンクデータ破壊 (`corrupt_chunk`)

指定チャンクのデータを部分的にランダム値で上書きする。

```rust
impl WebpGlitch {
    /// magnitude: 0.0〜1.0 で破壊するバイトの割合を指定。
    pub fn corrupt_chunk(&mut self, id: &[u8; 4], magnitude: f64, rng: &mut impl Rng) -> &mut Self;
}
```

### 3. チャンクサイズ改ざん (`tamper_chunk_size`)

チャンクサイズフィールドを実際のデータサイズと異なる値に設定する。多くのデコーダはサイズを信頼してバッファを確保するため、この操作はデコーダを混乱させる。

```rust
impl WebpGlitch {
    /// delta: 正の値で拡大、負の値で縮小。
    pub fn tamper_chunk_size(&mut self, id: &[u8; 4], delta: i32) -> &mut Self;
}
```

### 4. チャンク削除 (`remove_chunk`)

指定 ID のチャンクをコンテナから除去する。例: ALPH チャンクを削除するとアルファチャンネルが失われる。

```rust
impl WebpGlitch {
    pub fn remove_chunk(&mut self, id: &[u8; 4]) -> &mut Self;
}
```

### 5. チャンク複製・挿入 (`duplicate_chunk`)

既存チャンクを複製してコンテナに追加する。RIFF 仕様では同一 ID のチャンクは 1 つだが、意図的に複数挿入することでパーサの挙動を乱す。

```rust
impl WebpGlitch {
    pub fn duplicate_chunk(&mut self, id: &[u8; 4]) -> &mut Self;
}
```

## エラー処理

```rust
#[derive(Debug, thiserror::Error)]
pub enum WebpError {
    #[error("invalid RIFF signature")]
    InvalidSignature,
    #[error("invalid WEBP form type")]
    InvalidFormType,
    #[error("chunk not found: {0:?}")]
    ChunkNotFound([u8; 4]),
    #[error("decode error: {0}")]
    DecodeError(String),
    #[error("encode error: {0}")]
    EncodeError(String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}
```
