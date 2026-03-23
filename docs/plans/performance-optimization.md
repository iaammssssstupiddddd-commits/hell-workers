# パフォーマンス最適化計画

## 問題の概要

分析の結果、フレームごとの不要なメモリアロケーション、到達判定の重複探索、過剰な逐次実行候補が複数確認された。
大半はアーキテクチャの全面変更を要するものではなく、既存 API を活かした局所最適化で改善できる見込みがある。

## 対象ボトルネック（優先度順）

| 優先度 | 内容 | ファイル | 推定改善 |
|:---:|:---|:---|:---:|
| P1 | `find_path_to_boundary` の HashSet 毎回アロケーション | `hw_world/src/pathfinding.rs:297` | 5-10% |
| P2 | 残存する `get_nearby_in_radius` 呼び出し側の Vec 毎回アロケーション | `hw_soul_ai/src/**`, `hw_familiar_ai/src/**`, `hw_visual/src/**`, `hw_logistics/src/**` | 5-12% |
| P3 | `find_owner_yard` の intermediate Vec + sort | `hw_logistics/src/transport_request/producer/mod.rs:52` | 5% |
| P4 | `can_reach_target` での A* 二重実行 | `hw_world/src/pathfinding.rs:265` | 3-5% |
| P5 | `logic.rs` の `.chain()` 再評価と独立システムの切り出し | `bevy_app/src/plugins/logic.rs:67-96` | 要計測 |

---

## 各修正の詳細

### P1: PathfindingContext に HashSet フィールドを追加

**現状** (`crates/hw_world/src/pathfinding.rs:37-54` / `:297`):

```rust
// 構造体（現状）
pub struct PathfindingContext {
    pub g_scores: Vec<i32>,
    pub came_from: Vec<Option<usize>>,
    pub open_set: BinaryHeap<PathNode>,
    visited: Vec<usize>,
    // ← target_grid_set が無い
}

// find_path_to_boundary 内（line 297）— 毎回新規確保
let target_grid_set: HashSet<(i32, i32)> = target_grids.iter().copied().collect();
```

`PathfindingContext` は `reset()` で既存バッファを再利用する設計だが、
`target_grid_set` だけフィールド化されておらず毎回アロケートしている。

**対応**:

```rust
// 構造体に追加
pub struct PathfindingContext {
    // ... 既存フィールド ...
    pub target_grid_set: HashSet<(i32, i32)>,  // 追加
}

// Default impl に追加
target_grid_set: HashSet::with_capacity(64),

// find_path_to_boundary の冒頭で作業用 set を一時的に取り出す
let mut target_grid_set = std::mem::take(&mut context.target_grid_set);
target_grid_set.clear();
target_grid_set.extend(target_grids.iter().copied());

let result = find_path_with_policy(
    world_map,
    context,
    start_idx,
    heuristic,
    |pos| target_grid_set.contains(&pos),
    // ...
);

// 呼び出し後に context へ戻す
context.target_grid_set = target_grid_set;
```

**注意**: `&context.target_grid_set` を保持したまま `context` を `&mut` で
`find_path_with_policy` に渡すと borrow conflict になるため、set を一時的に
ローカル変数へ退避してから使う。

**変更ファイル**:
- `crates/hw_world/src/pathfinding.rs`（構造体・Default impl・`find_path_to_boundary` 本体）

---

### P2: 残存する `get_nearby_in_radius` 呼び出し側を `_into` ベースへ置換

**現状**:
- `get_nearby_in_radius_into()` は `SpatialGridOps`（`hw_world/src/spatial.rs:9`）と
  各 spatial ラッパー（`soul.rs`, `familiar.rs` など）にすでに実装済み
- `source_selector.rs:89, 329` など一部はすでに `_into` を使用済み
- ただし以下の **19 箇所** がまだ毎回 `Vec<Entity>` を確保している

| ファイル | 行 | ホット度 |
|:---|:---:|:---:|
| `hw_soul_ai/src/soul_ai/perceive/escaping.rs` | 62, 138, 229 | 🔥 全Soul×フレーム |
| `hw_soul_ai/src/soul_ai/update/vitals_influence.rs` | 42 | 🔥 全Soul×フレーム |
| `hw_soul_ai/src/soul_ai/helpers/gathering_motion.rs` | 60, 73, 103 | 🔥 集合中Soul×フレーム |
| `hw_soul_ai/src/soul_ai/helpers/gathering_positions.rs` | 34, 65 | 中 |
| `hw_soul_ai/src/soul_ai/execute/gathering_spawn.rs` | 51, 56 | 中 |
| `hw_soul_ai/src/soul_ai/decide/idle_behavior/system.rs` | 332 | 🔥 全Soul×フレーム |
| `hw_soul_ai/src/soul_ai/decide/idle_behavior/motion_dispatch.rs` | 150 | 🔥 全Soul×フレーム |
| `hw_soul_ai/src/soul_ai/decide/separation.rs` | 60 | 🔥 全Soul×フレーム |
| `hw_soul_ai/src/soul_ai/decide/gathering_mgmt.rs` | 164 | 中 |
| `hw_familiar_ai/src/familiar_ai/decide/encouragement.rs` | 59 | 中 |
| `hw_familiar_ai/src/familiar_ai/decide/recruitment.rs` | 149, 165 | 中 |
| `hw_logistics/src/transport_request/producer/mod.rs` | 110 | 中 |
| `hw_visual/src/speech/conversation/systems.rs` | 43 | 低 |
| `hw_visual/src/soul/idle.rs` | 82 | 低 |

