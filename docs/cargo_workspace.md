# Cargo Workspace Guide

本プロジェクトの Cargo workspace 構成と、コードをどの crate に置くべきかの判断基準をまとめたガイドです。

## 1. 目的

- root crate (`bevy_app`) を Bevy の app shell に寄せる
- 純粋ロジックや共有 model を責務ごとに別 crate へ置く
- `cargo check --workspace` を常に green に保ちながら段階的に分割する

## 2. 現在の workspace 構成

`Cargo.toml` の workspace member は以下です。

```text
.
crates/hw_core
crates/hw_world
crates/hw_logistics
crates/hw_jobs
crates/hw_ai
crates/hw_spatial
crates/hw_ui
crates/hw_visual
```

依存の向きは次を基本とします。

```text
hw_core
  ├─ hw_world
  ├─ hw_logistics
  ├─ hw_jobs
  ├─ hw_spatial
  ├─ hw_ai
  ├─ hw_visual
  └─ bevy_app

hw_world
  └─ bevy_app

hw_logistics
  └─ bevy_app

hw_jobs
  └─ bevy_app

hw_spatial (hw_core + hw_world + hw_logistics + hw_jobs)
  └─ bevy_app

hw_ai (hw_core + hw_jobs + hw_logistics + hw_world + hw_spatial)
  └─ bevy_app

hw_ui
  ├─ hw_core
  ├─ hw_jobs
  ├─ hw_logistics

hw_visual (hw_core + hw_jobs + hw_logistics + hw_spatial + hw_world + hw_ui)
  └─ bevy_app
```

重要な原則:

- leaf crate から root crate (`bevy_app`) へ逆依存しない
- `hw_components` のような雑多な共通箱は作らない
- 型定義とその主要 `impl` は同じ crate に置く

境界ルール（最終整理反映）:

- `hw_ui` は UI の構築・更新ロジックを集約し、`bevy_app` は shell/adapter とゲーム状態更新ハンドラを保持する。
- `bevy_app` → `hw_ui` は依存方向を維持し、`hw_ui` から `bevy_app` へ依存しない。
- `UiShell` 的な役割（Selection やカメラ・モード遷移）は `bevy_app` 側で管理し、`interface` では API 構造変更に耐える薄い再エクスポート層を残す。

### `hw_ui`

役割:

- UI ノード生成・更新ロジックおよびウィジェット/レイアウトの本体を集約
- `UiAssets` トレイトを通じてアセット（フォント・アイコン）を抽象化
- ゲームエンティティ（DamnedSoul, Familiar 等）への直接依存を持たない

代表例（主要モジュール）:

- `setup/` — `UiAssets` trait, `setup_ui` fn（UI ツリー構築。bottom_bar / submenus / panels / entity_list / time_control / dialogs）
- `components.rs` — UiNodeRegistry, UiSlot, UiMountSlot, MenuState, MenuButton, FamiliarListItem, SoulListItem 等 50+ 型
- `theme.rs` — `UiTheme` Resource（カラーパレット・フォントサイズ・スペーシング・サイズ定数）
- `intents.rs` — `UiIntent` enum（プレイヤー UI 操作メッセージ）
- `interaction/` — tooltip/dialog/hover_action/status_display システム群（FPS, speed, dream pool, area_edit_preview 等）
- `list/` — EntityListDirty, EntityListViewModel, EntityListNodeIndex, FamiliarSectionNodes, EntityListMinimizeState, EntityListResizeState, DragState, spawn（`spawn_familiar_section`, `spawn_soul_list_item_entity` 等）, sync（`sync_familiar_sections`, `sync_unassigned_souls`）, section_toggle（`entity_list_section_toggle_system`）, selection_focus, tree_ops, visual（apply_row_highlight, entity_list_visual_feedback_system）
- `panels/tooltip_builder/` — text_wrap, widgets (spawn_progress_bar 等), templates（Soul/Building/Resource/UiButton/Generic ツールチップ）
- `panels/info_panel/` — InfoPanelPinState, InfoPanelState, spawn_info_panel_ui, info_panel_system
- `panels/task_list/` — TaskEntry, TaskListDirty, work_type_icon, render（rebuild_task_list_ui）, interaction システム群
- `panels/menu.rs` — menu_visibility_system
- `models/inspection/` — EntityInspectionModel, EntityInspectionViewModel, SoulInspectionFields
- `selection/` — SelectedEntity, HoveredEntity, SelectionIndicator, placement validation API
- `camera.rs` — MainCamera マーカー
- `plugins/` — UiCorePlugin / UiEntityListPlugin / UiFoundationPlugin / UiInfoPanelPlugin / UiTooltipPlugin（fn ポインタ受け付けシェル）

