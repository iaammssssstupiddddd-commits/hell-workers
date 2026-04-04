# MS-WFC-2c: 生成後バリデータ（lightweight + debug）

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `wfc-ms2c-validator` |
| ステータス | `完了`（`crates/hw_world/src/mapgen/validate.rs`） |
| 作成日 | `2026-04-01` |
| 最終更新日 | `2026-04-05` |
| 親計画 | [`wfc-terrain-generation-plan-2026-04-01.md`](wfc-terrain-generation-plan-2026-04-01.md) |
| 前MS | [`wfc-ms2b-wfc-solver-constraints.md`](wfc-ms2b-wfc-solver-constraints.md) |
| 次MS | [`wfc-ms3-procedural-resources.md`](wfc-ms3-procedural-resources.md) |
| 前提 | `generate_world_layout()` が地形グリッドを返せる（MS-WFC-2b 完了） |

### サマリ

| 項目 | 内容 |
| --- | --- |
| 実装内容 | **完了**: `mapgen/validate.rs` に **lightweight**（起動必須・`Result`）と **debug**（`#[cfg(any(test, debug_assertions))]`・`Vec<ValidationWarning>`）を実装済み。`generate_world_layout` の retry ループで lightweight 通過レイアウトのみ採用 |
| やらないこと | 地形の局所修正・WFC 本体の変更・`log` crate 導入（診断は `eprintln!`） |
| 主な invariant | Site/Yard に River/Sand なし、Site↔Yard 連結、Yard から必須資源（水・砂、岩は候補があれば）到達、木・一輪車アンカーが Yard 内 |
| 出力 | 成功時 `ResourceSpawnCandidates`（到達確認済み）を `GeneratedWorldLayout` に格納。debug は比率・件数・fallback 等の警告のみ |
| 検証 | `cargo test -p hw_world`（golden seed）、`cargo check` / `cargo clippy --workspace` |

### 実装前確認事項（MS-WFC-2b 完了済み）

- `crates/hw_world/src/mapgen.rs` に `generate_world_layout(seed: u64) -> GeneratedWorldLayout` が実装済み
- `crates/hw_world/src/mapgen/wfc_adapter.rs` に `run_wfc()`, `post_process_tiles()`, `MAX_WFC_RETRIES` が実装済み
- `crates/hw_world/src/mapgen/types.rs` に `GeneratedWorldLayout`, `ResourceSpawnCandidates` が定義済み
- 全 25 テストが通過、`cargo clippy --workspace` 0 warnings

---

## 1. 目的

WFC で生成された地形が **ゲーム上の invariant** を満たしていることを、**コードで検証する**。

- MS-WFC-0 で設計した 2 段 validator（lightweight / debug）を実装する
- `hw_world::pathfinding` の `PathWorld` トレイトを実装したアダプタを経由して `can_reach_target` を使い、到達可能資源セルを抽出する
- validator は `GeneratedWorldLayout` を受け取る **pure 関数**として実装し、startup との結合を最小にする
- この MS で **生成後局所修正** は行わない（validator は **Err / warning と pure な派生データ**のみを返し、呼び出し側が retry / fallback / panic を判断する）
- `Sand` の斜め-only River 接触は、WFC 本体ではなく debug validator で診断する

---

## 2. モジュール構成

本リポジトリでは `mapgen` は **`src/mapgen.rs`** が親モジュールで、サブモジュールは **`src/mapgen/*.rs`** に置く（`src/mapgen/mod.rs` は使わない）。

```
crates/hw_world/src/
├── mapgen.rs                 ← `pub mod validate;` を追加
└── mapgen/
    ├── types.rs
    ├── wfc_adapter.rs
    └── validate.rs           ← 本 MS で新規作成
        ├── lightweight_validate()
        └── debug_validate()   (#[cfg(debug_assertions)])
```

---

## 3. lightweight_validate()

```rust
/// 起動時必須チェック。失敗時は Err を返す。
/// 成功時は validator が確認した到達可能資源候補を返す。
/// retry / fallback / panic の判断は validator の外側で行う。
pub fn lightweight_validate(
    layout: &GeneratedWorldLayout,
) -> Result<ResourceSpawnCandidates, ValidationError> {
    check_site_yard_no_river_sand(layout)?;
    check_site_yard_reachable(layout)?;
    let resource_spawn_candidates = collect_required_resource_candidates(layout)?;
    check_yard_anchors_present(layout)?;
    Ok(resource_spawn_candidates)
}
```

