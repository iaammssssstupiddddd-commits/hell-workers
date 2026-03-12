# task_execution hw_ai Extraction Plan

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `task-execution-hw-ai-extraction-plan-2026-03-12` |
| ステータス | `Completed` |
| 作成日 | `2026-03-12` |
| 最終更新日 | `2026-03-12` |
| 作成者 | `Codex` |
| 関連提案 | `N/A` |
| 関連Issue/PR | `N/A` |

## 1. 目的

- 解決したい課題: `src/systems/soul_ai/execute/task_execution/` が依然として root に残っており、`soul_ai` の crate 境界が中途半端なままになっている。
- 到達したい状態: `task_execution` の実行コアと query/context を `hw_ai` へ移し、root 側は system 登録・world/app shell・互換 re-export のみに縮小する。
- 成功指標: `task_execution` 本体が `crates/hw_ai` に存在し、root 側に残るのが `apply_pending_building_move_system` などの app shell と adapter のみになる。

## 2. スコープ

### 対象（In Scope）

- `FloorConstructionSite` / `WallConstructionSite` の crate 所有整理
- `task_execution/context/*` と `TaskQueries` 系の `hw_ai` 移設
- `task_execution` 実行本体の `hw_ai` 移設
- `GameAssets` → `hw_visual::SoulTaskHandles` への adapter 分離
- `unassign_task` / `move_plant` の adapter 分離
- `docs/cargo_workspace.md` / `docs/soul_ai.md` / `src/systems/soul_ai/README.md` の同期

### 非対象（Out of Scope）

- `unassign_task` の完全 pure 化
- `apply_pending_building_move_system` の `hw_ai` 移設
- `task_execution` 以外の soul/familiar system の追加分離
- visual handle 初期化方針全体の再設計

## 3. 現状とギャップ（コード調査済み）

### 現状の型所有状況

| 型 | 現在の所有場所 | 最終目標 |
|---|---|---|
| `FloorConstructionSite` | `src/systems/jobs/floor_construction/components.rs`（root 直接定義） | `hw_jobs::construction` |
| `WallConstructionSite` | `src/systems/jobs/wall_construction/components.rs`（root 直接定義） | `hw_jobs::construction` |
| `FloorTileBlueprint`, `FloorConstructionPhase` 等 | `hw_jobs::construction`（移設済み） | 現状維持 |
| `TaskExecutionContext` | `src/.../context/execution.rs`（root） | `hw_ai::soul_ai::execute::task_execution::context` |
| `TaskQueries`, `TaskAssignmentQueries` 等 | `src/.../context/queries.rs`（root） | `hw_ai::soul_ai::execute::task_execution::context` |
| `StorageAccess`, `MutStorageAccess` 等 | `src/.../context/access.rs`（root） | `hw_ai::soul_ai::execute::task_execution::context` |
| `task_execution_system` | `src/.../task_execution/mod.rs`（root） | `hw_ai::soul_ai::execute::task_execution` |
| `handle_move_plant_task` | `src/.../task_execution/move_plant.rs`（root） | `hw_ai::soul_ai::execute::task_execution::move_plant` |
| `apply_pending_building_move_system` | `src/.../task_execution/move_plant.rs`（root） | root shell（移設しない） |

### ブロッカー依存チェーン

```
M1（Construction Site 移設）
  ↓ 解除
M2（context 移設）
  ↓ 解除
M3（task_execution core 移設）
```

- **M2 のブロッカー**: `access.rs` の `StorageAccess` / `MutStorageAccess` / `ConstructionSiteAccess` が
  root 所有の `FloorConstructionSite` / `WallConstructionSite` を直接 Query している。
  → M1 完了後は M2 の依存型は全て crate 所有になる（追加の Cargo 依存不要）。

- **M3 の唯一の追加ブロッカー**: `GameAssets` が root 専用 Resource。
  → `hw_visual::SoulTaskHandles` を新設して 8 ハンドルのみ抽出すれば解消。

### M2 依存型の crate 所在（M1 完了後は全て crate 所有）

