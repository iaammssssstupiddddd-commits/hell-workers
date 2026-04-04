# MS-WFC-2d: River 派生の砂マスク再設計

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `wfc-ms2d-river-driven-sand-mask` |
| ステータス | `完了`（`crates/hw_world/src/{world_masks,river}.rs`, `crates/hw_world/src/mapgen/{wfc_adapter,validate}.rs`） |
| 作成日 | `2026-04-04` |
| 最終更新日 | `2026-04-04` |
| 親計画 | [`wfc-terrain-generation-plan-2026-04-01.md`](wfc-terrain-generation-plan-2026-04-01.md) |
| 前MS | [`wfc-ms2c-validator.md`](wfc-ms2c-validator.md) |
| 次MS | [`wfc-ms2e-sand-shore-shape.md`](wfc-ms2e-sand-shore-shape.md) |
| 前提 | `river_mask` / `post_process_tiles()` / `lightweight_validate()` が実装済み（MS-WFC-2b/2c 完了） |

### サマリ

| 項目 | 内容 |
| --- | --- |
| 実装内容 | **完了**: `river_mask` から **8 近傍 shoreline sand mask** を deterministic に生成し、さらに **連続した non-sand carve** を差し引いた `final_sand_mask` を `WorldMasks` に保持する |
| WFC との関係 | WFC 本体には weighted な Sand hard constraint を追加していない。`final_sand_mask` は `run_wfc()` の入力文脈として渡し、**`post_process_tiles()` と `fallback_terrain()` が最終地形へ反映**する |
| 置き換えたもの | 「Sand は WFC が出したものを cardinal-adjacent 条件で残す」という 2b の後処理方針 |
| 到達状態 | diagonal Sand を仕様として許容しつつ、砂地の形を River 派生で deterministic に制御できる |
| やらないこと | retry 条件の追加、WFC crate の置換、validator での自動修復、木・岩配置 |

---

## 1. 背景

現状の 2b 実装では、`Sand` は WFC 結果から選ばれ、`post_process_tiles()` が
「River に **4 近傍**で接していない `Sand`」を `Grass/Dirt` に落としている。

この方式には以下の欠点がある。

- 砂地の形が River 由来の deterministic な説明になっていない
- diagonal Sand を仕様として扱えない
- Sand の見た目が `wfc` の weighted pattern 出力に引きずられる
- `wfc` 0.10.x の priority queue 制約により、weighted `Sand` を `ForbidPattern` で直接制御しづらい

今回の方針では、**砂は WFC の「候補」ではなく River 由来の派生マスク**として扱う。

---

## 2. 目的

- `Sand` の責務を「WFC がたまたま選ぶ地形」から「River の岸辺を表す deterministic 地形」へ移す
- diagonal 方向の砂を仕様として許容する
- 一方で 8 近傍をそのまま全面 `Sand` にせず、seed 由来の **連続した non-sand carve** で砂浜の抜けを作る
- weighted pattern の追加制約を WFC 本体へ持ち込まず、現行アダプタ構造を維持する

---

## 3. 設計方針

### 3.1 生成順

砂に関する生成順を以下へ変更する。

1. `AnchorLayout::fixed()`
2. `WorldMasks::from_anchor()`
3. `fill_river_from_seed(master_seed)` で `river_mask` / `river_centerline`
4. **`WorldMasks::fill_sand_from_river_seed(master_seed)`**（名前は実装で調整可）で以下を生成
   - `sand_candidate_mask`
   - `sand_carve_mask`
   - `final_sand_mask`
5. WFC は River 固定を前提に Grass/Dirt を中心に collapse
6. 成功時は `post_process_tiles()`、fallback 時は `fallback_terrain()` が `final_sand_mask` を最終地形へ反映

**API の置き場**: 川と同様、**公開エントリは `WorldMasks` に置く**（`fill_river_from_seed` と対称）。`river_mask` から 8 近傍 candidate を立てる処理・carve の幾何は `river.rs` に **private helper** として切り出してよい。`mapgen.rs` では `masks.fill_river_from_seed(master_seed)` の直後に `masks.fill_sand_from_river_seed(master_seed)` を呼び、retry 成功時も fallback 時も同じ `masks.final_sand_mask` を使う。

