# MS-WFC-2b: WFC ソルバー統合と制約マスキング

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `wfc-ms2b-wfc-solver-constraints` |
| ステータス | `Draft` |
| 作成日 | `2026-04-01` |
| 最終更新日 | `2026-04-04` |
| 親計画 | [`wfc-terrain-generation-plan-2026-04-01.md`](wfc-terrain-generation-plan-2026-04-01.md) |
| 前MS | [`wfc-ms2a-crate-adapter-river-mask.md`](wfc-ms2a-crate-adapter-river-mask.md) |
| 次MS | [`wfc-ms2c-validator.md`](wfc-ms2c-validator.md) |
| 前提 | アダプタ骨格・川マスク生成が完成（MS-WFC-2a 完了） |

---

## 1. 目的

`run_wfc()` の `todo!()` を実装し、**実際に WFC ソルバーで地形グリッドを生成する**。

- `wfc::RunOwn::new_forbid()` + `collapse()` でグリッドを収束させる
- F4 は **WFC 制約 + post-process** の組み合わせで実装する（グローバル重みだけに依存しない）
- 収束失敗時の **deterministic retry + fallback** を実装する
- スタブ地形生成関数を WFC 実装に置き換え、`generate_world_layout()` を完成させる

---

## 2. MS-WFC-2a で完成済みの資産（変更不要）

実装前に以下が `wfc_adapter.rs` に存在することを確認する。

| 資産 | 内容 |
| --- | --- |
| `TERRAIN_PATTERN_{GRASS,DIRT,SAND,RIVER}: PatternId` | `u32` 定数（0–3） |
| `SAND_ADJACENT_TO_RIVER_WEIGHT: u32 = 10` | F4 用の定数。2b 実装では `WEIGHT_SAND` の基準値として使う |
| `SAND_NON_ADJACENT_WEIGHT: u32 = 1` | 将来、内陸 Sand を解禁する場合の予約値。**2b 初版では未使用でよい** |
| `TerrainTileMapping::to_pattern_id()` / `from_pattern_id()` | `TerrainType ↔ PatternId` 変換 |
| `build_pattern_table() -> PatternTable<PatternDescription>` | 隣接ルール定義（2a 時点では全パターン重み `1` → **2b で §4.1 に従い変更**） |
| `WorldConstraints: ForbidPattern` + `#[derive(Clone)]` | River 固定とマスク外 River 禁止。`RunOwn::new_wrap_forbid` でも `F: Clone + Send + Sync` が必要なため **`Clone` 必須** |
| `run_wfc(masks, seed, attempt) -> Result<Vec<TerrainType>, WfcError>` | `todo!()` プレースホルダ |

---

## 3. wfc 0.10.7 API クイックリファレンス（実ソースで確認済み）

| 型 / 関数 | シグネチャ / 説明 |
| --- | --- |
| `Size::new` | `Size::new(width: u32, height: u32)` (`coord_2d` から `wfc` が re-export) |
| `RunOwn::new_forbid` | `fn new_forbid<R: Rng>(output_size: Size, global_stats: &'a GlobalStats, forbid: F, rng: &mut R) -> Self` ただし `F: Clone + Sync + Send` |
| `RunOwn::collapse` | `fn collapse<R: Rng>(&mut self, rng: &mut R) -> Result<(), PropagateError>` |
| `RunOwn::into_wave` | `fn into_wave(self) -> Wave` — 成功後に呼ぶ |
| `Wave::grid` | `fn grid(&self) -> &Grid<WaveCell>` |
| `Grid::iter` | `&WaveCell` をイテレート。**実装時**に `wfc` / `grid_2d` の `Grid::iter` が **row-major（`idx = y * width + x`）**であることをソースまたは docs.rs で一度確認し、`terrain_tiles` の並びと一致させる |
| `WaveCell::chosen_pattern_id` | `fn chosen_pattern_id(&self) -> Result<PatternId, ChosenPatternIdError>` — collapse 後は必ず `Ok` |
| `GlobalStats::new` | `fn new(pattern_table: PatternTable<PatternDescription>) -> Self` |

---

## 4. 実装詳細

### 4.1 タイル重みの修正（`build_pattern_table()` の変更）

現在の `build_pattern_table()` は全パターン重み `1`。以下の重みに変更する。  
**重要**: `wfc` の重みはパターン種別にグローバルに設定されるため、「川隣接セルだけ Sand を出しやすくする」といった**位置依存の F4** は **重みだけでは実装できない**。  
実装では次のように分担した:

1. `WorldConstraints` では **River の固定** と **マスク外 River 禁止** だけを直接適用する
2. `build_pattern_table()` の重みで Grass / Dirt / Sand の頻度バイアスをかける
3. `post_process_tiles()` で **anchor 上の Sand 禁止** と **川非隣接 Sand の除去** を後段で強制する

この形にした理由は、`wfc` 0.10.7 の `ForbidPattern::forbid()` で weighted pattern を直接 `forbid_pattern` すると、priority queue 初期化との順序の都合で stale entry 問題を起こしうるため。

```rust
// パターン別の重み定数（wfc_adapter.rs に追加）
pub const WEIGHT_GRASS: u32 = 5;
pub const WEIGHT_DIRT: u32 = 2;
pub const WEIGHT_SAND: u32 = SAND_ADJACENT_TO_RIVER_WEIGHT;  // = 10
// River は WFC が自由に選択しないよう重みなし（None = unweighted、固定セルは hard constraint で管理）
```

`build_pattern_table()` の重み部分を以下に変更:

```rust
let weights: [Option<NonZeroU32>; 4] = [
    NonZeroU32::new(WEIGHT_GRASS),           // Grass
    NonZeroU32::new(WEIGHT_DIRT),            // Dirt
    NonZeroU32::new(WEIGHT_SAND),            // Sand
    None,                                    // River: unweighted（hard constraint で制御）
];
// ...
.map(|id| {
    let nbrs = allowed[id as usize].clone();
    PatternDescription::new(
        weights[id as usize],
        CardinalDirectionTable::new_array([nbrs.clone(), nbrs.clone(), nbrs.clone(), nbrs]),
    )
})
```

### 4.2 `WorldConstraints::forbid()` の実装方針

2b 実装では `WorldConstraints` に **River 系制約だけ**を持たせる。  
anchor 上の Sand 禁止や川非隣接 Sand 禁止は、WFC 完了後の `post_process_tiles()` で処理する。

理由:

- `River` は unweighted (`None`) にしているため、非 River セルへの `forbid_pattern(coord, RIVER)` は observer の weighted candidate 数を壊さない
- `Sand` は weighted pattern なので、`forbid()` 内で直接除去すると `wfc` 0.10.7 の priority queue 初期状態と不整合を起こすおそれがある
- そのため WFC 本体では「River を正しい位置に固定する」ことを優先し、F4 の細部はポスト処理で詰める

```rust
pub struct WorldConstraints {
    fixed_river: Vec<Coord>,
    river_forbidden_cells: Vec<Coord>,
}

impl WorldConstraints {
    pub fn from_masks(masks: &WorldMasks) -> Self {
        let mut fixed_river = Vec::new();
        let mut river_forbidden_cells = Vec::new();

        for y in 0..MAP_HEIGHT {
            for x in 0..MAP_WIDTH {
                let coord = Coord::new(x, y);
                if masks.river_mask.get((x, y)) {
                    fixed_river.push(coord);
                } else {
                    river_forbidden_cells.push(coord);
                }
            }
        }

        WorldConstraints {
            fixed_river,
            river_forbidden_cells,
        }
    }
}

impl ForbidPattern for WorldConstraints {
    fn forbid<W: Wrap, R: Rng>(&mut self, fi: &mut ForbidInterface<W>, rng: &mut R) {
        // River マスクセル → River に固定
        for &coord in &self.fixed_river {
            fi.forbid_all_patterns_except(coord, TERRAIN_PATTERN_RIVER, rng)
                .expect("river hard constraint caused contradiction");
        }
        // River マスク外の全セル → River を禁止（マスク外への River 伝播防止）
        for &coord in &self.river_forbidden_cells {
            fi.forbid_pattern(coord, TERRAIN_PATTERN_RIVER, rng)
                .expect("river forbid outside river_mask caused contradiction");
        }
    }
}
```

### 4.3 `derive_sub_seed()`

```rust
/// master_seed と attempt から deterministic に sub_seed を導出する。
/// splitmix64 の 1 ステップを使ってビットを分散させる。
pub(crate) fn derive_sub_seed(master_seed: u64, attempt: u32) -> u64 {
    master_seed.wrapping_add((attempt as u64).wrapping_mul(0x9e3779b97f4a7c15))
}
```