### 3.0 ValidatorPathWorld（内部ヘルパー）

`check_site_yard_reachable` / `collect_required_resource_candidates` で使う `PathWorld` 実装。
`PathWorld` は `pos_to_idx`, `idx_to_pos`, `is_walkable`, `get_door_cost` の 4 メソッドを要求する。

```rust
use crate::pathfinding::PathWorld;
use hw_core::constants::{MAP_HEIGHT, MAP_WIDTH};
use hw_core::world::GridPos;

/// validate.rs 内部専用。`terrain_tiles` スライスのみで PathWorld を実現する。
/// 扉コストは常に 0（マップ生成段階では扉エンティティが存在しない）。
struct ValidatorPathWorld<'a> {
    tiles: &'a [TerrainType],
}

impl PathWorld for ValidatorPathWorld<'_> {
    fn pos_to_idx(&self, x: i32, y: i32) -> Option<usize> {
        if x < 0 || x >= MAP_WIDTH || y < 0 || y >= MAP_HEIGHT {
            return None;
        }
        Some((y * MAP_WIDTH + x) as usize)
    }

    fn idx_to_pos(&self, idx: usize) -> GridPos {
        (idx as i32 % MAP_WIDTH, idx as i32 / MAP_WIDTH)
    }

    fn is_walkable(&self, x: i32, y: i32) -> bool {
        self.pos_to_idx(x, y)
            .map(|i| self.tiles[i].is_walkable())
            .unwrap_or(false)
    }

    fn get_door_cost(&self, _x: i32, _y: i32) -> i32 {
        0
    }
}
```

### チェック関数の責務

#### `check_site_yard_no_river_sand`

```
- layout.anchors.site（GridRect）の全セルを GridRect::iter_cells() でイテレート
- layout.anchors.yard（GridRect）の全セルも同様にイテレート
- terrain_tiles[(y * MAP_WIDTH + x) as usize] が River / Sand のセルを発見したら
  Err(ValidationError::ForbiddenTileInAnchorZone(pos)) を返す
- MAP_WIDTH は hw_core::constants::MAP_WIDTH を使用
```

```rust
fn check_site_yard_no_river_sand(layout: &GeneratedWorldLayout) -> Result<(), ValidationError> {
    for pos in layout.anchors.site.iter_cells().chain(layout.anchors.yard.iter_cells()) {
        let idx = (pos.1 * MAP_WIDTH + pos.0) as usize;
        let tile = layout.terrain_tiles[idx];
        if matches!(tile, TerrainType::River | TerrainType::Sand) {
            return Err(ValidationError::ForbiddenTileInAnchorZone(pos));
        }
    }
    Ok(())
}
```

#### `check_site_yard_reachable`

```
- Site の代表点(site.min_x, site.min_y) から Yard の代表点(yard.min_x, yard.min_y) への
  walkable 経路が存在するか（連結性チェック）
- check_site_yard_no_river_sand が通過済みなら両代表点は Grass/Dirt（walkable）が保証される
- can_reach_target(&world, &mut ctx, site_rep, yard_rep, true) で確認
  （target_walkable=true: yard_rep は walkable セル）
- PathfindingContext はこの関数内で生成・破棄（起動時の一回限り）
```

```rust
fn check_site_yard_reachable(layout: &GeneratedWorldLayout) -> Result<(), ValidationError> {
    let world = ValidatorPathWorld { tiles: &layout.terrain_tiles };
    let mut ctx = PathfindingContext::default();
    let site_rep = (layout.anchors.site.min_x, layout.anchors.site.min_y);
    let yard_rep = (layout.anchors.yard.min_x, layout.anchors.yard.min_y);
    if !can_reach_target(&world, &mut ctx, site_rep, yard_rep, true) {
        return Err(ValidationError::SiteYardNotReachable);
    }
    Ok(())
}
```

#### `collect_required_resource_candidates`

Yard 代表点 `(yard.min_x, yard.min_y)` から各資源への到達可能性を確認する。
**River タイルは walkable=false** であることに注意。`can_reach_target(..., pos, false)` を使うと、
non-walkable な River タイルについても「隣接到達できるか」を判定できる。

この関数は単に `Ok(())` を返すのではなく、**各セルを個別に到達確認した候補集合そのもの**を
`ResourceSpawnCandidates` として返す。`generate_world_layout()` はその返り値を
`GeneratedWorldLayout.resource_spawn_candidates` に格納する。