重要: **`final_sand_mask` は WFC に「直接 forbid/fix する hard constraint」としては渡さない。**
`WorldMasks` の一部として `run_wfc()` に渡し、後段 `post_process_tiles()` で最終地形を確定する。

### 3.2 なぜ WFC 側で直接やらないか

- 現行 `wfc` 0.10.x では weighted pattern の `forbid_pattern` が stale entry 問題を起こしうる
- `Sand` は weighted pattern なので、River 同様の制約適用をそのまま流用できない
- retry を増やして解く方針は採らない

したがって本 MS では、**WFC の役割を Grass/Dirt の分布に絞り、Sand は River 派生 mask として外側で決める。**

---

## 4. `WorldMasks` の拡張

`crates/hw_world/src/world_masks.rs` に以下を追加する。

### 4.1 フィールド追加

```rust
pub struct WorldMasks {
    // 既存フィールドはそのまま
    pub site_mask: BitGrid,
    pub yard_mask: BitGrid,
    pub anchor_mask: BitGrid,
    pub river_protection_band: BitGrid,
    pub rock_protection_band: BitGrid,
    pub tree_dense_protection_band: BitGrid,
    pub river_mask: BitGrid,
    pub river_centerline: Vec<GridPos>,

    // 新規（MS-WFC-2d）
    /// river_mask の 8 近傍から作る「砂にしてよい元候補」
    pub sand_candidate_mask: BitGrid,
    /// seed 由来で candidate から削る連続 non-sand 領域
    pub sand_carve_mask: BitGrid,
    /// sand_candidate_mask から sand_carve_mask を除いた結果。post_process が最終的に Sand にするセル
    pub final_sand_mask: BitGrid,
}
```

### 4.2 `from_anchor()` の変更

既存の構造体リテラルに 3 フィールドを追記するのみ。

```rust
WorldMasks {
    // 既存フィールド（変更なし）
    site_mask,
    yard_mask,
    anchor_mask: anchor_mask.clone(),
    river_protection_band: compute_protection_band(&anchor_mask, PROTECTION_BAND_RIVER_WIDTH),
    rock_protection_band: compute_protection_band(&anchor_mask, PROTECTION_BAND_ROCK_WIDTH),
    tree_dense_protection_band: compute_protection_band(&anchor_mask, PROTECTION_BAND_TREE_DENSE_WIDTH),
    river_mask: BitGrid::map_sized(),
    river_centerline: Vec::new(),

    // 追加
    sand_candidate_mask: BitGrid::map_sized(),  // fill_sand_from_river_seed で設定
    sand_carve_mask: BitGrid::map_sized(),       // fill_sand_from_river_seed で設定
    final_sand_mask: BitGrid::map_sized(),       // fill_sand_from_river_seed で設定
}
```

### 4.3 新規メソッド `fill_sand_from_river_seed()`

`fill_river_from_seed()` と対称な公開 API。`from_anchor()` + `fill_river_from_seed()` 済みであること。

```rust
/// `fill_river_from_seed()` 適用済みの `river_mask` を参照し、
/// seed から deterministic に 3 つの砂マスクを生成して設定する。
///
/// # Panics
/// `fill_river_from_seed` が先に呼ばれていない場合（river_mask が空）に debug_assert で検出する。
pub fn fill_sand_from_river_seed(&mut self, seed: u64) {
    debug_assert!(
        self.river_mask.count_set() > 0,
        "fill_sand_from_river_seed は fill_river_from_seed の後に呼ぶこと"
    );
    let (candidate, carve, final_mask) = crate::river::generate_sand_masks(
        seed,
        &self.river_mask,
        &self.anchor_mask,
        &self.river_protection_band,
    );
    self.sand_candidate_mask = candidate;
    self.sand_carve_mask = carve;
    self.final_sand_mask = final_mask;
}
```

---

## 5. 砂マスク生成アルゴリズム（`river.rs` への追加）

### 5.0 追加 import / module-level 定数

`river.rs` の先頭部分に追加する。