### 4.4 `run_wfc()` 本体の実装

現在の `todo!()` を以下で置き換える。実装では `WrapNone` を使って非ラップの平面グリッドとして実行する。

```rust
// wfc_adapter.rs の先頭に追加
use rand::SeedableRng;
use rand::rngs::StdRng;
use wfc::{RunOwn, Size};
use wfc::wrap::WrapNone;

pub const MAX_WFC_RETRIES: u32 = 64;

pub fn run_wfc(
    masks: &WorldMasks,
    seed: u64,
    attempt: u32,
) -> Result<Vec<TerrainType>, WfcError> {
    let _ = attempt; // ログ用（将来 tracing::debug! に差し替え可）

    let table = build_pattern_table();
    let global_stats = GlobalStats::new(table);
    let constraints = WorldConstraints::from_masks(masks);
    let size = Size::new(MAP_WIDTH as u32, MAP_HEIGHT as u32);
    let mut rng = StdRng::seed_from_u64(seed);

    let mut run = RunOwn::new_wrap_forbid(size, &global_stats, WrapNone, constraints, &mut rng);
    run.collapse(&mut rng).map_err(|_| WfcError::Contradiction)?;

    let wave = run.into_wave();
    let tiles = wave
        .grid()
        .iter()  // row-major (y * MAP_WIDTH + x) — WorldMasks と一致
        .map(|cell| {
            let pid = cell
                .chosen_pattern_id()
                .expect("WFC: cell not collapsed after successful collapse");
            TerrainTileMapping::from_pattern_id(pid)
                .expect("WFC: unknown PatternId in result")
        })
        .collect::<Vec<TerrainType>>();

    post_process_tiles(&mut tiles, masks, &mut rng);
    Ok(tiles)
}
```

### 4.5 `post_process_tiles()`

`ForbidPattern` で直接かけられない weighted 制約は、WFC 完了後に `post_process_tiles()` で補正する。

```rust
fn post_process_tiles(tiles: &mut [TerrainType], masks: &WorldMasks, rng: &mut StdRng) {
    for y in 0..MAP_HEIGHT {
        for x in 0..MAP_WIDTH {
            let idx = (y * MAP_WIDTH + x) as usize;
            if masks.river_mask.get((x, y)) {
                continue;
            }
            let is_river_adjacent = CARDINAL_DIRS
                .iter()
                .any(|(dx, dy)| masks.river_mask.get((x + dx, y + dy)));
            let is_anchor = masks.anchor_mask.get((x, y));
            if tiles[idx] == TerrainType::Sand && (!is_river_adjacent || is_anchor) {
                let total = WEIGHT_GRASS + WEIGHT_DIRT;
                let r = rng.gen_range(0..total);
                tiles[idx] = if r < WEIGHT_GRASS {
                    TerrainType::Grass
                } else {
                    TerrainType::Dirt
                };
            }
        }
    }
}
```

### 4.6 `fallback_terrain()`

```rust
/// WFC が全試行で収束しなかった場合の安全マップ。
/// hard constraint（River マスク・anchor 禁止）は維持し、残りを Grass で埋める。
/// **Sand は配置しない**（成功時の WFC より簡略）。River 以外は Grass のみ。
pub(crate) fn fallback_terrain(masks: &WorldMasks) -> Vec<TerrainType> {
    use hw_core::constants::{MAP_HEIGHT, MAP_WIDTH};
    let mut tiles = vec![TerrainType::Grass; (MAP_WIDTH * MAP_HEIGHT) as usize];
    for y in 0..MAP_HEIGHT {
        for x in 0..MAP_WIDTH {
            let pos = (x, y);
            let idx = (y * MAP_WIDTH + x) as usize;
            if masks.river_mask.get(pos) {
                tiles[idx] = TerrainType::River;
            }
            // anchor セルは Grass のまま（River / Sand 禁止を維持）
        }
    }
    tiles
}
```

### 4.7 `generate_world_layout()` の WFC 実装への置き換え

`mapgen.rs` の `generate_world_layout()` を以下で置き換える。  
`generate_stub_terrain_tiles_from_masks()` は削除する（dead code になるため）。

