# Phase 4: `bevy_app` スリム化 — 詳細実装計画

## 問題の概要

`bevy_app/src/systems/` および `entities/` には、本来 leaf crate に属するコードが残留している。
具体的には「全ファイルが `pub use hw_*::...` のみ」のファサード層と、GameAssets に依存しない純粋なロジックが bevy_app に存在する。
これらを整理することで `bevy_app` を真の「配線レイヤー」に近づける。

---

## 調査結果サマリ

### `systems/room/` — 全ファイルが hw_world の再エクスポート

| ファイル | 内容 |
|--------|------|
| `components.rs` | `pub use hw_world::{Room, RoomBounds, RoomOverlayTile};` |
| `detection.rs`  | `pub use hw_world::detect_rooms_system;` |
| `dirty_mark.rs` | `pub use hw_world::{mark_room_dirty_..., on_building_added, ...};` |
| `resources.rs`  | `pub use hw_world::{RoomDetectionState, RoomTileLookup, RoomValidationState};` |
| `validation.rs` | `pub use hw_world::validate_rooms_system;` |
| `visual.rs`     | `pub use hw_world::sync_room_overlay_tiles_system;` |

→ 全ファイルが不要なファサード。**削除して呼び出し側を `hw_world::` に直接変更できる。**

呼び出し元:
- `plugins/logic.rs` — `use crate::systems::room::{...}` (6シンボル)
- `plugins/visual.rs` — `use crate::systems::room::sync_room_overlay_tiles_system`

---

### `systems/world/` — 1ファイルのみ、全体が hw_world の再エクスポート

```rust
// mod.rs の全内容:
pub mod zones {
    pub use hw_world::zones::*;
}
```

呼び出し元 (11ファイル):
- `interface/selection/building_place/*.rs` (3ファイル) — `use crate::systems::world::zones::{Site, Yard}`
- `systems/command/area_selection/*.rs` (8ファイル) — `use crate::systems::world::zones::Site`

→ 全呼び出し元のパスを `hw_world::zones::Site` / `hw_world::zones::Yard` に変更し、`systems/world/` を削除できる。

---

### `systems/spatial/` — thin wrapper（一部は純再エクスポート済み）

| ファイル | 種別 | 呼び出す型 | 移動先案 |
|--------|------|-----------|---------|
| `soul.rs`        | 純再エクスポート | `hw_spatial::*` | caller を hw_spatial に向ける→削除 |
| `familiar.rs`    | 純再エクスポート | `hw_spatial::*` | caller を hw_spatial に向ける→削除 |
| `blueprint.rs`   | thin wrapper | `hw_jobs::Blueprint` | caller は既に hw_spatial をジェネリック直呼び→削除 |
| `designation.rs` | thin wrapper | `hw_jobs::Designation` | 同上→削除 |
| `stockpile.rs`   | thin wrapper | `hw_logistics::Stockpile` | 同上→削除（hw_spatial に hw_logistics 追加は循環依存のため不可） |
| `transport_request.rs` | thin wrapper | `hw_logistics::TransportRequest` | 同上→削除 |
| `resource.rs`    | thin wrapper | `hw_logistics::ResourceItem` | 同上→削除 |

呼び出し元 (3ファイル):
- `plugins/spatial.rs`
- `plugins/startup/mod.rs`
- `interface/ui/presentation/mod.rs`

---

### `entities/` 運動系 — leaf crate のみに依存

| ファイル | 使用する外部型 | GameAssets? | 移動先案 |
|--------|------------|------------|---------|
| `damned_soul/movement/locomotion.rs` | `hw_core::soul::*`, `hw_core::world::DoorState`, `hw_world::WorldMap/WorldMapRead`, `hw_core::relationships::PushingWheelbarrow` | ❌ なし | `hw_soul_ai::movement` |
| `familiar/movement.rs` | `hw_core::soul::Path`, `hw_core::familiar::Familiar`, `entities/familiar/components::FamiliarAnimation` | ❌ なし | `hw_familiar_ai::movement` |
| `familiar/components.rs::FamiliarAnimation` | `bevy::prelude` のみ | ❌ なし | `hw_familiar_ai::animation` (または `hw_core::familiar`) |

---

## サブフェーズ構成

```
4-A → 4-B → 4-C → 4-D
低リスク      低リスク   中リスク    中リスク
```

---

## Phase 4-A: `systems/room/` ファサード削除

### 目的
全ファイルが `pub use hw_world::...` のみ → 完全なデッドファサードを削除する。

### 実装手順

1. **`plugins/logic.rs`** の import を変更:
   ```rust
   // Before:
   use crate::systems::room::{
       detect_rooms_system, mark_room_dirty_from_building_changes_system,
       on_building_added, on_building_removed, on_door_added, on_door_removed,
       validate_rooms_system, Room, RoomDetectionState, RoomOverlayTile,
       RoomTileLookup, RoomValidationState,
   };
   // After:
   use hw_world::{
       detect_rooms_system, mark_room_dirty_from_building_changes_system,
       on_building_added, on_building_removed, on_door_added, on_door_removed,
       validate_rooms_system, Room, RoomDetectionState, RoomOverlayTile,
       RoomTileLookup, RoomValidationState,
   };
   ```

