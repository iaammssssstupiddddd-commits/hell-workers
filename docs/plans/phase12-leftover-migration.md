# フェーズ 1/2 取りこぼし移設計画

## 背景

フェーズ 3 調査の過程で、`GameAssets` 等の Root 固有型に依存せず、
leaf crate の型のみで完結しているにもかかわらず bevy_app に残留している
システムを 4 つ発見した。本計画でこれらを適切な leaf crate へ移設する。

---

## 対象一覧・進捗

| グループ | ファイル（bevy_app 内） | 移設先 | Cargo 変更 | 状態 |
|:---|:---|:---|:---|:---|
| A | `familiar_ai/execute/max_soul_apply.rs` | `hw_familiar_ai` + `hw_visual` | `hw_soul_ai` を追加済み | ✅ 完了 |
| A | `familiar_ai/execute/squad_apply.rs` | `hw_familiar_ai` + `hw_visual` | `hw_soul_ai` は追加済み | ✅ 完了 |
| B | `systems/room/detection.rs` | `hw_world` | 不要（既存依存で完結） | ✅ 完了 |
| B | `systems/room/validation.rs` | `hw_world` | 不要（同上） | ✅ 完了 |

---

## グループ A — hw_familiar_ai / hw_visual への移設

### 実装パターン（max_soul_apply で確立）

ロジックとビジュアルを **2 システムに分離**する方針で実施済み：

- **ロジック系** → `hw_familiar_ai`（タスク解除・ECS 操作）
- **ビジュアル系** → `hw_visual`（セリフバブル表示）

この分離により hw_familiar_ai に `hw_visual` 依存を追加せずに済む。

---

### max_soul_apply ✅ 完了

#### 実装内容

| システム | 実装場所 | 責務 |
|:---|:---|:---|
| `max_soul_logic_system` | `hw_familiar_ai::familiar_ai::execute::max_soul_logic` | 超過 Soul のタスク解除・CommandedBy 削除 |
| `max_soul_visual_system` | `hw_visual::speech::max_soul_visual` | "Abi" セリフバブル表示 |

bevy_app の `familiar_ai/execute/max_soul_apply.rs` は以下の re-export shell：

```rust
pub use hw_familiar_ai::max_soul_logic_system as handle_max_soul_changed_system;
pub use hw_visual::max_soul_visual_system;
```

両システムの登録は bevy_app の `FamiliarAiPlugin`（Execute フェーズ）が担当。

---

### squad_apply ✅ 完了

max_soul と同じ**ロジック／ビジュアル分離パターン**で実装する。

#### 分離方針

| システム名（案） | 実装場所 | 責務 |
|:---|:---|:---|
| `squad_logic_system` | `hw_familiar_ai::familiar_ai::execute::squad_logic` | AddMember/ReleaseMember の ECS 操作・イベント発火 |
| `squad_visual_system` | `hw_visual::speech::squad_visual` | Fatigued リリース時の "Abi" セリフバブル |

#### `squad_logic_system` の import 変換表

| `use` パス（bevy_app 現在） | 移設後パス |
|:---|:---|
| `crate::entities::familiar::Familiar` | `hw_core::familiar::Familiar` |
| `crate::events::{ReleaseReason, SquadManagementOperation, SquadManagementRequest}` | `hw_core::events::{ReleaseReason, SquadManagementOperation, SquadManagementRequest}` |
| `crate::events::{OnGatheringLeft, OnSoulRecruited, OnReleasedFromService}` | `hw_core::events::{OnGatheringLeft, OnSoulRecruited, OnReleasedFromService}` |
| `hw_core::relationships::{CommandedBy, ParticipatingIn}` | そのまま |
| `crate::systems::familiar_ai::FamiliarSoulQuery` | `crate::familiar_ai::decide::query_types::FamiliarSoulQuery` |
| `crate::systems::soul_ai::execute::task_execution::context::TaskAssignmentQueries` | `hw_soul_ai::soul_ai::execute::task_execution::TaskAssignmentQueries` |
| `crate::systems::soul_ai::helpers::work::unassign_task` | `hw_soul_ai::soul_ai::helpers::work::unassign_task` |
| `crate::world::map::WorldMapRead` | `hw_world::WorldMapRead` |
| `hw_visual::speech::*` / `hw_visual::SpeechHandles` | → `squad_visual_system` 側に分離 |

> `squad_apply.rs` の `queries.designation.targets.get(e)` は
> `TaskAssignmentQueries` の `designation` フィールド（`DesignationAccess`）経由。
> hw_soul_ai に TaskAssignmentQueries が定義されていることを確認済み。

#### `squad_visual_system` の依存（hw_visual 内で完結）

`hw_visual::speech` 内の既存型のみを使用するため、新規 Cargo 依存は不要。

#### 実装手順

**ステップ A-1**: `hw_familiar_ai/Cargo.toml` — すでに `hw_soul_ai` は追加済み。追加変更なし。

**ステップ A-2**: ファイル作成

- `crates/hw_familiar_ai/src/familiar_ai/execute/squad_logic.rs`
  - bevy_app の `squad_apply.rs` から ECS ロジック部分をポーティング
  - ビジュアル処理（Fatigued 分岐のセリフ）は除去し、`squad_visual_system` へ委譲
- `crates/hw_visual/src/speech/squad_visual.rs`
  - `max_soul_visual.rs` と同構造。`ReleaseReason::Fatigued` 時に "Abi" バブルを表示

**ステップ A-3**: モジュール登録

`crates/hw_familiar_ai/src/familiar_ai/execute/mod.rs` に追加：
```rust
pub mod squad_logic;
```

`crates/hw_visual/src/speech/mod.rs` に追加（`max_soul_visual` と同様の形式）：
```rust
pub mod squad_visual;
pub use squad_visual::squad_visual_system;
```

`crates/hw_visual/src/lib.rs` に追加：
```rust
pub use speech::squad_visual_system;
```

`crates/hw_familiar_ai/src/lib.rs` に追加：
```rust
pub use familiar_ai::execute::squad_logic::squad_logic_system;
```

**ステップ A-4**: bevy_app 側のファイル内容を re-export shell に置換

`crates/bevy_app/src/systems/familiar_ai/execute/squad_apply.rs` の内容を置換：

```rust
pub use hw_familiar_ai::squad_logic_system as apply_squad_management_requests_system;
pub use hw_visual::squad_visual_system;
```

**ステップ A-5**: bevy_app の `FamiliarAiPlugin` 登録を更新

`crates/bevy_app/src/systems/familiar_ai/mod.rs` の Execute ブロックを更新：

```rust
// 変更前
execute::squad_apply::apply_squad_management_requests_system,

// 変更後
execute::squad_apply::apply_squad_management_requests_system,  // squad_logic_system の alias
execute::squad_apply::squad_visual_system,
```

**ステップ A-6**: `cargo check` で確認

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
| `crate::world::map::WorldMap::world_to_grid` | `hw_world` | `crate::map::WorldMap::world_to_grid` |
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

## 完了

全 4 件の移設が完了し、`cargo check` エラーゼロを確認済み。
