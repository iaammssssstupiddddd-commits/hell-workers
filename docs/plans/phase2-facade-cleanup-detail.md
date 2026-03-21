# Phase 2 詳細実装計画: bevy_app ファサード整理

## 調査結果サマリー（Phase 1 計画からの修正）

当初計画の「pathfinding 重複」「wheelbarrow ロジック統合」は、実際のコードを確認した結果、
**重複ではなく責務が正しく分離されていた**。

| 当初想定 | 実際 |
|---------|------|
| pathfinding が3箇所に重複 | `hw_world`=A* コア、`hw_soul_ai`=ソウル固有オーケストレーション。適切分離 |
| wheelbarrow が3箇所に重複 | familiar_ai=決定ロジック、soul_ai=実行ロジック、bevy_app=薄いfacade（3行）|

**実際の改善対象**:
1. `hw_soul_ai::pathfinding::reuse.rs` に `find_path + adjacent fallback + grid→world変換` パターンが
   重複している → hw_world に便利関数を昇格
2. `bevy_app/src/systems/soul_ai/execute/task_execution/transport_common/` の5ファイルが
   **デッドコード**（bevy_app 内のどこからも利用されていない）
3. `bevy_app/src/systems/soul_ai/helpers/work.rs` が4箇所から利用されているが、
   `crate::systems::soul_ai::helpers::work::unassign_task` という長いパスを経由している

---

## Phase 2-A: `find_path_world_waypoints` を hw_world に昇格（小規模）

### 問題

`hw_soul_ai/src/soul_ai/pathfinding/reuse.rs` に以下のパターンが**2箇所**存在する：

```rust
find_path(world_map, pf_context, start_grid, goal_grid, PathGoalPolicy::RespectGoalWalkability)
    .or_else(|| find_path_to_adjacent(world_map, pf_context, start_grid, goal_grid, true))
    .map(|grid_path| {
        grid_path.iter().map(|&(x, y)| WorldMap::grid_to_world(x, y)).collect()
    })
```

### 対象ファイル

- 追加: `crates/hw_world/src/pathfinding.rs` （末尾に関数追加）
- 変更: `crates/hw_world/src/lib.rs` （pub use に追加）
- 変更: `crates/hw_soul_ai/src/soul_ai/pathfinding/reuse.rs` （2箇所を新関数で置換）

### 実装内容

#### `crates/hw_world/src/pathfinding.rs` — 末尾に追加

```rust
/// A* でパスを探索し、ワールド座標 waypoint 列として返す。
/// 直接到達不可なターゲットには隣接マスへの探索を fallback する。
///
/// `find_path` → `find_path_to_adjacent` の fallback + grid→world 変換を1関数に集約。
pub fn find_path_world_waypoints(
    world_map: &WorldMap,
    pf_context: &mut PathfindingContext,
    start_grid: (i32, i32),
    goal_grid: (i32, i32),
) -> Option<Vec<Vec2>> {
    find_path(
        world_map,
        pf_context,
        start_grid,
        goal_grid,
        PathGoalPolicy::RespectGoalWalkability,
    )
    .or_else(|| find_path_to_adjacent(world_map, pf_context, start_grid, goal_grid, true))
    .map(|grid_path| {
        grid_path
            .iter()
            .map(|&(x, y)| WorldMap::grid_to_world(x, y))
            .collect()
    })
}
```

#### `crates/hw_world/src/lib.rs` — pub use に追加

```rust
pub use pathfinding::{
    PathGoalPolicy, PathNode, PathWorld, PathfindingContext,
    can_reach_target, find_path, find_path_to_adjacent, find_path_to_boundary,
    find_path_world_waypoints,  // ← 追加
};
```

#### `crates/hw_soul_ai/src/soul_ai/pathfinding/reuse.rs`