```rust
pub fn generate_world_layout(master_seed: u64) -> types::GeneratedWorldLayout {
    use crate::anchor::AnchorLayout;
    use crate::world_masks::WorldMasks;
    use types::ResourceSpawnCandidates;
    use wfc_adapter::{
        derive_sub_seed, fallback_terrain, run_wfc, MAX_WFC_RETRIES,
    };

    let anchors = AnchorLayout::fixed();
    let mut masks = WorldMasks::from_anchor(&anchors);
    masks.fill_river_from_seed(master_seed);

    let (terrain_tiles, attempt, used_fallback) = (0..=MAX_WFC_RETRIES)
        .find_map(|attempt| {
            let sub_seed = derive_sub_seed(master_seed, attempt);
            run_wfc(&masks, sub_seed, attempt)
                .ok()
                .map(|tiles| (tiles, attempt, false))
        })
        .unwrap_or_else(|| {
            eprintln!("WFC: fallback terrain used for master_seed={master_seed}");
            (fallback_terrain(&masks), MAX_WFC_RETRIES + 1, true)
        });

    types::GeneratedWorldLayout {
        terrain_tiles,
        anchors,
        masks,
        resource_spawn_candidates: ResourceSpawnCandidates::default(),
        initial_tree_positions: Vec::new(),
        forest_regrowth_zones: Vec::new(),
        initial_rock_positions: Vec::new(),
        master_seed,
        generation_attempt: attempt,
        used_fallback,
    }
}
```

---

## 5. 隣接ルール（MS-WFC-2a 実装済み・変更なし）

F2 方針: カーディナル 4 近傍のみ、等方的（全方向同一ルール）。

| 許可する隣接 | 備考 |
| --- | --- |
| Grass ↔ Grass | |
| Grass ↔ Dirt | |
| Grass ↔ Sand | PatternTable 上は許可。ただし 2b 実装では `post_process_tiles()` が内陸 / anchor 上の Sand を除去する |
| Dirt ↔ Dirt | |
| Dirt ↔ Sand | PatternTable 上は許可。ただし 2b 実装では `post_process_tiles()` が内陸 / anchor 上の Sand を除去する |
| Sand ↔ Sand | |
| Sand ↔ River | 川沿い砂浜を形成 |
| River ↔ River | 連続する川セルに必要 |

**River ↔ Grass / River ↔ Dirt は禁止**（MS-WFC-2a 実装と一致）。  
River がマスク外に出ないことは §4.2 の `WorldConstraints`、Sand が川隣接かつ非 anchor に限定されることは §4.5 の `post_process_tiles()` で保証する。

---

## 6. 変更ファイルと責務

| ファイル | 変更内容 |
| --- | --- |
| `crates/hw_world/src/mapgen/wfc_adapter.rs` | ① `build_pattern_table()` の重み修正 ② `WorldConstraints` に `river_forbidden_cells` を追加 ③ `derive_sub_seed()` 追加 ④ `MAX_WFC_RETRIES` 定数追加 ⑤ `run_wfc()` 本体実装 ⑥ `post_process_tiles()` 追加 ⑦ `fallback_terrain()` 追加 ⑧ import 追加（`StdRng`, `SeedableRng`, `RunOwn`, `Size`, `WrapNone`） |
| `crates/hw_world/src/mapgen.rs` | `generate_world_layout()` を WFC 実装に置き換え、`generate_stub_terrain_tiles_from_masks()` を削除 |

`weights.rs` は作成しない。重み定数は `wfc_adapter.rs` の `WEIGHT_*` / `SAND_*` 定数に集約済み。

---

## 7. 完了条件チェックリスト

- [ ] `run_wfc()` が `wfc::RunOwn::new_forbid()` + `collapse()` でグリッドを生成する
- [ ] 生成結果が row-major の `Vec<TerrainType>` として返る
- [ ] Site/Yard（`anchor_mask`）内に River / Sand が生成されない
- [ ] River がリバーマスク外のセルに出現しない（`river_forbidden_cells` 禁止）
- [ ] 2b 実装では `post_process_tiles()` 後の Sand が「川隣接かつ非 anchor」のセルにしか残らない
- [ ] 同一 `master_seed` で同一マップが生成される（determinism テスト）
- [ ] 別 `master_seed` で地形分布が変化する（seed 差分テスト）
- [ ] fallback 到達時に `used_fallback == true` となり、ログで判別できる
- [ ] `generate_world_layout()` が WFC 実装を呼び出す（スタブを削除）
- [ ] `generate_stub_terrain_tiles_from_masks()` が削除されている（dead code なし）
- [ ] `WEIGHT_*` / `MAX_WFC_RETRIES` が定数として定義されている
- [ ] `CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check --workspace` が通る
- [ ] `CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo clippy --workspace` が通る

