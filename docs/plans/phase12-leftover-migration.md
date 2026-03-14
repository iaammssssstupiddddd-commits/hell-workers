# フェーズ 1/2 取りこぼし移設計画

## 背景

フェーズ 3 調査の過程で、`GameAssets` 等の Root 固有型に依存せず、
leaf crate の型のみで完結しているにもかかわらず bevy_app に残留している
システムを 4 つ発見した。本計画でこれらを適切な leaf crate へ移設する。

---

## 対象一覧

| グループ | ファイル（bevy_app 内） | 移設先 | Cargo 変更 |
|:---|:---|:---|:---|
| A | `familiar_ai/execute/max_soul_apply.rs` | `hw_familiar_ai` | `hw_visual`, `hw_soul_ai` を追加 |
| A | `familiar_ai/execute/squad_apply.rs` | `hw_familiar_ai` | 同上 |
| B | `systems/room/detection.rs` | `hw_world` | 不要（既存依存で完結） |
| B | `systems/room/validation.rs` | `hw_world` | 不要（同上） |

---

## グループ A — hw_familiar_ai への移設

### 対象システム

| ファイル | 関数名 | 責務 |
|:---|:---|:---|
| `familiar_ai/execute/max_soul_apply.rs` | `handle_max_soul_changed_system` | 使役数上限減少時に超過 Soul をリリース |
| `familiar_ai/execute/squad_apply.rs` | `apply_squad_management_requests_system` | `SquadManagementRequest` の AddMember / ReleaseMember を適用 |

### 依存分析

全依存型が leaf crate で定義されており、Root 固有型（`GameAssets` 等）は不使用。

**`max_soul_apply.rs` の import 変換表**

| `use` パス（bevy_app 現在） | 実体 | 移設後パス |
|:---|:---|:---|
| `crate::entities::damned_soul::{DamnedSoul, Path}` | `hw_core::soul` | `hw_core::soul::{DamnedSoul, Path}` |
| `crate::entities::familiar::{Familiar, FamiliarVoice}` | `hw_core::familiar` / `hw_visual::speech` | `hw_core::familiar::Familiar` / `hw_visual::speech::FamiliarVoice` |
| `crate::events::FamiliarOperationMaxSoulChangedEvent` | `hw_core::events` | `hw_core::events::FamiliarOperationMaxSoulChangedEvent` |
| `hw_core::relationships::{CommandedBy, Commanding}` | `hw_core` | そのまま |
| `crate::systems::soul_ai::execute::task_execution::AssignedTask` | `hw_jobs`（hw_soul_ai は re-export） | `hw_jobs::AssignedTask` |
| `crate::systems::soul_ai::execute::task_execution::context::TaskAssignmentQueries` | `hw_soul_ai::soul_ai::execute::task_execution` | `hw_soul_ai::soul_ai::execute::task_execution::TaskAssignmentQueries` |
| `crate::systems::soul_ai::helpers::work::unassign_task` | `hw_soul_ai::soul_ai::helpers::work` | `hw_soul_ai::soul_ai::helpers::work::unassign_task` |
| `crate::systems::logistics::Inventory` | `hw_logistics` | `hw_logistics::Inventory` |
| `crate::world::map::WorldMapRead` | `hw_world` | `hw_world::WorldMapRead` |
| `hw_visual::speech::*` | `hw_visual` | そのまま |

> `MessageReader` は `use bevy::prelude::*` に含まれるため変換不要。
> `AssignedTask` は `hw_soul_ai::soul_ai::execute::task_execution::types` が
> `pub use hw_jobs::assigned_task::*` をしているため、`hw_jobs::AssignedTask` が最短パス。

**`squad_apply.rs` の import 変換表**

| `use` パス（bevy_app 現在） | 実体 | 移設後パス |
|:---|:---|:---|
| `crate::entities::familiar::{Familiar, FamiliarVoice}` | `hw_core::familiar` / `hw_visual::speech` | `hw_core::familiar::Familiar` / `hw_visual::speech::FamiliarVoice` |
| `crate::events::{ReleaseReason, SquadManagementOperation, SquadManagementRequest}` | `hw_core::events` | `hw_core::events::{ReleaseReason, SquadManagementOperation, SquadManagementRequest}` |
| `crate::events::{OnGatheringLeft, OnSoulRecruited, OnReleasedFromService}` | `hw_core::events` | `hw_core::events::{OnGatheringLeft, OnSoulRecruited, OnReleasedFromService}` |
| `hw_core::relationships::{CommandedBy, ParticipatingIn}` | `hw_core` | そのまま |
| `crate::systems::familiar_ai::FamiliarSoulQuery` | `hw_familiar_ai`（re-export） | `crate::familiar_ai::decide::query_types::FamiliarSoulQuery` |
| `crate::systems::soul_ai::execute::task_execution::context::TaskAssignmentQueries` | `hw_soul_ai::soul_ai::execute::task_execution` | `hw_soul_ai::soul_ai::execute::task_execution::TaskAssignmentQueries` |
| `crate::systems::soul_ai::helpers::work::unassign_task` | `hw_soul_ai::soul_ai::helpers::work` | `hw_soul_ai::soul_ai::helpers::work::unassign_task` |
| `crate::world::map::WorldMapRead` | `hw_world` | `hw_world::WorldMapRead` |
| `hw_visual::speech::*` / `hw_visual::SpeechHandles` | `hw_visual` | そのまま |