**変更1** — `try_find_path_world_waypoints` 関数を削除し、`find_path_world_waypoints` を直接使用:
```rust
// use 行を変更
// Before:
use hw_world::{PathGoalPolicy, PathfindingContext, WorldMap, find_path, find_path_to_adjacent};
// After:
use hw_world::{PathGoalPolicy, PathfindingContext, WorldMap, find_path, find_path_to_adjacent, find_path_world_waypoints};

// try_find_path_world_waypoints 関数を丸ごと削除（hw_world の関数に置換）
```

**変更2** — `try_reuse_existing_path` 内の重複パターンを置換:
```rust
// Before:
if let Some(partial_grid_path) = find_path(
    world_map, pf_context, resume_grid, goal_grid,
    PathGoalPolicy::RespectGoalWalkability,
)
.or_else(|| find_path_to_adjacent(world_map, pf_context, resume_grid, goal_grid, true))
{
    let mut partial_world_path: Vec<Vec2> = partial_grid_path
        .iter()
        .map(|&(x, y)| WorldMap::grid_to_world(x, y))
        .collect();
    if partial_grid_path.first().copied() == Some(resume_grid)
        && !partial_world_path.is_empty()
    {
        partial_world_path.remove(0);
    }
    // ...
}

// After:
if let Some(mut partial_world_path) =
    find_path_world_waypoints(world_map, pf_context, resume_grid, goal_grid)
{
    // find_path_world_waypoints は Vec<Vec2> を返すため、
    // 先頭 waypoint の除去チェックはワールド座標で比較する
    let resume_world = WorldMap::grid_to_world(resume_grid.0, resume_grid.1);
    if partial_world_path.first().copied() == Some(resume_world)
        && !partial_world_path.is_empty()
    {
        partial_world_path.remove(0);
    }
    // ← 以降（path の truncate・extend・PathCooldown 除去）はそのまま
}
```

**変更3** — `try_find_path_world_waypoints` の呼び出し箇所を `find_path_world_waypoints` に変更:
- 呼び出し元: `hw_soul_ai/src/soul_ai/pathfinding/mod.rs`
- 変更: `reuse::try_find_path_world_waypoints(world_map, pf_context, start_grid, goal_grid, entity)` →
  `find_path_world_waypoints(world_map, pf_context, start_grid, goal_grid)`
- `entity` を使っていたデバッグログは直後に移動:
```rust
// Before（reuse.rs 内）:
//   find_path が失敗した直後の or_else クロージャ内でログを出力していた

// After（mod.rs 内の呼び出し箇所）:
let world_path = find_path_world_waypoints(world_map, pf_context, start_grid, goal_grid);
if world_path.is_none() {
    debug!(
        "PATH: Soul {:?} failed find_path and find_path_to_adjacent",
        entity
    );
}
if let Some(world_path) = world_path {
    // ...
}
```
> **注意**: 元のログは `find_path` が失敗した時点（`find_path_to_adjacent` の前）に出力されていたが、
> 移動後は**両方失敗した場合のみ**出力されるように変わる。ログの粒度が下がることを許容するなら削除でも可。

### 注意事項

- 変更2 のワールド座標比較: `WorldMap::grid_to_world(x, y)` は浮動小数点演算なので、
  A* が必ず resume_grid を先頭に返すことが保証されていれば比較は安全。
  不安な場合は比較ロジックを削除して無条件で先頭 waypoint を除去してもよい（パスは resume_grid から始まるため）。
- `entity` パラメータが不要になるため、`reuse.rs` の `use bevy::prelude::*` が不要になる可能性を確認する。

---

## Phase 2-B: `transport_common/` デッドコード削除（5ファイル）

### 問題

`bevy_app/src/systems/soul_ai/execute/task_execution/transport_common/` 以下の5ファイルは
`pub use hw_soul_ai::...` のみで構成されているが、**bevy_app 内のどのファイルからも
`crate::systems::soul_ai::execute::task_execution::transport_common::` パスで参照されていない**。
（調査コマンド: `grep -rn "transport_common::" crates/bevy_app/src --include="*.rs" | grep "use "` → 空）