ここに置かないもの:

- ゲームエンティティ（DamnedSoul, Familiar, Blueprint, Door 等）の ECS Query
- `BuildContext`, `ZoneContext`, `TaskContext`（app_contexts）への依存
- `GameAssets` の直接参照（`UiAssets` トレイト経由に抽象化する）
- `Res<GameAssets>` をシステム引数に取るシステム関数（Bevy の `Res<T>` はトレイトオブジェクト不可）
- PlayMode 遷移ロジック（`NextState<PlayMode>`）
- WorldMap / WorldMapWrite への依存

root 側の `bevy_app` 残留（adapter 責務）:

| ファイル/モジュール | 残留理由 |
|:---|:---|
| `interaction/intent_handler.rs` | BuildContext, ZoneContext, FamiliarOperation 等 |
| `interaction/mode.rs` | PlayMode 遷移、TaskMode、BuildingType |
| `list/change_detection.rs` | DamnedSoul, Familiar, AssignedTask の Changed 監視 |
| `list/view_model.rs` | Familiar, DamnedSoul, FamiliarAiState からビューモデル構築 |
| `list/sync.rs` | `sync_entity_list_from_view_model_system` / `sync_entity_list_value_rows_system`（hw_ui helpers の thin shell） |
| `list/drag_drop.rs` | SquadManagementRequest, SoulIdentity（DragState 型は hw_ui） |
| `list/interaction.rs`, `list/interaction/navigation.rs` | 行クリック・Tab 巡回・target 付き `UiIntent` 発行。TaskContext は navigation 側に残留（SectionToggle 操作は hw_ui の `entity_list_section_toggle_system` へ移設済み） |
| `interaction/intent_handler.rs` | `UiIntent::AdjustMaxControlledSoul*` を受けて `FamiliarOperation` 更新・`FamiliarOperationMaxSoulChangedEvent` 発行・Entity List ヘッダーの optimistic update を一元処理 |
| `panels/context_menu.rs` | Familiar, DamnedSoul, Building, Door の分類 |
| `panels/task_list/view_model.rs`, `presenter.rs`, `dirty.rs`（detect systems）| Designation, Blueprint, WorkType 等のゲームクエリ |
| `panels/task_list/update.rs` | `Res<GameAssets>` をシステム引数に取るため hw_ui 移動不可 |
| `presentation/` | EntityInspectionQuery（ゲームエンティティ 10+ 型のクエリ集約）|
| `vignette.rs` | TaskContext (DreamPlanting モード判定) |
- `hw_visual` はビジュアルシステム全体を集約し、`GameAssets` には依存しない。アセットハンドルは `handles.rs` の 7 つの Resource（`WallVisualHandles` 等）で保持し、root の `init_visual_handles` startup システムが `GameAssets` から注入する。
- `bevy_app` → `hw_visual` は依存方向を維持し、`hw_visual` から `bevy_app` へ依存しない。

### `hw_visual`

役割:

- `src/systems/visual/` および `src/systems/utils/` から抽出したビジュアルシステム全体
- `GameAssets` に依存しない独立ビジュアル crate
- アセットハンドルは `handles.rs` の 8 Resource として保持し、startup 時に root から注入される