| 資源 | 対象 | 判定方法 |
|------|------|----------|
| 水源 | `river_mask` が true **かつ** `terrain_tiles == TerrainType::River` のセル（両者の交差） | 各セルに対して `can_reach_target(..., pos, false)` でフィルタ。mask と terrain がずれている場合はバグのため交差で列挙し、不整合だけを拾う別チェックは本 MS では必須としない |
| 砂源 | `terrain_tiles` が Sand の全セル | 各 Sand タイルに対して `can_reach_target(..., pos, true)` でフィルタ |
| 岩源 | 呼び出し側が事前に詰めた `resource_spawn_candidates.rock_candidates` | `can_reach_target(..., pos, true)` — rock_candidates が空なら SKIP |

```rust
fn collect_required_resource_candidates(
    layout: &GeneratedWorldLayout,
) -> Result<ResourceSpawnCandidates, ValidationError> {
    use hw_core::constants::{MAP_HEIGHT, MAP_WIDTH};
    use crate::pathfinding::{can_reach_target, PathfindingContext};
    use crate::terrain::TerrainType;

    let world = ValidatorPathWorld { tiles: &layout.terrain_tiles };
    let mut ctx = PathfindingContext::default();
    let yard_rep = (layout.anchors.yard.min_x, layout.anchors.yard.min_y);
    let mut validated = ResourceSpawnCandidates {
        water_tiles: Vec::new(),
        sand_tiles: Vec::new(),
        rock_candidates: Vec::new(),
    };

    // 水源: mask と terrain の両方が River のセルのみ列挙（不整合セルは候補に含めない）
    let river_tiles: Vec<GridPos> = (0..MAP_HEIGHT)
        .flat_map(|y| {
            (0..MAP_WIDTH).filter_map(move |x| {
                let idx = (y * MAP_WIDTH + x) as usize;
                let is_river_terrain = layout.terrain_tiles[idx] == TerrainType::River;
                (layout.masks.river_mask.get((x, y)) && is_river_terrain).then_some((x, y))
            })
        })
        .collect();
    validated.water_tiles = river_tiles
        .into_iter()
        .filter(|&pos| can_reach_target(&world, &mut ctx, yard_rep, pos, false))
        .collect();
    if validated.water_tiles.is_empty() {
        return Err(ValidationError::RequiredResourceNotReachable);
    }

    // 砂源: 各 Sand タイルを個別に到達確認し、到達可能なものだけ保持する
    let sand_tiles: Vec<GridPos> = (0..MAP_HEIGHT)
        .flat_map(|y| {
            (0..MAP_WIDTH).filter_map(move |x| {
                let idx = (y * MAP_WIDTH + x) as usize;
                (layout.terrain_tiles[idx] == TerrainType::Sand).then_some((x, y))
            })
        })
        .collect();
    validated.sand_tiles = sand_tiles
        .into_iter()
        .filter(|&pos| can_reach_target(&world, &mut ctx, yard_rep, pos, true))
        .collect();
    if validated.sand_tiles.is_empty() {
        return Err(ValidationError::RequiredResourceNotReachable);
    }

    // 岩源: 入力候補がある場合だけ到達可能なものを残す。1 件も残らなければ Err。
    if !layout.resource_spawn_candidates.rock_candidates.is_empty() {
        validated.rock_candidates = layout
            .resource_spawn_candidates
            .rock_candidates
            .iter()
            .copied()
            .filter(|&pos| can_reach_target(&world, &mut ctx, yard_rep, pos, true))
            .collect();
        if validated.rock_candidates.is_empty() {
            return Err(ValidationError::RequiredResourceNotReachable);
        }
    }

    Ok(validated)
}
```

`check_required_resources_reachable()` という `Result<(), _>` ラッパーを別途置く場合は、
`collect_required_resource_candidates(layout).map(|_| ())` に留め、ロジックを二重化しない。

#### `check_yard_anchors_present`

```
- layout.anchors.initial_wood_positions の各座標が layout.anchors.yard に含まれるか
  → GridRect::contains(pos) を使用
  → 不正なら Err(ValidationError::YardAnchorOutOfBounds(pos))
- layout.anchors.wheelbarrow_parking（GridRect, 2×2）の全セルが layout.anchors.yard に含まれるか
  → wheelbarrow_parking.iter_cells() でイテレート
```

