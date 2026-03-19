# 500行超 Rust ソースファイルの責務分割計画

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `refactor-500plus-rs-files-plan-2026-03-18` |
| ステータス | `Completed` |
| 作成日 | `2026-03-18` |
| 最終更新日 | `2026-03-19` |
| 作成者 | `Codex` |
| レビュー | `Copilot (brush-up)` |
| 関連提案 | `N/A` |
| 関連Issue/PR | `N/A` |

## 1. 目的

- **解決したい課題**: tracked な Rust ソースのうち 500 行超のファイルが 4 本（計 2274 行）に集中し、
  責務・テスト・ECS 接続が一つのモジュールに混在している。
- **到達したい状態**: 各 root ファイルは「公開ファサード + オーケストレーション」に縮小し、
  純粋ロジック・検証・ECS 型定義・テストを個別 private submodule へ分離する。
- **成功指標**:
  - 4 つの root ファイルがそれぞれ明確な単一責務を持つ。
  - 公開 API パス（クレート外からの `use` パス）を一切壊さない。
  - `CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check --workspace` が通る。

## 2. スコープ

### 対象ファイルと現行行数

| ファイル | 現行行数 |
| --- | --- |
| `crates/hw_ui/src/selection/placement.rs` | 634 行 |
| `crates/hw_world/src/room_detection.rs` | 579 行 |
| `crates/hw_familiar_ai/src/familiar_ai/decide/state_decision.rs` | 559 行 |
| `crates/hw_soul_ai/src/…/haul_with_wheelbarrow/phases/unloading.rs` | 502 行 |
| **合計** | **2274 行** |

### 非対象（Out of Scope）

- `docs/` 仕様文書の大規模改稿
- `Cargo.lock` や生成物の整理
- 挙動変更・バランス調整・AI ロジックの再設計
- パフォーマンス改善の単独着手

## 3. 現状と分割ギャップ（詳細）

### 3.1 `placement.rs`（634 行）

| 行範囲 | 責務 | 分離先 |
| --- | --- | --- |
| 1-8 | imports + 定数 | root に残す |
| 9-91 | 公開型: `PlacementRejectReason`, `PlacementValidation` | root (pub facade) |
| 93-118 | 公開型: `PlacementGeometry`, `WorldReadApi`, `BuildingPlacementContext` | root (pub facade) |
| 120-133 | private: `grid_to_world`, `world_to_grid` | `geometry.rs` |
| 134-297 | pub fn 群（geometry + validation が混在） | 下記参照 |
| 299-523 | private + pub fn 群（validation 実装） | `validation.rs` |
| 525-634 | `#[cfg(test)] mod tests` | `tests.rs` |

**geometry.rs へ移動するシンボル**（すべて現行 `pub`）:

```
grid_to_world          (L120) → pub(super) に変更してよい
world_to_grid          (L127) → pub(super) に変更してよい
move_anchor_grid       (L157)
move_occupied_grids    (L171)
move_spawn_pos         (L187)
building_geometry      (L226)
bucket_storage_geometry(L241)
building_occupied_grids(L249)
building_spawn_pos     (L271)
building_size          (L288)
grid_is_nearby         (L299)
```

**validation.rs へ移動するシンボル**:

```
validate_area_size                    (L134) pub
validate_wall_area                    (L144) pub
can_place_moved_building              (L200) pub  ← geometry 呼び出しあり
reject_for_walkable_empty_tile        (L303) private
reject_for_bridge_tile                (L325) private
validate_building_placement           (L344) pub
validate_bucket_storage_placement     (L406) pub
validate_moved_bucket_storage_placement(L436) pub
validate_floor_tile                   (L475) pub
validate_wall_tile                    (L502) pub
```

**root に残すシンボル（公開ファサード）**:

```rust
// root facade (placement.rs)
pub use self::geometry::{
    move_anchor_grid, move_occupied_grids, move_spawn_pos,
    building_geometry, bucket_storage_geometry, building_occupied_grids,
    building_spawn_pos, building_size, grid_is_nearby,
};
pub use self::validation::{
    validate_area_size, validate_wall_area, can_place_moved_building,
    validate_building_placement, validate_bucket_storage_placement,
    validate_moved_bucket_storage_placement, validate_floor_tile, validate_wall_tile,
};
```

