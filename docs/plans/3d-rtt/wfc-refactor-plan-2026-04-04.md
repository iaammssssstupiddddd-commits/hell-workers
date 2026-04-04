# WFC 関連リファクタ計画

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `wfc-refactor-plan-2026-04-04` |
| ステータス | `提案・未着手` |
| 作成日 | `2026-04-04` |
| 親計画 | [`wfc-terrain-generation-plan-2026-04-01.md`](archived/wfc-terrain-generation-plan-2026-04-01.md) |
| 関連 | [`wfc-ms45-docs-tests.md`](archived/wfc-ms45-docs-tests.md)（golden seed・テスト入口の整理と重複しうる） |

---

## 1. 目的

WFC 地形生成まわり（`hw_world` の `mapgen`・`world_masks`・`river` / `terrain_zones` / `rock_fields` との境界）について、**挙動を変えずに**次を達成する。

- **責務の見通し**: オーケストレーション・ソルバー・検証・資源配置の境界を読み取りやすくする。
- **変更の局所化**: 将来の仕様変更（validator 追加・retry 方針変更・新マスク）の影響範囲を限定する。
- **検証の明確化**: 代表 seed・回帰テストの置き場を一箇所に寄せ、MS-WFC-4.5 の「curated golden seed」方針と整合させる。
- **重い／ノイズの多いテストの分離**: 分布プリント等を通常 `cargo test` から切り離す。

## 2. 非目標（やらないこと）

- WFC アルゴリズム本体（gridbugs `wfc`）の差し替えや版本質変更。
- ゲームバランス・マップサイズ・invariant の変更（別タスク）。
- `bevy_app` の startup 順序の変更（MS-WFC-4 済み経路の維持）。

---

## 3. 現状の構造と課題（2026-04 時点・コード実測）

### 3.1 ファイル規模（実測）

| パス | 実測行数 | 主な内容 |
| --- | ---: | --- |
| `mapgen.rs` | **453** | `generate_base_terrain_tiles`（~35行）、`generate_world_layout`（~110行）、`mod tests`（~100行）、`mod tile_dist_sim`（**~190行** / 4テスト全て `println!` 付き） |
| `mapgen/wfc_adapter.rs` | **444** | パターン定数・`PatternTable`・`WorldConstraints`/`ForbidPattern`・`run_wfc`・`post_process_tiles`・`apply_zone_post_process`・`fallback_terrain` |
| `mapgen/validate.rs` | **528** | `lightweight_validate`・`debug_validate`・`validate_post_resource`・内部BFS実装2本・テスト |
| `mapgen/resources.rs` | **519** | 定数12個・`ResourceLayout`・`generate_resource_layout_inner`・`generate_forest_zones`・`place_trees`・`place_rocks`・`build_tree_exclusion`・テスト |
| `mapgen/types.rs` | ~130 | `GeneratedWorldLayout`・`ResourceSpawnCandidates`・`WfcForestZone`・`stub` |
| `world_masks.rs` | **300** | `BitGrid`・`WorldMasks`・5本の `fill_*_from_seed`・`compute_protection_band` |

`mapgen/` 配下だけで **実質 ~2050 行**が集中している。

### 3.2 責務の混在

`generate_world_layout`（`mapgen.rs` L43〜L135）が**以下を 1 関数に集約**している:

1. `AnchorLayout` + `WorldMasks` 構築（5本の `fill_*_from_seed` 呼び出し）
2. retry ループ（`0..=MAX_WFC_RETRIES`、最大 65 回）内での
   - `derive_sub_seed` / `run_wfc`
   - `lightweight_validate` → `generate_resource_layout` → `validate_post_resource`
3. `GeneratedWorldLayout` の **`..candidate` spread を 3 段重ね**（Step3→Step4→Step5）で組み立て
4. fallback 分岐（`.unwrap_or_else`）でも同じ spread パターンを**さらに 3 回**繰り返す
5. `#[cfg(any(test, debug_assertions))]` ブロックでの `debug_validate` 呼び出し

特に `GeneratedWorldLayout` は現在フィールド 9 本あり、**spread ミス（フィールド追加時の取りこぼし）** が正常系・fallback 系の両方で潜在する。