```rust
fn check_yard_anchors_present(layout: &GeneratedWorldLayout) -> Result<(), ValidationError> {
    for &pos in &layout.anchors.initial_wood_positions {
        if !layout.anchors.yard.contains(pos) {
            return Err(ValidationError::YardAnchorOutOfBounds(pos));
        }
    }
    for pos in layout.anchors.wheelbarrow_parking.iter_cells() {
        if !layout.anchors.yard.contains(pos) {
            return Err(ValidationError::YardAnchorOutOfBounds(pos));
        }
    }
    Ok(())
}
```

---

## 4. debug_validate()

```rust
/// 開発時のみ有効な追加診断。
/// `#[cfg(any(test, debug_assertions))]` で有効化する。
#[cfg(any(test, debug_assertions))]
pub fn debug_validate(layout: &GeneratedWorldLayout) -> Vec<ValidationWarning> {
    let mut warnings = Vec::new();
    check_protection_band_clean(layout, &mut warnings);
    check_sand_river_adjacency_ratio(layout, &mut warnings);
    check_sand_diagonal_only_contacts(layout, &mut warnings);
    check_river_tile_count(layout, &mut warnings);
    check_no_fallback_reached(layout, &mut warnings);
    check_forbidden_diagonal_patterns(layout, &mut warnings);
    warnings
}
```

### debug チェックの内容

#### `check_protection_band_clean`

River タイルが `river_protection_band`（アンカー外周 3 タイル帯）に侵入していないことを確認する。

```rust
fn check_protection_band_clean(layout: &GeneratedWorldLayout, warnings: &mut Vec<ValidationWarning>) {
    for y in 0..MAP_HEIGHT {
        for x in 0..MAP_WIDTH {
            if layout.masks.river_mask.get((x, y))
                && layout.masks.river_protection_band.get((x, y))
            {
                warnings.push(ValidationWarning {
                    kind: ValidationWarningKind::ProtectionBandViolation,
                    message: format!("River at ({x},{y}) is inside river_protection_band"),
                });
            }
        }
    }
}
```

- `WorldMasks::combined_protection_band()` で全保護帯の合成マスクも取得可能だが、
  この MS 時点では River 保護帯のみ検査する（岩・木は MS-WFC-3 で配置されるため）

#### `check_sand_river_adjacency_ratio`

```
- Sand タイル総数のうち、河川タイルに辺接するものの割合を計算
- 80% を下回ったら ValidationWarning を追加
```

```rust
fn check_sand_river_adjacency_ratio(layout: &GeneratedWorldLayout, warnings: &mut Vec<ValidationWarning>) {
    use crate::mapgen::wfc_adapter::CARDINAL_DIRS;
    let mut total_sand = 0usize;
    let mut adjacent_to_river = 0usize;
    for y in 0..MAP_HEIGHT {
        for x in 0..MAP_WIDTH {
            if layout.terrain_tiles[(y * MAP_WIDTH + x) as usize] != TerrainType::Sand {
                continue;
            }
            total_sand += 1;
            if CARDINAL_DIRS.iter().any(|(dx, dy)| layout.masks.river_mask.get((x + dx, y + dy))) {
                adjacent_to_river += 1;
            }
        }
    }
    if total_sand > 0 && adjacent_to_river * 100 / total_sand < 80 {
        warnings.push(ValidationWarning {
            kind: ValidationWarningKind::SandRiverAdjacencyLow,
            message: format!(
                "Sand-river adjacency ratio {}/{} ({:.0}%) < 80%",
                adjacent_to_river, total_sand,
                adjacent_to_river as f32 / total_sand as f32 * 100.0
            ),
        });
    }
}
```

#### `check_sand_diagonal_only_contacts`

```
- Sand セルごとに、River との 4 近傍接触と斜め 4 マス接触を別々に数える
- 「River に斜めでは接しているが、4 近傍では接していない」Sand を
  diagonal-only 接触として扱う
- 初期閾値: 総 Sand 数に対して 10% 超なら warning を追加
- diagonal-only 接触は初版では warning 扱い（error にしない、retry 条件にも含めない）