| 型 | crate |
|---|---|
| `DamnedSoul`, `Destination`, `Path` | `hw_core::soul` |
| `Inventory` | `hw_logistics::types` |
| `WorldMapRead` | `hw_world` |
| `PathfindingContext` | `hw_world` |
| `SharedResourceCache` | `hw_logistics`（root は re-export のみ） |
| `Blueprint`, `Designation`, `Priority`, `TaskSlots` | `hw_jobs::model` |
| `ManagedBy`, `TaskWorkers` | `hw_core::relationships` |
| `ResourceReservationRequest`, `OnTaskCompleted`, `OnTaskAbandoned` | `hw_core::events` |
| `TaskAssignmentRequest` | `hw_jobs::events` |
| `ConstructionSitePositions` trait | `hw_ai::familiar_ai::decide::task_management` |
| `FloorConstructionSite` / `WallConstructionSite` | **root** → M1 で `hw_jobs::construction` へ |

## 4. 実装方針（詳細）

- 方針: 先にデータモデル blocker を除去し、その後 query/context を移し、最後に asset 依存を adapter に押し込む。
- Bevy 0.18 APIでの注意点:
  - `SystemParam` 移設時は Query 競合を増やさず、既存の `TaskQueries` 集約方針を維持する。
  - root 側と `hw_ai::SoulAiCorePlugin` 側で同じ system を二重登録しない。
  - Resource/Message/Event の型移動時は plugin 側の初期化責務も同時に確認する。
- `hw_ai` Cargo.toml には既に `hw_jobs`, `hw_logistics`, `hw_world`, `hw_spatial` が含まれているため、M2 では Cargo 依存追加は不要。
- M3 では `SoulTaskHandles` を `hw_visual` から参照するため、`hw_ai -> hw_visual` 依存を 1 件追加する。

## 5. マイルストーン

## M1: FloorConstructionSite / WallConstructionSite を hw_jobs へ移す

### 変更内容

`crates/hw_jobs/src/construction.rs` に 2 つの struct を追加する。

```rust
// hw_jobs/src/construction.rs に追加
use hw_core::area::TaskArea;
use bevy::prelude::*;

#[derive(Component, Clone, Debug)]
pub struct FloorConstructionSite {
    pub phase: FloorConstructionPhase,
    pub area_bounds: TaskArea,
    pub material_center: Vec2,
    pub tiles_total: u32,
    pub tiles_reinforced: u32,
    pub tiles_poured: u32,
    pub curing_remaining_secs: f32,
}
impl FloorConstructionSite { ... }

#[derive(Component, Clone, Debug)]
pub struct WallConstructionSite {
    pub phase: WallConstructionPhase,
    pub area_bounds: TaskArea,
    pub material_center: Vec2,
    pub tiles_total: u32,
    pub tiles_framed: u32,
    pub tiles_coated: u32,
}
impl WallConstructionSite { ... }
```

root `components.rs` 2 ファイルを thin re-export shell に切り替える。
他の 15 ファイルは `crate::systems::jobs::floor_construction::FloorConstructionSite` 等のパスを使い続けるため import 変更不要。

### 変更ファイル一覧

| ファイル | 変更種別 |
|---|---|
| `crates/hw_jobs/src/construction.rs` | **struct 追加**（`FloorConstructionSite`, `WallConstructionSite`） |
| `crates/hw_jobs/src/lib.rs` | `pub use construction::{FloorConstructionSite, WallConstructionSite};` 追加 |
| `src/systems/jobs/floor_construction/components.rs` | 定義削除 → `pub use hw_jobs::construction::FloorConstructionSite;` に置換 |
| `src/systems/jobs/wall_construction/components.rs` | 定義削除 → `pub use hw_jobs::construction::WallConstructionSite;` に置換 |

以下 14 ファイルは import パスが `crate::systems::jobs::floor/wall_construction::*` のまま維持できるため**変更不要**（re-export 経由）:
- `src/interface/selection/floor_place/floor_apply.rs`
- `src/interface/selection/floor_place/wall_apply.rs`
- `src/systems/familiar_ai/decide/auto_gather_for_blueprint.rs`
- `src/systems/jobs/floor_construction/cancellation.rs`
- `src/systems/jobs/floor_construction/phase_transition.rs`
- `src/systems/jobs/floor_construction/completion.rs`
- `src/systems/jobs/wall_construction/cancellation.rs`
- `src/systems/jobs/wall_construction/completion.rs`
- `src/systems/jobs/wall_construction/phase_transition.rs`
- `src/systems/logistics/transport_request/producer/floor_construction.rs`
- `src/systems/logistics/transport_request/producer/wall_construction.rs`
- `src/systems/soul_ai/execute/task_execution/context/access.rs`
- `src/systems/spatial/floor_construction.rs`
- `src/systems/visual/floor_construction.rs`
- `src/systems/visual/wall_construction.rs`