2. **`plugins/visual.rs`** の import を変更:
   ```rust
   // Before:
   use crate::systems::room::sync_room_overlay_tiles_system;
   // After:
   use hw_world::sync_room_overlay_tiles_system;
   ```

3. `systems/room/` の全 `.rs` ファイルを削除:
   - `components.rs`, `detection.rs`, `dirty_mark.rs`, `resources.rs`, `validation.rs`, `visual.rs`, `mod.rs`

4. `systems/room/README.md`, `_rules.md` は `hw_world/src/room_detection/README.md` 等に移設またはそのまま削除。

5. **`cargo check`** で確認。

### 変更ファイル
- 変更: `plugins/logic.rs`, `plugins/visual.rs`
- 削除: `systems/room/` (7ファイル + README等)
- `systems/mod.rs` から `pub mod room;` を削除

---

## Phase 4-B: `systems/world/` ファサード削除

### 目的
`mod.rs` が 1 行のファサードのみ → 削除して呼び出し側を直接 `hw_world::zones` に向ける。

### 実装手順

1. 以下の 11ファイルの import を変更:

   **`interface/selection/building_place/flow.rs`**, `placement.rs`, `mod.rs`:
   ```rust
   // Before:
   use crate::systems::world::zones::{Site, Yard};
   // After:
   use hw_world::zones::{Site, Yard};
   ```

   **`systems/command/area_selection/` の 8ファイル**:
   ```rust
   // Before:
   use crate::systems::world::zones::Site;
   // After:
   use hw_world::zones::Site;
   ```

2. `systems/world/mod.rs` を削除。

3. `systems/mod.rs` から `pub mod world;` を削除。

4. **`cargo check`** で確認。

### 変更ファイル
- 変更: `interface/selection/building_place/flow.rs`, `placement.rs`, `mod.rs`、`systems/command/area_selection/` の 8ファイル、`systems/mod.rs`
- 削除: `systems/world/mod.rs`, `systems/world/README.md`

---

## Phase 4-C: `systems/spatial/` ファサードを削除

### 目的
`systems/spatial/` の各ファイルは既に `hw_spatial` の再エクスポート or thin wrapper に過ぎず、
`plugins/spatial.rs` は**既に `hw_spatial` を直接使用している**。
実際の 3つの呼び出し元が参照するシンボルも全て `hw_spatial` の再エクスポートであるため、
import を付け替えて `systems/spatial/` を丸ごと削除する。

> **注意: `hw_spatial/Cargo.toml` の変更は不要。**  
> `hw_logistics` → `hw_spatial` の依存が既に存在するため、
> 逆向きに `hw_spatial` → `hw_logistics` を追加すると循環依存になる。  
> `plugins/spatial.rs` は既にジェネリック呼び出し（`update_*_system::<T>`）で `hw_spatial` を直接使用済み。

### 実際の呼び出し元と使用シンボル

| ファイル | インポートしているシンボル | 実体 |
|---------|----------------------|------|
| `plugins/spatial.rs` | `update_floor_construction_spatial_grid_system`, `update_gathering_spot_spatial_grid_system` | `hw_spatial` の再エクスポート |
| `plugins/startup/mod.rs` | `FloorConstructionSpatialGrid`, `GatheringSpotSpatialGrid` | `hw_spatial` の再エクスポート |
| `interface/ui/presentation/mod.rs` | `FamiliarSpatialGrid` | `hw_spatial` の再エクスポート |

### 実装手順

#### ステップ 1: 3つの呼び出し元の import を `hw_spatial::` に付け替える

**`plugins/spatial.rs`**:
```rust
// Before:
use crate::systems::spatial::{
    update_floor_construction_spatial_grid_system, update_gathering_spot_spatial_grid_system,
};
// After:
use hw_spatial::{
    update_floor_construction_spatial_grid_system, update_gathering_spot_spatial_grid_system,
};
```

**`plugins/startup/mod.rs`**:
```rust
// Before:
use crate::systems::spatial::{FloorConstructionSpatialGrid, GatheringSpotSpatialGrid};
// After:
use hw_spatial::{FloorConstructionSpatialGrid, GatheringSpotSpatialGrid};
```

**`interface/ui/presentation/mod.rs`**:
```rust
// Before:
use crate::systems::spatial::FamiliarSpatialGrid;
// After:
use hw_spatial::FamiliarSpatialGrid;
```

#### ステップ 2: `systems/spatial/` の全ファイルを削除

#### ステップ 3: `systems/mod.rs` から `pub mod spatial;` を削除

#### ステップ 4: **`cargo check`** で確認。

### 変更ファイル
- 変更: `plugins/spatial.rs`, `plugins/startup/mod.rs`, `interface/ui/presentation/mod.rs`, `systems/mod.rs`
- 削除: `systems/spatial/` (8ファイル + README)

---

## Phase 4-D: `entities/` 運動系を leaf crate に移動

