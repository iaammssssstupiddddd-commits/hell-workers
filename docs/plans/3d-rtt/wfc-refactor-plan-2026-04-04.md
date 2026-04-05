# WFC 関連リファクタ計画

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `wfc-refactor-plan-2026-04-04` |
| ステータス | `Phase A〜C 完了` |
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
| `mapgen.rs` | **453** | `generate_base_terrain_tiles`（~35行）、`generate_world_layout`（~110行）、`mod tests`（~100行）、`mod tile_dist_sim`（**~190行** / 3テスト全て `println!` 付き） |
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
| `terrain_zones.rs:449,528` | リテラル / 配列 | `12345`、`[42, 12345, 99, 0]` |
| `river.rs:594,625` | コメント内リテラル | `seed=42`（エラーメッセージ） |

**注意**: `tile_dist_sim` の 4 番目は **`12345`**、`rock_fields` テスト側は **`12_345_678`** であり、同一の「代表 seed 配列」ではない。配列を 1 本にまとめると **診断出力・分布が変わる**。Phase C では §5 のとおり **別名定数のまま共通化**するか、**値を揃える変更を明示して受け入れる**かを選ぶ。

→ **`GOLDEN_SEED_STANDARD` と `TEST_SEED_A` は同一の `u64` 値**なのに別定数として 3 ファイルに散在している。加えて `12345` 系の代表 seed も `mapgen` / `terrain_zones` に分散している。Phase C では **WFC 周辺テストで意味を持つ seed 群の置き場を一元化**する。

### 3.4 重いテスト（`tile_dist_sim` モジュール）

`mapgen.rs` 末尾の `mod tile_dist_sim` に含まれる 3 テスト:

| テスト名 | 内容 | 重さ |
| --- | --- | --- |
| `print_tile_distribution` | 4 seed × 全タイル走査 + `println!` | 高 |
| `print_neutral_breakdown` | 4 seed × 全タイル走査 + `println!` | 高 |
| `print_zone_coverage_by_distance` | 4 seed × 全タイル走査 + `println!` + `compute_anchor_distance_field` | 高 |

これらは **アサートが無く、診断目的の `println!` のみ**であるため、通常の回帰テストに含める必要がない。`#[ignore]` を付けて分離するだけで通常 `cargo test` が大幅に速くなる。

---

## 4. リファクタ方針（原則）

1. **小さく段階的に**: 各ステップ後に `cargo test -p hw_world` / `cargo clippy --workspace -- -D warnings` を通す。
2. **公開 API は維持**: `hw_world::generate_world_layout` / `GeneratedWorldLayout` / `mapgen` サブモジュールの **既存の公開パス**を壊さない（必要なら `pub use` で互換維持）。
3. **挙動不変**: 同一 `master_seed` での地形・資源・validator 結果が変わらないことを **既存テスト**で担保。定数の**名前**を変えるだけなら安全。**リテラル値を変える**（例: `12345` → `12_345_678`）と分布・ログが変わるため、Phase C では意図を明記してから行う。
4. **診断は後追い可能**: `debug_validate` / `eprintln` の有無は、分割後も同じタイミングで呼ぶ。

---

## 5. 提案フェーズ

### Phase A — `mapgen` モジュールの物理分割（優先度: 高）

**目的**: `mapgen.rs` からオーケストレーション本体とテストを分離する。

#### A1: `mapgen.rs` → `mapgen/mod.rs` へのモジュール化

現在 `hw_world/src/mapgen.rs` がモジュールルートで、`mapgen/resources.rs` 等はサブモジュールとしてぶら下がっている（`mapgen.rs` + `mapgen/` の併存はこの形で合法）。

`mapgen/mod.rs` へ寄せるときは **`mapgen.rs` を削除**し、同じ `pub mod resources;` 等を `mapgen/mod.rs` の先頭に移す（`lib.rs` の `pub mod mapgen;` はそのまま）。**`mapgen.rs` と `mapgen/mod.rs` を同時に置けない**。

```
# Before
src/mapgen.rs          ← 453行（pub mod resources; 等 + 実装混在）
src/mapgen/wfc_adapter.rs
src/mapgen/validate.rs
src/mapgen/resources.rs
src/mapgen/types.rs

# After
src/mapgen/mod.rs      ← pub mod 宣言 + generate_base_terrain_tiles の宣言、または pipeline への re-export
src/mapgen/pipeline.rs ← generate_world_layout の実装本体
src/mapgen/wfc_adapter.rs（変更なし）
src/mapgen/validate.rs（変更なし）
src/mapgen/resources.rs（変更なし）
src/mapgen/types.rs（変更なし）
```

#### A2: `tile_dist_sim` の `#[ignore]` 化

