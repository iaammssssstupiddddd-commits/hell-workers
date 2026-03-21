# Phase 1 詳細実装計画: ファイル分割・構造整理

## 概要

| タスク | 対象ファイル | 現在行数 | 作業内容 |
|--------|------------|---------|---------|
| 1-A | `crates/hw_jobs/src/assigned_task.rs` | 459行 | タスク種別ごとにサブモジュール分割 |
| 1-B | `crates/hw_world/src/map/mod.rs` | 453行 | `impl WorldMap` を機能別ファイルに分割 |

**前提**: 各ステップ後に `CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check` を実行してコンパイルを確認する。パブリック API は一切変更しない。

---

## 1-A: `assigned_task.rs` の分割

### 目標ディレクトリ構造

```
crates/hw_jobs/src/
├── lib.rs                ← `pub mod assigned_task` を `pub mod tasks` に変更
├── tasks/                ← 新規ディレクトリ（assigned_task.rs の内容を移動）
│   ├── mod.rs            ← AssignedTask enum + impl AssignedTask
│   ├── gather.rs         ← GatherData, GatherPhase
│   ├── haul.rs           ← HaulData, HaulPhase, HaulToBlueprintData, HaulToBpPhase
│   ├── build.rs          ← BuildData, BuildPhase, ReinforceFloorTileData, ReinforceFloorPhase,
│   │                        PourFloorTileData, PourFloorPhase, FrameWallTileData, FrameWallPhase,
│   │                        CoatWallData, CoatWallPhase
│   ├── bucket.rs         ← BucketTransportData, BucketTransportSource,
│   │                        BucketTransportDestination, BucketTransportPhase,
│   │                        impl BucketTransportData
│   ├── collect.rs        ← CollectSandData, CollectSandPhase,
│   │                        CollectBoneData, CollectBonePhase
│   ├── refine.rs         ← RefineData, RefinePhase, HaulToMixerData, HaulToMixerPhase
│   ├── move_plant.rs     ← MovePlantData, MovePlantTask, MovePlantPhase
│   └── wheelbarrow.rs    ← HaulWithWheelbarrowData, HaulWithWheelbarrowPhase
├── construction.rs
├── events.rs
...
```

> **Note**: `HaulToMixerData` / `HaulToMixerPhase` は Mixer への搬送なので `refine.rs` に配置する。

---

### 各ファイルの詳細内容

#### `tasks/gather.rs`
```rust
use bevy::prelude::*;
use hw_core::jobs::WorkType;

#[derive(Reflect, Clone, Debug, PartialEq)]
pub struct GatherData {
    pub target: Entity,
    pub work_type: WorkType,
    pub phase: GatherPhase,
}

#[derive(Clone, Copy, Debug, PartialEq, Reflect, Default)]
pub enum GatherPhase {
    #[default]
    GoingToResource,
    Collecting { progress: f32 },
    Done,
}
```

#### `tasks/haul.rs`
```rust
use bevy::prelude::*;

#[derive(Reflect, Clone, Debug, PartialEq, Eq)]
pub struct HaulData {
    pub item: Entity,
    pub stockpile: Entity,
    pub phase: HaulPhase,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Reflect, Default)]
pub enum HaulPhase {
    #[default]
    GoingToItem,
    GoingToStockpile,
    Dropping,
}

#[derive(Reflect, Clone, Debug, PartialEq, Eq)]
pub struct HaulToBlueprintData {
    pub item: Entity,
    pub blueprint: Entity,
    pub phase: HaulToBpPhase,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Reflect, Default)]
pub enum HaulToBpPhase {
    #[default]
    GoingToItem,
    GoingToBlueprint,
    Delivering,
}
```