代表例:

- `HwVisualPlugin` — hw_visual の全システムを一括登録する Plugin
- `handles::{WallVisualHandles, BuildingAnimHandles, WorkIconHandles, MaterialIconHandles, HaulItemHandles, SpeechHandles, PlantTreeHandles, GatheringVisualHandles}` — ビジュアルハンドルリソース（GameAssets の代替）
- `handles::GatheringVisualHandles` — 集会スポット visual 用ハンドルリソース（`aura_circle`, `card_table`, `campfire`, `barrel`）
- `blueprint::*` — 設計図ビジュアル（アニメーション、プログレスバー、資材表示、完成エフェクト）
- `dream::*` — Dream UI パーティクル、ドリームバブル、フローティングポップアップ（custom Material2d / UiMaterial）
- `gather::*` — 採取リソースハイライト、ワーカーインジケータ
- `haul::*` — 運搬アイテム表示、手押し車追従
- `plant_trees::*` — 植樹ビジュアルエフェクト
- `soul::*` — Soul プログレスバー、ステータスビジュアル、タスクリンク表示
- `soul::gathering_spawn::spawn_gathering_spot` — 集会スポット ECS entity 生成（aura + object sprite）。`GatheringVisualHandles` 経由で `GameAssets` に依存しない
- `speech::*` — 吹き出し、ラテン語フレーズ、FamiliarVoice、SpeechPlugin
- `mud_mixer::*`, `tank::*` — 建物アニメーション
- `wall_connection::*` — 壁接続スプライト切替
- `site_yard_visual::*` — サイト・ヤード境界描画
- `fade::*`, `floating_text::*`, `animations::*`, `progress_bar::*`, `worker_icon::*` — 汎用ビジュアルユーティリティ
- `task_area_visual::{TaskAreaMaterial, TaskAreaVisual}` — タスクエリアシェーダー型定義

ここに置かないもの:

- `GameAssets`（struct・ロード処理）— root 残留
- `placement_ghost.rs`、`floor_construction.rs`、`wall_construction.rs`、`task_area_visual.rs`（システム関数）— `BuildContext` / `TaskContext` など app_contexts 依存のため root 残留
- `DebugVisible` による条件付き system 登録 — root 側 `VisualPlugin` が担当

root 側の責務（`src/systems/visual/` 残留ファイル）:

| ファイル | 残留理由 |
|:---|:---|
| `floor_construction.rs` | `FloorConstructionSite` → `TaskArea` 依存（root 型） |
| `wall_construction.rs` | `WallConstructionSite` → `TaskArea` 依存（root 型） |
| `placement_ghost.rs` | `BuildContext`, `CompanionPlacementState` 依存（app_contexts） |
| `task_area_visual.rs` | `update_task_area_material_system` が `TaskContext` 依存 |

startup 注入パターン:

```rust
// src/plugins/startup/visual_handles.rs
pub fn init_visual_handles(mut commands: Commands, game_assets: Res<GameAssets>) {
    commands.insert_resource(WallVisualHandles { stone_isolated: game_assets.wall_isolated.clone(), ... });
    // ... 8 Resource すべてを insert
}
// PostStartup チェーンの先頭で実行（GameAssets 挿入後）
```

### `hw_ai`

役割:

- Root crate に依存しない AI コアロジック
- Soul AI および Familiar AI の純粋なシステム実装
- hw_core / hw_jobs / hw_logistics / hw_world を組み合わせた AI ドメインロジック

代表例:

- `SoulAiCorePlugin` — Soul AI の Update/Execute/Decide ヘルパーフェーズコアシステム
- `FamiliarAiCorePlugin` — Familiar AI の Perceive/Decide/Execute フェーズコアシステム
- `soul_ai::update::*` — 疲労・バイタル・夢・集会・休憩所の更新システム
- `soul_ai::execute::designation_apply` — Designation 要求適用
- `soul_ai::execute::gathering_apply` — 集会管理要求適用（Merge / Dissolve / Recruit / Leave）
- `soul_ai::execute::gathering_spawn` — 集会発生判定と `GatheringSpawnRequest` 発行
- `soul_ai::execute::task_assignment_apply` — `TaskAssignmentRequest` 適用。system 登録責務も `hw_ai::SoulAiCorePlugin` が持つ
- `soul_ai::decide::idle_behavior::idle_behavior_decision_system` — IdleBehavior 決定本体
- `soul_ai::decide::idle_behavior::transitions` — IdleBehavior 遷移判定ヘルパー（次の行動選択・持続時間計算）
- `soul_ai::decide::idle_behavior::task_override` — タスク割り当て時の集会・休憩解除ヘルパー
- `soul_ai::decide::idle_behavior::exhausted_gathering` — 疲労集会（ExhaustedGathering）状態処理ヘルパー
- `soul_ai::helpers::gathering` — `hw_core::gathering` の互換 re-export と gathering timer helper
- `soul_ai::helpers::gathering_positions` — 集会周辺ランダム位置生成・overlap 回避（`PathWorld + SpatialGridOps` 経由）
- `soul_ai::helpers::gathering_motion` — 集会中移動先選定（Wandering / Still retreat）
- `soul_ai::helpers::work::is_soul_available_for_work` — 作業可否判定ヘルパー
- `soul_ai::decide::work::auto_refine` — MudMixer の自動精製指定発行
- `soul_ai::decide::work::auto_build` — 資材完了 Blueprint への自動割り当て
- `soul_ai::decide::escaping` / `soul_ai::perceive::escaping` — 逃走判断ロジック
- `soul_ai::decide::gathering_mgmt` — 集会管理要求生成
- `soul_ai::helpers::drifting::{choose_drift_edge, is_near_map_edge, random_wander_target, drift_move_target}` — 純粋 drifting 計算（`Commands` / root resource 不要）
- **（追加予定 M4）** `soul_ai::helpers::navigation::{is_near_target, is_near_target_or_dest, is_adjacent_grid, can_pickup_item, is_near_blueprint, update_destination_if_needed}` — 純粋距離・グリッド判定
- `familiar_ai::perceive::state_detection` — 使い魔 AI 状態遷移検知
- `familiar_ai::decide::following` — 使い魔追尾システム（hw_core 型のみ依存）
- `familiar_ai::decide::query_types` — Familiar Decide 用の narrow query 定義
- `familiar_ai::decide::helpers` — `finalize_state_transitions` / `process_squad_management` など pure helper
- `familiar_ai::decide::recruitment` — `SpatialGridOps` ベースのリクルート選定・スカウト開始判定
- `familiar_ai::decide::encouragement` — 激励対象選定と `EncouragementCooldown`
- `familiar_ai::decide::auto_gather_for_blueprint::{planning,demand,supply,helpers}` — Blueprint auto gather の純計画層
- `familiar_ai::decide::squad` / `scouting` / `supervising` / `state_handlers` — 使い魔の状態機械・分隊管理の純ロジック
- `familiar_ai::execute::state_apply` — `FamiliarStateRequest` 適用
- `familiar_ai::execute::state_log` — 状態遷移ログ出力

ここに置かないもの:

- `GameAssets` 依存の sprite spawn
- root 固有の `WorldMapRead/Write` SystemParam wrapper や pathfinding context を前提にした adapter
- full-fat query から narrow view への変換や、root-only resource を伴う request 出力 adapter
- UI システム
- `Commands` で複雑な Entity 生成を行うもの
- pathfinding / blueprint entity query を伴う auto-gather orchestration
- `unassign_task`（`helpers/work.rs`）は `WheelbarrowMovement` / `Visibility` / `Transform` など root 依存が強いため core 化対象外
- `task_execution/context/access.rs` — `FloorConstructionSite` / `WallConstructionSite` が root-only のため、これらが `hw_jobs` に移設されるまで root 残留（後述 **task_execution 全面移設 blocker** 参照）
- `update_destination_to_adjacent` / `update_destination_to_blueprint` — `PathfindingContext` 自体は `hw_world` 所有だが、呼び出し側がまだ root の re-export path（`crate::world::pathfinding::*`）に結合しているため、その整理が済むまで hw_ai 移設対象外