### 目的
`locomotion.rs` と `familiar/movement.rs` + `FamiliarAnimation` を対応する leaf crate に移す。

### 4-D-1: `FamiliarAnimation` を `hw_familiar_ai` に移動

`FamiliarAnimation` は `bevy::prelude` のみに依存 → `hw_familiar_ai/src/animation.rs` に移動。

> **注意: `hw_familiar_ai/Cargo.toml` の変更は不要。** 既に必要な全依存が揃っている。

1. `hw_familiar_ai/src/animation.rs` を作成し、`FamiliarAnimation` を定義
2. `hw_familiar_ai/src/lib.rs` に `pub mod animation; pub use animation::FamiliarAnimation;` を追加
3. `entities/familiar/components.rs` から `FamiliarAnimation` 定義を削除し `pub use hw_familiar_ai::FamiliarAnimation;` に変更
4. `FamiliarAnimation` を参照している以下のファイルも import を修正:
   - `entities/familiar/animation.rs` — `super::components::FamiliarAnimation` → `hw_familiar_ai::FamiliarAnimation`
   - `entities/familiar/range_indicator.rs` — 同様
   - `entities/familiar/spawn.rs` — 同様
5. **`cargo check`** で確認

### 4-D-2: `familiar/movement.rs` を `hw_familiar_ai` に移動

依存型が全て leaf crate にある:
- `hw_core::soul::Path`
- `hw_core::familiar::Familiar`
- `hw_familiar_ai::FamiliarAnimation`（4-D-1 で移動済み）

> **注意: `hw_familiar_ai/Cargo.toml` の変更は不要。**

1. `hw_familiar_ai/src/movement.rs` を作成し、`familiar_movement` システムを移動
2. `hw_familiar_ai/src/lib.rs` に `pub mod movement; pub use movement::familiar_movement;` を追加
3. `entities/familiar/movement.rs` を `pub use hw_familiar_ai::familiar_movement;` の 1行ファサードに縮小
4. 登録側 (`EntityPlugin` / plugin 登録箇所) を確認・修正
5. **`cargo check`** で確認

### 4-D-3: `locomotion.rs` を `hw_soul_ai` に移動

依存型が全て leaf crate にある:
- `hw_core::soul::{DamnedSoul, AnimationState, IdleBehavior, IdleState, Path, StressBreakdown}`
- `hw_core::world::DoorState`
- `hw_world::{WorldMap, WorldMapRead}`
- `hw_core::relationships::PushingWheelbarrow`

> **注意: `hw_soul_ai/Cargo.toml` の変更は不要。** 既に必要な全依存が揃っている。

1. `hw_soul_ai/src/movement.rs` を作成し、`soul_movement` システムを移動
2. `hw_soul_ai/src/lib.rs` に `pub mod movement; pub use movement::soul_movement;` を追加
3. `entities/damned_soul/movement/locomotion.rs` を `pub use hw_soul_ai::soul_movement;` の 1行ファサードに縮小
4. **`cargo check`** で確認

---

## リスク評価

| サブフェーズ | リスク | 理由 |
|------------|-------|------|
| 4-A (room) | 低 | 2ファイルの import 変更のみ |
| 4-B (world) | 低 | 11ファイルの import 変更のみ、ロジック変更なし |
| 4-C (spatial) | 低 | import の付け替えのみ（hw_spatial/Cargo.toml 変更なし）|
| 4-D (movement) | 中 | crate 間移動 + 登録箇所の追跡が必要 |

---

## 対象外（今回のスコープ外）

以下は `GameAssets` または `app_contexts` に強く依存するため、今回は移動しない:

- `systems/command/` — `app_contexts::TaskContext` に依存（bevy_app 固有の UI 状態）
- `systems/visual/` — `GameAssets` に依存（スプライト選択）
- `systems/jobs/building_completion/spawn.rs` — `GameAssets` + `Building3dHandles` に依存
- `entities/*/spawn.rs` — `GameAssets` + `Building3dHandles` に依存
- `entities/*/animation.rs` — `GameAssets` に依存

---

## 検証方法

各ステップ完了後:
```bash
CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check
```

---

## ファイル変更一覧（総計）

### 削除（計 18ファイル以上）
- `systems/room/` (7ファイル)
- `systems/world/mod.rs`
- `systems/spatial/` (8ファイル)

### 新規追加（hw_familiar_ai / hw_soul_ai）
- `hw_familiar_ai/src/animation.rs` (FamiliarAnimation)
- `hw_familiar_ai/src/movement.rs` (familiar_movement)
- `hw_soul_ai/src/movement.rs` (soul_movement)

### 変更（import 修正）
- `plugins/logic.rs`, `plugins/visual.rs`
- `plugins/spatial.rs`, `plugins/startup/mod.rs`
- `interface/selection/building_place/` (3ファイル)
- `systems/command/area_selection/` (8ファイル)
- `interface/ui/presentation/mod.rs`
- `entities/familiar/components.rs`, `animation.rs`, `range_indicator.rs`, `spawn.rs`