#### `tasks/build.rs`
```rust
use bevy::prelude::*;

#[derive(Reflect, Clone, Debug, PartialEq)]
pub struct BuildData {
    pub blueprint: Entity,
    pub phase: BuildPhase,
}

#[derive(Clone, Copy, Debug, PartialEq, Reflect, Default)]
pub enum BuildPhase {
    #[default]
    GoingToBlueprint,
    Building { progress: f32 },
    Done,
}

#[derive(Reflect, Clone, Debug, PartialEq, Eq)]
pub struct ReinforceFloorTileData {
    pub tile: Entity,
    pub site: Entity,
    pub phase: ReinforceFloorPhase,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Reflect, Default)]
pub enum ReinforceFloorPhase {
    #[default]
    GoingToMaterialCenter,
    PickingUpBones,
    GoingToTile,
    Reinforcing { progress_bp: u16 },
    Done,
}

#[derive(Reflect, Clone, Debug, PartialEq, Eq)]
pub struct PourFloorTileData {
    pub tile: Entity,
    pub site: Entity,
    pub phase: PourFloorPhase,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Reflect, Default)]
pub enum PourFloorPhase {
    #[default]
    GoingToMaterialCenter,
    PickingUpMud,
    GoingToTile,
    Pouring { progress_bp: u16 },
    Done,
}

#[derive(Reflect, Clone, Debug, PartialEq, Eq)]
pub struct FrameWallTileData {
    pub tile: Entity,
    pub site: Entity,
    pub phase: FrameWallPhase,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Reflect, Default)]
pub enum FrameWallPhase {
    #[default]
    GoingToMaterialCenter,
    PickingUpWood,
    GoingToTile,
    Framing { progress_bp: u16 },
    Done,
}

#[derive(Reflect, Clone, Debug, PartialEq, Eq)]
pub struct CoatWallData {
    pub tile: Entity,
    pub site: Entity,
    pub wall: Entity,
    pub phase: CoatWallPhase,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Reflect, Default)]
pub enum CoatWallPhase {
    #[default]
    GoingToMaterialCenter,
    PickingUpMud,
    GoingToTile,
    Coating { progress_bp: u16 },
    Done,
}
```

#### `tasks/bucket.rs`
```rust
use bevy::prelude::*;

// BucketTransportData + Source/Destination/Phase + impl BucketTransportData
// (assigned_task.rs の 29〜109行をそのまま移動)
```

#### `tasks/collect.rs`
```rust
use bevy::prelude::*;

// CollectSandData, CollectSandPhase, CollectBoneData, CollectBonePhase
// (assigned_task.rs の CollectSandData〜CollectBonePhase 定義をそのまま移動)
// ※ 同ファイル内の RefineData/HaulToMixerData は refine.rs に移動するため含めない
```

#### `tasks/refine.rs`
```rust
use bevy::prelude::*;
use hw_core::logistics::ResourceType;

// RefineData, RefinePhase, HaulToMixerData, HaulToMixerPhase
// (assigned_task.rs の RefineData〜HaulToMixerPhase 定義をそのまま移動)
```

#### `tasks/move_plant.rs`
```rust
use bevy::prelude::*;

// MovePlantData, MovePlantTask, MovePlantPhase
// (assigned_task.rs の 327〜352行をそのまま移動)
```

#### `tasks/wheelbarrow.rs`
```rust
use bevy::prelude::*;
use hw_core::logistics::{ResourceType, WheelbarrowDestination};

// HaulWithWheelbarrowData, HaulWithWheelbarrowPhase
// (assigned_task.rs の 238〜260行をそのまま移動)
```

#### `tasks/mod.rs`（核心ファイル）
```rust
//! タスク実行関連の型定義

pub mod bucket;
pub mod build;
pub mod collect;
pub mod gather;
pub mod haul;
pub mod move_plant;
pub mod refine;
pub mod wheelbarrow;

pub use bucket::{
    BucketTransportData, BucketTransportDestination, BucketTransportPhase, BucketTransportSource,
};
pub use build::{
    BuildData, BuildPhase, CoatWallData, CoatWallPhase, FrameWallPhase, FrameWallTileData,
    PourFloorPhase, PourFloorTileData, ReinforceFloorPhase, ReinforceFloorTileData,
};
pub use collect::{CollectBoneData, CollectBonePhase, CollectSandData, CollectSandPhase};
pub use gather::{GatherData, GatherPhase};
pub use haul::{HaulData, HaulPhase, HaulToBlueprintData, HaulToBpPhase};
pub use move_plant::{MovePlantData, MovePlantPhase, MovePlantTask};
pub use refine::{HaulToMixerData, HaulToMixerPhase, RefineData, RefinePhase};
pub use wheelbarrow::{HaulWithWheelbarrowData, HaulWithWheelbarrowPhase};

use bevy::prelude::*;
use hw_core::jobs::WorkType;

#[derive(Component, Reflect, Clone, Debug, Default)]
#[reflect(Component)]
pub enum AssignedTask {
    #[default]
    None,
    Gather(GatherData),
    Haul(HaulData),
    HaulToBlueprint(HaulToBlueprintData),
    Build(BuildData),
    MovePlant(MovePlantData),
    BucketTransport(BucketTransportData),
    CollectSand(CollectSandData),
    CollectBone(CollectBoneData),
    Refine(RefineData),
    HaulToMixer(HaulToMixerData),
    HaulWithWheelbarrow(HaulWithWheelbarrowData),
    ReinforceFloorTile(ReinforceFloorTileData),
    PourFloorTile(PourFloorTileData),
    FrameWallTile(FrameWallTileData),
    CoatWall(CoatWallData),
}

impl AssignedTask {
    // (assigned_task.rs の 366〜459行をそのまま移動)
    // bucket_transport_data(), work_type(), get_target_entity(),
    // get_amount_if_haul_water(), expected_item(), requires_item_in_inventory()
}
```