### 完了条件

- [ ] `FloorConstructionSite` / `WallConstructionSite` の定義実体が `hw_jobs::construction` にある
- [ ] root `components.rs` 2 ファイルが thin re-export になっている
- [ ] `context/access.rs` の blocker コメントを「M1 完了済み」に更新できる

### 検証

```bash
CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check --workspace
```

---

## M2: task_execution Context / Query を hw_ai へ移す

### 前提

M1 完了済みであること。M2 の全依存型が crate 所有になっており、`hw_ai` Cargo.toml への追記は不要。

### 変更内容

`src/systems/soul_ai/execute/task_execution/context/` の 3 ファイルを `crates/hw_ai/src/soul_ai/execute/task_execution/context/` に移す。

移設時の主な import パス変更（crate 側の新ファイル内）:
- `crate::systems::familiar_ai::perceive::resource_sync::SharedResourceCache` → `hw_logistics::SharedResourceCache`
- `crate::systems::jobs::floor_construction::FloorConstructionSite` → `hw_jobs::construction::FloorConstructionSite`
- `crate::systems::jobs::wall_construction::WallConstructionSite` → `hw_jobs::construction::WallConstructionSite`
- `crate::relationships::ManagedBy, TaskWorkers` → `hw_core::relationships::{ManagedBy, TaskWorkers}`
- `crate::systems::jobs::{Blueprint, Designation, Priority, TaskSlots}` → `hw_jobs::{Blueprint, Designation, Priority, TaskSlots}`
- `crate::systems::logistics::Stockpile` → `hw_logistics::zone::Stockpile`
- `crate::events::ResourceReservationRequest` → `hw_core::events::ResourceReservationRequest`
- `crate::events::TaskAssignmentRequest` → `hw_jobs::events::TaskAssignmentRequest`
- `crate::entities::damned_soul::{DamnedSoul, Destination, Path}` → `hw_core::soul::{DamnedSoul, Destination, Path}`
- `crate::systems::logistics::Inventory` → `hw_logistics::Inventory`
- `crate::world::pathfinding::PathfindingContext` → `hw_world::PathfindingContext`
- `crate::world::map::WorldMapRead` → `hw_world::WorldMapRead`

root 側 `context/mod.rs` は全型を `pub use hw_ai::soul_ai::execute::task_execution::context::*;` で re-export する shell に縮小する。

### 変更ファイル一覧

| ファイル | 変更種別 |
|---|---|
| `crates/hw_ai/src/soul_ai/execute/task_execution/context/access.rs` | **新規作成**（root から移設） |
| `crates/hw_ai/src/soul_ai/execute/task_execution/context/queries.rs` | **新規作成**（root から移設） |
| `crates/hw_ai/src/soul_ai/execute/task_execution/context/execution.rs` | **新規作成**（root から移設） |
| `crates/hw_ai/src/soul_ai/execute/task_execution/context/mod.rs` | **新規作成**（pub use 列挙） |
| `crates/hw_ai/src/soul_ai/execute/task_execution/mod.rs` | `pub mod context;` 追加 |
| `crates/hw_ai/src/soul_ai/execute/mod.rs` | 必要に応じて `pub mod task_execution;` 追加 |
| `src/systems/soul_ai/execute/task_execution/context/access.rs` | 定義削除 → re-export shell |
| `src/systems/soul_ai/execute/task_execution/context/queries.rs` | 定義削除 → re-export shell |
| `src/systems/soul_ai/execute/task_execution/context/execution.rs` | 定義削除 → re-export shell |
| `src/systems/soul_ai/execute/task_execution/context/mod.rs` | `pub use hw_ai::...::context::*;` に縮小 |
| `src/systems/familiar_ai/decide/task_delegation.rs` | import 更新（必要に応じて） |
| `src/systems/familiar_ai/decide/familiar_processor.rs` | import 更新（必要に応じて） |
| `src/systems/jobs/wall_construction/cancellation.rs` | import 更新（必要に応じて） |
| `src/systems/soul_ai/execute/cleanup.rs` | import 更新（必要に応じて） |

### 完了条件