```rust
fn check_sand_diagonal_only_contacts(layout: &GeneratedWorldLayout, warnings: &mut Vec<ValidationWarning>) {
    use crate::mapgen::wfc_adapter::CARDINAL_DIRS;
    let mut total_sand = 0usize;
    let mut diagonal_only = 0usize;
    for y in 0..MAP_HEIGHT {
        for x in 0..MAP_WIDTH {
            let idx = (y * MAP_WIDTH + x) as usize;
            if layout.terrain_tiles[idx] != TerrainType::Sand {
                continue;
            }
            total_sand += 1;
            let cardinal_river = CARDINAL_DIRS
                .iter()
                .any(|(dx, dy)| layout.masks.river_mask.get((x + dx, y + dy)));
            let diagonal_dirs = [(1, 1), (1, -1), (-1, 1), (-1, -1)];
            let diagonal_river = diagonal_dirs
                .iter()
                .any(|(dx, dy)| layout.masks.river_mask.get((x + dx, y + dy)));
            if diagonal_river && !cardinal_river {
                diagonal_only += 1;
            }
        }
    }
    if total_sand > 0 && diagonal_only * 100 / total_sand > 10 {
        warnings.push(ValidationWarning {
            kind: ValidationWarningKind::SandDiagonalOnlyContact,
            message: format!(
                "Sand cells with diagonal-only river contact: {diagonal_only}/{total_sand} (>10%)"
            ),
        });
    }
}
```

閾値比較は実装時に整数除算の丸めと一致させる（上記は「10% 超」で warning とする一例）。

#### `check_river_tile_count`

```
- layout.masks.river_mask.count_set() で river タイル数を取得
- RIVER_TOTAL_TILES_TARGET_MIN / RIVER_TOTAL_TILES_TARGET_MAX（river.rs の定数）の範囲内か確認
  → 現在の値: MIN=200, MAX=500
- 範囲外なら ValidationWarning を追加
```

```rust
fn check_river_tile_count(layout: &GeneratedWorldLayout, warnings: &mut Vec<ValidationWarning>) {
    use crate::river::{RIVER_TOTAL_TILES_TARGET_MAX, RIVER_TOTAL_TILES_TARGET_MIN};
    let count = layout.masks.river_mask.count_set();
    if !(RIVER_TOTAL_TILES_TARGET_MIN..=RIVER_TOTAL_TILES_TARGET_MAX).contains(&count) {
        warnings.push(ValidationWarning {
            kind: ValidationWarningKind::RiverTileCountOutOfRange,
            message: format!(
                "River tile count {count} outside [{RIVER_TOTAL_TILES_TARGET_MIN}, {RIVER_TOTAL_TILES_TARGET_MAX}]"
            ),
        });
    }
}
```

#### `check_no_fallback_reached`

```
- layout.used_fallback == true なら「WFC fallback に到達した」として ValidationWarning を追加
- 現行実装ではフォールバック時も生成は継続し、debug/test でも panic しない
- 厳格検知は golden seed テストと debug warning ログで担保する
```

```rust
fn check_no_fallback_reached(layout: &GeneratedWorldLayout, warnings: &mut Vec<ValidationWarning>) {
    if layout.used_fallback {
        warnings.push(ValidationWarning {
            kind: ValidationWarningKind::FallbackReached,
            message: format!(
                "WFC fallback terrain used (master_seed={}, attempt={})",
                layout.master_seed, layout.generation_attempt
            ),
        });
    }
}
```

#### `check_forbidden_diagonal_patterns`

```
- 2×2 禁止パターン（例: River の孤立点、Dirt の孤立点）を検出
- F2: 斜め整合は WFC 後の validator で扱う方針
- Sand の diagonal-only 接触はここでまとめず、check_sand_diagonal_only_contacts に分離する
- 初版では River の孤立点（4 近傍に River がない River タイル）を検出する
```

---

## 5. ValidationError / ValidationWarning 型

座標は **`hw_core::world::GridPos`（`type GridPos = (i32, i32)`）** を使う。

`thiserror` は **workspace に未導入**。手動 `impl std::fmt::Display` + `impl std::error::Error` で実装する。

```rust
use hw_core::world::GridPos;

#[derive(Debug)]
pub enum ValidationError {
    ForbiddenTileInAnchorZone(GridPos),
    SiteYardNotReachable,
    RequiredResourceNotReachable,
    YardAnchorOutOfBounds(GridPos),
}

impl std::fmt::Display for ValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ForbiddenTileInAnchorZone(pos) => {
                write!(f, "Site/Yard contains River or Sand at {pos:?}")
            }
            Self::SiteYardNotReachable => write!(f, "Site to Yard is not reachable"),
            Self::RequiredResourceNotReachable => {
                write!(f, "No required resource reachable from Yard")
            }
            Self::YardAnchorOutOfBounds(pos) => {
                write!(f, "Yard anchor not in Yard bounds: {pos:?}")
            }
        }
    }
}

impl std::error::Error for ValidationError {}

#[derive(Debug)]
pub struct ValidationWarning {
    pub kind: ValidationWarningKind,
    pub message: String,
}

#[derive(Debug)]
pub enum ValidationWarningKind {
    ProtectionBandViolation,
    SandRiverAdjacencyLow,
    SandDiagonalOnlyContact,
    RiverTileCountOutOfRange,
    FallbackReached,
    ForbiddenPattern,
}
```