> **注意**: `geometry.rs` 内の `grid_to_world` / `world_to_grid` は `validation.rs` からも呼ばれる。
> `use super::geometry::{grid_to_world, world_to_grid};` を `validation.rs` に追加する。

### 3.2 `room_detection.rs`（579 行）

| 行範囲 | 責務 | 分離先 |
| --- | --- | --- |
| 1-17 | module doc + imports | root (保持) |
| 19-265 | 純粋アルゴリズム型 + 関数 | `core.rs` |
| 269-341 | ECS Component / Resource 定義 | `ecs.rs` |
| 343-579 | `#[cfg(test)] mod tests` | `tests.rs` |

**core.rs へ移動するシンボル**:

```
RoomBounds                   (L19)  pub
RoomDetectionBuildingTile    (L48)  pub
RoomDetectionInput           (L61)  pub
DetectedRoom                 (L72)  pub
build_detection_input        (L85)  pub fn
detect_rooms                 (L112) pub fn
room_is_valid_against_input  (L130) pub fn
flood_fill_room              (L176) private fn
cardinal_neighbors           (L247) private fn
is_in_map_bounds             (L251) private fn
CARDINAL_OFFSETS             (定数)
```

**ecs.rs へ移動するシンボル**:

```
Room                  (L269)  pub Component
RoomOverlayTile       (L279)  pub Component
RoomTileLookup        (L285)  pub Resource
RoomDetectionState    (L291)  pub Resource
RoomValidationState   (L327)  pub Resource
```

> `ecs.rs` の imports: `use bevy::prelude::*;` + `use super::core::RoomBounds;`
> （`RoomDetectionState` が `RoomBounds` を保持するため）

**root の pub use**（`hw_world::lib.rs` が現在 `pub use room_detection::*` 相当を行っているため、
root facade が `pub use self::core::…; pub use self::ecs::…;` を出せばクレート外パスは変わらない）:

```rust
// room_detection.rs (root facade)
mod core;
mod ecs;
#[cfg(test)]
mod tests;

pub use self::core::{
    RoomBounds, RoomDetectionBuildingTile, RoomDetectionInput, DetectedRoom,
    build_detection_input, detect_rooms, room_is_valid_against_input,
};
pub use self::ecs::{
    Room, RoomOverlayTile, RoomTileLookup, RoomDetectionState, RoomValidationState,
};
```

> **注意**: `hw_world/src/lib.rs` は現在 `pub mod room_detection;` + 個別 `pub use room_detection::{…}`
> を行っている。root の再 export が一致していれば lib.rs は無変更でよい。

### 3.3 `state_decision.rs`（559 行）

| 行範囲 | 責務 | 分離先 |
| --- | --- | --- |
| 1-45 | module doc + use 宣言 | root (保持) |
| 49-103 | `FamiliarDecisionPath` + `determine_decision_path` | `path.rs` |
| 104-176 | `FamiliarStateDecisionResult` + builders | `result.rs` |
| 178-250 | `write_add_member_request`, `write_release_requests`, `emit_state_decision_messages` | `result.rs` |
| 252-559 | `FamiliarAiStateDecisionParams` + `familiar_ai_state_system` | `system.rs` |

**path.rs の内容**:

```rust
// 依存: hw_core::familiar のみ（ECS 不要）
pub enum FamiliarDecisionPath { … }
pub fn determine_decision_path(…) -> FamiliarDecisionPath { … }
```

**result.rs の内容**:

```rust
// 依存: hw_core::events, bevy::prelude::Entity
pub struct FamiliarStateDecisionResult { … }
impl FamiliarStateDecisionResult { … }
pub(super) fn write_add_member_request(…) { … }
pub(super) fn write_release_requests(…) { … }
pub(super) fn emit_state_decision_messages(…) { … }
// emit_state_decision_messages は system.rs からも呼ばれるため pub(super) とする
```

**system.rs の内容**:

```rust
// 依存: super::{path::*, result::*} + bevy SystemParam
#[derive(SystemParam)]
pub struct FamiliarAiStateDecisionParams<'w, 's> { … }
pub fn familiar_ai_state_system(params: FamiliarAiStateDecisionParams) { … }
```

