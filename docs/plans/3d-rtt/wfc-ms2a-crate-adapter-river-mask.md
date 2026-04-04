# MS-WFC-2a: 外部 WFC crate 選定・アダプタ骨格・川マスク生成

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `wfc-ms2a-crate-adapter-river-mask` |
| ステータス | `Done` |
| 作成日 | `2026-04-01` |
| 最終更新日 | `2026-04-04` (ブラッシュアップ + 実装完了) |
| 親計画 | [`wfc-terrain-generation-plan-2026-04-01.md`](wfc-terrain-generation-plan-2026-04-01.md) |
| 前MS | [`wfc-ms1-anchor-data-model.md`](wfc-ms1-anchor-data-model.md) |
| 次MS | [`wfc-ms2b-wfc-solver-constraints.md`](wfc-ms2b-wfc-solver-constraints.md) |
| 前提 | `GeneratedWorldLayout` / `WorldMasks` / `AnchorLayout` が定義済み（MS-WFC-1 完了） |

---

## 1. 目的

MS-WFC-2 を構成する3つのサブステップのうち、**WFC ソルバーへの入力を準備する段階**。

1. 外部 WFC crate を選定して `Cargo.toml` に追加する
2. `hw_world/src/mapgen/wfc_adapter.rs` に **アダプタ骨格**を作る（型変換のみ、ソルバーは次 MS）
3. `WorldMasks` の要素別保護帯を anchor から生成する
4. 川マスクを **seed 付き帯生成**に移行し、`WorldMasks::river_mask` と `river_centerline` を確定する

川マスクは WFC ソルバーへの hard constraint として使うため、**ソルバー実装より先に確定**させる。

---

## 2. 外部 WFC crate 選定基準と確定

### 選定結果: `wfc = "0.10"` (gridbugs/wfc)

| 項目 | 内容 |
| --- | --- |
| crate 名 | `wfc` |
| バージョン | `0.10` |
| ライセンス | MIT |
| リポジトリ | https://github.com/gridbugs/wfc |
| 選定理由 | seed 指定 (`Rng` トレイト受け取り)、`ForbidPattern` トレイトで hard constraint（セル固定・禁止）が実装可能、Bevy 非依存の pure ロジック crate、rand 0.8 と互換 |

### `Cargo.toml` 追加内容

**`Cargo.toml`（ワークスペースルート）**の `[workspace.dependencies]` に追加:
```toml
wfc = "0.10"   # gridbugs/wfc — MIT, seed + ForbidPattern hard constraint
direction = "0.18"  # gridbugs/direction — CardinalDirectionTable
```

**`crates/hw_world/Cargo.toml`** の `[dependencies]` に追加:
```toml
wfc = { workspace = true }
direction = { workspace = true }
```

`bevy_app` には追加しない（WFC ロジックは `hw_world` に閉じる）。

### gridbugs `wfc 0.10` の主要 API

MS-WFC-2a でアダプタ骨格を設計するうえで押さえるべき型:

| 型 / トレイト | 役割 |
| --- | --- |
| `PatternId = u32` | タイル ID（`TerrainType` を整数にマップする） |
| `PatternDescription { weight, allowed_neighbours }` | タイルの重みと 4 近傍許可リストを宣言する |
| `PatternTable<PatternDescription>` | `PatternDescription` の配列。インデックスが `PatternId` に対応 |
| `GlobalStats::new(PatternTable<PatternDescription>)` | 隣接ルールをコンパイルして `RunOwn` に渡す統計テーブルを作る |
| `RunOwn::new_forbid(size, &global_stats, forbid, &mut rng)` | hard constraint 付きでソルバーを生成する |
| `ForbidPattern` トレイト | `forbid(&mut self, fi: &mut ForbidInterface, rng)` を実装してセルへの固定・禁止を記述する |
| `ForbidInterface::forbid_all_patterns_except(coord, pattern_id, rng)` | 指定セルを 1 パターンに固定（River hard constraint） |
| `ForbidInterface::forbid_pattern(coord, pattern_id, rng)` | 指定セルで特定パターンを禁止（Site/Yard 制約） |
| `RunOwn::collapse(&mut rng)` | ソルバーを収束まで実行する（`Result<(), PropagateError>`） |

`PatternDescription::allowed_neighbours` は `direction` crate の `CardinalDirectionTable<Vec<PatternId>>` 型。  
Direction は `North / South / East / West` の 4 値。

> `direction` は `wfc` の推移依存に頼らず、`hw_world` から直接 `use direction::...` できるよう明示依存に追加する。

---

## 3. wfc_adapter モジュール（骨格）

### ファイル

`crates/hw_world/src/mapgen/wfc_adapter.rs`（新規）  
`crates/hw_world/src/mapgen.rs` に `pub mod wfc_adapter;` を追加する。

### 責務

- `TerrainType` ↔ `wfc::PatternId` の固定マッピング
- `PatternTable<PatternDescription>` による隣接ルール定義
- `WorldConstraints: ForbidPattern` として river 固定セルと Site/Yard 制約を記述
- ソルバー呼び出しシグネチャ（実装は MS-WFC-2b）

