# MS-WFC-2a: 外部 WFC crate 選定・アダプタ骨格・川マスク生成

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `wfc-ms2a-crate-adapter-river-mask` |
| ステータス | `Draft` |
| 作成日 | `2026-04-01` |
| 最終更新日 | `2026-04-01` |
| 親計画 | [`wfc-terrain-generation-plan-2026-04-01.md`](wfc-terrain-generation-plan-2026-04-01.md) |
| 前MS | [`wfc-ms1-anchor-data-model.md`](wfc-ms1-anchor-data-model.md) |
| 次MS | [`wfc-ms2b-wfc-solver-constraints.md`](wfc-ms2b-wfc-solver-constraints.md) |
| 前提 | `GeneratedWorldLayout` / `WorldMasks` / `AnchorLayout` が定義済み（MS-WFC-1 完了） |

---

## 1. 目的

MS-WFC-2 を構成する3つのサブステップのうち、**WFC ソルバーへの入力を準備する段階**。

1. 外部 WFC crate を選定して `Cargo.toml` に追加する
2. `hw_world/src/mapgen/wfc_adapter.rs` に **アダプタ骨格**を作る（型変換のみ、ソルバーは次 MS）
3. 川マスクを **seed 付き帯生成**に移行し、`WorldMasks::river_mask` と `river_centerline` を確定する

川マスクは WFC ソルバーへの hard constraint として使うため、**ソルバー実装より先に確定**させる。

---

## 2. 外部 WFC crate 選定基準

以下の基準で選定する。選定結果を `Cargo.toml` のコメントに残す。

| 基準 | 要件 |
| --- | --- |
| ライセンス | MIT または Apache-2.0 |
| メンテナンス | 直近 1 年以内にコミットがある |
| API | seed 指定・hard constraint（セルへの初期値強制）が可能 |
| `no_std` | 不要（標準 `std` 環境で使う） |
| 型柔軟性 | タイルの型を整数 ID またはカスタム型で渡せる |

### 候補調査先

- crates.io で `wfc` キーワード検索
- `wave-function-collapse`, `wfc-rs`, `wavefront-collapse` など

### 選定後にやること

- `Cargo.toml` の `[workspace.dependencies]` または `[dependencies]` に追加
- `hw_world/Cargo.toml` に追加（`bevy_app` には追加しない）
- `wfc_adapter.rs` のコメントにバージョン・選定理由を 1 行記録

---

## 3. wfc_adapter モジュール（骨格）

### ファイル

`crates/hw_world/src/mapgen/wfc_adapter.rs`

### 責務

- `TerrainType` ↔ WFC タイル ID の変換
- `WorldMasks` の hard constraint → WFC ソルバー入力形式への変換
- WFC 隣接ルール（カーディナル 4 近傍）の定義
- ソルバー呼び出しシグネチャ（実装は MS-WFC-2b）

```rust
/// WFC に渡すタイル ID（外部 crate のタイル型へのマッピング）
pub struct WfcTileId(u32);

/// TerrainType <-> WfcTileId 変換テーブル
pub struct TerrainTileMapping { ... }

/// 隣接ルール（カーディナル 4 近傍）
/// (from_tile, direction, to_tile) の許可セットを保持
pub struct AdjacencyRules { ... }

impl AdjacencyRules {
    /// ゲームロジックに基づく隣接許可ルールを構築する
    pub fn default_terrain_rules() -> Self { ... }
}

/// hard constraint マップ（マスク済みセルの初期タイル ID）
pub struct HardConstraints {
    /// セル index → WfcTileId の Optional マッピング
    pub cells: Vec<Option<WfcTileId>>,
    pub width: usize,
    pub height: usize,
}

impl HardConstraints {
    /// WorldMasks から river_mask, site_mask, yard_mask を読み込んで初期化
    pub fn from_masks(masks: &WorldMasks, mapping: &TerrainTileMapping) -> Self { ... }
}

/// ソルバーを呼び出して TerrainType グリッドを返す（実装は MS-WFC-2b）
pub fn run_wfc(
    constraints: &HardConstraints,
    rules: &AdjacencyRules,
    seed: u64,
    attempt: u32,
) -> Result<Vec<TerrainType>, WfcError> {
    todo!("MS-WFC-2b で実装")
}

#[derive(Debug)]
pub enum WfcError {
    Contradiction,
    MaxIterationsReached,
}
```

この MS では `run_wfc` は `todo!()` で置く。

---