- [ ] `TaskQueries` / `TaskExecutionContext` / `StorageAccess` 等の定義実体が `hw_ai` にある
- [ ] root 側 `context/` が thin re-export shell のみになっている
- [ ] `task_execution` 各 handler が crate 側 context を参照してビルドできる

### 検証

```bash
CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check --workspace
```

---

## M3: task_execution Core と Adapter を分離する

### 前提

M2 完了済みであること。

### M3-a: SoulTaskHandles リソースを hw_visual に追加する

`GameAssets` から task_execution が実際に参照するハンドルは以下の 8 フィールドのみ（調査済み）:

```rust
// crates/hw_visual/src/handles.rs に追加
#[derive(Resource)]
pub struct SoulTaskHandles {
    // gather.rs が使用
    pub wood: Handle<Image>,
    pub tree_animes: Vec<Handle<Image>>,
    pub rock: Handle<Image>,
    // collect_bone.rs が使用
    pub icon_bone_small: Handle<Image>,
    // collect_sand.rs が使用
    pub icon_sand_small: Handle<Image>,
    // refine.rs が使用
    pub icon_stasis_mud_small: Handle<Image>,
    // bucket_transport/phases/filling.rs が使用
    pub bucket_water: Handle<Image>,
    // bucket_transport/phases/pouring.rs が使用
    pub bucket_empty: Handle<Image>,
}
```

root `src/plugins/startup/visual_handles.rs` で `GameAssets` から値をコピーして `SoulTaskHandles` を init する。

`hw_visual` Cargo.toml への追記は不要（既に `bevy` のみで完結する）。

**注意**: `hw_ai` は現状 `hw_visual` に依存していないため、`hw_ai` の Cargo.toml に `hw_visual = { path = "../hw_visual" }` を追加する必要がある。

### M3-b: TaskHandler トレイトのシグネチャを変更する

```rust
// 変更前
pub trait TaskHandler<T> {
    fn execute(
        ctx: &mut TaskExecutionContext,
        data: T,
        commands: &mut Commands,
        game_assets: &Res<GameAssets>,  // root 専用
        time: &Res<Time>,
        world_map: &WorldMap,
        breakdown_opt: Option<&StressBreakdown>,
    );
}

// 変更後（hw_ai 側）
pub trait TaskHandler<T> {
    fn execute(
        ctx: &mut TaskExecutionContext,
        data: T,
        commands: &mut Commands,
        soul_handles: &SoulTaskHandles,  // hw_visual::SoulTaskHandles（参照渡し）
        time: &Res<Time>,
        world_map: &WorldMap,
        breakdown_opt: Option<&StressBreakdown>,
    );
}
```

この変更は `handler/impls.rs` の全 `impl TaskHandler<T>` に波及する（約 15 impls、多くは `_soul_handles` として未使用）。

`dispatch.rs` の `run_task_handler` と `task_execution_system` のシグネチャも `Res<SoulTaskHandles>` を受け取るよう変更する。

### M3-c: task_execution モジュール群を hw_ai へ移す

`src/systems/soul_ai/execute/task_execution/` の各ハンドラモジュールを
`crates/hw_ai/src/soul_ai/execute/task_execution/` へ移す。

移設対象（`apply_pending_building_move_system` を**含まない**モジュール）:
- `bucket_transport/`
- `build.rs`
- `coat_wall.rs`
- `collect_bone.rs`
- `collect_sand.rs`
- `common.rs`
- `frame_wall.rs`
- `gather.rs`
- `handler/`
- `haul.rs`, `haul_to_blueprint.rs`, `haul_to_mixer.rs`, `haul_with_wheelbarrow/`
- `move_plant.rs`（`handle_move_plant_task` のみ。`apply_pending_building_move_system` は root shell に残す）
- `pour_floor.rs`
- `refine.rs`
- `reinforce_floor.rs`
- `transport_common/`
- `types.rs`

root 側 `task_execution/mod.rs` は以下のみになる:
```rust
// root 側に残す
pub use hw_ai::soul_ai::execute::task_execution::*;
pub use hw_ai::soul_ai::execute::task_assignment_apply::apply_task_assignment_requests_system;

// root-only: WorldMapWrite を必要とする
pub mod move_plant_apply;  // apply_pending_building_move_system を含む
```

`task_execution_system` 本体も hw_ai へ移す。`unassign_task` 呼び出しは関数ポインタまたはクロージャ渡しで対応する（または root 側でラップ system を作り hw_ai の純粋関数を呼ぶ）。