### 3.1 TerrainType → PatternId 固定マッピング

タイル種は 4 つ（Grass / Dirt / Sand / River）。`PatternId` は `u32` で、配列インデックスが ID。

```rust
// Pattern IDs (配列インデックスとして PatternTable に登録する順序)
pub const TERRAIN_PATTERN_GRASS: PatternId = 0;
pub const TERRAIN_PATTERN_DIRT:  PatternId = 1;
pub const TERRAIN_PATTERN_SAND:  PatternId = 2;
pub const TERRAIN_PATTERN_RIVER: PatternId = 3;

/// TerrainType <-> PatternId の固定変換
pub struct TerrainTileMapping;

impl TerrainTileMapping {
    pub fn to_pattern_id(terrain: TerrainType) -> PatternId {
        match terrain {
            TerrainType::Grass => TERRAIN_PATTERN_GRASS,
            TerrainType::Dirt  => TERRAIN_PATTERN_DIRT,
            TerrainType::Sand  => TERRAIN_PATTERN_SAND,
            TerrainType::River => TERRAIN_PATTERN_RIVER,
        }
    }

    pub fn from_pattern_id(id: PatternId) -> Option<TerrainType> {
        match id {
            TERRAIN_PATTERN_GRASS => Some(TerrainType::Grass),
            TERRAIN_PATTERN_DIRT  => Some(TerrainType::Dirt),
            TERRAIN_PATTERN_SAND  => Some(TerrainType::Sand),
            TERRAIN_PATTERN_RIVER => Some(TerrainType::River),
            _ => None,
        }
    }
}
```

### 3.2 隣接ルール（AdjacencyRules）

ゲームロジックに基づく隣接許可テーブル。**River の隣は Sand のみ**（Grass/Dirt が川に直接接しない）。

| from ＼ to | Grass | Dirt | Sand | River |
| --- | --- | --- | --- | --- |
| **Grass** | ✓ | ✓ | ✓ | ✗ |
| **Dirt**  | ✓ | ✓ | ✓ | ✗ |
| **Sand**  | ✓ | ✓ | ✓ | ✓ |
| **River** | ✗ | ✗ | ✓ | ✓ |

隣接ルールは**対称**（A → B が許可なら B → A も許可）。

```rust
use direction::CardinalDirectionTable;
use wfc::{PatternDescription, PatternTable, PatternId};
use std::num::NonZeroU32;

/// ゲームロジックに基づく隣接ルールを PatternTable として構築する。
///
/// 各エントリのインデックスが PatternId に対応する（Grass=0, Dirt=1, Sand=2, River=3）。
/// weight はすべて 1（2b でタイル重みを調整する）。
pub fn build_pattern_table() -> PatternTable<PatternDescription> {
    // 許可ペア（対称なので片方を定義し、双方に追加する）
    // (A, B) = A の隣として B が許可、かつ B の隣として A が許可
    let allowed_pairs: &[(PatternId, PatternId)] = &[
        (TERRAIN_PATTERN_GRASS, TERRAIN_PATTERN_GRASS),
        (TERRAIN_PATTERN_GRASS, TERRAIN_PATTERN_DIRT),
        (TERRAIN_PATTERN_GRASS, TERRAIN_PATTERN_SAND),
        (TERRAIN_PATTERN_DIRT,  TERRAIN_PATTERN_DIRT),
        (TERRAIN_PATTERN_DIRT,  TERRAIN_PATTERN_SAND),
        (TERRAIN_PATTERN_SAND,  TERRAIN_PATTERN_SAND),
        (TERRAIN_PATTERN_SAND,  TERRAIN_PATTERN_RIVER),
        (TERRAIN_PATTERN_RIVER, TERRAIN_PATTERN_RIVER),
    ];

    // 4 パターン分の許可リスト（North/South/East/West は同一ルール）
    let mut allowed: [Vec<PatternId>; 4] = [vec![], vec![], vec![], vec![]];
    for &(a, b) in allowed_pairs {
        allowed[a as usize].push(b);
        if a != b {
            allowed[b as usize].push(a);
        }
    }
    let w = NonZeroU32::new(1).unwrap();

    let descriptions: Vec<PatternDescription> = (0..4_u32)
        .map(|id| {
            let nbrs = allowed[id as usize].clone();
            PatternDescription::new(
                Some(w),
                // 4 方向すべて同一ルール（等方的地形）
                CardinalDirectionTable::new_array([nbrs.clone(), nbrs.clone(), nbrs.clone(), nbrs]),
            )
        })
        .collect();

    PatternTable::from_vec(descriptions)
}
```

### 3.3 WorldConstraints（ForbidPattern 実装）

`SolverInput` の代わりに `wfc::ForbidPattern` を直接実装した構造体を使う。  
`ForbidInterface::forbid_all_patterns_except` で River セルを固定し、`forbid_pattern` で Site/Yard セルから River・Sand を除外する。