**root facade**:

```rust
// state_decision.rs (root)
mod path;
mod result;
mod system;

pub use self::path::{FamiliarDecisionPath, determine_decision_path};
pub use self::result::FamiliarStateDecisionResult;
pub use self::system::{FamiliarAiStateDecisionParams, familiar_ai_state_system};
```

> **注意**: `determine_transition_reason` は `crate::familiar_ai::perceive::state_detection` から呼ばれており、
> `system.rs` が `use crate::…::determine_transition_reason;` を保持する。

### 3.4 `unloading.rs`（502 行）

このファイルは他 3 本と構造が異なる。既存 private fn は既に分離しやすい粒度だが、
Stockpile / Blueprint / Mixer の各ロジックは `pub fn handle` の inline match arm として書かれており、
そのまま別ファイルに「移動」できない。

**Phase A（安全・低リスク）**: 既存 private fn を submodule に移す。

| 移動先 | シンボル | 現行行 |
| --- | --- | --- |
| `item_ops.rs` | `try_drop_item`, `try_despawn_item` | L49, L79 |
| `capacity.rs` | `floor_site_remaining`, `wall_site_remaining`, `provisional_wall_remaining` | L87, L111, L135 |
| `finalize.rs` | `finalize_unload_task`, `finish_partial_unload` | L160, L187 |

root に残す: `has_pending_wheelbarrow_task`（L23）, `pub fn handle`（L226）

**Phase B（オプション）**: `handle` の inline match arm を関数に切り出す。

下記シグネチャを持つ関数を新規作成し、`stockpile.rs` / `blueprint.rs` / `mixer.rs` に配置する。

```rust
// 戻り値型: 各 handler が local 集計値を返す
pub(super) struct DestinationUnloadResult {
    pub unloaded_count: usize,
    pub destination_store_count: usize,
    pub delivered_items: HashSet<Entity>,
    pub mixer_release_types: Vec<ResourceType>,
    pub cancelled: bool,
}

// stockpile.rs
pub(super) fn unload_to_stockpile(
    ctx: &mut TaskExecutionContext,
    dest_stockpile: Entity,
    item_types: &[(Entity, Option<ResourceType>)],
    commands: &mut Commands,
    soul_pos: Vec2,
) -> DestinationUnloadResult;

// blueprint.rs
pub(super) fn unload_to_blueprint(
    ctx: &mut TaskExecutionContext,
    blueprint_entity: Entity,
    item_types: &[(Entity, Option<ResourceType>)],
    commands: &mut Commands,
) -> DestinationUnloadResult;

// mixer.rs
pub(super) fn unload_to_mixer(
    ctx: &mut TaskExecutionContext,
    mixer_entity: Entity,
    resource_type: ResourceType,
    item_types: &[(Entity, Option<ResourceType>)],
    commands: &mut Commands,
    soul_pos: Vec2,
) -> DestinationUnloadResult;
```

> **Phase B は Phase A 完了後に独立して実施する。**
> Phase A だけでも root のコード量は大幅に削減できるため、B はオプション扱いとする。

**root facade (unloading.rs)**:

```rust
mod item_ops;
mod capacity;
mod finalize;
// Phase B 追加時:
// mod stockpile;
// mod blueprint;
// mod mixer;

use item_ops::{try_drop_item, try_despawn_item};
use capacity::{floor_site_remaining, wall_site_remaining, provisional_wall_remaining};
use finalize::{finalize_unload_task, finish_partial_unload};

fn has_pending_wheelbarrow_task(…) { … }
pub fn handle(…) { … }  // match arm は当面 inline のまま (Phase A)
```

## 4. 実装方針（共通）

- 公開 API は維持する。呼び出し側から見えるシンボル名とパスは変えない。
- root ファイルは `mod` 宣言 + `pub use` + 最小オーケストレーションのみとする。
- `Component` / `Resource` / `SystemParam` / `MessageWriter` / `Query` の Bevy 0.18 系シグネチャは変更しない。
- テストは `#[cfg(test)] mod tests;` で外部ファイルに出し、`use super::*;` で依存型を取り込む。
- 期待される性能影響はほぼない。incremental compile の局所性が改善する。