```rust
use std::collections::VecDeque;
// rand::Rng は既存 generate_river_mask が use rand::Rng; をブロック内 use しているため
// モジュールレベルに引き上げる

// 既存定数に追加
/// 砂マスク生成用 8 近傍
const EIGHT_DIRS: [(i32, i32); 8] = [
    (0, -1), (1, 0), (0, 1), (-1, 0),
    (1, -1), (1, 1), (-1, 1), (-1, -1),
];
/// 砂 carve flood-fill 用 4 近傍（wfc_adapter 依存を避けて独立定義）
const CARDINAL_DIRS_4: [(i32, i32); 4] = [(0, -1), (1, 0), (0, 1), (-1, 0)];

// ── 砂 carve 定数 ─────────────────────────────────────────────────────────────
/// carve 起点数の下限
pub const SAND_CARVE_SEED_COUNT_MIN: u32 = 2;
/// carve 起点数の上限
pub const SAND_CARVE_SEED_COUNT_MAX: u32 = 5;
/// candidate 全体に対する最大 carve 割合（%）
pub const SAND_CARVE_MAX_RATIO_PERCENT: usize = 35;
/// 1 carve region の最小面積（セル数）
pub const SAND_CARVE_REGION_SIZE_MIN: u32 = 6;
/// 1 carve region の最大面積（セル数）
pub const SAND_CARVE_REGION_SIZE_MAX: u32 = 24;
/// river RNG と区別するための seed XOR マスク
const SAND_SEED_SALT: u64 = 0xA5A5_A5A5_A5A5_A5A5;
```

### 5.1 公開エントリ `generate_sand_masks()`

`WorldMasks::fill_sand_from_river_seed()` から呼ばれる。

```rust
/// seed から deterministic に砂マスク 3 点セットを生成して返す。
///
/// # 戻り値
/// `(sand_candidate_mask, sand_carve_mask, final_sand_mask)`
pub fn generate_sand_masks(
    seed: u64,
    river_mask: &BitGrid,
    anchor_mask: &BitGrid,
    river_protection_band: &BitGrid,
) -> (BitGrid, BitGrid, BitGrid) {
    use rand::Rng;
    // river RNG と区別するため XOR salt で別シード化
    let mut rng = StdRng::seed_from_u64(seed ^ SAND_SEED_SALT);

    let candidate = build_sand_candidate_mask(river_mask, anchor_mask, river_protection_band);
    let mut carve = build_sand_carve_mask(&candidate, &mut rng);

    // final = candidate & !carve
    let mut final_mask = candidate.clone();
    for y in 0..MAP_HEIGHT {
        for x in 0..MAP_WIDTH {
            if carve.get((x, y)) {
                final_mask.set((x, y), false);
            }
        }
    }

    // フォールバック: final が空なら carve を全捨てして candidate 全面を採用
    if final_mask.count_set() == 0 {
        carve = BitGrid::map_sized();
        final_mask = candidate.clone();
    }

    (candidate, carve, final_mask)
}
```

**`sand_candidate_mask` が空の場合**: `final_sand_mask` も空のままとなり、`lightweight_validate` の砂源到達（`RequiredResourceNotReachable`）で **失敗し得る**。本 MS の `final_sand_mask_is_non_empty`（§10.1）等は **shoreline 候補が少なくとも 1 セルある seed**（例: `TEST_SEED_A = 10_182_272_928_891_625_829`）で成立することを回帰とする。河形状によって候補ゼロになり得る場合は、**retry / 別 seed** や `generate_river_mask` 側の前提を親計画で別途扱う。

### 5.2 private helper: `build_sand_candidate_mask()`

```rust
fn build_sand_candidate_mask(
    river_mask: &BitGrid,
    anchor_mask: &BitGrid,
    river_protection_band: &BitGrid,
) -> BitGrid {
    let mut candidate = BitGrid::map_sized();
    for y in 0..MAP_HEIGHT {
        for x in 0..MAP_WIDTH {
            if !river_mask.get((x, y)) {
                continue;
            }
            for (dx, dy) in EIGHT_DIRS {
                let nx = x + dx;
                let ny = y + dy;
                if !(0..MAP_WIDTH).contains(&nx) || !(0..MAP_HEIGHT).contains(&ny) {
                    continue;
                }
                let p = (nx, ny);
                if !river_mask.get(p) && !anchor_mask.get(p) && !river_protection_band.get(p) {
                    candidate.set(p, true);
                }
            }
        }
    }
    candidate
}
```

### 5.3 private helper: `build_sand_carve_mask()`

