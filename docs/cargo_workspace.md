# Cargo Workspace Guide

本プロジェクトの Cargo workspace 構成と、コードをどの crate に置くべきかの判断基準をまとめたガイドです。
なお、クレート間の依存関係と副作用（ECS）の取り扱いに関する厳密なアーキテクチャ規則については **[`docs/crate-boundaries.md`](crate-boundaries.md)** を参照してください。

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
crates/hw_familiar_ai
crates/hw_soul_ai
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
  ├─ hw_familiar_ai
  ├─ hw_soul_ai
  ├─ hw_ui
  ├─ hw_visual
  └─ bevy_app

hw_world (hw_core + hw_jobs)
  └─ bevy_app

hw_logistics (hw_core + hw_world + hw_jobs + hw_spatial)
  └─ bevy_app

hw_jobs
  └─ bevy_app

hw_spatial (hw_core + hw_world + hw_jobs)
  └─ bevy_app

hw_familiar_ai (hw_core + hw_jobs + hw_logistics + hw_world + hw_spatial)
  └─ bevy_app

hw_soul_ai (hw_core + hw_jobs + hw_logistics + hw_world + hw_spatial)
  └─ bevy_app

hw_ui (hw_core + hw_jobs + hw_logistics)
  └─ bevy_app

hw_visual (hw_core + hw_spatial + hw_world + hw_ui)
  └─ bevy_app