```rust
use wfc::{Coord, ForbidInterface, ForbidPattern, PropagateError};
use wfc::wrap::Wrap;
use rand::Rng;

/// WorldMasks の制約を ForbidPattern として WFC ソルバーに渡す。
///
/// `from_masks` で生成し、`RunOwn::new_forbid` に渡す。
pub struct WorldConstraints {
    /// river_mask が true のセル → RIVER に固定
    fixed_river: Vec<Coord>,
    /// anchor_mask（site | yard）が true のセル → River / Sand を禁止
    anchor_cells: Vec<Coord>,
}

impl WorldConstraints {
    /// WorldMasks の river_mask と anchor_mask から制約を構築する。
    ///
    /// **注意**: `masks.river_mask` / `masks.anchor_mask` は
    /// `WorldMasks::from_anchor()` + `fill_river_from_seed()` が完了した後に呼ぶこと。
    pub fn from_masks(masks: &WorldMasks) -> Self {
        let mut fixed_river = Vec::new();
        let mut anchor_cells = Vec::new();

        for y in 0..MAP_HEIGHT {
            for x in 0..MAP_WIDTH {
                let pos = (x, y);
                let coord = Coord::new(x as i32, y as i32);
                if masks.river_mask.get(pos) {
                    fixed_river.push(coord);
                }
                if masks.anchor_mask.get(pos) {
                    anchor_cells.push(coord);
                }
            }
        }

        WorldConstraints { fixed_river, anchor_cells }
    }
}

impl ForbidPattern for WorldConstraints {
    fn forbid<W: Wrap, R: Rng>(
        &mut self,
        fi: &mut ForbidInterface<'_, '_, W>,
        rng: &mut R,
    ) {
        // River 固定セル: River 以外のすべてのパターンを禁止
        for &coord in &self.fixed_river {
            fi.forbid_all_patterns_except(coord, TERRAIN_PATTERN_RIVER, rng)
                .expect("river hard constraint caused contradiction");
        }
        // Site/Yard セル: River と Sand を禁止（Grass / Dirt のみ許可）
        for &coord in &self.anchor_cells {
            fi.forbid_pattern(coord, TERRAIN_PATTERN_RIVER, rng)
                .expect("anchor river forbid caused contradiction");
            fi.forbid_pattern(coord, TERRAIN_PATTERN_SAND, rng)
                .expect("anchor sand forbid caused contradiction");
        }
    }
}
```

**なぜ anchor_cells に Sand も禁止するか**: Site/Yard 内では「Grass または Dirt のみ」が契約のため（wfc-ms0 §3.1）。

### 3.4 ソルバー呼び出しシグネチャ

```rust
use wfc::{GlobalStats, RunOwn, Size, OwnedObserve, PropagateError};
use rand::SeedableRng;
use rand::rngs::StdRng;

#[derive(Debug)]
pub enum WfcError {
    Contradiction,
    MaxIterationsReached,
}

/// ソルバーを呼び出して TerrainType グリッドを返す（実装は MS-WFC-2b）。
///
/// # 引数
/// - `masks`: `fill_river_from_seed()` 適用済みの WorldMasks
/// - `seed`: サブシード（`master_seed + attempt * OFFSET` などで caller が計算する）
/// - `attempt`: 試行回数（ログ用）
pub fn run_wfc(
    masks: &WorldMasks,
    seed: u64,
    attempt: u32,
) -> Result<Vec<TerrainType>, WfcError> {
    let _ = (masks, seed, attempt);
    todo!("MS-WFC-2b で実装: GlobalStats::new(build_pattern_table()) + RunOwn::new_forbid + collapse")
}
```

`run_wfc` の実装骨格（MS-WFC-2b 向けメモ）:
```rust
// 2b での実装イメージ（ここでは todo! のまま）
let table = build_pattern_table();
let global_stats = GlobalStats::new(table);
let constraints = WorldConstraints::from_masks(masks);
let size = Size::new(MAP_WIDTH as u32, MAP_HEIGHT as u32);
let mut rng = StdRng::seed_from_u64(seed);
let mut run = RunOwn::new_forbid(size, &global_stats, constraints, &mut rng);
run.collapse(&mut rng).map_err(|_| WfcError::Contradiction)?;
// wave_cell_ref_iter で各セルの確定パターンを取り出して TerrainType に変換
```

### セルインデックス規約

`WorldConstraints::from_masks` は `BitGrid` の `get(pos)` を使いセルを走査する。  
`wfc::Coord::new(x as i32, y as i32)` の x/y はそのまま `GridPos` の `.0` / `.1` に対応する。  
**row-major 規約は `BitGrid` と同一**: `idx = x + y * MAP_WIDTH`（`0 <= x < MAP_WIDTH`, `0 <= y < MAP_HEIGHT`）。

---

## 4. 保護帯生成（anchor 由来）

### ファイル

`crates/hw_world/src/world_masks.rs`（既存 — MS-WFC-1 完了済み）

### 改修内容

`WorldMasks::from_anchor()` は MS-WFC-1 で実装済みだが、保護帯フィールドは空スタブになっている。  
この MS でそれを埋める。