## 4. 川マスク生成（seed 付き帯生成）

### ファイル

`crates/hw_world/src/river.rs`（既存）を改修

### 現状

- 固定の River タイル列挙（`RIVER_*` 定数）でハードコード

### 改修後

seed から決まる deterministic な川生成。F3 方針を実装する。

```
川生成アルゴリズム:
1. seed から RNG を初期化する（例: rand::SeedableRng::seed_from_u64(seed)）
2. 「横断水系」を満たす中心線を生成する
   - マップの一辺（例: 左端）から対辺（右端）へ横断する経路
   - 経路はランダムウォーク（または Bezier 中心点等）で蛇行
   - Site/Yard とは交差しないよう protection_band を尊重
3. 中心線の各点から幅 2〜4 タイルを確保（各セグメントで RNG から幅を決める）
4. 川タイル総数は seed から決まる RIVER_TOTAL_TILES_TARGET ± 許容幅にクリップ
5. river_mask と river_centerline を WorldMasks に格納する
```

### 公開 API

```rust
/// seed と anchor から川マスクを生成する。Site/Yard との交差は除外済み。
pub fn generate_river_mask(
    seed: u64,
    anchors: &AnchorLayout,
    map_width: usize,
    map_height: usize,
) -> (BitGrid, Vec<IVec2>);  // (river_mask, river_centerline)
```

### 定数（定数名は src/river.rs に集約）

```rust
pub const RIVER_TOTAL_TILES_TARGET: usize = ...;  // seed に依存しない全体目標
pub const RIVER_MIN_WIDTH: u32 = 2;
pub const RIVER_MAX_WIDTH: u32 = 4;
```

---

## 5. タイル重みと砂の方針（F4）

このモジュールで **砂の重み定数** だけ先に定義する（ソルバーへの入力になるため）。
実際のソルバーへの渡し方は MS-WFC-2b で実装。

```rust
/// Sand タイルの重み設定（F4: 川隣接を主、それ以外は低頻度）
pub const SAND_ADJACENT_TO_RIVER_WEIGHT: f32 = 10.0;
pub const SAND_NON_ADJACENT_WEIGHT: f32 = 1.0;
/// 目安: 全砂タイルの 8 割が川隣接を満たすように調整する
```

---

## 6. 変更ファイルと責務

| ファイル | 変更内容 |
| --- | --- |
| `Cargo.toml` (workspace) | WFC 外部 crate 追加 |
| `crates/hw_world/Cargo.toml` | WFC 外部 crate 追加 |
| `crates/hw_world/src/mapgen/wfc_adapter.rs` (新規) | アダプタ型・変換テーブル・隣接ルール骨格 |
| `crates/hw_world/src/mapgen/mod.rs` | `wfc_adapter` モジュール追加 |
| `crates/hw_world/src/river.rs` | 川マスク生成を seed 付き帯生成に改修 |
| `crates/hw_world/src/lib.rs` | 公開 API 調整（必要なら） |

---

## 7. 完了条件チェックリスト

- [ ] WFC 外部 crate が `Cargo.toml` に追加されている（ライセンス・バージョン・選定理由コメント付き）
- [ ] `wfc_adapter.rs` に `TerrainTileMapping` / `AdjacencyRules` / `HardConstraints` / `run_wfc(todo!)` が定義されている
- [ ] `river.rs` の川マスク生成が seed 付き帯生成に変わっている
- [ ] `generate_river_mask()` が Site/Yard を回避した川マスクを返す
- [ ] `river_mask` と `river_centerline` が `WorldMasks` に設定される流れになっている
- [ ] `RIVER_MIN_WIDTH` / `RIVER_MAX_WIDTH` / `RIVER_TOTAL_TILES_TARGET` が定数化されている
- [ ] `SAND_ADJACENT_TO_RIVER_WEIGHT` / `SAND_NON_ADJACENT_WEIGHT` が定数化されている
- [ ] `cargo check --workspace` が通る
- [ ] `cargo clippy --workspace` が通る

---

## 8. 検証

```sh
CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check --workspace
cargo clippy --workspace
cargo test -p hw_world  # river.rs のユニットテストが通ること
```

手動確認:
- 川生成の `generate_river_mask()` を直接呼び出してセル数を確認する単体テストを書く
- Site/Yard 内に River セルが含まれないことをアサートする

---

## 9. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| `2026-04-01` | `Copilot` | wfc-terrain-generation-plan-2026-04-01.md の MS-WFC-2 を分割・詳細化 |