移設済み system の登録ルール:

- 実装本体を `hw_ai` / `hw_visual` / `hw_jobs` へ移した system は、原則として所有 crate の Plugin が唯一の登録者になる。
- root 側の `pub use` / thin shell は互換パス維持と ordering 参照のために残してよいが、同じ system function を再登録してはいけない。
- root shell は `.after(...)` / `.before(...)` で移設済み system に順序制約を付けるだけにとどめる。二重登録すると Bevy 0.18 の schedule 初期化で `SystemTypeSet` が曖昧になり panic する。

## 3. 各 crate の責務

### `bevy_app`

役割:

- Bevy plugin / system 登録
- `Commands`, `Res`, `Query` を使う app shell
- Sprite spawn, UI, ECS wiring
- crate 間の接着層

ここに残すもの:

- plugin 定義
- startup / visual / UI system
- ECS resource と shell system
- root 側の互換 re-export 層
- `GameAssets`・sprite spawn・root 固有 resource を伴う adapter
- root 固有の `WorldMapRead/Write` wrapper、pathfinding context、full-fat query を扱う system
- request 消費時に app 側状態を再検証して副作用を確定する adapter

### `hw_core`

役割:

- ドメイン横断で使う基礎型
- 安定した enum / message / relationship / constants

代表例:

- `constants`
- `game_state`
- `gathering`
- `relationships`
- `events`
- `WorkType`
- `ResourceType`
- `DoorState`
- `AreaBounds`, `TaskArea`（矩形エリア抽象型）
- `command` 系 pure helper の一部（`wall_line_area`, `count_positions_in_area`, `overlap_summary_from_areas`, `get_drag_start`）

### `hw_world`

役割:

- world の純粋ロジック
- pathfinding, terrain, map helper, 座標変換
- room detection core（入力分類、flood-fill、validator、`RoomBounds`）
- AI helper が使用する read-only 空間トレイト

代表例:

- terrain / river / mapgen / borders / regrowth
- spawn grid helper
- `WorldMap`
- `world_to_grid`, `grid_to_world`
- nearest walkable / river query
- `room_detection::{build_detection_input, detect_rooms, room_is_valid_against_input}`
- `PathWorld` trait — `is_walkable` など通行判定 API（`WorldMap` の impl は root）
- `SpatialGridOps` trait — `get_nearby_in_radius` など空間グリッド read-only API（concrete resource の本体は `hw_spatial`）
- `Yard`, `Site`, `PairedYard`, `PairedSite`（zone 系コンポーネント）

ここに置かないもの:

- `Commands` を使う sprite spawn
- `GameAssets` 依存の texture 選択
- root 固有の `WorldMapRead/Write` SystemParam wrapper
- `Room` entity の spawn/despawn、`RoomTileLookup` 更新、dirty scheduling
- `SpatialGrid` resource 実体と update system（8 種 concrete）は `hw_spatial` が保持

### `hw_spatial`

役割:

- SpatialGrid の concrete resource / update 系（8 種）
- `GridData` と空間検索ヘルパの共通化
- 2D 空間スナップショットの初期化時の query 補助

ここに置くもの:

- `SpatialGrid`, `FamiliarSpatialGrid`, `BlueprintSpatialGrid`, `DesignationSpatialGrid`, `ResourceSpatialGrid`, `StockpileSpatialGrid`, `TransportRequestSpatialGrid`, `GatheringSpotSpatialGrid`