```rust
impl WorldMasks {
    pub fn from_anchor(anchor: &AnchorLayout) -> Self {
        // ... site/yard/anchor_mask の設定（MS-WFC-1 で実装済み）

        // ★ 以下を MS-WFC-2a で追加する ★
        let river_protection_band =
            compute_protection_band(&anchor_mask, PROTECTION_BAND_RIVER_WIDTH);
        let rock_protection_band =
            compute_protection_band(&anchor_mask, PROTECTION_BAND_ROCK_WIDTH);
        let tree_dense_protection_band =
            compute_protection_band(&anchor_mask, PROTECTION_BAND_TREE_DENSE_WIDTH);

        WorldMasks {
            site_mask, yard_mask, anchor_mask,
            river_protection_band,
            rock_protection_band,
            tree_dense_protection_band,
            river_mask: BitGrid::map_sized(),    // fill_river_from_seed で設定
            river_centerline: Vec::new(),         // fill_river_from_seed で設定
        }
    }
}
```

### 保護帯定数（wfc-ms0 §3.1 より）

```rust
// crates/hw_world/src/world_masks.rs に集約する
pub const PROTECTION_BAND_RIVER_WIDTH:      u32 = 3;
pub const PROTECTION_BAND_ROCK_WIDTH:       u32 = 2;
pub const PROTECTION_BAND_TREE_DENSE_WIDTH: u32 = 2;
```

### 4.1 BFS による距離変換ヘルパー

wfc-ms0 §3.1.1 の「アンカー外周からの 4 近傍距離」を BFS で実装する。  
この関数は `world_masks.rs` の `pub(crate)` または `pub` 自由関数として置く。

```rust
use std::collections::VecDeque;
use hw_core::constants::{MAP_WIDTH, MAP_HEIGHT};

/// anchor_mask の外周から 4 近傍 BFS で距離変換し、
/// 距離 1..=width のセルを true にした BitGrid を返す。
///
/// wfc-ms0 §3.1.1 準拠:
/// - アンカー占有セル自体は含まない（d = 0 相当）
/// - マップ外は到達不可
pub fn compute_protection_band(anchor_mask: &BitGrid, width: u32) -> BitGrid {
    let w = MAP_WIDTH;
    let h = MAP_HEIGHT;
    let mut band = BitGrid::map_sized();
    let mut dist: Vec<u32> = vec![u32::MAX; (w * h) as usize];
    let mut queue: VecDeque<GridPos> = VecDeque::new();

    const DIRS: [(i32, i32); 4] = [(0, 1), (0, -1), (1, 0), (-1, 0)];

    // Seed: anchor 境界に隣接する非アンカーセルを距離1としてキューに積む
    for y in 0..h {
        for x in 0..w {
            if !anchor_mask.get((x, y)) {
                continue;
            }
            for (dx, dy) in DIRS {
                let nx = x + dx;
                let ny = y + dy;
                if nx < 0 || nx >= w || ny < 0 || ny >= h {
                    continue;
                }
                let idx = (ny * w + nx) as usize;
                if !anchor_mask.get((nx, ny)) && dist[idx] == u32::MAX {
                    dist[idx] = 1;
                    queue.push_back((nx, ny));
                }
            }
        }
    }

    // BFS で距離を伝播; width を超えたら band には含めない
    while let Some(pos) = queue.pop_front() {
        let d = dist[(pos.1 * w + pos.0) as usize];
        if d > width {
            continue;
        }
        band.set(pos, true);
        for (dx, dy) in DIRS {
            let nx = pos.0 + dx;
            let ny = pos.1 + dy;
            if nx < 0 || nx >= w || ny < 0 || ny >= h {
                continue;
            }
            let idx = (ny * w + nx) as usize;
            if !anchor_mask.get((nx, ny)) && dist[idx] == u32::MAX {
                dist[idx] = d + 1;
                queue.push_back((nx, ny));
            }
        }
    }

    band
}
```

### マスク組み立てパイプライン（呼び出し順）

保護帯と川生成の**二重計算を避ける**ため、次の順序を固定する。

1. `WorldMasks::from_anchor(anchor)` — site/yard/anchor マスクと三種保護帯を設定  
   （`river_mask` / `river_centerline` は空のまま）
2. `masks.fill_river_from_seed(seed)` — `anchor_mask` と `river_protection_band` を参照して川を生成

`generate_world_layout()` からの呼び出し例:

```rust
pub fn generate_world_layout(master_seed: u64) -> GeneratedWorldLayout {
    let anchors = AnchorLayout::fixed();
    let mut masks = WorldMasks::from_anchor(&anchors);
    masks.fill_river_from_seed(master_seed);
    // ... (MS-WFC-2b 以降: run_wfc を呼んで terrain_tiles を決定)
    //
    // 注意: 現行の GeneratedWorldLayout::stub(master_seed) は内部で
    // AnchorLayout::fixed() + WorldMasks::from_anchor() を再実行するため、
    // ここで構築した masks を保持できない。
    //
    // MS-WFC-2a では次のいずれかで整合を取る:
    // 1. GeneratedWorldLayout::stub_with_masks(master_seed, anchors, masks) を追加する
    // 2. generate_world_layout() 側で直接 GeneratedWorldLayout を組み立てる
    //
    // 2a 完了条件は「river/protection-band を含む masks が最終 layout に入る」こと。
    todo!("2a 実装では masks を捨てない形で stub を組み立てる")
}
```