---

## 8. テスト

既存テスト `generated_world_layout_river_mask_matches_terrain_tiles` が WFC 置き換え後も通ることを確認すること。

追加テスト（`crates/hw_world/src/mapgen.rs` の `mod tests` に追加）:

```rust
/// シード定数（テスト専用）
const TEST_SEED_A: u64 = 42;
const TEST_SEED_B: u64 = 12345678;

#[test]
fn test_wfc_determinism() {
    let layout1 = generate_world_layout(TEST_SEED_A);
    let layout2 = generate_world_layout(TEST_SEED_A);
    assert_eq!(layout1.terrain_tiles, layout2.terrain_tiles);
}

#[test]
fn test_wfc_different_seeds_differ() {
    let layout_a = generate_world_layout(TEST_SEED_A);
    let layout_b = generate_world_layout(TEST_SEED_B);
    assert_ne!(layout_a.terrain_tiles, layout_b.terrain_tiles);
}

#[test]
fn test_site_yard_no_river_sand() {
    use crate::terrain::TerrainType;
    use hw_core::constants::{MAP_HEIGHT, MAP_WIDTH};

    let layout = generate_world_layout(TEST_SEED_A);
    for y in 0..MAP_HEIGHT {
        for x in 0..MAP_WIDTH {
            if layout.masks.anchor_mask.get((x, y)) {
                let tile = layout.terrain_tiles[(y * MAP_WIDTH + x) as usize];
                assert!(
                    !matches!(tile, TerrainType::River | TerrainType::Sand),
                    "anchor cell ({x},{y}) has forbidden terrain {tile:?}"
                );
            }
        }
    }
}

#[test]
fn test_river_stays_in_mask() {
    use crate::terrain::TerrainType;
    use hw_core::constants::{MAP_HEIGHT, MAP_WIDTH};

    let layout = generate_world_layout(TEST_SEED_A);
    for y in 0..MAP_HEIGHT {
        for x in 0..MAP_WIDTH {
            let is_river_tile = layout.terrain_tiles[(y * MAP_WIDTH + x) as usize]
                == TerrainType::River;
            let in_river_mask = layout.masks.river_mask.get((x, y));
            assert_eq!(
                is_river_tile, in_river_mask,
                "river mask mismatch at ({x},{y}): tile={is_river_tile}, mask={in_river_mask}"
            );
        }
    }
}

#[test]
fn test_sand_is_only_river_adjacent_in_ms2b() {
    use wfc_adapter::CARDINAL_DIRS;

    let layout = generate_world_layout(TEST_SEED_A);
    for y in 0..MAP_HEIGHT {
        for x in 0..MAP_WIDTH {
            let idx = (y * MAP_WIDTH + x) as usize;
            if layout.terrain_tiles[idx] != TerrainType::Sand {
                continue;
            }
            assert!(
                !layout.masks.anchor_mask.get((x, y)),
                "sand must not appear in anchor at ({x},{y})"
            );
            let is_river_adjacent = CARDINAL_DIRS
                .iter()
                .any(|(dx, dy)| layout.masks.river_mask.get((x + dx, y + dy)));
            assert!(
                is_river_adjacent,
                "sand at ({x},{y}) is not cardinally adjacent to river"
            );
        }
    }
}
```

---

## 9. 検証コマンド

```sh
CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo test -p hw_world
CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check --workspace
CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo clippy --workspace 2>&1 | grep "^warning:" | grep -v generated
```

---

## 10. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| `2026-04-01` | `Copilot` | wfc-terrain-generation-plan-2026-04-01.md の MS-WFC-2 を分割・詳細化 |
| `2026-04-04` | `Copilot` | wfc 0.10.7 実ソース確認に基づき全面ブラッシュアップ：具体的な実装コード追加、River 伝播抑制ルール追加、重みアーキテクチャ明確化、テスト・ファイル構成修正 |
| `2026-04-04` | — | 実装同期: weighted `forbid_pattern` の stale entry 回避のため、River 制約は WFC 本体、Sand / anchor 制約は `post_process_tiles()` に分離した。fallback は常にログ付き続行に合わせて文書を更新。 |