## 5. マイルストーン（実施順）

### M1: `hw_ui` — `selection/placement` を geometry / validation に分割

**新規作成ファイル**:

```
crates/hw_ui/src/selection/placement/
  geometry.rs    ← grid_to_world, world_to_grid + move_* + building_* + grid_is_nearby
  validation.rs  ← validate_*, can_place_moved_building + private reject_* helpers
  tests.rs       ← #[cfg(test)] mod tests { use super::*; … }
```

**root (placement.rs) への追加**:

```rust
mod geometry;
mod validation;
#[cfg(test)]
mod tests;

pub use self::geometry::{
    move_anchor_grid, move_occupied_grids, move_spawn_pos,
    building_geometry, bucket_storage_geometry, building_occupied_grids,
    building_spawn_pos, building_size, grid_is_nearby,
};
pub use self::validation::{
    validate_area_size, validate_wall_area, can_place_moved_building,
    validate_building_placement, validate_bucket_storage_placement,
    validate_moved_bucket_storage_placement, validate_floor_tile, validate_wall_tile,
};
```

**validation.rs が必要な追加 use**:

```rust
use super::geometry::{grid_to_world, world_to_grid};
// WorldReadApi, BuildingPlacementContext はまだ root にある (super::*)
use super::{
    PlacementRejectReason, PlacementValidation, PlacementGeometry,
    WorldReadApi, BuildingPlacementContext,
    TANK_NEARBY_BUCKET_STORAGE_TILES,
};
```

**完了条件**:

- root が ≤ 120 行（型定義 + pub use のみ）に収まる
- `CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check -p hw_ui` が通る

---

### M2: `hw_world` — `room_detection` を core / ecs に分割

**新規作成ファイル**:

```
crates/hw_world/src/room_detection/
  core.rs   ← RoomBounds, RoomDetectionBuildingTile, RoomDetectionInput, DetectedRoom
              + build_detection_input, detect_rooms, room_is_valid_against_input
              + flood_fill_room, cardinal_neighbors, is_in_map_bounds, CARDINAL_OFFSETS
  ecs.rs    ← Room, RoomOverlayTile, RoomTileLookup, RoomDetectionState, RoomValidationState
  tests.rs  ← #[cfg(test)] mod tests { use super::*; … }
```

**ecs.rs が必要な追加 use**:

```rust
use bevy::prelude::*;
use std::collections::HashSet;
use super::core::RoomBounds;  // Room コンポーネントが bounds: RoomBounds を保持するため
use hw_core::constants::{MAP_HEIGHT, MAP_WIDTH, ROOM_MAX_TILES};
```

**root (room_detection.rs) の最終形**:

```rust
//! Room detection facade.
mod core;
mod ecs;
#[cfg(test)]
mod tests;

pub use self::core::{
    RoomBounds, RoomDetectionBuildingTile, RoomDetectionInput, DetectedRoom,
    build_detection_input, detect_rooms, room_is_valid_against_input,
};
pub use self::ecs::{
    Room, RoomOverlayTile, RoomTileLookup, RoomDetectionState, RoomValidationState,
};
```

> `hw_world/src/lib.rs` L11-46 は無変更。`pub use room_detection::{…}` がそのまま通る。

**完了条件**:

- ECS 型と純粋アルゴリズムが別ファイルになる
- `CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check -p hw_world` が通る

---

### M3: `hw_familiar_ai` — `state_decision` を path / result / system に分割

**新規作成ファイル**:

```
crates/hw_familiar_ai/src/familiar_ai/decide/state_decision/
  path.rs    ← FamiliarDecisionPath (enum), determine_decision_path (pure fn)
  result.rs  ← FamiliarStateDecisionResult + impl, write_add_member_request,
               write_release_requests, emit_state_decision_messages
  system.rs  ← FamiliarAiStateDecisionParams (SystemParam), familiar_ai_state_system
```

**path.rs の imports**（ECS なし、hw_core 型のみ）:

```rust
use hw_core::familiar::{FamiliarAiState, FamiliarCommand};
```

**result.rs の imports**:

```rust
use bevy::prelude::Entity;
use hw_core::events::{
    FamiliarAiStateChangedEvent, FamiliarIdleVisualRequest, FamiliarStateRequest,
    ReleaseReason, SquadManagementOperation, SquadManagementRequest,
};
use super::super::FamiliarDecideOutput;  // decide module
use crate::familiar_ai::perceive::state_detection::determine_transition_reason;
// MessageWriter の import は元ファイルの use 群から必要な型を確認して追加すること
```

> `determine_transition_reason` は実際には
> `crate::familiar_ai::perceive::state_detection` にある。
> `system.rs` が使っていたのと同じ import パスを `result.rs` が引き継ぐ。

**system.rs の imports**（Bevy 0.18 SystemParam など）:

```rust
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use super::path::{FamiliarDecisionPath, determine_decision_path};
use super::result::{FamiliarStateDecisionResult, emit_state_decision_messages};
// 残りの use は元の system のものをそのまま移植
```

**root facade (state_decision.rs) の最終形**:

```rust
//! 使い魔 AI 状態判断 — public facade
mod path;
mod result;
mod system;

pub use self::path::{FamiliarDecisionPath, determine_decision_path};
pub use self::result::FamiliarStateDecisionResult;
pub use self::system::{FamiliarAiStateDecisionParams, familiar_ai_state_system};
```

**完了条件**:

- `determine_decision_path` が ECS 型に依存しない単体テスト可能な関数になる
- message emission 順序が不変（`write_release_requests` → `write_add_member_request` → `emit_idle_visual` → `state_request` の順）
- `CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check -p hw_familiar_ai` が通る

---

### M4: `hw_soul_ai` — `unloading` の private fn を submodule に分離（Phase A）

**新規作成ファイル**:

```
crates/hw_soul_ai/src/…/haul_with_wheelbarrow/phases/unloading/
  item_ops.rs  ← try_drop_item (L49), try_despawn_item (L79)
  capacity.rs  ← floor_site_remaining (L87), wall_site_remaining (L111),
                 provisional_wall_remaining (L135)
  finalize.rs  ← finalize_unload_task (L160), finish_partial_unload (L187)
```

**各 submodule の最低限 imports**:

- `item_ops.rs`: `use bevy::prelude::*;` + 元ファイルの use 群から必要な型のみ抽出
- `capacity.rs`: `use crate::soul_ai::execute::task_execution::TaskExecutionContext;` + 関連型
- `finalize.rs`: `use super::item_ops::*; use super::capacity::*;` + TaskExecutionContext

**root (unloading.rs) への追加**:

```rust
mod item_ops;
mod capacity;
mod finalize;

use item_ops::{try_drop_item, try_despawn_item};
use capacity::{floor_site_remaining, wall_site_remaining, provisional_wall_remaining};
use finalize::{finalize_unload_task, finish_partial_unload};
```

**完了条件**:

- 既存 private fn が root から消え、submodule 経由で使われる
- `pub fn handle` の挙動・シグネチャは一切変わらない
- `CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check -p hw_soul_ai` が通る

#### M4-B（オプション）: `handle` の inline match arm を handler 関数に切り出す

Phase A 完了後に着手。`DestinationUnloadResult` 型を `unloading.rs` に定義し、
セクション 3.4 に記載した 3 関数を `stockpile.rs` / `blueprint.rs` / `mixer.rs` に実装する。
`handle` 関数は match arm 呼び出しと `finish_partial_unload` のオーケストレーションのみに縮小する。

---

## 6. リスクと対策

| リスク | 影響度 | 対策 |
| --- | --- | --- |
| 公開 API の破壊 | 高 | root facade で `pub use` を必ず残し、クレート外パスを維持する |
| `use super::*` の汚染 | 中 | submodule は必要な型だけを明示 `use` する |
| 挙動の微妙な差分 | 中 | 既存テストをそのまま移し、分岐条件・演算子を変更しない |
| 分割しすぎ | 低 | root 1 本につき 2〜4 submodule に抑える（M4-B は別途判断） |
| ビルド不能期間の長期化 | 高 | 1 マイルストーン単位で `cargo check` を通してからコミットする |

## 7. 検証計画

```bash
# M1 後
CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check -p hw_ui

# M2 後
CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check -p hw_world

# M3 後
CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check -p hw_familiar_ai

# M4 後（最終）
CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check --workspace
```