---

## 5. 川マスク生成（seed 付き帯生成）

### ファイル

`crates/hw_world/src/river.rs`（既存）を改修し、`world_masks.rs` の `fill_river_from_seed` から呼ぶ。

### 現状

`generate_fixed_river_tiles()` による固定矩形帯（`RIVER_Y_MIN=65..=RIVER_Y_MAX=69`, x=0..=99）。

### 改修後

seed から決まる deterministic な左端→右端横断川生成に切り替える。

### 5.1 公開 API

```rust
// crates/hw_world/src/world_masks.rs
impl WorldMasks {
    /// `from_anchor` 済みの `anchor_mask` と `river_protection_band` を参照し、
    /// seed から deterministic に `river_mask` と `river_centerline` を生成して上書きする。
    ///
    /// # Panics
    /// `from_anchor` が先に呼ばれていない場合（anchor_mask が空）に debug_assert で検出する。
    pub fn fill_river_from_seed(&mut self, seed: u64) {
        debug_assert!(
            self.anchor_mask.count_set() > 0,
            "fill_river_from_seed は from_anchor の後に呼ぶこと"
        );
        let (river_mask, centerline) = crate::river::generate_river_mask(
            seed,
            &self.anchor_mask,
            &self.river_protection_band,
        );
        self.river_mask = river_mask;
        self.river_centerline = centerline;
    }
}
```

```rust
// crates/hw_world/src/river.rs
/// seed から deterministic な左端→右端横断川を生成する。
///
/// # 引数
/// - `anchor_mask`: Site ∪ Yard の占有セル（`WorldMasks::from_anchor` 済み）
/// - `river_protection_band`: アンカー外周 PROTECTION_BAND_RIVER_WIDTH の禁止帯
///
/// # 戻り値
/// `(river_mask, river_centerline)`
pub fn generate_river_mask(
    seed: u64,
    anchor_mask: &BitGrid,
    river_protection_band: &BitGrid,
) -> (BitGrid, Vec<GridPos>) {
    // 実装は §5.2 参照
    todo!()
}
```

`generate_fixed_river_tiles()` は **MS-WFC-2a 完了後も残す**（`generate_base_terrain_tiles` がまだ依存している）。  
`generate_base_terrain_tiles` の `river.rs` 依存を切るのは MS-WFC-2b のタイミングでよい（§5.5 参照）。

### 5.2 生成アルゴリズム（ランダムウォーク左→右横断）

```
アルゴリズム概要:
1. RNG 初期化
   let mut rng = StdRng::seed_from_u64(seed);

2. 開始 y を決定（マップ中央帯）
   let start_y = rng.gen_range(RIVER_START_Y_MIN..=RIVER_START_Y_MAX);
   let mut current_y = start_y as i32;

3. x=0 から x=MAP_WIDTH-1 まで 1 列ずつ進む
   for x in 0..MAP_WIDTH {

   3a. y 方向のステップを決定（蛇行バイアス）
       let step = *[-1i32, -1, 0, 0, 0, 1, 1].choose(&mut rng).unwrap();
       let next_y = (current_y + step).clamp(RIVER_Y_CLAMP_MIN, RIVER_Y_CLAMP_MAX);

   3b. next_y が river_protection_band 内なら補正
       → protection_band に隣接しない y に clamp / 再抽選（簡易実装: skip して straight）

   3c. current_y = next_y とし、centerline に (x, current_y) を追加

   3d. セグメント幅を決定
       let width = rng.gen_range(RIVER_MIN_WIDTH..=RIVER_MAX_WIDTH) as i32;
       let top = current_y - width / 2;
       let bottom = top + width - 1;

   3e. top..=bottom の各 y で river_mask を立てる（マップ外クリップ）
       for ry in top..=bottom { river_mask.set((x, ry), true); }
   }

4. river_mask と centerline を返す
```

### 5.3 定数（`crates/hw_world/src/river.rs` に集約）

```rust
/// 川生成の y 範囲（マップを縦に三等分した中央帯で開始する）
pub const RIVER_START_Y_MIN: i32 = 40;
pub const RIVER_START_Y_MAX: i32 = 70;

/// 川の y がマップ端に貼り付かないよう clamp する範囲
pub const RIVER_Y_CLAMP_MIN: i32 = 5;
pub const RIVER_Y_CLAMP_MAX: i32 = MAP_HEIGHT as i32 - 6;  // = 94

/// セグメントごとの幅（タイル数、両端含む）
pub const RIVER_MIN_WIDTH: i32 = 2;
pub const RIVER_MAX_WIDTH: i32 = 4;

/// 全体タイル数の目安（検証テスト用; seed によって変動可）
/// 旧実装: 5行 × 100列 = 500 タイル; 新実装では 200～400 程度を想定
pub const RIVER_TOTAL_TILES_TARGET_MIN: usize = 200;
pub const RIVER_TOTAL_TILES_TARGET_MAX: usize = 500;
```