```rust
fn build_sand_carve_mask(candidate: &BitGrid, rng: &mut StdRng) -> BitGrid {
    use rand::Rng;

    let candidate_positions: Vec<GridPos> = (0..MAP_HEIGHT)
        .flat_map(|y| {
            (0..MAP_WIDTH).filter_map(move |x| candidate.get((x, y)).then_some((x, y)))
        })
        .collect();

    let mut carve = BitGrid::map_sized();
    if candidate_positions.is_empty() {
        return carve;
    }

    let max_carve_cells =
        candidate_positions.len() * SAND_CARVE_MAX_RATIO_PERCENT / 100;
    let num_seeds = rng.gen_range(SAND_CARVE_SEED_COUNT_MIN..=SAND_CARVE_SEED_COUNT_MAX);
    let k = (num_seeds as usize).min(candidate_positions.len());

    // choose_multiple（`rand::seq::SliceRandom`）: `k` を candidate 長で cap（rand 版差に依存させない）
    let origins: Vec<GridPos> = candidate_positions
        .choose_multiple(rng, k)
        .copied()
        .collect();

    let mut total_carved = 0usize;
    for origin in origins {
        if total_carved >= max_carve_cells {
            break;
        }
        let region_size =
            rng.gen_range(SAND_CARVE_REGION_SIZE_MIN..=SAND_CARVE_REGION_SIZE_MAX) as usize;
        let limit = region_size.min(max_carve_cells - total_carved);
        total_carved += flood_fill_carve_region(candidate, &mut carve, origin, limit);
    }
    carve
}
```

### 5.4 private helper: `flood_fill_carve_region()`

```rust
/// candidate 内を 4 近傍 BFS で最大 `limit` セル carve する。
/// 戻り値: 実際に carve したセル数。
fn flood_fill_carve_region(
    candidate: &BitGrid,
    carve: &mut BitGrid,
    origin: GridPos,
    limit: usize,
) -> usize {
    if !candidate.get(origin) || carve.get(origin) {
        return 0;
    }
    let mut queue: VecDeque<GridPos> = VecDeque::new();
    queue.push_back(origin);
    carve.set(origin, true);
    let mut count = 1;

    while let Some(pos) = queue.pop_front() {
        if count >= limit {
            break;
        }
        for (dx, dy) in CARDINAL_DIRS_4 {
            if count >= limit {
                break;
            }
            let nx = pos.0 + dx;
            let ny = pos.1 + dy;
            let p = (nx, ny);
            if candidate.get(p) && !carve.get(p) {
                carve.set(p, true);
                count += 1;
                queue.push_back(p);
            }
        }
    }
    count
}
```

初版の仕様: **高々 `limit` セル**をこの region で carve する目標とする。BFS の打ち切りにより **ちょうど `limit` に満たない**場合がある（許容）。厳密に `limit` 枚だけに揃める必要はない。

---

## 6. WFC との統合

### 6.1 `run_wfc()` の責務

`run_wfc()` の基本方針は維持する。

- WFC 本体では River の固定・マスク外 River 禁止のみ直接適用
- `Sand` の fixed / forbid は `ForbidPattern` に入れない
- `post_process_tiles()` が最終地形へ `final_sand_mask` を適用する

### 6.2 `mapgen.rs` の変更

`fill_river_from_seed` の直後に 1 行追加し、fallback でも同じ `masks` を使い回す前提を明記する。

```rust
// mapgen.rs の generate_world_layout() 内
let mut masks = WorldMasks::from_anchor(&anchors);
masks.fill_river_from_seed(master_seed);
masks.fill_sand_from_river_seed(master_seed); // ← 追加（2d）
```

これにより `run_wfc()` 成功時だけでなく、`fallback_terrain(&masks)` を使う経路でも `final_sand_mask` を参照できる。

#### `mapgen.rs` テストの変更

`test_sand_is_only_river_adjacent_in_ms2b` は **2b の仕様**（Sand が必ず cardinal 隣接）を前提とし、2d では diagonal Sand が許容されるため **削除**する。代わりに §10 の 2d 向けテストを追加する。

### 6.3 `post_process_tiles()` の全面置き換え（`wfc_adapter.rs`）

現行実装（river 非隣接 Sand を Grass/Dirt に落とす）を `final_sand_mask` 主導へ置き換える。