---

### `hw_jobs/src/lib.rs` の変更点

```diff
-pub mod assigned_task;
+pub mod tasks;
 pub mod construction;
 pub mod events;
...

-pub use assigned_task::{
+pub use tasks::{
     AssignedTask,
     BucketTransportData, BucketTransportSource, BucketTransportDestination, BucketTransportPhase,
     GatherData, GatherPhase,
     HaulData, HaulPhase,
     HaulToBlueprintData, HaulToBpPhase,
     BuildData, BuildPhase,
     CollectSandData, CollectSandPhase,
     CollectBoneData, CollectBonePhase,
     RefineData, RefinePhase,
     HaulToMixerData, HaulToMixerPhase,
     HaulWithWheelbarrowData, HaulWithWheelbarrowPhase,
     ReinforceFloorTileData, ReinforceFloorPhase,
     PourFloorTileData, PourFloorPhase,
     FrameWallTileData, FrameWallPhase,
     CoatWallData, CoatWallPhase,
     MovePlantData, MovePlantTask, MovePlantPhase,
 };
```

> **ポイント**: `pub use tasks::{...}` の内容は現在の `pub use assigned_task::{...}` と完全に同一にする。
> クレート外からの `use hw_jobs::AssignedTask` などは変更不要。

> **⚠️ 要更新ファイル**: `crates/hw_soul_ai/src/soul_ai/execute/task_execution/types.rs` が
> `pub use hw_jobs::assigned_task::{...}` を直接参照している。手順 5 で合わせて更新すること。

---

### 実装手順（1-A）

1. `crates/hw_jobs/src/tasks/` ディレクトリを作成
2. 各サブファイルを作成（gather.rs, haul.rs, build.rs, bucket.rs, collect.rs, refine.rs, move_plant.rs, wheelbarrow.rs）
3. `tasks/mod.rs` を作成（AssignedTask enum + impl + pub use）
4. `lib.rs` の `pub mod assigned_task` → `pub mod tasks` に変更
5. `lib.rs` の `pub use assigned_task::` → `pub use tasks::` に変更、かつ
   `crates/hw_soul_ai/src/soul_ai/execute/task_execution/types.rs` の
   `pub use hw_jobs::assigned_task::{...}` → `pub use hw_jobs::tasks::{...}` に変更
6. `cargo check` でコンパイル確認
7. `assigned_task.rs` を削除

---

## 1-B: `WorldMap` メソッドの分割

### 目標ディレクトリ構造

```
crates/hw_world/src/map/
├── mod.rs         ← WorldMap 構造体定義 + Default impl + PathWorld impl のみ
├── access.rs      ← (既存) WorldMapRead / WorldMapWrite — 変更なし
├── tiles.rs       ← タイル・座標変換メソッド群
├── obstacles.rs   ← 障害物管理メソッド群
├── buildings.rs   ← 建物占有管理メソッド群
├── doors.rs       ← ドア管理メソッド群
├── stockpiles.rs  ← 備蓄スポット管理メソッド群
└── bridges.rs     ← 橋タイル管理メソッド群
```

---

### 各ファイルに移すメソッド一覧

#### `tiles.rs`（17メソッド）
```
pos_to_idx, idx_to_pos,
is_walkable, is_river_tile, is_walkable_world,
terrain_at_idx, terrain_tiles, set_terrain_at_idx,
tile_entity_at_idx, set_tile_entity_at_idx,
world_to_grid, grid_to_world,
snap_to_grid_center, snap_to_grid_edge,
get_nearest_walkable_grid, get_nearest_river_grid
```

必要な imports:
```rust
use super::WorldMap;
use crate::{
    TerrainType, find_nearest_river_grid, find_nearest_walkable_grid,
    grid_to_world, idx_to_pos, snap_to_grid_center, snap_to_grid_edge, world_to_grid,
};
use bevy::prelude::*;
```

#### `obstacles.rs`（8メソッド）
```
obstacle_count, obstacle_indices,
add_obstacle, remove_obstacle,
add_grid_obstacle, remove_grid_obstacle,
add_grid_obstacles, reserve_building_footprint_tiles
```

必要な imports:
```rust
use super::WorldMap;
```