### 対象ファイル（削除）

```
crates/bevy_app/src/systems/soul_ai/execute/task_execution/transport_common/
├── mod.rs        # pub mod cancel; pub mod reservation; pub mod sand_collect; pub mod wheelbarrow;
├── cancel.rs     # pub use hw_soul_ai::...cancel::{cancel_haul_to_blueprint, ...}
├── reservation.rs # pub use hw_soul_ai::...reservation::{record_picked_source, ...}
├── sand_collect.rs # pub use hw_soul_ai::...sand_collect::{...}
└── wheelbarrow.rs # pub use hw_soul_ai::...wheelbarrow::{park_wheelbarrow_entity, ...}
```

### 変更内容

#### ステップ1: 5ファイルを削除

```bash
rm -r crates/bevy_app/src/systems/soul_ai/execute/task_execution/transport_common/
```

#### ステップ2: 親 `mod.rs` から宣言を削除

**ファイル**: `crates/bevy_app/src/systems/soul_ai/execute/task_execution/mod.rs`

```rust
// 削除する行:
pub mod transport_common;
```

現在の `task_execution/mod.rs` 全体:
```rust
//! タスク実行モジュール — 実装は hw_soul_ai に移設済み。

pub mod common {
    pub use hw_soul_ai::soul_ai::execute::task_execution::common::*;
}
pub mod context;
pub mod handler {
    pub use hw_soul_ai::soul_ai::execute::task_execution::handler::{
        TaskHandler, dispatch::execute_haul_with_wheelbarrow, dispatch::run_task_handler,
    };
}
pub mod move_plant {
    pub use hw_soul_ai::soul_ai::execute::task_execution::move_plant::*;
}
pub mod transport_common;  // ← この行を削除
pub mod types {
    pub use hw_soul_ai::soul_ai::execute::task_execution::types::*;
}

pub use types::AssignedTask;
pub use hw_soul_ai::soul_ai::execute::task_assignment_apply::apply_task_assignment_requests_system;
pub use hw_soul_ai::soul_ai::execute::task_execution_system::task_execution_system;
```

---

## Phase 2-C: `soul_ai/helpers/work.rs` facade の整理

### 問題

`crates/bevy_app/src/systems/soul_ai/helpers/work.rs` は `hw_soul_ai` から
`unassign_task`, `cleanup_task_assignment`, `is_soul_available_for_work` を re-export するだけの
facade ファイルだが、4箇所から `crate::systems::soul_ai::helpers::work::unassign_task` という
長いパスで利用されている。

### 対象ファイル

- 変更: `crates/hw_soul_ai/src/lib.rs`（re-export 追加）
- 変更（4箇所）: bevy_app 内の callers
  - `crates/bevy_app/src/entities/damned_soul/observers.rs`
  - `crates/bevy_app/src/interface/selection/building_move/mod.rs`
  - `crates/bevy_app/src/systems/jobs/floor_construction/cancellation.rs`
  - `crates/bevy_app/src/systems/jobs/wall_construction/cancellation.rs`
- 削除: `crates/bevy_app/src/systems/soul_ai/helpers/work.rs`
- 削除: `crates/bevy_app/src/systems/soul_ai/helpers/mod.rs` （work.rs のみなので空になる）
- 変更: `crates/bevy_app/src/systems/soul_ai/mod.rs`（`pub mod helpers;` を削除）

### 実装内容

#### ステップ1: `hw_soul_ai/src/lib.rs` に re-export を追加

```rust
// 現在:
pub mod soul_ai;
pub use soul_ai::SoulAiCorePlugin;
pub use soul_ai::decide::drifting::{DriftingDecisionTimer, drifting_decision_system};

// 追加:
pub use soul_ai::helpers::work::{cleanup_task_assignment, is_soul_available_for_work, unassign_task};
```