### 3.3 seed・テストの分散（実態）

コードベースで判明している seed 定数の散在状況:

| ファイル | 定数名 / リテラル | 値 |
| --- | --- | --- |
| `mapgen/validate.rs:497` | `GOLDEN_SEED_STANDARD` | `10_182_272_928_891_625_829` |
| `mapgen/resources.rs:298` | `TEST_SEED_A` | `10_182_272_928_891_625_829`（上と同値！） |
| `mapgen/resources.rs:299` | `TEST_SEED_B` | `12_345_678` |
| `mapgen.rs:162` | `TEST_SEED_A` | `10_182_272_928_891_625_829`（3箇所目） |
| `mapgen.rs:163` | `TEST_SEED_B` | `12_345_678`（2箇所目） |
| `mapgen.rs:266,326,385` | リテラル配列 | `[0u64, 42, 99, 12345]`（`tile_dist_sim` 3テスト） |
| `rock_fields.rs:223` | リテラル配列 | `[0u64, 42, 99, 12_345_678]` |
| `river.rs:594,625` | コメント内リテラル | `seed=42`（エラーメッセージ） |

→ **`GOLDEN_SEED_STANDARD` と `TEST_SEED_A` は同一の `u64` 値**なのに別定数として 3 ファイルに散在している。Phase C で一元化する。

### 3.4 重いテスト（`tile_dist_sim` モジュール）

`mapgen.rs` 末尾の `mod tile_dist_sim` に含まれる 4 テスト:

| テスト名 | 内容 | 重さ |
| --- | --- | --- |
| `print_tile_distribution` | 4 seed × 全タイル走査 + `println!` | 高 |
| `print_neutral_breakdown` | 4 seed × 全タイル走査 + `println!` | 高 |
| `print_zone_coverage_by_distance` | 4 seed × 全タイル走査 + `println!` | 高 |
| （距離場計算） | `compute_anchor_distance_field` を追加呼び出し | 中 |

これらは **アサートが無く、診断目的の `println!` のみ**であるため、通常の回帰テストに含める必要がない。`#[ignore]` を付けて分離するだけで通常 `cargo test` が大幅に速くなる。

---

## 4. リファクタ方針（原則）

1. **小さく段階的に**: 各ステップ後に `cargo test -p hw_world` / `cargo clippy --workspace -- -D warnings` を通す。
2. **公開 API は維持**: `hw_world::generate_world_layout` / `GeneratedWorldLayout` / `mapgen` サブモジュールの **既存の公開パス**を壊さない（必要なら `pub use` で互換維持）。
3. **挙動不変**: 同一 `master_seed` での地形・資源・validator 結果が変わらないことを **既存テスト**で担保。seed 定数の置き換えは値を変えないので安全。
4. **診断は後追い可能**: `debug_validate` / `eprintln` の有無は、分割後も同じタイミングで呼ぶ。

---

## 5. 提案フェーズ

### Phase A — `mapgen` モジュールの物理分割（優先度: 高）

**目的**: `mapgen.rs` からオーケストレーション本体とテストを分離する。

#### A1: `mapgen.rs` → `mapgen/mod.rs` へのモジュール化

現在 `hw_world/src/mapgen.rs` + `hw_world/src/mapgen/` が共存しているため、Rust の慣例（`mapgen.rs` 単独 or `mapgen/mod.rs` + サブモジュール）に統一する。

```
# Before
src/mapgen.rs          ← 453行（pub mod resources; 等の宣言 + 実装が混在）
src/mapgen/wfc_adapter.rs
src/mapgen/validate.rs
src/mapgen/resources.rs
src/mapgen/types.rs

# After
src/mapgen/mod.rs      ← re-export + pub fn generate_base_terrain_tiles / generate_world_layout の宣言のみ
src/mapgen/pipeline.rs ← generate_world_layout の実装本体
src/mapgen/wfc_adapter.rs（変更なし）
src/mapgen/validate.rs（変更なし）
src/mapgen/resources.rs（変更なし）
src/mapgen/types.rs（変更なし）
```

`lib.rs` の `pub mod mapgen;` は変更不要（Rust がディレクトリ優先で `mod.rs` を探す）。