### 変更ファイル一覧

| ファイル | 変更種別 |
|---|---|
| `crates/hw_visual/src/handles.rs` | `SoulTaskHandles` struct 追加 |
| `crates/hw_visual/src/lib.rs` | `pub use handles::SoulTaskHandles;` 追加 |
| `crates/hw_ai/Cargo.toml` | `hw_visual = { path = "../hw_visual" }` 追加 |
| `crates/hw_ai/src/soul_ai/execute/task_execution/**` | ハンドラ群を移設 |
| `src/plugins/startup/visual_handles.rs` | `SoulTaskHandles` 初期化コード追加 |
| `src/systems/soul_ai/execute/task_execution/mod.rs` | re-export shell + move_plant_apply のみに縮小 |
| `src/systems/soul_ai/execute/task_execution/move_plant.rs` | `apply_pending_building_move_system` のみ残す |
| `src/systems/soul_ai/mod.rs` | system 登録の責務を確認・整理 |
| `docs/cargo_workspace.md` | `hw_ai → hw_visual` 依存を追記 |
| `docs/soul_ai.md` | task_execution の所有 crate を更新 |
| `src/systems/soul_ai/README.md` | 移設後のモジュール構成を更新 |

### 完了条件

- [ ] `task_execution_system` の定義実体が `hw_ai` 側にある
- [ ] `GameAssets` が `task_execution` core の公開 API から外れている
- [ ] root 側 `task_execution/` が shell + `apply_pending_building_move_system` のみになっている
- [ ] `move_plant.rs` の `apply_pending_building_move_system` が root-only として独立している
- [ ] `hw_ai` に `hw_visual` 依存が追記されており循環なし

### 検証

```bash
CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check --workspace
CARGO_HOME=/home/satotakumi/.cargo cargo run
```

---

## 6. リスクと対策

| リスク | 影響 | 対策 |
| --- | --- | --- |
| M1 の re-export を忘れて root 側 components.rs が定義を残したまま hw_jobs にも追加される（二重定義） | 高 | hw_jobs に追加したらすぐ root 側を削除し `pub use` に切り替える |
| M2 移設時に `SharedResourceCache` のパスを root 経由（`crate::systems::familiar_ai::perceive::resource_sync`）のまま使ってしまう | 中 | hw_ai 側では直接 `hw_logistics::SharedResourceCache` を参照する |
| `TaskHandler` トレイトのシグネチャ変更で `_game_assets` を使っていた約 15 impls が全て更新対象になる | 中 | `_game_assets` を `_soul_handles` に変えるだけなので機械的に対応可能 |
| `hw_ai → hw_visual` の新依存を追加することで `hw_visual` が `hw_ai` に依存していた場合に循環が生じる | 高 | 追加前に `hw_visual/Cargo.toml` に `hw_ai` 依存がないことを確認する（現時点では存在しない） |
| root と crate の system 二重登録 | 高 | 登録責務を `hw_ai::SoulAiCorePlugin` か root plugin のどちらかに固定し、docs も同時更新する |
| `apply_pending_building_move_system` を誤って crate 側に移して WorldMap 更新責務が曖昧になる | 中 | apply system は root shell のまま残し、task phase handler だけを先に分離する |

## 7. 検証計画

- 必須:
  ```bash
  CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check --workspace
  ```
- 手動確認シナリオ:
  - Tree/Rock gather が資源 drop と fade out を維持する
  - Floor/Wall construction の worker task 実行が継続する
  - wheelbarrow haul 中断時に handcart と積載物が正しく地面へ戻る
  - plant move が task 実行後に footprint と付随 entity を正しく移す
- パフォーマンス確認（必要時）:
  ```bash
  cargo run -- --spawn-souls 500 --spawn-familiars 30 --perf-scenario
  ```

## 8. ロールバック方針

- M1, M2, M3 を別コミットで進めれば段階ロールバック可能
- M3 は M3-a（SoulTaskHandles 追加）→ M3-b（trait 変更）→ M3-c（移設）の順に分けて戻せるようにする
- crate 側へ移した型/モジュールを root へ戻す前に、先に re-export を復元する
- plugin の system 登録責務を元に戻し、二重登録がないことを `cargo check` で確認する

## 9. AI引継ぎメモ（最重要）