```rust
/// WFC 後のポスト処理（MS-WFC-2d 版）。
///
/// 処理順:
/// 1. river_mask セルは常に River のまま（skip）
/// 2. final_sand_mask セルは強制 Sand（WFC 結果に関わらず上書き）
/// 3. final_sand_mask 外で terrain == Sand の stray Sand を Grass/Dirt に置換
fn post_process_tiles(tiles: &mut [TerrainType], masks: &WorldMasks, rng: &mut StdRng) {
    let total = WEIGHT_GRASS + WEIGHT_DIRT;
    for y in 0..MAP_HEIGHT {
        for x in 0..MAP_WIDTH {
            let idx = (y * MAP_WIDTH + x) as usize;
            if masks.river_mask.get((x, y)) {
                // River は WFC で固定済み。変更しない。
                continue;
            }
            if masks.final_sand_mask.get((x, y)) {
                // マスク上のセルは必ず Sand に揃える
                tiles[idx] = TerrainType::Sand;
            } else if tiles[idx] == TerrainType::Sand {
                // マスク外の stray Sand を Grass/Dirt に落とす
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

これにより:
- `final_sand_mask` 上は必ず `Sand`
- `final_sand_mask` 外の stray `Sand` は残らない
- WFC の weighted `Sand` 出力は「Grass/Dirt と同等に落とせる候補」に戻る

### 6.4 `fallback_terrain()` の変更

2d では fallback でも `final_sand_mask` と最終地形を一致させる。現行の「River 以外はすべて Grass」方針のままだと、
debug validator と `test_sand_matches_final_sand_mask()` が fallback 時に必ず不一致になるため、
fallback 側でも `final_sand_mask` を適用する。

```rust
/// WFC が全試行で収束しなかった場合の安全マップ（MS-WFC-2d 版）。
/// hard constraint（River マスク・anchor 禁止）は維持しつつ、
/// final_sand_mask 上は Sand、残りは Grass で埋める。
pub(crate) fn fallback_terrain(masks: &WorldMasks) -> Vec<TerrainType> {
    let mut tiles = vec![TerrainType::Grass; (MAP_WIDTH * MAP_HEIGHT) as usize];
    for y in 0..MAP_HEIGHT {
        for x in 0..MAP_WIDTH {
            let idx = (y * MAP_WIDTH + x) as usize;
            if masks.river_mask.get((x, y)) {
                tiles[idx] = TerrainType::River;
            } else if masks.final_sand_mask.get((x, y)) {
                tiles[idx] = TerrainType::Sand;
            }
        }
    }
    tiles
}
```

これにより、fallback は引き続き deterministic でありつつ、2d の `Sand = final_sand_mask` 契約を壊さない。

### 6.5 `build_pattern_table()` の重み（WEIGHT_SAND）

Sand を WFC の主要地形から外す方向に寄せるため、初版では **現状維持**（`WEIGHT_SAND = SAND_ADJACENT_TO_RIVER_WEIGHT = 10`）でよい。2d の主目的は shape control であり、WFC の地形味付け最適化ではない。

`build_pattern_table()` の隣接ルール（`Sand ↔ Grass`、`Sand ↔ River` など）は変更しない。`post_process_tiles` がマスクで上書きするため WFC のパターン互換性は最終地形に直接影響しない。

### 6.6 `WorldConstraints` のコメント更新

`wfc_adapter.rs` の `WorldConstraints` コメントに「Sand 制約は 2d 以降 `final_sand_mask` / `post_process_tiles` で対応」を明記する。ロジック変更は不要。

---

## 7. validator の見直し（`validate.rs`）

`MS-WFC-2c` の debug validator は、2d 採用後に以下を見直す。

### 7.1 廃止するチェック

以下の 2 関数を `debug_validate()` の呼び出しリストから**削除**し、関数定義ごと除去する。

| 関数 | 理由 |
| --- | --- |
| `check_sand_river_adjacency_ratio` | diagonal Sand は 2d では仕様であり「4 近傍比率」は無意味 |
| `check_sand_diagonal_only_contacts` | diagonal Sand を警告するのは 2d の仕様と矛盾する |

`ValidationWarningKind` から対応 variant も削除する:
- `SandRiverAdjacencyLow` → 削除
- `SandDiagonalOnlyContact` → 削除

### 7.2 新規追加チェック

`ValidationWarningKind` に以下を追加する:

```rust
SandMaskMismatch,  // final_sand_mask と terrain_tiles の Sand が一致しない
```

`debug_validate()` の呼び出しリストを更新する:

```rust
#[cfg(any(test, debug_assertions))]
pub fn debug_validate(layout: &GeneratedWorldLayout) -> Vec<ValidationWarning> {
    let mut warnings = Vec::new();
    check_protection_band_clean(layout, &mut warnings);
    // check_sand_river_adjacency_ratio は削除
    // check_sand_diagonal_only_contacts は削除
    check_river_tile_count(layout, &mut warnings);
    check_no_fallback_reached(layout, &mut warnings);
    check_forbidden_diagonal_patterns(layout, &mut warnings);
    // 2d 新規チェック
    check_final_sand_mask_applied(layout, &mut warnings);
    check_no_stray_sand_outside_mask(layout, &mut warnings);
    check_sand_mask_not_in_anchor_or_band(layout, &mut warnings);
    warnings
}
```

#### `check_final_sand_mask_applied`

`final_sand_mask == true` のセルがすべて `TerrainType::Sand` になっているか確認する。

```rust
#[cfg(any(test, debug_assertions))]
fn check_final_sand_mask_applied(
    layout: &GeneratedWorldLayout,
    warnings: &mut Vec<ValidationWarning>,
) {
    for y in 0..MAP_HEIGHT {
        for x in 0..MAP_WIDTH {
            if layout.masks.final_sand_mask.get((x, y)) {
                let idx = (y * MAP_WIDTH + x) as usize;
                if layout.terrain_tiles[idx] != TerrainType::Sand {
                    warnings.push(ValidationWarning {
                        kind: ValidationWarningKind::SandMaskMismatch,
                        message: format!(
                            "final_sand_mask=true but terrain != Sand at ({x},{y}): {:?}",
                            layout.terrain_tiles[idx]
                        ),
                    });
                }
            }
        }
    }
}
```

#### `check_no_stray_sand_outside_mask`

`final_sand_mask == false` のセルに `TerrainType::Sand` が残っていないか確認する。

```rust
#[cfg(any(test, debug_assertions))]
fn check_no_stray_sand_outside_mask(
    layout: &GeneratedWorldLayout,
    warnings: &mut Vec<ValidationWarning>,
) {
    for y in 0..MAP_HEIGHT {
        for x in 0..MAP_WIDTH {
            if !layout.masks.final_sand_mask.get((x, y)) {
                let idx = (y * MAP_WIDTH + x) as usize;
                if layout.terrain_tiles[idx] == TerrainType::Sand {
                    warnings.push(ValidationWarning {
                        kind: ValidationWarningKind::SandMaskMismatch,
                        message: format!("Stray Sand outside final_sand_mask at ({x},{y})"),
                    });
                }
            }
        }
    }
}
```

#### `check_sand_mask_not_in_anchor_or_band`

`final_sand_mask` が `anchor_mask` / `river_protection_band` と交差しないか確認する。

```rust
#[cfg(any(test, debug_assertions))]
fn check_sand_mask_not_in_anchor_or_band(
    layout: &GeneratedWorldLayout,
    warnings: &mut Vec<ValidationWarning>,
) {
    for y in 0..MAP_HEIGHT {
        for x in 0..MAP_WIDTH {
            if !layout.masks.final_sand_mask.get((x, y)) {
                continue;
            }
            if layout.masks.anchor_mask.get((x, y)) {
                warnings.push(ValidationWarning {
                    kind: ValidationWarningKind::SandMaskMismatch,
                    message: format!("final_sand_mask overlaps anchor_mask at ({x},{y})"),
                });
            }
            if layout.masks.river_protection_band.get((x, y)) {
                warnings.push(ValidationWarning {
                    kind: ValidationWarningKind::SandMaskMismatch,
                    message: format!(
                        "final_sand_mask overlaps river_protection_band at ({x},{y})"
                    ),
                });
            }
        }
    }
}
```

### 7.3 `lightweight_validate` への影響なし

`collect_required_resource_candidates()` は `terrain_tiles` の `Sand` セルを走査するため、2d 後も **ロジックの変更は不要**。`post_process_tiles` / `fallback_terrain` が `final_sand_mask` と terrain を一致させた後に `lightweight_validate` が呼ばれるため、通常は整合する。

**例外**: §5.1 のとおり **`final_sand_mask` が空**（shoreline 候補ゼロ）のレイアウトでは、砂源が無く `RequiredResourceNotReachable` になり得る。回帰テスト用 seed は非空を保証すること。

---

## 8. 変更ファイル

| ファイル | 変更内容 |
| --- | --- |
| `crates/hw_world/src/world_masks.rs` | `sand_candidate_mask` / `sand_carve_mask` / `final_sand_mask` を struct に追加。`from_anchor` で空初期化。`fill_sand_from_river_seed()` を公開 API として追加（§4） |
| `crates/hw_world/src/river.rs` | `VecDeque` import 追加。`EIGHT_DIRS` / `CARDINAL_DIRS_4` / `SAND_CARVE_*` / `SAND_SEED_SALT` 定数追加。`generate_sand_masks()` / `build_sand_candidate_mask()` / `build_sand_carve_mask()` / `flood_fill_carve_region()` 追加（§5） |
| `crates/hw_world/src/mapgen.rs` | `fill_river_from_seed` の直後に `masks.fill_sand_from_river_seed(master_seed);` を追加（§6.2）。`test_sand_is_only_river_adjacent_in_ms2b` テストを削除し、2d 向けテストに置き換える（§10） |
| `crates/hw_world/src/mapgen/wfc_adapter.rs` | `post_process_tiles()` を `final_sand_mask` 主導へ全面置き換え（§6.3）。`fallback_terrain()` も `final_sand_mask` を反映する形へ更新（§6.4）。`WorldConstraints` コメントに 2d 方針を追記（§6.6） |
| `crates/hw_world/src/mapgen/validate.rs` | `check_sand_river_adjacency_ratio` / `check_sand_diagonal_only_contacts` を削除。`SandRiverAdjacencyLow` / `SandDiagonalOnlyContact` variant を削除。`SandMaskMismatch` variant 追加。3 つの新チェック関数追加。`debug_validate()` 呼び出しリスト更新（§7） |
| `docs/plans/3d-rtt/archived/wfc-terrain-generation-plan-2026-04-01.md` | MS-WFC-2d の行をサブ計画表に追加、実装状況を更新 |

**削除した enum variant の波及確認**: 実装後、`SandRiverAdjacencyLow` / `SandDiagonalOnlyContact` を workspace 全体で grep し、`match` や表示分岐の取りこぼしがないか確認する。

---

## 9. 完了条件

- [ ] `WorldMasks` に `sand_candidate_mask` / `sand_carve_mask` / `final_sand_mask` が追加されている
- [ ] 同一 seed で `final_sand_mask` が deterministic
- [ ] `final_sand_mask` は `river_mask` / `anchor_mask` / `river_protection_band` と交差しない
- [ ] `post_process_tiles()` が `final_sand_mask` 上を強制的に `Sand` にしている
- [ ] `fallback_terrain()` でも `final_sand_mask` 上が `Sand` になる
- [ ] `final_sand_mask` 外に stray `Sand` が残らない
- [ ] debug validator が diagonal-only sand を warning しない
- [ ] `cargo test -p hw_world` / `cargo check --workspace` / `cargo clippy --workspace` が通る

---

## 10. テスト

### 10.1 `river.rs` に追加するテスト

`WorldMasks` と一体の検証のため `river.rs` に置く想定。モジュール分割の都合で **`mapgen.rs` の `#[cfg(test)]` に移す**選択も可（同じ assertion でよい）。