対象: `mapgen.rs` 内 `mod tile_dist_sim` の 3 テスト全て

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
- `cargo test -p hw_world -- --ignored` で 3 テストが実行可能なままである
- `docs/map_generation.md` / `docs/world_layout.md` の関連ファイル参照が `mapgen.rs` から新パスへ更新される

---

### Phase B — `generate_world_layout` のレイアウト構築ヘルパ抽出（優先度: 中）

**目的**: `GeneratedWorldLayout` の struct spread 3 段重ね × 2（正常系・fallback 系）を安全にする。

#### B1: 初期化ヘルパとマージの分離（循環参照を避ける）

`resources.rs` は既に `types::GeneratedWorldLayout` を import しているため、**`types.rs` から `resources::ResourceLayout` を引数に取る `impl` を置くとモジュール間の循環が起きやすい**。

推奨:

- **`mapgen/types.rs`**: `GeneratedWorldLayout::initial(...)` だけを追加（`ResourceLayout` は参照しない）。
- **`mapgen/resources.rs`**: `impl GeneratedWorldLayout { pub(crate) fn with_resources(self, res: ResourceLayout, water_tiles: Vec<GridPos>, sand_tiles: Vec<GridPos>) -> Self }` を追加（同一ファイル内で `ResourceLayout` が見える）。

```rust
// mapgen/types.rs — ResourceLayout を知らない
impl GeneratedWorldLayout {
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
}

// mapgen/resources.rs — pipeline からは GeneratedWorldLayout::initial → … → with_resources の順で呼ぶ
impl GeneratedWorldLayout {
    pub(crate) fn with_resources(
        self,
        res: ResourceLayout,
        water_tiles: Vec<GridPos>,
        sand_tiles: Vec<GridPos>,
    ) -> Self {
        Self {
            initial_tree_positions: res.initial_tree_positions,
            forest_regrowth_zones: res.forest_regrowth_zones,
            initial_rock_positions: res.initial_rock_positions,
            resource_spawn_candidates: crate::mapgen::types::ResourceSpawnCandidates {
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
// water_tiles / sand_tiles は lightweight_validate 済み candidate の resource_spawn_candidates から取得
Some(candidate.with_resources(res, water_tiles, sand_tiles))
```

**完了条件**: `generate_world_layout` 正常系の実装が ~60 行以内（retry ループ含む）で読める。

---

### Phase C — Golden seed モジュール（優先度: 中・MS-WFC-4.5 と連携）

**目的**: `GOLDEN_SEED_STANDARD` / `TEST_SEED_A`（同値 `10_182_272_928_891_625_829`）が 3 ファイルに散在している問題を解消する。

#### C1: テスト用 seed モジュールを新設

配置の推奨: **`hw_world/src/test_seeds.rs`** を crate 直下に置き、`lib.rs` 側で **`#[cfg(test)] mod test_seeds;`** と宣言する。ファイル内では **入れ子の `mod test_seeds { ... }` を作らず**、定数をトップレベルに置く。これにより `mapgen` / `rock_fields` / `terrain_zones` のテストから **`crate::test_seeds::*`** をそのまま参照できる。`mapgen/test_seeds.rs` に閉じる場合は `rock_fields` から `crate::mapgen::test_seeds` を見る形でもよいが、**モジュール境界は実装時に一つに決める**。

```rust
// 例: crates/hw_world/src/test_seeds.rs
//! WFC 回帰・診断テスト用 seed。本番ビルドには含めない。

/// 主回帰（旧 GOLDEN_SEED_STANDARD / TEST_SEED_A と同値）
pub(crate) const GOLDEN_SEED_PRIMARY: u64 = 10_182_272_928_891_625_829;
/// 異種確認（旧 TEST_SEED_B）
pub(crate) const GOLDEN_SEED_SECONDARY: u64 = 12_345_678;

/// `rock_fields` 回帰テスト用（4 番目は 12_345_678）
pub(crate) const SEED_SUITE_ROCK_REGRESSION: &[u64] = &[0, 42, 99, 12_345_678];

/// `tile_dist_sim` の診断プリント用（4 番目は **12345** — rock スイートと別。値を変えない限り分布は現状維持）
pub(crate) const SEED_SUITE_DIAG_PRINT: &[u64] = &[0, 42, 99, 12345];

/// `terrain_zones` の deterministic 確認に使う固定 seed
pub(crate) const TERRAIN_ZONE_DETERMINISM_SEED: u64 = 12345;

/// `terrain_zones` の代表候補 seed 群（現行テストの意味をそのまま維持）
pub(crate) const SEED_SUITE_TERRAIN_ZONE_CANDIDATES: &[u64] = &[42, 12345, 99, 0];
```

**誤りやすい点**: 上記 2 配列を 1 本にまとめないこと。まとめると **12345 と 12_345_678 のどちらかに寄せる変更**になり、§4 原則 3 と衝突する。

#### C2: 既存ファイルの定数を置き換え