**対応**:
1. Bevy system では `Local<Vec<Entity>>` を引数に追加してフレーム間で再利用する
2. system 外の helper 関数では `&mut Vec<Entity>` を引数で受ける形に変更する
3. 低頻度・可読性優先の箇所（visual 系）は無理に置換しない

```rust
// 変更前（system 関数）
let nearby = soul_grid.get_nearby_in_radius(soul_pos, search_radius);

// 変更後（system 引数に mut nearby_buf: Local<Vec<Entity>> を追加）
soul_grid.get_nearby_in_radius_into(soul_pos, search_radius, &mut nearby_buf);
// 以降 &*nearby_buf で参照（or nearby_buf.iter() で反復）
```

**変更ファイル**:
- 上表の 14 ファイル（ホット度 🔥 を優先。visual 系 2 ファイルは任意）

---

### P3: `find_owner_yard` を単一パスイテレータに変更

**現状** (`crates/hw_logistics/src/transport_request/producer/mod.rs:52-65`):

```rust
// 中間 Vec に collect → sort_by → first という3ステップ
let mut candidates: Vec<&(Entity, Yard)> = yards
    .iter()
    .filter(|(_, yard)| yard.contains(pos))
    .collect();
if candidates.is_empty() { return None; }
candidates.sort_by(|(_, yard_a), (_, yard_b)| {
    let da = (yard_a.min.distance_squared(pos) + yard_a.max.distance_squared(pos))
        .partial_cmp(&(yard_b.min.distance_squared(pos) + yard_b.max.distance_squared(pos)))
        .unwrap_or(std::cmp::Ordering::Equal);
    da
});
candidates.first().map(|(entity, yard)| (*entity, yard))
```

同ファイルの `find_owner`（line 39-49）は既に `min_by()` 単一パスで実装済み。

**対応**: `find_owner_yard` を同じパターンに統一し、
`find_owner_for_position` 内の `candidates: Vec<_>` も合わせて `min_by()` に変更する。

```rust
pub fn find_owner_yard(pos: Vec2, yards: &[(Entity, Yard)]) -> Option<(Entity, &Yard)> {
    yards
        .iter()
        .filter(|(_, yard)| yard.contains(pos))
        .min_by(|(_, a), (_, b)| {
            let da = a.min.distance_squared(pos) + a.max.distance_squared(pos);
            let db = b.min.distance_squared(pos) + b.max.distance_squared(pos);
            da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|(entity, yard)| (*entity, yard))
}
```

`find_owner_for_position`（line 75-87）の `candidates.collect()` → `min_by()` も同時に修正する。

**変更ファイル**:
- `crates/hw_logistics/src/transport_request/producer/mod.rs`

---

### P4: `can_reach_target` の重複 A* を削減

**現状**:
- `target_walkable = true` の場合、`find_path` と `find_path_to_adjacent` の両方を実行する可能性がある
- `assignment_loop.rs` の呼び出しは `reachable_with_cache`（`HashMap<ReachabilityCacheKey, bool>` キャッシュ付き）でラップ済みのため、同一フレーム内の重複 A* はキャッシュで保護されている
- `assign_task_system.rs:82` の呼び出しはユーザー操作時のみ（毎フレームではない）

**対応方針**:
- まず `assignment_loop.rs` と `assign_task_system` の両経路で呼び出し頻度を確認する
- `target_walkable = true` 時の意味は「対象セルに直接到達可能、または隣接到達で十分」のどちらを期待しているか整理する
- `find_path_to_adjacent` への単純統一は意味変更リスクがあるため、既存の到達条件を保つ形で共通化する
- 効果が小さい場合は見送る

**変更ファイル**:
- `crates/hw_world/src/pathfinding.rs`
- `crates/hw_familiar_ai/src/familiar_ai/decide/task_management/delegation/assignment_loop.rs`
- `crates/bevy_app/src/systems/command/assign_task.rs`

---

### P5: `logic.rs` の `.chain()` を再評価し、独立システムだけを切り出す

**現状** (`crates/bevy_app/src/plugins/logic.rs:67-96`):
14 以上のシステムが単一の `.chain()` で全て直列化されている。

**各システムのリソース依存と独立可否**:

| システム | 主な読み書き対象 | 独立可否 |
|:---|:---|:---:|
| `assign_task_system` | `ResMut<TaskContext>`, Commands, Designation, WorldMapRead | ⚠️ |
| `familiar_command_input_system` | `ResMut<TaskContext>`, `Query<&mut ActiveCommand>` | ⚠️ |
| `task_area_selection_system` | `ResMut<TaskContext>`, `ResMut<NextState<PlayMode>>`, Commands, `ResMut<AreaEditSession>`, `ResMut<AreaEditHistory>` | ⚠️ |
| `zone_placement_system` | `ResMut<TaskContext>`, `ResMut<NextState<PlayMode>>`, WorldMapWrite, Commands | ⚠️ |
| `zone_removal_system` | `ResMut<TaskContext>`, `ResMut<NextState<PlayMode>>`, WorldMapWrite, Commands, `ResMut<ZoneRemovalPreviewState>` | ⚠️ |
| `task_area_edit_history_shortcuts_system` | `ResMut<SelectedEntity>`, `ResMut<AreaEditHistory>`, Commands | ⚠️ |
| `familiar_spawning_system` | Commands, MessageReader, `ResMut<FamiliarColorAllocator>`, WorldMapRead | ✅ |
| `tree_regrowth_system` | RegrowthManager, Tree query | ✅ |
| `obstacle_cleanup_system` | Commands, Obstacle query | ✅ |
| `blueprint_cancel_cleanup_system` | Commands, WorldMapWrite, RemovedComponents<Blueprint> | ⚠️ |
| `despawn_expired_items_system` | Commands, Item query | ✅ |
| `dream_tree_planting_system` | Commands, DreamTree query | ✅ |
| `floor_construction_cancellation_system` | **WorldMapWrite**, TaskQueries, Soul query | ⚠️ |
| `floor_construction_phase_transition_system` | FloorConstructionSite (mut) | ✅ |
| `floor_construction_completion_system` | **WorldMapWrite**, FloorConstructionSite (mut) | ⚠️ |
| `wall_construction_cancellation_system` | Commands, WallSite | ✅ |
| `wall_framed_tile_spawn_system` | Commands, WallSite query | ✅ |
| `wall_construction_phase_transition_system` | Commands, WallSite (mut) | ✅ |
| `wall_construction_completion_system` | **WorldMapWrite**, WallSite (mut) | ⚠️ |

`TaskContext` / `AreaEdit*` / `SelectedEntity` / `WorldMapWrite` を共有する command 系は
直列維持が必要。`WorldMapWrite` を保持する `blueprint_cancel_cleanup_system`、
floor 系、wall completion 系は scheduler 上も同時実行不可。

**対応**: blanket な `.chain()` をやめ、以下の4グループに再編する。

```rust
// グループA: command 系（直列維持 — TaskContext / AreaEdit / WorldMapWrite 競合あり）
(
    assign_task_system.run_if(in_state(PlayMode::TaskDesignation)),
    familiar_command_input_system.run_if(...),
    task_area_selection_system,
    zone_placement_system.run_if(in_state(PlayMode::TaskDesignation)),
    zone_removal_system.run_if(in_state(PlayMode::TaskDesignation)),
    task_area_edit_history_shortcuts_system.run_if(in_state(PlayMode::TaskDesignation)),
)
.chain()
.in_set(GameSystemSet::Logic),

// グループB: maintenance / spawn 系（非 chain。競合は scheduler に委ねる）
(
    familiar_spawning_system,
    tree_regrowth_system,
    obstacle_cleanup_system,
    blueprint_cancel_cleanup_system,
    despawn_expired_items_system,
    dream_tree_planting_system,
)
.in_set(GameSystemSet::Logic),

// グループC: floor construction（順序必要）
(
    floor_construction_cancellation_system,
    floor_construction_phase_transition_system,
    floor_construction_completion_system,
)
.chain()
.in_set(GameSystemSet::Logic),

// グループD: wall construction（順序必要）
(
    wall_construction_cancellation_system,
    debug_instant_complete_walls_system.run_if(...),
    wall_framed_tile_spawn_system,
    wall_construction_phase_transition_system,
    wall_construction_completion_system,
)
.chain()
.in_set(GameSystemSet::Logic),

// Room detection（現状維持）
(mark_room_dirty_from_building_changes_system, validate_rooms_system, detect_rooms_system)
    .chain()
    .after(dream_tree_planting_system)
    .in_set(GameSystemSet::Logic),
```

**変更ファイル**:
- `crates/bevy_app/src/plugins/logic.rs`

---

## 実装ステップ

1. P3: `find_owner_yard` のリファクタ（低リスク・低工数）
2. P1: `PathfindingContext` への HashSet 追加（低リスク・低工数）
3. P2: 残存 `get_nearby_in_radius` 呼び出しのホットパス置換（中工数・高効果）
4. P4: `can_reach_target` の呼び出し頻度を確認してから最適化判断（中リスク・要計測）
5. P5: `logic.rs` の依存関係を棚卸しし、独立グループだけ分離（中工数・要計測）

## 検証方法

- 各変更後に `CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check --workspace` でコンパイル確認
- `cargo run -p bevy_app -- --spawn-souls 500 --spawn-familiars 30 --perf-scenario` で高負荷シナリオを再現する
- 既存の FPS 表示と familiar delegation の perf ログを使って、変更前後の傾向を比較する
- `cargo test -p hw_world pathfinding` を優先し、必要に応じて `cargo test --workspace` を追加実行する