旧定数 `RIVER_X_MIN / RIVER_X_MAX / RIVER_Y_MIN / RIVER_Y_MAX` は  
`generate_fixed_river_tiles()` が削除される MS-WFC-2b 以降に廃止する（2a では残す）。

### 5.4 テスト

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::anchor::AnchorLayout;
    use crate::world_masks::WorldMasks;

    fn make_masks() -> WorldMasks {
        let anchor = AnchorLayout::fixed();
        let mut masks = WorldMasks::from_anchor(&anchor);
        masks.fill_river_from_seed(42);
        masks
    }

    #[test]
    fn river_mask_crosses_map_left_to_right() {
        let masks = make_masks();
        for x in 0..MAP_WIDTH {
            let col_has_river = (0..MAP_HEIGHT).any(|y| masks.river_mask.get((x, y)));
            assert!(col_has_river, "x={x} に River セルがない（横断が途切れている）");
        }
    }

    #[test]
    fn river_mask_does_not_enter_anchor() {
        let masks = make_masks();
        for y in 0..MAP_HEIGHT {
            for x in 0..MAP_WIDTH {
                let pos = (x, y);
                assert!(
                    !(masks.river_mask.get(pos) && masks.anchor_mask.get(pos)),
                    "pos {pos:?} が river かつ anchor に属している"
                );
            }
        }
    }

    #[test]
    fn river_mask_does_not_enter_protection_band() {
        let masks = make_masks();
        for y in 0..MAP_HEIGHT {
            for x in 0..MAP_WIDTH {
                let pos = (x, y);
                assert!(
                    !(masks.river_mask.get(pos) && masks.river_protection_band.get(pos)),
                    "pos {pos:?} が river かつ protection_band に属している"
                );
            }
        }
    }

    #[test]
    fn river_total_tile_count_in_range() {
        let masks = make_masks();
        let count = masks.river_mask.count_set();
        assert!(
            (RIVER_TOTAL_TILES_TARGET_MIN..=RIVER_TOTAL_TILES_TARGET_MAX).contains(&count),
            "river tile count {count} が想定範囲外"
        );
    }

    #[test]
    fn river_generation_is_deterministic() {
        let masks_a = make_masks();
        let masks_b = make_masks();
        assert_eq!(
            masks_a.river_centerline, masks_b.river_centerline,
            "同一 seed で centerline が異なる"
        );
    }
}
```

### 5.5 `generate_base_terrain_tiles` との関係

現状、`generate_base_terrain_tiles` は `generate_fixed_river_tiles()` に依存している。  
**MS-WFC-2a の完了時点では切り替えを行わない**（スタブ地形が既存ロジックで動いていれば十分）。  
`generate_base_terrain_tiles` を `WorldMasks::river_mask` 駆動に切り替えるのは **MS-WFC-2b** でまとめて行う。  
2a では `generate_fixed_river_tiles` を残したまま、`generate_river_mask` との**整合性をテストで担保**する。

---

## 6. タイル重みと砂の方針（F4）

**砂の重み定数**はソルバー入力（MS-WFC-2b）に向けて先に定数化だけ行う。  
置き場所は **`mapgen/wfc_adapter.rs`** に集約する（WFC 入力仕様と同じファイル）。

```rust
// crates/hw_world/src/mapgen/wfc_adapter.rs