### 現在地

- 進捗: `0%`
- 完了済みマイルストーン: なし
- 未着手: M1 → M2 → M3 の順で進める

### 次のAIが最初にやること（M1）

1. `crates/hw_jobs/src/construction.rs` を開き、ファイル末尾に `FloorConstructionSite` / `WallConstructionSite` の struct 定義と impl を追加する（`TaskArea` は `hw_core::area::TaskArea` で import）
2. `crates/hw_jobs/src/lib.rs` に `pub use construction::{FloorConstructionSite, WallConstructionSite};` を追加
3. `src/systems/jobs/floor_construction/components.rs` の struct 定義を削除し、`pub use hw_jobs::construction::FloorConstructionSite;` に置換（`use crate::systems::command::TaskArea;` と `use bevy::prelude::*;` も削除）
4. `src/systems/jobs/wall_construction/components.rs` も同様に置換
5. `CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check --workspace` で確認

### M2 を始める前のチェック

- `context/access.rs` の blocker コメントを「M1 完了済み、移設可能」に書き換える
- `hw_ai/Cargo.toml` に追加依存が不要なことを再確認（`hw_jobs`, `hw_logistics`, `hw_world` は既に存在）

### M3 を始める前のチェック

- `hw_visual/Cargo.toml` に `hw_ai` 依存がないことを確認（循環防止）
- `hw_ai/Cargo.toml` に `hw_visual = { path = "../hw_visual" }` を追加

### ブロッカー/注意点

- `FloorTileBlueprint`, `WallTileBlueprint` 等の tile/phase 型は既に `hw_jobs::construction` 所有。移設対象は Site 親型のみ
- `unassign_task` は root shell のまま（`WorldMap` 等の副作用を含むため）
- `move_plant.rs` は一括移設しない。`apply_pending_building_move_system` は root-only 扱いのままでよい
- M3 で `hw_ai → hw_visual` 依存が生じる。これは現状の依存グラフに存在しないため Cargo.toml 追記が必要

### 参照必須ファイル

- `crates/hw_jobs/src/construction.rs`（M1 の追加先）
- `src/systems/jobs/floor_construction/components.rs`（M1 の変更先）
- `src/systems/jobs/wall_construction/components.rs`（M1 の変更先）
- `src/systems/soul_ai/execute/task_execution/context/access.rs`（M2 の移設元・blocker コメント確認）
- `src/systems/soul_ai/execute/task_execution/context/queries.rs`（M2 の移設元）
- `src/systems/soul_ai/execute/task_execution/context/execution.rs`（M2 の移設元）
- `src/systems/soul_ai/execute/task_execution/mod.rs`（M3 の主要変更先）
- `src/systems/soul_ai/execute/task_execution/handler/task_handler.rs`（M3-b の trait 変更先）
- `src/systems/soul_ai/execute/task_execution/handler/impls.rs`（M3-b の impl 変更先）
- `crates/hw_visual/src/handles.rs`（M3-a の SoulTaskHandles 追加先）
- `src/plugins/startup/visual_handles.rs`（M3-a の初期化追加先）
- `docs/cargo_workspace.md`, `docs/soul_ai.md`, `src/systems/soul_ai/README.md`（完了後のドキュメント更新先）

### 最終確認ログ

- 最終 `cargo check`: `2026-03-12` / `pass`
- 未解決エラー: `なし（計画書作成時点）`

### Definition of Done

- [ ] `FloorConstructionSite` / `WallConstructionSite` の定義が `hw_jobs::construction` にある
- [ ] `TaskQueries` / `TaskExecutionContext` 等の定義が `hw_ai` にある
- [ ] `task_execution_system` の定義が `hw_ai` にある
- [ ] `GameAssets` が `task_execution` core の公開 API から外れている
- [ ] root 側 `task_execution/` が shell + `apply_pending_building_move_system` のみになっている
- [ ] `cargo check --workspace` が成功
- [ ] 影響ドキュメントが更新済み

## 10. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| `2026-03-12` | `Codex` | 初版作成 |
| `2026-03-12` | `Copilot` | コード調査に基づき全マイルストーンを具体化。型所在テーブル・import パス変換表・SoulTaskHandles フィールド一覧・変更ファイル一覧を追加 |
| `2026-03-12` | `Codex` | 実装完了に伴い Completed へ更新。crate 境界ドキュメントを同期。 |