| ファイル | 変更前 | 変更後 |
| --- | --- | --- |
| `mapgen/validate.rs` | `GOLDEN_SEED_STANDARD` | `crate::test_seeds::GOLDEN_SEED_PRIMARY`（パスは配置に合わせる） |
| `mapgen/resources.rs` | `TEST_SEED_A` / `TEST_SEED_B` | 同上 PRIMARY / SECONDARY |
| `mapgen.rs` / `pipeline.rs` テスト | 同上 | 同上 |
| `mapgen.rs` `tile_dist_sim` | `[0u64, 42, 99, 12345]` | `SEED_SUITE_DIAG_PRINT` |
| `rock_fields.rs` | `[0u64, 42, 99, 12_345_678]` | `SEED_SUITE_ROCK_REGRESSION` |
| `terrain_zones.rs` | `12345` / `[42, 12345, 99, 0]` | `TERRAIN_ZONE_DETERMINISM_SEED` / `SEED_SUITE_TERRAIN_ZONE_CANDIDATES` |

`lib.rs` に `#[cfg(test)] mod test_seeds;` を追加し、`test_seeds.rs` を crate 直下に置く場合は、`mapgen` / `rock_fields` / `terrain_zones` の各テストから `crate::test_seeds::*` で参照する。

**完了条件**: 主回帰用 `10_182_272...` / `12_345_678` と、WFC 周辺テストで意味を持つ `12345` / `[42, 12345, 99, 0]` / `[0, 42, 99, 12_345_678]` の **定義が `test_seeds.rs` に集約**されている。`seed=42` のようなエラーメッセージ文字列は除外してよい。

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
| `pub use` パス変更で downstream が壊れる | `bevy_app` が `hw_world::mapgen::*` を参照している場合 | `mod.rs` で `pub use pipeline::generate_world_layout;` を維持。変更前に `rg -n "hw_world::mapgen" crates/bevy_app/src crates/hw_world/src` で確認 |
| リファクタで挙動が変わる | `pipeline.rs` へのコードコピー時のミス | `GOLDEN_SEED_PRIMARY` テスト（`test_wfc_determinism` 等）がそのまま通ることで検知できる |
| `test_seeds.rs` の gating を二重化する | `lib.rs` 側でも `test_seeds.rs` 側でも `mod test_seeds` を作ると `crate::test_seeds::*` で見えない | `lib.rs` にだけ `#[cfg(test)] mod test_seeds;` を置き、`test_seeds.rs` 内はトップレベル定数だけにする |
| `#[ignore]` 化で `tile_dist_sim` の println が永続的に放置される | 分布が変わっても誰も気づかなくなる | `docs/DEVELOPMENT.md` に「`cargo test -p hw_world -- --ignored` で診断テスト実行」を明記 |
| `rock_fields` / `terrain_zones` / `mapgen` の seed 集約範囲がずれる | 一部だけ `test_seeds` に寄せて、他が取り残される | **`hw_world/src/test_seeds.rs` を共通置き場**にし、`mapgen` / `rock_fields` / `terrain_zones` はどれも `crate::test_seeds::*` を参照する |
| ファイル分割後に docs の参照先が古いまま残る | `docs/map_generation.md` / `docs/world_layout.md` が削除済み `mapgen.rs` を指す | Phase A の完了条件に docs 更新を含め、変更後にリンク先を目視確認する |

---

## 7. 検証コマンド（各フェーズ後）

```bash
# フェーズ完了後の必須確認（この順で実行）
CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo test -p hw_world
CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check --workspace
CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo clippy --workspace -- -D warnings

# Phase A2 完了後の追加確認（ignore テストが独立して動くこと）
CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo test -p hw_world -- --ignored

# Phase C 完了後の seed 分散確認（test_seeds.rs への集約を確認）
rg -n "GOLDEN_SEED|TEST_SEED_[AB]|SEED_SUITE_|TERRAIN_ZONE_DETERMINISM_SEED|12345|12_345_678" crates/hw_world/src
```

---

## 8. 推奨実施順序

1. **Phase A**（モジュール化 + `tile_dist_sim` の `#[ignore]`）
   - `mapgen.rs` → `mapgen/mod.rs` + `mapgen/pipeline.rs` の 2 ファイル化
   - `tile_dist_sim` の 3 テストに `#[ignore]` 付与（診断用 `println` 付き）
   - 効果: **通常テストが速くなる**・パイプライン読解の入口が明確になる
2. **Phase C**（golden seed 一元化）
   - `src/test_seeds.rs`（推奨）または `mapgen/test_seeds.rs` を新設 + 各テストの定数置換（診断用 `12345` は `SEED_SUITE_DIAG_PRINT` に分離）
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
| `2026-04-04` | v4: Phase A〜C 実装完了。全テスト通過（50 passed / 3 ignored）・clippy 0 warnings |