ここに置かないもの:

- `FloorConstructionSpatialGrid`
- root `WorldMap` shell、`WorldMapRead/Write`、startup/wiring

### `hw_logistics`

役割:

- 物流の共有 model / helper
- transport request の完全な実行ロジック（producer / arbitration / plugin）
- GameAssets・UI に依存しない物流システムの集約

代表例:

- `ResourceItem`, `Wheelbarrow`, `Stockpile`
- water / ground resource helper
- `TransportRequest*`, `TransportRequestPlugin`, `TransportRequestSet`
- transport metrics / state sync / lifecycle cleanup
- `SharedResourceCache`（タスク間リソース予約キャッシュ）
- `apply_reservation_op` / `apply_reservation_requests_system`（予約操作の反映 helper）
- `TileSiteIndex`（タイル→サイト逆引き）
- producer 全系（`blueprint`, `bucket`, `consolidation`, `mixer`, `task_area`, `wheelbarrow` 等）
- 手押し車仲裁システム（`arbitration/`）
- 建設系需要計算ヘルパー（`floor_construction`, `wall_construction`, `provisional_wall`）

補足:

- `apply_reservation_requests_system` の実装は `hw_logistics` にあるが、`ResourceReservationRequest` の `add_message` と `SharedResourceCache` の `init_resource` は root app shell が担当する

ここに置かないもの:

- `GameAssets` 依存の初期スポーン（`initial_spawn.rs` は root 残留）
- UI ロジスティクス表示
- `FloorConstructionSpatialGrid` を直接参照する producer（Optional M_extra 完了まで root 残留）

### `hw_jobs`

役割:

- jobs の共有 model
- building / blueprint / designation 系の基礎型

代表例:

- `BuildingType`, `Building`, `Blueprint`
- `Designation`, `Priority`, `TaskSlots`
- `MudMixerStorage`
- `AssignedTask`（ワーカー実行中タスク状態 + 全フェーズ型）
- `TaskAssignmentRequest`（`hw_jobs::events`）
- `MovePlanned`（建物移動タスクの計画状態）
- `FloorTileBlueprint`, `WallTileBlueprint`（タイル単位建設状態）
- `FloorTileState`, `WallTileState`（建設フェーズ enum）
- `TargetFloorConstructionSite`, `TargetWallConstructionSite`
- `FloorConstructionCancelRequested`, `WallConstructionCancelRequested`

ここに置かないもの:

- `FloorConstructionSite` / `WallConstructionSite`（`TaskArea` 依存のためまだ root に残留）
  - これらが `hw_jobs` に移設されるまで、`task_execution/context/access.rs` も root に残る（**task_execution 全面移設 blocker**）
  - 移設には `TaskArea` (root `app_contexts`) との結合を解消する必要がある
- floor / wall construction system
- building completion shell
- door system

## 3.x. task_execution 全面移設 blocker

`src/systems/soul_ai/execute/task_execution/` を `hw_ai` に完全移設するには下記の条件が揃う必要がある。

### blocker の実体

`task_execution/context/access.rs` が `StorageAccess` / `MutStorageAccess` として以下の **root-only 型** の Query を保持している：

```rust
// StorageAccess
floor_sites: Query<(&FloorConstructionSite, &TaskWorkers)>
wall_sites:  Query<(&WallConstructionSite, &TaskWorkers)>

// MutStorageAccess
floor_sites: Query<(&mut FloorConstructionSite, &TaskWorkers)>
wall_sites:  Query<(&mut WallConstructionSite, &TaskWorkers)>
```

| 型 | 定義場所 | 移設状態 |
|:--|:--|:--|
| `FloorConstructionSite` | `src/systems/jobs/floor_construction/components.rs` | root-only（`TaskArea` 依存のため未移設） |
| `WallConstructionSite` | `src/systems/jobs/wall_construction/components.rs` | root-only（`TaskArea` 依存のため未移設） |