#### A2: `tile_dist_sim` の `#[ignore]` 化

対象: `mapgen.rs` 内 `mod tile_dist_sim` の 4 テスト全て

```rust
// Before
#[test]
fn print_tile_distribution() { ... }

// After
#[test]
#[ignore = "diagnostic only – run with: cargo test -p hw_world -- --ignored"]
fn print_tile_distribution() { ... }
```

同様に `print_neutral_breakdown`、`print_zone_coverage_by_distance` にも `#[ignore]` を付与。

**完了条件**:
- `mapgen/mod.rs` が re-export + 公開関数の転送のみになる（実装行 ~30 行以下）
- `cargo test -p hw_world` の通常実行に `tile_dist_sim` が含まれなくなる
- `cargo test -p hw_world -- --ignored` で 4 テストが実行可能なままである

---

### Phase B — `generate_world_layout` のレイアウト構築ヘルパ抽出（優先度: 中）

**目的**: `GeneratedWorldLayout` の struct spread 3 段重ね × 2（正常系・fallback 系）を安全にする。

#### B1: `types.rs` への初期化ヘルパ追加

`GeneratedWorldLayout` に `from_terrain` コンストラクタを追加し、フィールドをデフォルト値で初期化する:

```rust
// mapgen/types.rs に追加
impl GeneratedWorldLayout {
    /// WFC パイプライン内部用初期値（resource フィールドはすべて空）。
    pub(crate) fn initial(
        terrain_tiles: Vec<TerrainType>,
        anchors: AnchorLayout,
        masks: WorldMasks,
        master_seed: u64,
        generation_attempt: u32,
        used_fallback: bool,
    ) -> Self {
        Self {
            terrain_tiles,
            anchors,
            masks,
            resource_spawn_candidates: ResourceSpawnCandidates::default(),
            initial_tree_positions: Vec::new(),
            forest_regrowth_zones: Vec::new(),
            initial_rock_positions: Vec::new(),
            master_seed,
            generation_attempt,
            used_fallback,
        }
    }

    /// ResourceLayout の内容を self にマージして新しい Self を返す。
    pub(crate) fn with_resources(self, res: resources::ResourceLayout, water_tiles: Vec<GridPos>, sand_tiles: Vec<GridPos>) -> Self {
        Self {
            initial_tree_positions: res.initial_tree_positions,
            forest_regrowth_zones: res.forest_regrowth_zones,
            initial_rock_positions: res.initial_rock_positions,
            resource_spawn_candidates: ResourceSpawnCandidates {
                water_tiles,
                sand_tiles,
                rock_candidates: res.rock_candidates,
            },
            ..self
        }
    }
}
```

#### B2: `pipeline.rs` の retry ループ書き換え

上記ヘルパを使い、`find_map` の中身を以下のように整理する:

```rust
// 変更前（conceptual）: GeneratedWorldLayout { ... 9フィールド ... } × 3段
// 変更後
let candidate = GeneratedWorldLayout::initial(terrain_tiles, anchors.clone(), masks.clone(), master_seed, attempt, false);
let validated_candidates = validate::lightweight_validate(&candidate).ok()?;
let candidate = GeneratedWorldLayout { resource_spawn_candidates: validated_candidates, ..candidate };
let res = resources::generate_resource_layout(&candidate, sub_seed)?;
validate::validate_post_resource(&candidate, &res).ok()?;
Some(candidate.with_resources(res, ...))
```

**完了条件**: `generate_world_layout` 正常系の実装が ~60 行以内（retry ループ含む）で読める。

---

### Phase C — Golden seed モジュール（優先度: 中・MS-WFC-4.5 と連携）

**目的**: `GOLDEN_SEED_STANDARD` / `TEST_SEED_A`（同値 `10_182_272_928_891_625_829`）が 3 ファイルに散在している問題を解消する。

#### C1: `mapgen/test_seeds.rs` を新設