#### ステップ2: 4 callers を更新

**`entities/damned_soul/observers.rs`**:
```rust
// Before:
use crate::systems::soul_ai::helpers::work::unassign_task;
// After:
use hw_soul_ai::unassign_task;
```

**`interface/selection/building_move/mod.rs`**:
```rust
// Before:
use crate::systems::soul_ai::helpers::work::unassign_task;
// After:
use hw_soul_ai::unassign_task;
```

**`systems/jobs/floor_construction/cancellation.rs`**:
```rust
// Before:
use crate::systems::soul_ai::helpers::work::unassign_task;
// After:
use hw_soul_ai::unassign_task;
```

**`systems/jobs/wall_construction/cancellation.rs`**:
```rust
// Before:
use crate::systems::soul_ai::helpers::work::unassign_task;
// After:
use hw_soul_ai::unassign_task;
```

#### ステップ3: facade ファイルを削除

```bash
rm crates/bevy_app/src/systems/soul_ai/helpers/work.rs
rm crates/bevy_app/src/systems/soul_ai/helpers/mod.rs
```

#### ステップ4: `soul_ai/mod.rs` から `helpers` 宣言を削除

```rust
// 削除する行:
pub mod helpers;
```

---

## 実施手順

```
Phase 2-A → cargo check
Phase 2-B → cargo check
Phase 2-C → cargo check
```

各フェーズは独立しているので順序不問（ただし 2-A → 2-B → 2-C が自然）。

## 検証コマンド

```bash
CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check
```

## 変更ファイル一覧

### Phase 2-A
| 操作 | ファイル |
|------|---------|
| 追加 | `crates/hw_world/src/pathfinding.rs` (末尾に `find_path_world_waypoints` 追加) |
| 変更 | `crates/hw_world/src/lib.rs` (pub use に追加) |
| 変更 | `crates/hw_soul_ai/src/soul_ai/pathfinding/reuse.rs` (`try_find_path_world_waypoints` 削除 + 置換) |
| 変更 | `crates/hw_soul_ai/src/soul_ai/pathfinding/mod.rs` (呼び出し箇所を更新) |

### Phase 2-B
| 操作 | ファイル |
|------|---------|
| 削除 | `crates/bevy_app/src/systems/soul_ai/execute/task_execution/transport_common/mod.rs` |
| 削除 | `crates/bevy_app/src/systems/soul_ai/execute/task_execution/transport_common/cancel.rs` |
| 削除 | `crates/bevy_app/src/systems/soul_ai/execute/task_execution/transport_common/reservation.rs` |
| 削除 | `crates/bevy_app/src/systems/soul_ai/execute/task_execution/transport_common/sand_collect.rs` |
| 削除 | `crates/bevy_app/src/systems/soul_ai/execute/task_execution/transport_common/wheelbarrow.rs` |
| 変更 | `crates/bevy_app/src/systems/soul_ai/execute/task_execution/mod.rs` (`pub mod transport_common;` 削除) |

### Phase 2-C
| 操作 | ファイル |
|------|---------|
| 変更 | `crates/hw_soul_ai/src/lib.rs` (work helpers を pub use 追加) |
| 変更 | `crates/bevy_app/src/entities/damned_soul/observers.rs` |
| 変更 | `crates/bevy_app/src/interface/selection/building_move/mod.rs` |
| 変更 | `crates/bevy_app/src/systems/jobs/floor_construction/cancellation.rs` |
| 変更 | `crates/bevy_app/src/systems/jobs/wall_construction/cancellation.rs` |
| 削除 | `crates/bevy_app/src/systems/soul_ai/helpers/work.rs` |
| 削除 | `crates/bevy_app/src/systems/soul_ai/helpers/mod.rs` |
| 変更 | `crates/bevy_app/src/systems/soul_ai/mod.rs` (`pub mod helpers;` 削除) |