### 次計画の前提条件

`task_execution` を `hw_ai` に全面移設するには以下をすべて満たす必要がある：

1. `FloorConstructionSite` / `WallConstructionSite` から `TaskArea`（root の `app_contexts` 依存）への依存を解消する
2. 両型を `hw_jobs` に移設する
3. `context/access.rs` の import を `crate::` → `hw_jobs::` に書き換えて `hw_ai` に移動する
4. `task_execution_system` 本体（`handler.rs` / `context/mod.rs` 他）を `hw_ai` に移設する

この作業が完了するまで、`src/systems/soul_ai/execute/task_execution/` は root に残置される。

## 4. どこに置くかの判断基準

### `hw_core` に置く

- 複数ドメインから参照される
- Bevy app shell から独立している
- 安定した基礎型として使いたい

### `hw_world` に置く

- world/map/pathfinding の純粋ロジック
- `WorldMap` を trait や引数で抽象化できる
- `Commands` や asset に依存しない

### `hw_logistics` に置く

- transport / stockpile / resource 搬送の共有型
- producer 間で共通に使う helper
- app shell がなくても意味がある

### `hw_jobs` に置く

- building / designation / blueprint の基礎 model
- 複数 system から広く参照される component
- construction shell ではなく shared state として再利用される

### root (`bevy_app`) に残す

- Bevy system registration が主責務
- `Commands` / asset / UI / plugin order に強く依存する
- app shell としての意味が大きい
- room detection では `RoomDetectionBuildingTile` 収集、`DetectedRoom` → `Room` 変換、`RoomTileLookup` 再構築を担当する

## 5. compatibility layer の扱い

分割後すぐに import path を全面変更しない場合、root 側に互換 re-export を置いてよいです。

例:

- `src/systems/jobs/mod.rs` -> `pub use hw_jobs::model::*;`
- `src/systems/logistics/types.rs` -> `pub use hw_logistics::types::*;`
- `src/world/river.rs` -> `pub use hw_world::river::*;`

ルール:

- root wrapper は薄く保つ
- wrapper に独自ロジックを足し始めたら責務を見直す
- 参照がなくなった re-export は削除する

## 6. `WorldMap` の境界

`WorldMap` は root crate に残す resource です。

`WorldMap` の責務:

- terrain / tile entity / building / stockpile / obstacle の状態保持
- occupancy / footprint / door / stockpile の更新 API
- Bevy resource としての公開面

`hw_world` 側へ寄せる責務:

- 座標変換
- pathfinding
- terrain 判定
- nearest walkable / river helper
- mapgen / border / regrowth の純粋ロジック

`src/world/map/spawn.rs`, `src/world/map/terrain_border.rs`, `src/world/regrowth.rs` は app shell です。これらは `GameAssets`, `Commands`, `Resource` を扱い、純粋ロジックは `hw_world` から呼び出します。

## 7. crate を増やすときの手順

1. `crates/<name>/Cargo.toml` と `src/lib.rs` を作る
2. root `Cargo.toml` に path dependency を追加する
3. shared model / helper から移す
4. root 側を re-export または import 修正でつなぐ
5. `cargo check --workspace` を通す
6. docs の責務表と `docs/README.md` を更新する

## 8. 検証コマンド

全体確認:

```bash
cargo check --workspace
```

timing 記録:

```bash
cargo check --workspace --timings
```

root app 起動:

```bash
cargo run
```

個別 crate の確認:

```bash
cargo check -p hw_core
cargo check -p hw_world
cargo check -p hw_logistics
cargo check -p hw_jobs
cargo check -p hw_ai
cargo check -p hw_visual
```

## 9. やらないこと

- `jobs` / `logistics` / `world` / `UI` を一度に全部分割する
- 広すぎる共通 crate に型をまとめて押し込む
- root wrapper に再びロジックを戻す
- `cargo check` を通さずに crate 分割を進める