> `squad_apply.rs` の `queries.designation.targets.get(e)` は
> `TaskAssignmentQueries` の `designation` フィールド（`DesignationAccess`）経由。
> hw_soul_ai に TaskAssignmentQueries が定義されていることを確認済み。

### 循環依存チェック

```
追加依存:
  hw_familiar_ai → hw_visual  (hw_visual は hw_familiar_ai に依存しない ✓)
  hw_familiar_ai → hw_soul_ai (hw_soul_ai は hw_familiar_ai に依存しない ✓)
```

### 実装手順

#### ステップ A-1: `hw_familiar_ai/Cargo.toml` に依存追加

```toml
[dependencies]
# 追加（hw_core, hw_jobs, hw_logistics, hw_world, hw_spatial は既存）
hw_visual  = { path = "../hw_visual" }
hw_soul_ai = { path = "../hw_soul_ai" }
```

#### ステップ A-2: hw_familiar_ai 側にファイル作成

- `crates/hw_familiar_ai/src/familiar_ai/execute/max_soul_apply.rs`
  - bevy_app 版をコピーし、**max_soul_apply 変換表**に従い `use` パスを書き換える
- `crates/hw_familiar_ai/src/familiar_ai/execute/squad_apply.rs`
  - bevy_app 版をコピーし、**squad_apply 変換表**に従い `use` パスを書き換える

#### ステップ A-3: hw_familiar_ai の execute/mod.rs に追加

現在の `crates/hw_familiar_ai/src/familiar_ai/execute/mod.rs`：
```rust
pub mod encouragement_apply;
pub mod state_apply;
pub mod state_log;
```

以下の 2 行を追加：
```rust
pub mod max_soul_apply;
pub mod squad_apply;
```

#### ステップ A-4: FamiliarAiCorePlugin への登録移設

`crates/hw_familiar_ai/src/familiar_ai/mod.rs` の `FamiliarAiCorePlugin::build()` の
Execute フェーズ登録ブロックに 2 システムを追加する：

```rust
.add_systems(
    Update,
    (
        execute::state_apply::familiar_state_apply_system,
        execute::state_log::handle_state_changed_system,
        // ↓ 追加
        execute::max_soul_apply::handle_max_soul_changed_system,
        execute::squad_apply::apply_squad_management_requests_system,
    )
        .in_set(FamiliarAiSystemSet::Execute),
);
```

#### ステップ A-5: bevy_app 側のファイル内容を re-export に置換

`crates/bevy_app/src/systems/familiar_ai/execute/max_soul_apply.rs` の内容を置換：

```rust
pub use hw_familiar_ai::familiar_ai::execute::max_soul_apply::*;
```

`crates/bevy_app/src/systems/familiar_ai/execute/squad_apply.rs` の内容を置換：

```rust
pub use hw_familiar_ai::familiar_ai::execute::squad_apply::*;
```

> `state_apply` / `state_log` がインラインモジュール形式の re-export になっている点と
> 一貫性を持たせるため、**ファイル形式（pub use ...::*）** で統一する。
> bevy_app の execute/mod.rs の `pub mod max_soul_apply;` 宣言は変更不要。

#### ステップ A-6: bevy_app の FamiliarAiPlugin から登録を削除

`crates/bevy_app/src/systems/familiar_ai/mod.rs` の `add_systems` Execute ブロックから
以下の 2 行を削除する：

```rust
execute::max_soul_apply::handle_max_soul_changed_system,   // 削除
execute::squad_apply::apply_squad_management_requests_system,  // 削除
```

> **注意**: `FamiliarAiCorePlugin`（hw_familiar_ai）が Execute フェーズで登録するため、
> bevy_app 側で二重登録すると Bevy 0.18 の schedule が panic する。

---

## グループ B — hw_world への移設

### 対象システム

| ファイル | 関数名 | 責務 |
|:---|:---|:---|
| `systems/room/detection.rs` | `detect_rooms_system`, `collect_building_tiles` | 建物タイルを収集し Room ECS エンティティを再構築 |
| `systems/room/validation.rs` | `validate_rooms_system` | 既存 Room の整合性を定期検証し、無効なものを再検出キューへ |