---

## 6. generate_world_layout への統合

`hw_world` に **`log` crate は現状ない**。debug 診断の出力は **`eprintln!`** でよい。

重要: validator は `generate_world_layout()` の **retry ループ内** で評価し、失敗した layout はその試行を破棄して次 attempt に進める。
`lightweight_validate()` 自体は pure に `Err` を返すだけで、validator 内で `panic!` はしない。
成功時は `ResourceSpawnCandidates` を返し、その値を最終 layout に載せる。

**現状の `generate_world_layout()`（MS-WFC-2b 時点）**:
```rust
// terrain_tiles だけを find_map の戻り値にしている
let (terrain_tiles, attempt, used_fallback) = (0..=MAX_WFC_RETRIES)
    .find_map(|attempt| {
        let sub_seed = derive_sub_seed(master_seed, attempt);
        run_wfc(&masks, sub_seed, attempt)
            .ok()
            .map(|tiles| (tiles, attempt, false))
    })
    ...;
```

**MS-WFC-2c で変更する形式**: `find_map` 内で一度「検証用 layout」を組み立て、
`lightweight_validate()` が返した `ResourceSpawnCandidates` を載せた最終 layout だけを返す。
`anchors.clone()` と `masks.clone()` が必要になる点に注意。

```rust
pub fn generate_world_layout(master_seed: u64) -> types::GeneratedWorldLayout {
    use crate::anchor::AnchorLayout;
    use crate::world_masks::WorldMasks;
    use types::ResourceSpawnCandidates;
    use wfc_adapter::{derive_sub_seed, fallback_terrain, run_wfc, MAX_WFC_RETRIES};

    let anchors = AnchorLayout::fixed();
    let mut masks = WorldMasks::from_anchor(&anchors);
    masks.fill_river_from_seed(master_seed);

    let layout = (0..=MAX_WFC_RETRIES)
        .find_map(|attempt| {
            let sub_seed = derive_sub_seed(master_seed, attempt);
            let terrain_tiles = run_wfc(&masks, sub_seed, attempt).ok()?;
            let candidate_layout = types::GeneratedWorldLayout {
                terrain_tiles,
                anchors: anchors.clone(),
                masks: masks.clone(),
                resource_spawn_candidates: ResourceSpawnCandidates::default(),
                initial_tree_positions: Vec::new(),
                forest_regrowth_zones: Vec::new(),
                initial_rock_positions: Vec::new(),
                master_seed,
                generation_attempt: attempt,
                used_fallback: false,
            };
            match validate::lightweight_validate(&candidate_layout) {
                Ok(resource_spawn_candidates) => Some(types::GeneratedWorldLayout {
                    resource_spawn_candidates,
                    ..candidate_layout
                }),
                Err(err) => {
                    eprintln!("[WFC validate] attempt={attempt} seed={sub_seed}: {err}");
                    None
                }
            }
        })
        .unwrap_or_else(|| {
            eprintln!("WFC: fallback terrain used for master_seed={master_seed}");
            types::GeneratedWorldLayout {
                terrain_tiles: fallback_terrain(&masks),
                anchors,
                masks,
                resource_spawn_candidates: ResourceSpawnCandidates::default(),
                initial_tree_positions: Vec::new(),
                forest_regrowth_zones: Vec::new(),
                initial_rock_positions: Vec::new(),
                master_seed,
                generation_attempt: MAX_WFC_RETRIES + 1,
                used_fallback: true,
            }
        });

    #[cfg(any(test, debug_assertions))]
    {
        let warnings = validate::debug_validate(&layout);
        for w in &warnings {
            eprintln!("[WFC debug] {:?}: {}", w.kind, w.message);
        }
    }

    layout
}
```

**注意**: fallback レイアウトは `lightweight_validate` を通さない（fallback は River/Sand 制約を維持するが
Sand が存在しないため `collect_required_resource_candidates` が必ず失敗する）。
`debug_validate` は fallback にも適用して `FallbackReached` warning を出す。

---

## 7. 変更ファイルと責務