```

重要な原則:

- leaf crate から root crate (`bevy_app`) へ逆依存しない
- `hw_components` のような雑多な共通箱は作らない
- 型定義とその主要 `impl` は同じ crate に置く

境界ルール（最終整理反映）:

- `hw_ui` は UI の構築・更新ロジックを集約し、`bevy_app` は shell/adapter とゲーム状態更新ハンドラを保持する。
- `bevy_app` → `hw_ui` は依存方向を維持し、`hw_ui` から `bevy_app` へ依存しない。
- `UiShell` 的な役割（Selection やカメラ・モード遷移）は `bevy_app` 側で管理する。`MainCamera` / `world_cursor_pos` の所有権は `hw_ui::camera` にあり、使用箇所はそこから直接 import する。`bevy_app` 側に 1 段噛ませるだけの `interface::camera` wrapper は置かない。

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
- `selection/` — SelectedEntity, HoveredEntity, SelectionIndicator, SelectionIntent, cleanup_selection_references_system, placement validation API
- `camera.rs` — MainCamera マーカー、`world_cursor_pos`（スクリーン座標→ワールド座標変換ユーティリティ）
- `plugins/` — UiCorePlugin / UiEntityListPlugin / UiFoundationPlugin / UiInfoPanelPlugin / UiTooltipPlugin（fn ポインタ受け付けシェル）
- **`area_edit/`** — エリア選択・編集状態の純粋データ型（`AreaEditHandleKind`, `AreaEditOperation`, `AreaEditDrag`, `AreaEditSession`, `AreaEditHistory`, `AreaEditHistoryEntry`, `AreaEditClipboard`, `AreaEditPresets`）。`AreaEditClipboard` 等は `bevy_app/command/area_selection.rs` から直接 `pub use hw_ui::area_edit::*` として re-export。`AreaEditHandleKind` は `bevy_app/command/mod.rs` からも re-export。`area_edit/interaction.rs` に `detect_area_edit_operation`・`apply_area_edit_drag`・`cursor_icon_for_operation` の pure helper を所有（M1 移設済み）

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
- `hw_visual` はビジュアルシステム全体を集約し、`GameAssets` には依存しない。`hw_visual::handles` の 8 Resource（`WallVisualHandles` 等）は root の `init_visual_handles` startup システムが `GameAssets` から注入する。`SoulTaskHandles` は `hw_core::visual`、`TerrainVisualHandles` / `DoorVisualHandles` は `hw_world`、`ResourceItemVisualHandles` は `hw_logistics` が所有し、同じ startup 注入パターンを共有する。visual marker (`FadeOut`, `WheelbarrowMovement`) は `hw_core::visual` に置き、`hw_familiar_ai` / `hw_soul_ai` / `hw_visual` の共有型として扱う。
- `bevy_app` → `hw_visual` は依存方向を維持し、`hw_visual` から `bevy_app` へ依存しない。

### `hw_visual`

役割:

- `crates/bevy_app/src/systems/visual/` および `crates/bevy_app/src/systems/utils/` から抽出したビジュアルシステム全体
- `GameAssets` に依存しない独立ビジュアル crate
- アセットハンドルは `handles.rs` の 8 Resource として保持し、startup 時に root から注入される

代表例:

- `HwVisualPlugin` — hw_visual の全システムを一括登録する Plugin
- `handles::{WallVisualHandles, BuildingAnimHandles, WorkIconHandles, MaterialIconHandles, HaulItemHandles, SpeechHandles, PlantTreeHandles, GatheringVisualHandles}` — ビジュアルハンドルリソース（GameAssets の代替）
- `hw_core::visual::{SoulTaskHandles, FadeOut, WheelbarrowMovement}` — `hw_familiar_ai` / `hw_soul_ai` と `hw_visual` が共有する visual resource / marker component
- `handles::GatheringVisualHandles` — 集会スポット visual 用ハンドルリソース（`aura_circle`, `card_table`, `campfire`, `barrel`）
- `blueprint::*` — 設計図ビジュアル（アニメーション、プログレスバー、資材表示、完成エフェクト）
- `dream::*` — Dream UI パーティクル、ドリームバブル、フローティングポップアップ（custom Material2d / UiMaterial）
- `gather::*` — 採取リソースハイライト、ワーカーインジケータ
- `haul::*` — 運搬アイテム表示、手押し車追従
- `plant_trees::*` — 植樹ビジュアルエフェクト
- `soul::*` — Soul プログレスバー、ステータスビジュアル、タスクリンク表示
- `soul::gathering_spawn::spawn_gathering_spot` — 集会スポット ECS entity 生成（aura + object sprite）。`GatheringVisualHandles` 経由で `GameAssets` に依存しない
- `speech::*` — 吹き出し、ラテン語フレーズ、FamiliarVoice、SpeechPlugin
- `speech::max_soul_visual::max_soul_visual_system` — 使役数上限減少時の "Abi" セリフバブル（ロジックは `hw_familiar_ai::max_soul_logic_system` が担当）
- `speech::squad_visual::squad_visual_system` — Fatigued リリース時の "Abi" セリフバブル（ロジックは `hw_familiar_ai::squad_logic_system` が担当）
- `mud_mixer::*`, `tank::*` — 建物アニメーション
- `wall_connection::*` — 壁接続スプライト切替
- `site_yard_visual::*` — サイト・ヤード境界描画
- `fade::*`, `floating_text::*`, `animations::*`, `progress_bar::*`, `worker_icon::*` — 汎用ビジュアルユーティリティ
- `floor_construction::*` — 床建設タイル進捗バー・資材 visual・骨 visual システム
- `wall_construction::*` — 壁建設タイル progress / 資材 visual システム
- `task_area_visual::{TaskAreaMaterial, TaskAreaVisual}` — タスクエリアシェーダー型定義
- `selection_indicator::update_selection_indicator` — 選択エンティティを追従する黄色スプライト indicator（`SelectionIndicator` コンポーネントは `hw_ui::selection` が所有。実装は `hw_visual`、登録は同フレーム反映のため root `Interface` フェーズで行う）

ここに置かないもの:

- `GameAssets`（struct・ロード処理）— root 残留
- `placement_ghost.rs`、`task_area_visual.rs`（システム関数）— `BuildContext` / `TaskContext` など app_contexts 依存のため root 残留
- `DebugVisible` による条件付き system 登録 — root 側 `VisualPlugin` が担当

root 側の責務（`crates/bevy_app/src/systems/visual/` 残留ファイル）:

| ファイル | 残留理由 |
|:---|:---|
| `floor_construction.rs` | `hw_visual::floor_construction` への re-export shell |
| `wall_construction.rs` | `hw_visual::wall_construction` への re-export shell |
| `placement_ghost.rs` | `BuildContext`, `CompanionPlacementState` 依存（app_contexts） |
| `task_area_visual.rs` | `update_task_area_material_system` が `TaskContext` 依存 |

startup 注入パターン:

```rust
// crates/bevy_app/src/plugins/startup/visual_handles.rs
pub fn init_visual_handles(mut commands: Commands, game_assets: Res<GameAssets>) {
    commands.insert_resource(WallVisualHandles { stone_isolated: game_assets.wall_isolated.clone(), ... });
    commands.insert_resource(BuildingAnimHandles { mud_mixer_idle: game_assets.mud_mixer.clone(), ... });
    // ... hw_visual / hw_core::visual / hw_world / hw_logistics の各 handle Resource を insert
}
// PostStartup チェーンの先頭で実行（GameAssets 挿入後）
```

### `hw_familiar_ai` / `hw_soul_ai`

役割:

- Root crate に依存しない AI コアロジック
- `hw_familiar_ai`: Familiar AI の純粋なシステム実装。`FamiliarAnimation` コンポーネント（`animation.rs`）と `familiar_movement` システム（`movement.rs`）を含む
- `hw_soul_ai`: Soul AI の純粋なシステム実装。`soul_movement` システム（`movement.rs`）を含む
- hw_core / hw_jobs / hw_logistics / hw_world を組み合わせた AI ドメインロジック
代表例（`hw_soul_ai`）:

- `soul_movement` — パス追従移動システム（ドア待機・衝突スライド解決・速度変調を含む）。`bevy_app/entities/damned_soul/movement/mod.rs` から直接 `pub use hw_soul_ai::soul_movement` として再公開
- `SoulAiCorePlugin` — Soul AI の Update/Execute/Decide ヘルパーフェーズコアシステム
- `FamiliarAiCorePlugin` — Familiar AI の Perceive/Decide/Execute フェーズコアシステム
- `FamiliarAnimation` — アニメーション状態コンポーネント（`is_moving`, `facing_right`）。`bevy_app/entities/familiar/components.rs` は `pub use` の薄いラッパー
- `familiar_movement` — Familiar のパス追従移動システム。`bevy_app/entities/familiar/mod.rs` から直接 `pub use hw_familiar_ai::familiar_movement` として再公開
- `soul_ai::update::*` — 疲労・バイタル・夢・集会・休憩所の更新システム
- `soul_ai::execute::designation_apply` — Designation 要求適用
- `soul_ai::execute::gathering_apply` — 集会管理要求適用（Merge / Dissolve / Recruit / Leave）
- `soul_ai::execute::gathering_spawn` — 集会発生判定と `GatheringSpawnRequest` 発行
- `soul_ai::execute::task_assignment_apply` — `TaskAssignmentRequest` 適用。system 登録責務も `hw_soul_ai::SoulAiCorePlugin` が持つ
- `soul_ai::pathfinding` — `pathfinding_system`（パス再利用・再探索・フォールバック）と `soul_stuck_escape_system`。`GameSystemSet::Actor` で登録。`hw_world::pathfinding` の探索関数を呼び出す
- `soul_ai::building_completed::on_building_completed` — `BuildingCompletedEvent` Observer。WorldMap 更新・ObstaclePosition spawn・Soul 押し出しを担当。`SoulAiCorePlugin` が `app.add_observer()` で登録
- `soul_ai::decide::idle_behavior::idle_behavior_decision_system` — IdleBehavior 決定本体
- `soul_ai::decide::idle_behavior::transitions` — IdleBehavior 遷移判定ヘルパー（次の行動選択・持続時間計算）
- `soul_ai::decide::idle_behavior::task_override` — タスク割り当て時の集会・休憩解除ヘルパー
- `soul_ai::decide::idle_behavior::exhausted_gathering` — 疲労集会（ExhaustedGathering）状態処理ヘルパー
- `soul_ai::helpers::gathering` — `hw_core::gathering` の明示的 re-export（wildcard → 明示列挙済み）と gathering timer helper
- `soul_ai::helpers::gathering_positions` — 集会周辺ランダム位置生成・overlap 回避（`PathWorld + SpatialGridOps` 経由）
- `soul_ai::helpers::gathering_motion` — 集会中移動先選定（Wandering / Still retreat）
- `soul_ai::helpers::work::{is_soul_available_for_work, unassign_task, cleanup_task_assignment}` — 作業可否判定・タスク解除・後片付けヘルパー
- `soul_ai::execute::task_execution::types::AssignedTask` — タスク実行バリアント定義
- `soul_ai::execute::task_execution::context::{TaskExecutionContext, TaskQueries, TaskAssignmentQueries, ConstructionSiteAccess}` — タスク実行/割り当て用 context・SystemParam
- `soul_ai::execute::task_execution::handler::{TaskHandler, run_task_handler, execute_haul_with_wheelbarrow}` — タスクハンドラトレイト・ディスパッチ
- `soul_ai::execute::task_execution::{gather, build, coat_wall, collect_bone, collect_sand, frame_wall, haul, haul_to_blueprint, haul_to_mixer, move_plant, pour_floor, refine, reinforce_floor, common}` — 各タスク種別実装
- `soul_ai::execute::task_execution::{haul_with_wheelbarrow, bucket_transport, transport_common}` — 輸送系タスク実装
- `soul_ai::decide::work::auto_refine` — MudMixer の自動精製指定発行
- `soul_ai::decide::work::auto_build` — 資材完了 Blueprint への自動割り当て
- `soul_ai::decide::escaping` / `soul_ai::perceive::escaping` — 逃走判断ロジック
- `soul_ai::decide::gathering_mgmt` — 集会管理要求生成
- `soul_ai::helpers::drifting::{choose_drift_edge, is_near_map_edge, random_wander_target, drift_move_target}` — 純粋 drifting 計算（`Commands` / root resource 不要）
- `soul_ai::helpers::navigation::{is_near_target, is_near_target_or_dest, is_adjacent_grid, can_pickup_item, is_near_blueprint, update_destination_if_needed}` — 純粋距離・グリッド判定（`Commands` / root resource 不要）
- `soul_ai::execute::cleanup::cleanup_commanded_souls_system` — 使い魔消滅後の被使役 Soul 解放
- `soul_ai::execute::task_execution_system::task_execution_system` — Soul タスク実行ループ本体（`WorldMapRead` / `unassign_task` / `OnTaskCompleted` を束ねる）
- `soul_ai::execute::task_execution::transport_common::{cancel,reservation,sand_collect,wheelbarrow}` — 輸送系 cancel/reservation/collect/wheelbarrow 共通処理

- `familiar_ai::perceive::state_detection` — 使い魔 AI 状態遷移検知
- `familiar_ai::decide::following` — 使い魔追尾システム（hw_core 型のみ依存）
- `familiar_ai::decide::query_types` — Familiar Decide 用の narrow query 定義
- `familiar_ai::decide::helpers` — `finalize_state_transitions` / `process_squad_management` など pure helper
- `familiar_ai::decide::recruitment` — `SpatialGridOps` ベースのリクルート選定・スカウト開始判定
- `familiar_ai::decide::state_decision` — branch dispatch (`FamiliarDecisionPath`, `determine_decision_path`) と結果型 (`FamiliarStateDecisionResult`)、message emission を含む Decide system 本体。system 登録責務は `FamiliarAiCorePlugin`
- `familiar_ai::decide::encouragement` — 激励対象選定・`EncouragementCooldown` + `encouragement_decision_system`（MessageWriter 使用）
- `familiar_ai::decide::task_management` — Familiar の task search / scoring / source selector / reservation shadow / assignment build の core
- `familiar_ai::decide::auto_gather_for_blueprint::{planning,demand,supply,helpers,actions}` — Blueprint auto gather の純計画層（`is_reachable` を含む）
- `familiar_ai::decide::blueprint_auto_gather::{BlueprintAutoGatherTimer, blueprint_auto_gather_system}` — auto-gather オーケストレーター（`WorldMapRead` / `PathfindingContext` / Bevy Query 依存を含む）。system 登録責務は `FamiliarAiCorePlugin`
- `familiar_ai::decide::squad` / `scouting` / `supervising` / `state_handlers` — 使い魔の状態機械・分隊管理の純ロジック
- `familiar_ai::execute::state_apply` — `FamiliarStateRequest` 適用
- `familiar_ai::execute::state_log` — 状態遷移ログ出力
- `familiar_ai::execute::encouragement_apply` — `EncouragementRequest` 適用と cooldown クリーンアップ
- `familiar_ai::execute::max_soul_logic::max_soul_logic_system` — 使役数上限減少時に超過 Soul のタスク解除・`CommandedBy` 削除（ビジュアルは `hw_visual::max_soul_visual_system` が担当）
- `familiar_ai::execute::squad_logic::squad_logic_system` — `SquadManagementRequest` の AddMember/ReleaseMember ECS 操作（Fatigued セリフは `hw_visual::squad_visual_system` が担当）

ここに置かないもの:

- `GameAssets` 依存の sprite spawn
- `GameAssets`、UI 状態、`bevy_app` 固有 resource を必要とする adapter
- UI システム
- `Commands` で複雑な Entity 生成を行うもの
- `execute/task_execution/mod.rs` — `common`・`handler`・`move_plant`・`types`・`context` の `pub mod` をインラインで保有する thin shell（bevy_app 側に残留）。`context` は `pub mod context { ... }` としてインライン化済み。
- `familiar_ai/decide/mod.rs` / `familiar_ai/execute/mod.rs` / `familiar_ai/helpers/mod.rs` — 互換 import path の pub use を `mod.rs` にインライン化済み。実体は `hw_familiar_ai` 側にある
- `familiar_ai/perceive/resource_sync.rs` — root perceive system。`SharedResourceCache` の再構築と実ワールドとの同期は root の責務

移設済み system の登録ルール:

- 実装本体を `hw_familiar_ai` / `hw_soul_ai` / `hw_visual` / `hw_jobs` へ移した system は、原則として所有 crate の Plugin が唯一の登録者になる。
- root 側の `pub use` / thin shell は互換パス維持と ordering 参照のために残してよいが、同じ system function を再登録してはいけない。
- root shell は `.after(...)` / `.before(...)` で移設済み system に順序制約を付けるだけにとどめる。二重登録すると Bevy 0.18 の schedule 初期化で `SystemTypeSet` が曖昧になり panic する。
- 用語は次のように使い分ける:
  - thin shell: `pub use` のみを持つ互換モジュール
  - root wrapper: root-only query/resource/event を束ねる system
  - root facade/helper: root 側公開契約や互換 helper を持ち、低レベル実装へ委譲する層

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
- `hw_world::WorldMapRead/Write`、pathfinding context、full-fat query を扱う root adapter system
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
- `GameTime`（`hw_core::time`）— ゲーム内時間 Resource。`game_time_system` は `ClockText` 依存のため bevy_app に残留するが、型自体は `hw_core::GameTime` を直接使う。1 段だけの pass-through re-export は置かない

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
- `WorldMapRead`, `WorldMapWrite` — `WorldMap` access を system 境界で統一する `SystemParam`
- `world_to_grid`, `grid_to_world`
- nearest walkable / river query
- `room_detection::{build_detection_input, detect_rooms, room_is_valid_against_input}`
- `PathWorld` trait — `is_walkable` など通行判定 API（`WorldMap` の impl も `hw_world` が所有）
- `SpatialGridOps` trait — `get_nearby_in_radius` など空間グリッド read-only API（concrete resource の本体は `hw_spatial`）
- `Yard`, `Site`, `PairedYard`, `PairedSite`（zone 系コンポーネント）
- `zone_ops::identify_removal_targets` — 削除対象タイル + 孤立フラグメント特定（Flood Fill）
- `zone_ops::area_tile_size`, `rectangles_overlap_site`, `rectangles_overlap`, `expand_yard_area` — ゾーン geometry helper
- `terrain_visual::{TerrainVisualHandles, obstacle_cleanup_system}` — 地形スプライト更新を伴う障害物 cleanup。`GameAssets` は root から専用 Resource で注入する
- `door_systems::{DoorVisualHandles, apply_door_state, door_auto_open_system, door_auto_close_system}` — ドア通行状態とスプライト更新を扱う world 系 system。`GameAssets` は root から専用 Resource で注入する
- **`Room`, `RoomOverlayTile`（Component）** — Room ECS 型
- **`RoomTileLookup`, `RoomDetectionState`, `RoomValidationState`（Resource）** — Room 管理リソース
- **`DreamTreePlantingPlan`（`tree_planting.rs`）** — Dream 植林計画の純粋データ構造。ビルダー関数（`build_dream_tree_planting_plan`）は `GameAssets`/`DreamPool` 依存のため bevy_app に残留
- **`room_systems::{detect_rooms_system, validate_rooms_system, mark_room_dirty_from_building_changes_system, on_building_added, on_building_removed, on_door_added, on_door_removed, sync_room_overlay_tiles_system}`** — Room ECS adapter 層（検出・検証・dirty マーキング・オーバーレイ同期をすべて所有）。`bevy_app/src/systems/room/` は削除済みで、`plugins/logic.rs`・`plugins/visual.rs` が直接 import する

ここに置かないもの:

- root 固有アセットを前提にした初期 sprite spawn
- `GameAssets` への直接依存
- root 側の startup / plugin wiring / entity spawn facade
- `SpatialGrid` resource 実体と update system（8 種 concrete）は `hw_spatial` が保持

### `hw_spatial`

役割:

- SpatialGrid の concrete resource / update 系（8 種）
- `GridData` と空間検索ヘルパの共通化
- 2D 空間スナップショットの初期化時の query 補助

ここに置くもの:

- `SpatialGrid`, `FamiliarSpatialGrid`, `BlueprintSpatialGrid`, `DesignationSpatialGrid`, `ResourceSpatialGrid`, `StockpileSpatialGrid`, `TransportRequestSpatialGrid`, `GatheringSpotSpatialGrid`, `FloorConstructionSpatialGrid`

ここに置かないもの:

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
- **`LogisticsPlugin`**：`apply_reservation_requests_system` を `SoulAiSystemSet::Execute` に登録する Plugin（`src/plugin.rs`）
- `TileSiteIndex`（タイル→サイト逆引き）
- `construction_helpers::{ResourceItemVisualHandles, spawn_refund_items}` — `GameAssets` 依存を root 注入 Resource に抽象化した建設キャンセル共通 helper
- producer 全系（`blueprint`, `bucket`, `consolidation`, `mixer`, `task_area`, `wheelbarrow`, `floor_construction`, `wall_construction`, `provisional_wall` 等）
- `manual_haul_selector::{select_stockpile_anchor, find_existing_request}` — 手動 haul 選定アルゴリズム（`DesignationTargetQuery` 非依存）

補足:

- `apply_reservation_requests_system` は `hw_logistics::LogisticsPlugin` が `SoulAiSystemSet::Execute` に登録する。`ResourceReservationRequest` の `add_message` と `SharedResourceCache` の `init_resource` は root app shell が担当する。

ここに置かないもの:

- `GameAssets` 依存の初期スポーン（`crates/bevy_app/src/systems/logistics/initial_spawn/` は root 残留。`mod.rs` が facade、`layout.rs` / `terrain_resources.rs` / `facilities.rs` / `report.rs` に責務分割）
- UI ロジスティクス表示

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
- `Door`, `DoorCloseTimer`
- `FloorConstructionSite`, `WallConstructionSite`（親 site component）
- `FloorTileBlueprint`, `WallTileBlueprint`（タイル単位建設状態）
- `FloorTileState`, `WallTileState`（建設フェーズ enum）
- `TargetFloorConstructionSite`, `TargetWallConstructionSite`
- `FloorConstructionCancelRequested`, `WallConstructionCancelRequested`
- `remove_tile_task_components`（designation 系コンポーネントの一括 remove helper）

ここに置かないもの:

- floor / wall construction system
- building completion shell
- door system

## 4. どこに置くかの判断基準

### `hw_core` に置く

- 複数ドメインから参照される
- Bevy app shell から独立している
- 安定した基礎型として使いたい

### `hw_world` に置く

- world/map/pathfinding の純粋ロジック
- `WorldMap` を trait や引数で抽象化できる
- root 固有の `GameAssets` を直接参照しない world 系 system
- `GameAssets` の一部フィールドだけが必要な場合は、`TerrainVisualHandles` / `DoorVisualHandles` のような専用 Resource を leaf crate 側で定義し、root の startup system が注入する

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

## 5. compatibility layer の扱い

分割後すぐに import path を全面変更しない場合、root 側に互換 re-export を置いてよいです。

### facade として維持するモジュール

以下のモジュールは app shell の正規入口として維持します。

| モジュール | 用途 |
| --- | --- |
| `crate::plugins` | plugin 型の app-shell 入口 |
| `crate::entities::damned_soul` | ECS 型・plugin・spawn API の入口 |
| `crate::entities::familiar` | ECS 型・plugin・spawn API の入口 |
| `crate::systems::spatial` | grid resource / update system の単一入口 |
| `crate::systems::logistics::transport_request` | request domain の公開面 |
| `crate::systems::command` | UI / input / mode 系の facade |
| `crate::world::map` | world map 読み書きと座標 helper の入口 |

### re-export の公開方針

- **1 シンボルにつき正規 public path は 1 つ**。同一シンボルを複数経路から公開しない
- facade に残す re-export は「複数の呼び出し側が共有する app shell 入口」に限定する
- `pub use ...::*` (wildcard) は原則使用しない。必要なシンボルを明示列挙する
- leaf module は型定義・実装・adapter を持つ場合だけ public re-export を残す

例（明示列挙に変換済みの例）:

- `crates/bevy_app/src/systems/jobs/mod.rs` -> `pub use hw_jobs::model::{Blueprint, Building, ...};`
- `crates/bevy_app/src/systems/logistics/mod.rs` -> `pub use hw_logistics::types::{ResourceItem, BelongsTo, ...};`
- `crates/bevy_app/src/world/river.rs` -> `pub use hw_world::river::{generate_fixed_river_tiles, generate_sand_tiles};`
- `crates/hw_jobs/src/lib.rs` -> `pub use assigned_task::{AssignedTask, GatherData, HaulData, ...};`（wildcard → 明示列挙）
- `crates/hw_soul_ai/src/soul_ai/helpers/gathering.rs` -> `pub use hw_core::gathering::{GatheringSpot, GatheringObjectType, ...};`（同上）
- `crates/hw_visual/src/blueprint/mod.rs` -> `pub use components::{BlueprintState, BlueprintVisual, ...};`（同上）

wildcard が許容される例外（変更しない）:

- `crates/hw_core/src/constants/mod.rs` の `pub use ai::*` 等 10 件: 100 件超の定数をフラット化する互換パターンで、コメントに明記済み。明示列挙は保守性を下げるため維持。
- `crates/hw_world/src/zones.rs` をラップする `pub mod zones { pub use hw_world::zones::*; }`: 外部クレート全シンボルを sub-namespace に公開する慣用パターン。

ルール:

- root wrapper は薄く保つ
- wrapper に独自ロジックを足し始めたら責務を見直す
- 参照がなくなった re-export は削除する
- 新しい re-export を追加する場合は wildcard でなく明示列挙で書く
- 呼び出し側が局所的で、定義元をそのまま import しても境界が明確な場合は re-export を増やさず直接 import を選ぶ

## 6. `WorldMap` の境界

`WorldMap` の**型定義と `WorldMapRead/Write` の `SystemParam` wrapper は `hw_world` が所有**する。  
root (`bevy_app`) は app shell として `init_resource::<WorldMap>()`、startup/wiring、互換 facade を担当する。

`WorldMap` の責務:

- terrain / tile entity / building / stockpile / obstacle の状態保持
- occupancy / footprint / door / stockpile の更新 API
- Bevy resource としての公開面（型は `hw_world`、初期化と app 配線は root）

`hw_world` 側へ寄せる責務:

- 座標変換
- pathfinding
- terrain 判定
- nearest walkable / river helper
- mapgen / border / regrowth の純粋ロジック
- `WorldMapRead` / `WorldMapWrite` の `SystemParam`
- `obstacle_cleanup_system` のような WorldMap 同期 +地形スプライト更新 system（`TerrainVisualHandles` 注入）
- door 自動開閉のような world state 更新 system（`DoorVisualHandles` 注入）

`crates/bevy_app/src/world/map/spawn.rs`, `crates/bevy_app/src/world/map/terrain_border.rs`, `crates/bevy_app/src/world/regrowth.rs` は app shell です。これらは `GameAssets`, `Commands`, `Resource` を扱い、純粋ロジックと `WorldMap` access wrapper は `hw_world` から呼び出します。

## 7. crate を増やすときの手順

1. `crates/<name>/Cargo.toml` と `crates/<name>/src/lib.rs` を作る
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
cargo check -p hw_familiar_ai
cargo check -p hw_soul_ai
cargo check -p hw_visual
```

## 9. やらないこと

- `jobs` / `logistics` / `world` / `UI` を一度に全部分割する
- 広すぎる共通 crate に型をまとめて押し込む
- root wrapper に再びロジックを戻す
- `cargo check` を通さずに crate 分割を進める