### 依存分析

| `use` パス（bevy_app 現在） | 実体 | 移設後パス |
|:---|:---|:---|
| `super::components::Room` | `hw_world`（lib.rs re-export） | `crate::room_detection::Room` |
| `super::resources::{RoomDetectionState, RoomTileLookup, RoomValidationState}` | `hw_world`（lib.rs re-export） | `crate::{RoomDetectionState, RoomTileLookup, RoomValidationState}` |
| `crate::systems::jobs::Building` | `hw_jobs` | `hw_jobs::Building` |
| `crate::world::map::WorldMapRead` | `hw_world` | `crate::WorldMapRead` |
| `crate::world::map::WorldMap::world_to_grid` | `hw_world::coords` | `crate::world_to_grid` （`hw_world::coords::world_to_grid` として lib.rs re-export 済み） |
| `hw_world::room_detection::{DetectedRoom, RoomDetectionBuildingTile, build_detection_input, detect_rooms}` | `hw_world` | `crate::room_detection::{...}` |
| `hw_world::room_detection::{RoomDetectionBuildingTile, build_detection_input, room_is_valid_against_input}` | `hw_world` | `crate::room_detection::{...}` |

hw_world は既に `hw_jobs` に依存しているため **Cargo.toml の変更は不要**。

### 実装手順

#### ステップ B-1: `hw_world` に新ファイル `room_systems.rs` を作成

> `crates/hw_world/src/room_detection.rs` は先頭コメントで「ECS system logic を持たない」と
> 明記されているため、ECS システム関数は **別ファイルに分離** する。

`crates/hw_world/src/room_systems.rs` を新規作成し、
bevy_app の `detection.rs`・`validation.rs` の内容を上記変換表に従いポーティングする。

ファイル冒頭の `use` は以下の形になる：
```rust
use bevy::prelude::*;
use hw_jobs::Building;
use crate::map::{WorldMap, WorldMapRead};
use crate::room_detection::{
    DetectedRoom, Room, RoomDetectionBuildingTile, RoomDetectionState, RoomTileLookup,
    RoomValidationState, build_detection_input, detect_rooms, room_is_valid_against_input,
};
use std::collections::HashMap;
```

> `WorldMap::world_to_grid` と `crate::world_to_grid`（coords.rs の standalone fn）は
> 同実装。ECS system ファイルでは `WorldMap::world_to_grid` の形を維持する。

また、`crates/hw_world/src/room_detection.rs` の以下のコメント（5 行目付近）を更新する：

```rust
// 変更前
//! Root systems are responsible for collecting [`RoomDetectionBuildingTile`]s from
//! queries and applying the resulting [`DetectedRoom`]s back to ECS state.

// 変更後
//! ECS system logic is in [`crate::room_systems`].
```

#### ステップ B-2: hw_world の lib.rs に追加

```rust
pub mod room_systems;
pub use room_systems::{detect_rooms_system, validate_rooms_system};
```

#### ステップ B-3: bevy_app 側のファイル内容を re-export に置換

`crates/bevy_app/src/systems/room/detection.rs` の内容を置換：

```rust
pub use hw_world::detect_rooms_system;
```

`crates/bevy_app/src/systems/room/validation.rs` の内容を置換：

```rust
pub use hw_world::validate_rooms_system;
```

> `collect_building_tiles` は private 関数（`pub` なし）のため re-export は不要。
> lib.rs の re-export は `detect_rooms_system`・`validate_rooms_system` のみで十分。

#### ステップ B-4: Plugin 登録はそのまま維持

`crates/bevy_app/src/plugins/logic.rs` の `detect_rooms_system` /
`validate_rooms_system` の登録は **変更しない**。

理由: hw_world には現時点で Plugin が存在しない。
`GameSystemSet` を使った登録を hw_world 側に移すには HwWorldPlugin の新設が必要で、
本タスクのスコープを超える。実装本体の移設のみで十分な整合性が得られる。

---

## 検証方法

```bash
CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check
```

エラーゼロ・警告ゼロで完了。

---

## 実施順序

1. **グループ B から着手**（Cargo.toml 変更なし・影響範囲が小さい）
   - B-1（room_systems.rs 新規作成）→ B-2（lib.rs）→ B-3（bevy_app re-export）→ B-4 → `cargo check`
2. **グループ A を実施**
   - A-1（Cargo.toml）→ A-2（ファイル作成・import 変換）→ A-3（execute/mod.rs）→ A-4（Plugin 登録）→ A-5（bevy_app re-export 置換）→ A-6（重複登録削除）→ `cargo check`