/// Sand タイルの重み設定（F4: 川隣接を主、それ以外は低頻度）
///
/// MS-WFC-2b でソルバーに渡す PatternDescription::weight として使用する。
/// 目安: 全砂タイルの 8 割が川隣接を満たすように調整する。
pub const SAND_ADJACENT_TO_RIVER_WEIGHT: u32 = 10;
pub const SAND_NON_ADJACENT_WEIGHT:      u32 = 1;
```

`PatternDescription::weight` は `Option<NonZeroU32>` のため、2b では:
```rust
NonZeroU32::new(SAND_ADJACENT_TO_RIVER_WEIGHT)  // Some(10)
```
として渡す。実際の重み適用ロジック（川隣接か否かを判定してパターンを分岐させるか、
単一 Sand パターンに統一した重みを使うか）は MS-WFC-2b で決定する。

---

## 7. 変更ファイルと責務

凡例: **新規** = このMSで作成、**変更** = このMSで編集、*既存（MS-WFC-1完了）* = 実装済みで変更なし

| ファイル | 変更種別 | 内容 |
| --- | --- | --- |
| `Cargo.toml` (workspace) | **変更** | `wfc = "0.10"` を `[workspace.dependencies]` に追加 |
| `crates/hw_world/Cargo.toml` | **変更** | `wfc = { workspace = true }` と `direction = { workspace = true }` を `[dependencies]` に追加 |
| `crates/hw_world/src/mapgen/wfc_adapter.rs` | **新規** | `TerrainTileMapping` / `TERRAIN_PATTERN_*` / `build_pattern_table()` / `WorldConstraints: ForbidPattern` / `run_wfc(todo!)` / 砂の重み定数 |
| `crates/hw_world/src/mapgen.rs` | **変更** | `pub mod wfc_adapter;` を追加 |
| `crates/hw_world/src/world_masks.rs` | **変更** | `compute_protection_band()` 追加、`from_anchor()` で三種保護帯を設定、`fill_river_from_seed()` 追加 |
| `crates/hw_world/src/river.rs` | **変更** | `generate_river_mask(seed, anchor_mask, river_protection_band)` 追加、定数追加。旧 `generate_fixed_river_tiles()` は残す |
| `crates/hw_world/src/anchor.rs` | *既存（MS-WFC-1完了）* | 変更なし |
| `crates/hw_world/src/mapgen/types.rs` | **変更** または *既存維持* | `stub_with_masks(...)` を追加するか、`generate_world_layout()` 側で直接 `GeneratedWorldLayout` を組み立てるかを選ぶ |
| `crates/hw_world/src/lib.rs` | **変更** | `wfc_adapter` の公開 API 調整（必要なら `pub use mapgen::wfc_adapter::...`） |

---

## 8. 完了条件チェックリスト

- [ ] `wfc = "0.10"` と `direction = "0.18"` が workspace `Cargo.toml` と `hw_world/Cargo.toml` に追加されている（ライセンス・バージョン・選定理由コメント付き）
- [ ] `wfc_adapter.rs` に `TERRAIN_PATTERN_*` 定数・`TerrainTileMapping` が定義されている
- [ ] `build_pattern_table()` が River の隣は Sand のみ（Grass/Dirt は River に直接隣接不可）のルールを返す
- [ ] `WorldConstraints: ForbidPattern` が River 固定セルと Site/Yard の Sand/River 禁止を正しく実装している
- [ ] `run_wfc` が `todo!()` で `cargo check` を通すシグネチャで定義されている
- [ ] `SAND_ADJACENT_TO_RIVER_WEIGHT` / `SAND_NON_ADJACENT_WEIGHT` が `wfc_adapter.rs` に定数化されている
- [ ] `compute_protection_band(anchor_mask, width)` が BFS で wfc-ms0 §3.1.1 準拠の保護帯を返す
- [ ] `WorldMasks::from_anchor()` が三種の `*_protection_band` を正しく設定する（空スタブ「MS-WFC-2a で設定」コメントが解消されている）
- [ ] `fill_river_from_seed(&mut self, seed)` が `from_anchor` 済みの `anchor_mask` / `river_protection_band` を使って川を生成する（距離変換の二重計算なし）
- [ ] `generate_river_mask(seed, anchor_mask, river_protection_band)` が左端→右端横断川を生成する
- [ ] `generate_world_layout()` が 2a で生成した `masks` を捨てず、`GeneratedWorldLayout` に `river_mask` / `river_centerline` / 各 `*_protection_band` が入る
- [ ] `river_mask` が anchor_mask セルを含まない
- [ ] `river_mask` が `river_protection_band` セルを含まない
- [ ] 全 x=0..MAP_WIDTH に少なくとも 1 つの River セルが存在する（横断保証）
- [ ] `river_centerline` の長さが MAP_WIDTH と等しい（x=0..99 の各列に 1 点）
- [ ] 川生成が同一 seed で同一結果を返す（deterministic）
- [ ] `RIVER_MIN_WIDTH` / `RIVER_MAX_WIDTH` / `RIVER_TOTAL_TILES_TARGET_MIN/MAX` / `RIVER_START_Y_MIN/MAX` / `RIVER_Y_CLAMP_MIN/MAX` が `river.rs` に定数化されている
- [ ] `PROTECTION_BAND_RIVER_WIDTH` / `PROTECTION_BAND_ROCK_WIDTH` / `PROTECTION_BAND_TREE_DENSE_WIDTH` が `world_masks.rs` に定数化されている
- [ ] `CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check --workspace` がゼロエラー
- [ ] `CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo clippy --workspace` がゼロ警告

---

## 9. 実装手順（推奨順序）

1. **Cargo.toml 更新**
   - workspace の `[workspace.dependencies]` に `wfc = "0.10"` と `direction = "0.18"` を追加（いずれも gridbugs 系・上記 §2 のスニペット参照）
   - `crates/hw_world/Cargo.toml` の `[dependencies]` に `wfc = { workspace = true }` と `direction = { workspace = true }` を追加
   - `cargo check --workspace` でビルドが通ることを確認

2. **`world_masks.rs` — 保護帯定数と `compute_protection_band` 追加**
   - `PROTECTION_BAND_RIVER_WIDTH` 等の定数を追加
   - `compute_protection_band(anchor_mask, width)` BFS 関数を実装
   - `from_anchor()` の保護帯スタブを実際の `compute_protection_band()` 呼び出しに置き換え

3. **保護帯テスト通過確認**
   ```sh
   CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo test -p hw_world protection_band
   ```
   （テスト関数 `protection_band_*` を `world_masks.rs` の `#[cfg(test)]` に追加）