```rust
// crates/hw_world/src/mapgen/test_seeds.rs
//! WFC 回帰テスト用の代表 seed 定数。
//!
//! ## Seed 一覧
//!
//! | 定数名 | 値 | 用途 |
//! | --- | --- | --- |
//! | `GOLDEN_SEED_PRIMARY` | `10_182_272_928_891_625_829` | 主回帰・validate / resources / mapgen 全テスト |
//! | `GOLDEN_SEED_SECONDARY` | `12_345_678` | 決定論性・異種チェック用 |
//! | `GOLDEN_SEED_FALLBACK_SUITE` | `[0, 42, 99, 12_345_678]` | fallback / tile_dist_sim 用代表配列 |

/// 主回帰 seed。validate・resources・pipeline のほぼ全テストがこれを使う。
#[cfg(test)]
pub(crate) const GOLDEN_SEED_PRIMARY: u64 = 10_182_272_928_891_625_829;

/// 異種確認用 seed（`GOLDEN_SEED_PRIMARY` と異なる地形が生成されることを保証）。
#[cfg(test)]
pub(crate) const GOLDEN_SEED_SECONDARY: u64 = 12_345_678;

/// tile_dist_sim / fallback スイート用代表 seed 配列。
#[cfg(test)]
pub(crate) const GOLDEN_SEED_FALLBACK_SUITE: &[u64] = &[0, 42, 99, 12_345_678];
```

#### C2: 既存 3 ファイルの定数を `test_seeds` 参照に置き換え

| ファイル | 変更前 | 変更後 |
| --- | --- | --- |
| `mapgen/validate.rs:497` | `const GOLDEN_SEED_STANDARD: u64 = 10_182_272_928_891_625_829;` | `use crate::mapgen::test_seeds::GOLDEN_SEED_PRIMARY;` |
| `mapgen/resources.rs:298` | `const TEST_SEED_A: u64 = 10_182_272_928_891_625_829;` | `use crate::mapgen::test_seeds::GOLDEN_SEED_PRIMARY;` |
| `mapgen/resources.rs:299` | `const TEST_SEED_B: u64 = 12_345_678;` | `use crate::mapgen::test_seeds::GOLDEN_SEED_SECONDARY;` |
| `mapgen.rs / pipeline.rs` | 同上 × 2 | 同上 |
| `mapgen.rs tile_dist_sim:266,326,385` | `[0u64, 42, 99, 12345]` | `GOLDEN_SEED_FALLBACK_SUITE` |
| `rock_fields.rs:223` | `[0u64, 42, 99, 12_345_678]` | `GOLDEN_SEED_FALLBACK_SUITE`（`pub(crate)` を `hw_world` 内で共有） |

> `rock_fields.rs` は `mapgen` モジュール外なので、`test_seeds.rs` の定数を `pub(crate)` にするか、`hw_world` crate root レベルの `src/test_seeds.rs` に置く（Phase C 実施時に判断）。

**完了条件**: `grep -rn "10_182_272\|12_345_678\|seed.*=.*42\b" src/` のヒットが定義元 1 箇所のみになる（`river.rs` のエラーメッセージは例外）。

---

### Phase D — `wfc_adapter` の内部分割（優先度: 低・444 行で困ったとき）

**目的**: ファイル単位の認知負荷を下げる。挙動変更は伴わない。

現状 `wfc_adapter.rs` の責務は自然に 3 層に分かれている:

| 層 | 現在の内容 | 分割先（案） |
| --- | --- | --- |
| パターンデータ | `TERRAIN_PATTERN_*` 定数・`TerrainTileMapping`・`build_pattern_table` | `wfc_adapter/pattern_table.rs` |
| 制約記述 | `WorldConstraints`・`CARDINAL_DIRS`・`ForbidPattern impl` | `wfc_adapter/constraints.rs` |
| ソルバー + 後処理 | `run_wfc`・`post_process_tiles`・`apply_zone_post_process`・`derive_sub_seed`・`fallback_terrain`・`WfcError` | `wfc_adapter/mod.rs`（残留） |

全ての公開シンボル（`pub const`・`pub fn`・`pub struct`）は `mod.rs` で `pub use` 再公開することで既存呼び出し元を無変更に保つ。

---

### Phase E — `validate.rs` の内部分割（優先度: 低・528 行で困ったとき）

現状 `validate.rs` の責務:

| 層 | 内容 | 行数目安 |
| --- | --- | --- |
| 型定義 | `ValidationError`・`ValidationWarning`・`ValidationWarningKind` | ~30 |
| 地形フェーズ | `lightweight_validate`・`check_*`（4本）・`ValidatorPathWorld`・`collect_required_resource_candidates` | ~200 |
| 資源フェーズ | `validate_post_resource`・`ResourceObstaclePathWorld` | ~90 |
| デバッグ診断 | `debug_validate`・`check_*`（6本） | ~170 |
| テスト | `#[cfg(test)]` | ~40 |

分割するならば `validate/terrain.rs` / `validate/post_resource.rs` / `validate/debug.rs` + `validate/mod.rs` の 4 ファイル構成が自然。**Phase A〜C 完了後に、実際に`validate.rs` の読みづらさが問題になるまで保留が推奨**。

---

### Phase F — `WorldMasks` の分割（優先度: 低・別軸）

`world_masks.rs`（300 行）は現状過密ではない。`fill_*_from_seed` の内部実装は既に `river.rs`・`terrain_zones.rs`・`rock_fields.rs` に委譲されているため、**Phase A〜C 後も困っていなければ着手不要**。

---

## 6. リスクと緩和

| リスク | 具体的な状況 | 緩和策 |
| --- | --- | --- |
| `pub use` パス変更で downstream が壊れる | `bevy_app` が `hw_world::mapgen::*` を参照している場合 | `mod.rs` で `pub use pipeline::generate_world_layout;` を維持。変更前に `grep -rn "hw_world::mapgen" ../bevy_app/src/` で確認 |
| リファクタで挙動が変わる | `pipeline.rs` へのコードコピー時のミス | `GOLDEN_SEED_PRIMARY` テスト（`test_wfc_determinism` 等）がそのまま通ることで検知できる |
| `test_seeds.rs` の定数を `#[cfg(test)]` 外に置いてしまう | `pub(crate) const` が本番ビルドに含まれて clippy 警告になる | 必ず `#[cfg(test)]` ブロック内に置く |
| `#[ignore]` 化で `tile_dist_sim` の println が永続的に放置される | 分布が変わっても誰も気づかなくなる | `docs/DEVELOPMENT.md` に「`cargo test -p hw_world -- --ignored` で診断テスト実行」を明記 |
| `rock_fields.rs` が `mapgen::test_seeds` に依存してしまう | クレート内循環はないが `mapgen` モジュールへの依存が増える | `test_seeds.rs` を `mapgen/` 配下ではなく `hw_world/src/` 直下に置き、`pub(crate)` で `mapgen` / `rock_fields` 双方から参照できるようにする |

---

## 7. 検証コマンド（各フェーズ後）

```bash
# フェーズ完了後の必須確認（この順で実行）
CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo test -p hw_world
CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check --workspace
CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo clippy --workspace -- -D warnings

# Phase A2 完了後の追加確認（ignore テストが独立して動くこと）
CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo test -p hw_world -- --ignored

# Phase C 完了後の seed 分散確認
grep -rn "10_182_272\|12_345_678\|const.*SEED" crates/hw_world/src/
```

---

## 8. 推奨実施順序

1. **Phase A**（モジュール化 + `tile_dist_sim` の `#[ignore]`）
   - `mapgen.rs` → `mapgen/mod.rs` + `mapgen/pipeline.rs` の 2 ファイル化
   - `tile_dist_sim` の 4 テストに `#[ignore]` 付与
   - 効果: **通常テストが速くなる**・パイプライン読解の入口が明確になる
2. **Phase C**（golden seed 一元化）
   - `test_seeds.rs` 新設 + 3 ファイルの定数置換
   - MS-WFC-4.5 の残件と並行して進めやすい
3. **Phase B**（`GeneratedWorldLayout` ヘルパ）
   - Phase A でパイプラインが独立してから重複削減の効果が見えやすい
4. **Phase D / E / F** — 「具体的に読みにくい・変更しにくい」が明確なときのみ

---

## 9. 更新履歴

| 日付 | 内容 |
| --- | --- |
| `2026-04-04` | 初版（現状コード規模・課題・フェーズ分割・検証・リスク） |
| `2026-04-04` | v2: コード実測に基づき具体化（実際の行数・関数名・seed 値・before/after 構造・コード例を追記） |