| ファイル | 変更内容 |
| --- | --- |
| `crates/hw_world/src/mapgen/validate.rs` (新規) | `lightweight_validate` / `collect_required_resource_candidates` / `debug_validate` / `ValidationError` / `ValidationWarning` / `ValidatorPathWorld` |
| `crates/hw_world/src/mapgen.rs` | `pub mod validate;` を追加。`generate_world_layout()` の retry ループを「候補 layout を validate し、返ってきた `ResourceSpawnCandidates` を載せて採用する」形式に書き換え |
| `crates/hw_world/Cargo.toml` | 変更なし（`thiserror` は手動 `Display` + `Error` impl で代替） |

### validate.rs のインポート構成（参考）

```rust
// crates/hw_world/src/mapgen/validate.rs

use hw_core::constants::{MAP_HEIGHT, MAP_WIDTH};
use hw_core::world::GridPos;

use crate::mapgen::types::{GeneratedWorldLayout, ResourceSpawnCandidates};
use crate::pathfinding::{
    can_reach_target, PathfindingContext, PathWorld,
};
use crate::terrain::TerrainType;

#[cfg(any(test, debug_assertions))]
use crate::mapgen::wfc_adapter::CARDINAL_DIRS;
#[cfg(any(test, debug_assertions))]
use crate::river::{RIVER_TOTAL_TILES_TARGET_MAX, RIVER_TOTAL_TILES_TARGET_MIN};
```

---

## 8. Golden seed 定数（テスト用）

[wfc-ms0-invariant-spec.md](wfc-ms0-invariant-spec.md) §3.0 の方針（4 本: `STANDARD`, `WINDING_RIVER`, `TIGHT_BAND`, `RETRY`）に合わせ、`mapgen.rs` の `#[cfg(test)]` ブロックに **`u64` 定数**を定義する（`validate.rs` のテストからは `super::super::tests` の再利用か、`validate.rs` 内に同じ値を定数で持つ）。

**`GOLDEN_SEED_STANDARD`**: mapgen.rs の既存テストで使用している `TEST_SEED_A = 42` と同じ値を使用する（lightweight_validate が通ることを確認済み）。

```rust
// mapgen.rs の #[cfg(test)] ブロック内（または validate.rs の tests モジュール内）
pub(crate) const GOLDEN_SEED_STANDARD: u64 = 42;       // TEST_SEED_A と同値
pub(crate) const GOLDEN_SEED_WINDING_RIVER: u64 = 0;   // 実装時に snake-like river が生成される seed を探して確定
pub(crate) const GOLDEN_SEED_TIGHT_BAND: u64 = 0;      // 実装時に sand band が狭い seed を探して確定
// RETRY 用 seed は必要に応じて追加
```

初期値 `0` はプレースホルダ。実装時に `generate_world_layout(seed)` を数値を変えながら呼び出して確認し、**`lightweight_validate` が通ること**を CI で保証する。**CI に載せる前にプレースホルダを実 seed に差し替える**（`0` のままではテストが不安定になり得る）。

---

## 9. 完了条件チェックリスト

- [x] `lightweight_validate()` が 4 チェックを実装している
- [x] `lightweight_validate()` が成功時に `ResourceSpawnCandidates` を返す
- [x] `debug_validate()` が **6 チェック**を実装している（`check_protection_band_clean` / `check_sand_river_adjacency_ratio` / `check_sand_diagonal_only_contacts` / `check_river_tile_count` / `check_no_fallback_reached` / `check_forbidden_diagonal_patterns`）。有効化は `#[cfg(any(test, debug_assertions))]`（§4・§6 と一致）
- [x] `ValidationError` / `ValidationWarning` が定義されている
- [x] Site/Yard 内に River/Sand がある場合、`lightweight_validate` が Err を返す
- [x] Site ↔ Yard が非連結の場合、`lightweight_validate` が Err を返す
- [x] 到達確認済みの `water_tiles` / `sand_tiles` が最終 `GeneratedWorldLayout.resource_spawn_candidates` に保持される
- [x] `generate_world_layout()` が lightweight_validate に失敗した試行を破棄し、次 attempt を試す
- [x] `generate_world_layout()` が最終的に返す non-fallback layout は lightweight_validate を通過している
- [x] fallback に到達した場合（`used_fallback == true`）、`debug_validate` が警告を出す
- [x] Sand の diagonal-only River 接触が多すぎる場合、`debug_validate` が warning を出す
- [x] `cargo test -p hw_world` の golden seed テストが全て通る
- [x] `CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check --workspace` / `cargo clippy --workspace` が通る

