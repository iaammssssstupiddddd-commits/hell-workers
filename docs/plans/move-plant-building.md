# Plant 建物移動タスク 実装計画（詳細版）

> 注記: 本計画は初期ドラフトです。現行実装はこれより進んでおり、
> `MovePlantTask`/`MovePlanned`/移動先予約障害物/Taskキャンセル/Tank companion 再指定（2段階入力）まで反映済みです。
> 仕様確認は `docs/building.md`（Plant 建物移動）と `docs/state.md`（BuildingMove）を優先してください。

## 現状（実装済み）

| 機能 | ファイル | 状態 |
|---|---|---|
| Move ボタン UI（ホバー表示） | `src/interface/ui/interaction/hover_action.rs` | ✅ 完了 |
| `PlayMode::BuildingMove` / `MoveContext` | `src/game_state.rs` | ✅ 完了 |
| `move_plant_building_action_system` | `src/interface/ui/interaction/mod.rs` | ✅ 完了 |
| `building_move_system`（クリック受付のみ） | `src/interface/selection/building_move/mod.rs` | ✅ 骨格のみ |
| タスク発行・Soul AI 実行・WorldMap 更新 | — | ❌ 未実装 |

---

## Step 1: データ型定義

**ファイル**: `src/systems/soul_ai/execute/task_execution/types.rs`

`AssignedTask` に struct variant を追加（既存パターンに準拠）:

```rust
#[derive(Component, Reflect, Clone, Debug, Default)]
#[reflect(Component)]
pub enum AssignedTask {
    // ... 既存バリアント ...
    MovePlant(MovePlantData),   // ← 追加
}
```

データ構造:

```rust
#[derive(Reflect, Clone, Debug)]
pub struct MovePlantData {
    pub building: Entity,
    pub destination_grid: IVec2,   // WorldMap::world_to_grid() の戻り値型に合わせる
    pub destination_pos: Vec2,     // grid_to_world(destination_grid)
    pub phase: MovePlantPhase,
}

#[derive(Reflect, Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum MovePlantPhase {
    #[default]
    GoToBuilding,   // 建物の隣接グリッドへ移動
    Moving,         // 建物の Transform を目的地へ更新
    Done,
}
```

`AssignedTask` のメソッドに追加（`get_target_entity()` など。既存バリアントの実装パターンを踏襲する）。

---

## Step 2: 配置チェックと WorldMap グリッド

### 2-1. 建物サイズの取得

`building_spawn_pos` 関数（`src/interface/selection/placement_common.rs` あたり）か
`BuildingType` に関連する既存のグリッドサイズ情報を確認して使用する。

Tank・MudMixer のサイズを確認:

```bash
grep -rn "Tank\|MudMixer\|building_size\|spawn_pos" src/interface/selection/ src/systems/jobs/
```

### 2-2. WorldMap への空きチェックと更新

```rust
// 移動前: 移動元グリッドの通行可能設定（建物を撤去）
world_map.set_passable_rect(old_grid, building_size);

// 移動後: 移動先グリッドに建物を配置
world_map.set_impassable_rect(new_grid, building_size);
```

`WorldMap` に `set_passable_rect` / `set_impassable_rect` が存在しない場合は追加する。
または既存の `world_map.tiles` フィールドを直接操作している箇所を参考にする:

```bash
grep -rn "world_map\|WorldMap" src/systems/jobs/ src/interface/selection/blueprint*
```

---

## Step 3: タスク発行（`building_move_system` の完成）

**ファイル**: `src/interface/selection/building_move/mod.rs`

現在の TODO 部分を実装:

```rust
use bevy::prelude::*;
use crate::game_state::{MoveContext, PlayMode};
use crate::interface::camera::MainCamera;
use crate::interface::ui::UiInputState;
use crate::systems::jobs::Building;
use crate::systems::soul_ai::execute::task_execution::types::{AssignedTask, MovePlantData, MovePlantPhase};
use crate::world::map::WorldMap;

pub fn building_move_system(
    buttons: Res<ButtonInput<MouseButton>>,
    q_window: Query<&Window, With<bevy::window::PrimaryWindow>>,
    q_camera: Query<(&Camera, &GlobalTransform), With<MainCamera>>,
    ui_input_state: Res<UiInputState>,
    mut world_map: ResMut<WorldMap>,
    mut move_context: ResMut<MoveContext>,
    mut next_play_mode: ResMut<NextState<PlayMode>>,
    q_buildings: Query<(&Building, &Transform)>,
    mut commands: Commands,
) {
    if ui_input_state.pointer_over_ui { return; }

    if buttons.just_pressed(MouseButton::Right) {
        move_context.0 = None;
        next_play_mode.set(PlayMode::Normal);
        return;
    }

    if !buttons.just_pressed(MouseButton::Left) { return; }

    let Some(world_pos) = crate::interface::selection::placement_common::world_cursor_pos(&q_window, &q_camera) else {
        return;
    };
    let destination_grid = WorldMap::world_to_grid(world_pos);

    let Some(target_entity) = move_context.0 else { return; };

    let Ok((_building, _transform)) = q_buildings.get(target_entity) else {
        move_context.0 = None;
        next_play_mode.set(PlayMode::Normal);
        return;
    };

    // TODO: 配置可能チェック（building_size 分グリッドが空きか）
    // if !world_map.can_place_at(destination_grid, building_size) { ... }

    let destination_pos = WorldMap::grid_to_world(destination_grid);

    commands.spawn(AssignedTask::MovePlant(MovePlantData {
        building: target_entity,
        destination_grid,
        destination_pos,
        phase: MovePlantPhase::GoToBuilding,
    }));

    move_context.0 = None;
    next_play_mode.set(PlayMode::Normal);
}
```

**注意**: `AssignedTask` を spawn するだけでは魂へ割り当てられない。Step 5 の Familiar AI 連携も必要。

---

## Step 4: タスクハンドラー実装

**新規ファイル**: `src/systems/soul_ai/execute/task_execution/move_plant.rs`

既存の `build.rs` ハンドラーのパターンを参考に実装:

```rust
use super::types::{AssignedTask, MovePlantData, MovePlantPhase};
use super::context::TaskExecutionContext;
use super::common::clear_task_and_path;
use crate::constants::TILE_SIZE;
use crate::world::map::WorldMap;
use bevy::prelude::*;

pub fn handle_move_plant_task(
    ctx: &mut TaskExecutionContext,
    data: MovePlantData,
    q_building_transforms: &mut Query<&mut Transform, With<crate::systems::jobs::Building>>,
    world_map: &mut ResMut<WorldMap>,
) {
    match data.phase {
        MovePlantPhase::GoToBuilding => {
            // 建物の現在位置（目的地ではなく建物に隣接）を目標にする
            let Ok((_, building_transform)) = ctx.queries.storage.buildings.get(data.building) else {
                clear_task_and_path(ctx.task, ctx.path);
                return;
            };
            let building_pos = building_transform.translation.truncate();

            // 建物への隣接チェック（既存ヘルパーを使用）
            let soul_pos = ctx.soul_transform.translation.truncate();
            if soul_pos.distance(building_pos) < TILE_SIZE * 1.5 {
                // 到着: Moving フェーズへ
                *ctx.task = AssignedTask::MovePlant(MovePlantData {
                    phase: MovePlantPhase::Moving,
                    ..data
                });
                ctx.dest.0 = soul_pos; // 移動停止
            } else {
                ctx.dest.0 = building_pos; // 建物へ移動
            }
        }

        MovePlantPhase::Moving => {
            // 建物 Transform を目的地へ更新
            if let Ok(mut building_transform) = q_building_transforms.get_mut(data.building) {
                building_transform.translation.x = data.destination_pos.x;
                building_transform.translation.y = data.destination_pos.y;
                // TODO: WorldMap コリジョン更新（移動元解放、移動先設定）
            }
            *ctx.task = AssignedTask::MovePlant(MovePlantData {
                phase: MovePlantPhase::Done,
                ..data
            });
        }

        MovePlantPhase::Done => {
            clear_task_and_path(ctx.task, ctx.path);
        }
    }
}
```