4. **`river.rs` — `generate_river_mask` 実装**
   - `RIVER_START_Y_MIN/MAX`, `RIVER_Y_CLAMP_MIN/MAX`, `RIVER_MIN/MAX_WIDTH`, `RIVER_TOTAL_TILES_TARGET_*` を定数化
   - `generate_river_mask(seed, anchor_mask, river_protection_band)` をランダムウォーク実装
   - 旧 `generate_fixed_river_tiles` は残す

5. **`world_masks.rs` — `fill_river_from_seed` 追加**
   - `generate_river_mask` を呼んで `river_mask` / `river_centerline` を設定する
   - `debug_assert!(self.anchor_mask.count_set() > 0, ...)` で呼び出し順を検証

6. **川テスト通過確認**
   ```sh
   CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo test -p hw_world river
   ```
   §5.4 のテスト関数 5 本が通ることを確認

7. **`generate_world_layout` / `GeneratedWorldLayout` の一時接着**
   - `GeneratedWorldLayout::stub(master_seed)` が `masks` を再構築して捨てる現状を解消する
   - 方式は次のどちらかに統一する:
     - `GeneratedWorldLayout::stub_with_masks(master_seed, anchors, masks)` を追加
     - `generate_world_layout()` 側で `GeneratedWorldLayout` を直接組み立てる
   - 2a の時点で、保護帯と川マスクが最終 layout に見える状態にする

8. **`mapgen/wfc_adapter.rs` 新規作成**
   - `TERRAIN_PATTERN_*` 定数
   - `TerrainTileMapping::to_pattern_id / from_pattern_id`
   - `build_pattern_table()` — 隣接ルール + 等重み
   - `WorldConstraints: ForbidPattern`
   - `WfcError` / `run_wfc(todo!())`
   - `SAND_ADJACENT_TO_RIVER_WEIGHT` / `SAND_NON_ADJACENT_WEIGHT`

9. **`mapgen.rs` に `pub mod wfc_adapter;` を追加**

10. **`lib.rs` の公開 API 調整**（必要なら `pub use mapgen::wfc_adapter::...`）

11. **最終確認**
    ```sh
    CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check --workspace
    CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo clippy --workspace
    CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo test -p hw_world
    ```

---

## 10. 検証

```sh
CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check --workspace
CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo clippy --workspace
CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo test -p hw_world
```

テスト関数一覧（追加・通過を確認するもの）:

| ファイル | テスト関数名 | 確認内容 |
| --- | --- | --- |
| `world_masks.rs` | `protection_band_excludes_anchor_cells` | 保護帯がアンカー占有セルを含まない |
| `world_masks.rs` | `protection_band_river_width_is_correct` | アンカー外周から距離 3 のリングが `river_protection_band` に含まれ、距離 4 が含まれない |
| `world_masks.rs` | `protection_band_rock_width_is_correct` | 同上・距離 2 |
| `river.rs` | `river_mask_crosses_map_left_to_right` | 全 x 列に River セルが存在する |
| `river.rs` | `river_mask_does_not_enter_anchor` | river ∩ anchor = ∅ |
| `river.rs` | `river_mask_does_not_enter_protection_band` | river ∩ river_protection_band = ∅ |
| `river.rs` | `river_total_tile_count_in_range` | タイル数が `RIVER_TOTAL_TILES_TARGET_MIN..=MAX` 内 |
| `river.rs` | `river_generation_is_deterministic` | 同一 seed で同一 centerline |

---

## 11. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| `2026-04-01` | `Copilot` | wfc-terrain-generation-plan-2026-04-01.md の MS-WFC-2 を分割・詳細化 |
| `2026-04-04` | `Codex` | レビュー反映。`Site/Yard` の許可集合を表現できる adapter 入力モデルへ修正し、保護帯生成を MS-WFC-2a の成果物に追加。`mapgen.rs` 前提と `GridPos` 規約に合わせて API/変更ファイル一覧を更新。 |
| `2026-04-04` | `Copilot` | ブラッシュアップ: gridbugs `wfc 0.10` に確定・Cargo.toml スニペット追加。§3 を `PatternDescription` / `CardinalDirectionTable` / `ForbidPattern` の実 API ベースに全面改訂（隣接テーブル・`WorldConstraints` 実装例）。§4 に BFS 距離変換の完全コードと保護帯定数を追加。§5 に `StdRng` ランダムウォーク・全定数・テスト関数 5 本を追加。§6 でf32→u32修正・置き場所を wfc_adapter.rs に確定。§7 を MS-WFC-1 完了分を反映した表に更新。§9 実装手順（10 ステップ）を新設、§10 検証にテスト関数一覧を追加。 |
| `2026-04-04` | — | レビュー確認: `coord_2d::Size::new` は `Result` を返さないため 2b メモから `.unwrap()` を削除。§9 手順の番号重複を 10–11 に整理。ステップ1 の `direction` 追記を1行に集約。 |