```rust
#[test]
fn sand_mask_is_deterministic_for_same_seed() {
    let anchor = AnchorLayout::fixed();
    let mut masks_a = WorldMasks::from_anchor(&anchor);
    masks_a.fill_river_from_seed(42);
    masks_a.fill_sand_from_river_seed(42);

    let mut masks_b = WorldMasks::from_anchor(&anchor);
    masks_b.fill_river_from_seed(42);
    masks_b.fill_sand_from_river_seed(42);

    for y in 0..MAP_HEIGHT {
        for x in 0..MAP_WIDTH {
            let pos = (x, y);
            assert_eq!(
                masks_a.final_sand_mask.get(pos),
                masks_b.final_sand_mask.get(pos),
                "final_sand_mask differs at {pos:?}"
            );
        }
    }
}

#[test]
fn sand_mask_does_not_overlap_anchor_or_protection_band() {
    let anchor = AnchorLayout::fixed();
    let mut masks = WorldMasks::from_anchor(&anchor);
    masks.fill_river_from_seed(42);
    masks.fill_sand_from_river_seed(42);

    for y in 0..MAP_HEIGHT {
        for x in 0..MAP_WIDTH {
            let pos = (x, y);
            if masks.final_sand_mask.get(pos) {
                assert!(
                    !masks.anchor_mask.get(pos),
                    "final_sand_mask overlaps anchor_mask at {pos:?}"
                );
                assert!(
                    !masks.river_protection_band.get(pos),
                    "final_sand_mask overlaps river_protection_band at {pos:?}"
                );
                assert!(
                    !masks.river_mask.get(pos),
                    "final_sand_mask overlaps river_mask at {pos:?}"
                );
            }
        }
    }
}

#[test]
fn final_sand_mask_is_non_empty() {
    let anchor = AnchorLayout::fixed();
    let mut masks = WorldMasks::from_anchor(&anchor);
    masks.fill_river_from_seed(42);
    masks.fill_sand_from_river_seed(42);
    assert!(
        masks.final_sand_mask.count_set() > 0,
        "final_sand_mask が空（seed=42）— shoreline 候補が無い seed の可能性。§5.1 参照"
    );
}
```