#### `buildings.rs`（17メソッド）
```
building_entity, has_building, set_building, clear_building,
set_building_occupancy, set_building_occupancies,
clear_building_occupancy, clear_building_footprint,
clear_building_occupancy_if_owned,
release_building_grid_if_owned, release_building_grids_if_owned,
release_building_footprint_if_owned, release_building_footprint_if_matches,
release_building_grid_if_matches, building_entries,
reserve_building_footprint, register_completed_building_footprint
```

必要な imports:
```rust
use super::WorldMap;
use bevy::prelude::*;
use hw_core::world::DoorState;
use hw_jobs::BuildingType;
```

> **注意**: `register_completed_building_footprint` 内で `self.register_door(...)` を呼ぶが、
> `register_door` は `doors.rs` の `impl WorldMap` に定義される。
> Rust では同一型の `impl` ブロックが複数ファイルにまたがっていても相互参照できるので問題なし。

#### `doors.rs`（8メソッド）
```
add_door, remove_door,
set_door_state, door_entity, door_state,
get_door_cost, register_door, sync_door_passability
```

必要な imports:
```rust
use super::WorldMap;
use bevy::prelude::*;
use hw_core::constants::DOOR_OPEN_COST;
use hw_core::world::DoorState;
```

#### `stockpiles.rs`（9メソッド）
```
stockpile_entity, has_stockpile,
set_stockpile, clear_stockpile, register_stockpile_tile,
move_stockpile_tile, clear_stockpile_tile_if_owned,
take_stockpile_tiles, stockpile_entries
```

必要な imports:
```rust
use super::WorldMap;
use bevy::prelude::*;
```

#### `bridges.rs`（2メソッド）
```
add_bridged_tile, register_bridge_tile
```

必要な imports:
```rust
use super::WorldMap;
use bevy::prelude::*;
```

---

### `map/mod.rs` の変更後の姿

```rust
mod access;
mod bridges;
mod buildings;
mod doors;
mod obstacles;
mod stockpiles;
mod tiles;

pub use access::{WorldMapRead, WorldMapWrite};

use crate::pathfinding::PathWorld;
use bevy::prelude::*;
use hw_core::constants::*;
use hw_core::world::DoorState;
use hw_jobs::BuildingType;
use std::collections::{HashMap, HashSet};
use crate::TerrainType;

#[derive(Resource)]
pub struct WorldMap {
    pub tiles: Vec<TerrainType>,
    pub tile_entities: Vec<Option<Entity>>,
    pub buildings: HashMap<(i32, i32), Entity>,
    pub doors: HashMap<(i32, i32), Entity>,
    pub door_states: HashMap<(i32, i32), DoorState>,
    pub stockpiles: HashMap<(i32, i32), Entity>,
    pub bridged_tiles: HashSet<(i32, i32)>,
    pub obstacles: Vec<bool>,
}

impl Default for WorldMap {
    fn default() -> Self { ... }  // そのまま維持
}

// PathWorld trait 実装のみここに残す
impl PathWorld for WorldMap {
    fn pos_to_idx(...) { WorldMap::pos_to_idx(self, x, y) }
    fn idx_to_pos(...) { WorldMap::idx_to_pos(idx) }
    fn is_walkable(...) { WorldMap::is_walkable(self, x, y) }
    fn get_door_cost(...) { WorldMap::get_door_cost(self, x, y) }
}
```

> **注意**: `mod tiles`, `mod obstacles` 等は `pub` にしない。外部からは `WorldMap` のメソッドとしてのみアクセスする。

---

### `hw_world/src/lib.rs` の変更点

変更なし。`pub use map::WorldMap` のまま。

---

### 実装手順（1-B）

1. `tiles.rs` を新規作成し、対象メソッドを `mod.rs` から切り出す
2. `cargo check` で確認
3. `obstacles.rs` を新規作成し、対象メソッドを切り出す
4. `cargo check` で確認
5. `buildings.rs` を新規作成し、対象メソッドを切り出す
6. `cargo check` で確認
7. `doors.rs` を新規作成し、対象メソッドを切り出す
8. `cargo check` で確認
9. `stockpiles.rs` → `bridges.rs` と順次実施
10. 最終 `cargo check` で全体確認
11. `mod.rs` の不要な `use` 宣言を整理

> **ポイント**: 1ファイルずつ `cargo check` を挟むことで、エラー箇所を特定しやすくする。

---

## 完了基準

- [ ] `cargo check` がエラーなし
- [ ] `cargo test` が通る（既存テスト）
- [ ] `hw_jobs::AssignedTask` など既存のパス経由でのアクセスが変わらない
- [ ] `WorldMap` の全メソッドが引き続き呼び出し可能
- [ ] `assigned_task.rs` が削除されている