---

## 10. テスト

`validate.rs` の `#[cfg(test)]` モジュールに最低限の単体テストを置く。
integration テスト（golden seeds）は `mapgen.rs` の既存 `mod tests` に追加する。

```rust
// crates/hw_world/src/mapgen/validate.rs の末尾
#[cfg(test)]
mod tests {
    use super::*;
    use crate::mapgen::generate_world_layout;
    use hw_core::constants::MAP_WIDTH;

    const GOLDEN_SEED_STANDARD: u64 = 42;

    #[test]
    fn test_golden_seeds_pass_lightweight_validate() {
        for seed in [GOLDEN_SEED_STANDARD] {
            let layout = generate_world_layout(seed);
            assert!(
                lightweight_validate(&layout).is_ok(),
                "seed={seed}: lightweight_validate failed"
            );
            assert!(
                !layout.resource_spawn_candidates.water_tiles.is_empty(),
                "seed={seed}: validated water_tiles missing"
            );
            assert!(
                !layout.resource_spawn_candidates.sand_tiles.is_empty(),
                "seed={seed}: validated sand_tiles missing"
            );
        }
    }

    #[test]
    fn test_fake_invalid_layout_fails_validate() {
        use crate::terrain::TerrainType;

        let mut layout = generate_world_layout(GOLDEN_SEED_STANDARD);
        // Site の左上角を River に書き換える
        let min_x = layout.anchors.site.min_x;
        let min_y = layout.anchors.site.min_y;
        let idx = (min_y * MAP_WIDTH + min_x) as usize;
        layout.terrain_tiles[idx] = TerrainType::River;
        assert!(lightweight_validate(&layout).is_err());
    }
}
```

`TerrainType` は `crate::terrain::TerrainType` をテストで import する。

---

## 11. 検証コマンド

```sh
CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo test -p hw_world
CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check --workspace
CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo clippy --workspace 2>&1 | grep "^warning:" | grep -v generated
```

---

## 12. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| `2026-04-01` | `Copilot` | wfc-terrain-generation-plan-2026-04-01.md の MS-WFC-2 を分割・詳細化 |
| `2026-04-04` | — | レビュー反映: `AnchorLayout` / `ResourceSpawnCandidates` の実フィールド名、`mapgen.rs`+`mapgen/validate.rs` 構成、保護帯は `combined_protection_band` 等、`used_fallback` と `debug_assert` 非使用、`GridPos`・`eprintln`・検証コマンドの `CARGO_HOME` 統一、golden seed 節・テストの `GridRect` 修正。 |
| `2026-04-04` | — | レビュー反映: `lightweight_validate()` が `ResourceSpawnCandidates` を返して `GeneratedWorldLayout.resource_spawn_candidates` を埋める流れを明記。fallback 方針を親計画と同期し、debug/test でも warning+ログで継続する前提に整理。 |
| `2026-04-04` | `Copilot` | ブラッシュアップ: `ValidatorPathWorld` の 4 メソッド全実装を明示。必須資源チェックを pathfinding ベースで具体化し、River 非 walkable 問題を明記。`thiserror` 未導入を明記し手動 `Display`+`Error` に差し替え。`check_river_tile_count` を `RIVER_TOTAL_TILES_TARGET_MIN/MAX` 定数参照に修正。`check_protection_band_clean` の具体実装を追加。`§6` の統合コードを実際の `generate_world_layout()` パターンに合わせて修正（fallback が validate をスキップする設計を明記）。`§7` に validate.rs のインポート構成を追加。`§8` の golden seed を `TEST_SEED_A=42` と一致させプレースホルダ値を明記。`§10` テストを validate.rs 内 `#[cfg(test)]` に整理。 |
| `2026-04-04` | — | レビュー反映: メタを `Ready` に変更。冒頭にサマリ表を追加。水源列挙を `river_mask` と `TerrainType::River` の交差に統一（サンプルコード更新）。`check_sand_diagonal_only_contacts` に疑似コードを追加。§9 の `debug_validate` を 6 チェック・`cfg(any(test, debug_assertions))` に整合。§8 にプレースホルダ seed の注意を追記。 |
| `2026-04-05` | — | 実装完了を反映: メタを `完了` に変更。§9 チェックリストを全 `[x]` に。恒久ドキュメント（`world_layout.md`、親計画、`milestone-roadmap.md`、`hw_world/README.md`、`debug-features.md`）を 2c 実装済みに同期。 |