### 10.2 `mapgen.rs` に追加するテスト（2d 向け置き換え）

`test_sand_is_only_river_adjacent_in_ms2b` を削除し、以下の 2d 版テストを追加する。

```rust
#[test]
fn test_sand_matches_final_sand_mask() {
    let layout = generate_world_layout(TEST_SEED_A);
    for y in 0..MAP_HEIGHT {
        for x in 0..MAP_WIDTH {
            let idx = (y * MAP_WIDTH + x) as usize;
            let is_sand = layout.terrain_tiles[idx] == TerrainType::Sand;
            let in_mask = layout.masks.final_sand_mask.get((x, y));
            assert_eq!(
                is_sand, in_mask,
                "Sand/mask mismatch at ({x},{y}): terrain={is_sand}, mask={in_mask}"
            );
        }
    }
}
```

### 10.3 golden seed カバレッジ

3 系統すべてのシードで全テストが通ることを確認する。

| seed 名 | 想定シーン |
| --- | --- |
| `TEST_SEED_A = 10_182_272_928_891_625_829` | 基準（直線に近い川） |
| `TEST_SEED_B = 12_345_678` | 既存 determinism テストで使用 |
| 追加予定 | winding river / tight band（seed 値は調整中） |

---

## 11. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| `2026-04-04` | `Codex` | 初版作成。River 派生の 8 近傍 sand candidate mask、連続 non-sand carve、`post_process_tiles()` による最終反映方針を定義。 |
| `2026-04-04` | — | レビュー反映: `WorldMasks::fill_sand_from_river_seed` を公開 API と明示（`river.rs` は helper）。`BitGrid` 差し引きを実装手順付きに修正。carve 後空マスクのフォールバック、`post_process` の river 優先・処理順、§6.3 `WorldConstraints` 見直し、§8 に `mapgen.rs`・親計画表の追記。メタ最終更新日を整合。 |
| `2026-04-04` | `Copilot` | ブラッシュアップ: 全関数の完全なシグネチャ・実装コード・import 一覧・定数定義を追記。`post_process_tiles()` / `debug_validate()` / 3 新チェック関数の完全 Rust コードを掲載。テストボディを具体的な assertion 付きに更新。`test_sand_is_only_river_adjacent_in_ms2b` の削除を明示。ステータスを `Ready` へ昇格。 |
| `2026-04-04` | `Codex` | fallback 時も `final_sand_mask` を最終地形へ適用する方針を追加し、2d の sand-mask 整合テストと validator 契約が fallback 経路でも成立するよう明記。 |
| `2026-04-04` | `Codex` | 実装完了に合わせてステータスを `完了` に更新し、`post_process_tiles()` / `fallback_terrain()` / sand-mask validator 差し替えが実装済みであることを反映。 |
| `2026-04-04` | — | レビュー反映: `choose_multiple` の `k` を candidate 長で cap。§5.1 付記で candidate 空・`lightweight_validate` 失敗の関係。`flood_fill` を「高々 limit」として明記。§7.3 に空マスク例外。§8 に enum 削除の grep 手順。§10.1 にテスト配置の注記と assertion メッセージ補足。 |