追加で見る観点:

- `hw_ui`: placement の既存テスト（`door_requires_adjacent_wall_pair`, `structure_requires_site`, `moved_bucket_storage_allows_existing_owned_stockpile`）
- `hw_world`: room detection の既存 10 テスト（`test_closed_room_with_door` 等）
- `hw_familiar_ai`: `familiar_ai_state_system` の message emission 順序
- `hw_soul_ai`: `handle` の cancel / partial unload / completion 経路

## 8. ロールバック方針

- **単位**: 1 マイルストーン = 1 ロールバック単位（コミット粒度に合わせる）
- **手順**:
  1. `git revert <M-commit>` または追加 submodule を `git rm -r` で削除
  2. root ファイルへ元の実装を git checkout で戻す
  3. `CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check --workspace` を確認

## 9. AI 引継ぎメモ（最重要）

### 現在地

- 進捗: `100%`（全マイルストーン完了）
- 完了済みマイルストーン: M1, M2, M3, M4-A
- 未着手: M4-B(opt)（Phase A のみ完了で運用上問題なし）

### 次の AI が最初にやること

（実装完了済み。M4-B を着手する場合は下記を参照）

1. `handle` 関数の Stockpile / Blueprint / Mixer 分岐を `DestinationUnloadResult` 型と handler fn に切り出す。
2. `cargo check -p hw_soul_ai` を通してから workspace check を実施する。

### ブロッカー / 注意点

- `can_place_moved_building` は geometry 関数を呼ぶが、**validation の責務**（placement 可否判定）なので `validation.rs` に置く。`use super::geometry::*;` で解決する。
- `room_detection.rs` の `RoomDetectionState` は `dirty_tiles: HashSet<(i32, i32)>` と `RoomBounds` を持つため、`ecs.rs` は `core.rs` に依存する（`use super::core::RoomBounds;`）。循環はない。
- `emit_state_decision_messages` は `familiar_ai_state_system` 内から呼ばれる。`system.rs` が `use super::result::emit_state_decision_messages;` で取り込む（`pub(super)` で十分）。
- `unloading.rs` の inline match arm は **Phase A では触らない**。手を入れると cancel / partial unload の複雑なフローで挙動差分が生じるリスクがある。

### 参照必須ファイル

```
docs/README.md
docs/DEVELOPMENT.md
crates/hw_ui/src/selection/placement.rs
crates/hw_world/src/room_detection.rs
crates/hw_world/src/lib.rs                           ← room_detection の pub use パス確認
crates/hw_familiar_ai/src/familiar_ai/decide/state_decision.rs
crates/hw_soul_ai/src/…/haul_with_wheelbarrow/phases/unloading.rs
```

### 最終確認ログ

- 最終 `cargo check --workspace`: `2026-03-19` 実施、0 errors / 0 warnings で完了
- 未解決エラー: なし

### Definition of Done

- [x] M1: `placement.rs` が facade のみ（≤ 120 行）
- [x] M2: `room_detection.rs` が facade のみ
- [x] M3: `state_decision.rs` が facade のみ
- [x] M4-A: `unloading.rs` の private fn が submodule に移動
- [x] `cargo check --workspace` が成功
- [x] 影響ドキュメント（`docs/architecture.md` 等）が必要に応じて更新済み

## 10. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| `2026-03-18` | `Codex` | 初版作成 |
| `2026-03-18` | `Copilot` | 実ファイル精査に基づき全マイルストーンを具体化。行番号・シンボル一覧・pub use テンプレ・import chain・M4 Phase A/B 分離方針を追記 |
| `2026-03-19` | `Claude` | M3 result.rs の `determine_transition_reason` import パス修正（`super::path` → `crate::familiar_ai::perceive::state_detection`）、M3 `MessageWriter` 補足注記追加、M2 `RoomBounds` コメント誤帰属修正（RoomDetectionState → Room）、M1 tests.rs の `use super::super::*` → `use super::*` 修正 |
| `2026-03-19` | `Copilot` | M4-A 実装完了（`item_ops.rs` / `capacity.rs` / `finalize.rs`）。全マイルストーン完了、`cargo check --workspace` グリーン確認。ステータスを `Completed` に変更 |