**`TaskExecutionContext` への Query 追加が必要な場合**:

`src/systems/soul_ai/execute/task_execution/context.rs` の `TaskQueries` に
`buildings_mut: Query<&mut Transform, With<Building>>` を追加するか、
`handle_move_plant_task` の引数として渡す（既存パターンを確認して選ぶ）。

---

## Step 5: Dispatch への登録

**ファイル**: `src/systems/soul_ai/execute/task_execution/handler/dispatch.rs`

`run_task_handler` の match に追加:

```rust
AssignedTask::MovePlant(data) => {
    move_plant::handle_move_plant_task(ctx, data, &mut queries.storage.buildings_mut, world_map);
}
```

**ファイル**: `src/systems/soul_ai/execute/task_execution/mod.rs`

```rust
pub mod move_plant;
```

---

## Step 6: Familiar AI によるタスク割り当て

`building_move_system` でスポーンした `AssignedTask` を魂へ割り当てる仕組みを確認する。

```bash
grep -rn "Designation\|assign_task\|AssignedTask" src/systems/soul_ai/ | head -30
```

既存の `Designation` コンポーネント経由ならば:

```rust
commands.spawn((
    AssignedTask::MovePlant(MovePlantData { ... }),
    Designation,
));
```

---

## Step 7: 移動前の関連タスクキャンセル（Phase 2 対応）

建物が移動する前に、その建物を対象にした既存タスク（納品、生産）をキャンセルする必要がある。

```bash
# 既存のタスクキャンセルパターンを確認
grep -rn "cancel\|Cancel\|AbortTask\|clear_task" src/systems/soul_ai/
```

既存のキャンセル機能があればそれを利用。なければ `WorkingOn` 関係を持つ魂の `AssignedTask` を `None` に設定する。

---

## 変更ファイル一覧

| ファイル | 変更内容 |
|---|---|
| `src/systems/soul_ai/execute/task_execution/types.rs` | `MovePlantData`, `MovePlantPhase`, `AssignedTask::MovePlant` 追加 |
| `src/systems/soul_ai/execute/task_execution/move_plant.rs` | **新規** — `handle_move_plant_task` |
| `src/systems/soul_ai/execute/task_execution/handler/dispatch.rs` | `MovePlant` アーム追加 |
| `src/systems/soul_ai/execute/task_execution/mod.rs` | `pub mod move_plant;` |
| `src/interface/selection/building_move/mod.rs` | `commands.spawn(AssignedTask::MovePlant(...))` |
| `src/world/map.rs` | `can_place_at()` / `move_building()` ヘルパー追加（必要なら） |

---

## 実装順序

1. **types.rs** — データ型定義（他のステップのコンパイルに必要）
2. **mod.rs** — `pub mod move_plant;`
3. **move_plant.rs** — ハンドラー骨格（まず `GoToBuilding` のみ実装してコンパイル確認）
4. **dispatch.rs** — dispatch に追加
5. **building_move/mod.rs** — タスク spawn 追加
6. **cargo check** — ここで一度確認
7. **Familiar AI 連携**、**WorldMap 更新**、**タスクキャンセル** を追加

---

## 調査事項（実装前に必ず確認）

```bash
# 1. AssignedTask のメソッド群（work_type, get_target_entity）
grep -n "fn work_type\|fn get_target_entity\|impl AssignedTask" src/systems/soul_ai/execute/task_execution/types.rs

# 2. TaskQueries の構造（builders_mut が追加可能か）
cat src/systems/soul_ai/execute/task_execution/context.rs

# 3. Designation フロー（どこで AssignedTask を魂に割り当てるか）
grep -rn "Designation" src/systems/soul_ai/ | head -20

# 4. 建物サイズ情報（Tank / MudMixer のグリッドサイズ）
grep -rn "Tank\|MudMixer\|building_size\|grid_size" src/systems/jobs/ src/interface/selection/

# 5. WorldMap のコリジョン更新パターン
grep -rn "set_impassable\|set_passable\|world_map\." src/systems/jobs/ src/world/
```
