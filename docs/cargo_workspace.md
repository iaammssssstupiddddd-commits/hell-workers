# Cargo Workspace Guide

本プロジェクトの Cargo workspace 構成と、コードをどの crate に置くべきかの判断基準をまとめたガイドです。
なお、クレート間の依存関係と副作用（ECS）の取り扱いに関する厳密なアーキテクチャ規則については **[`docs/crate-boundaries.md`](crate-boundaries.md)** を参照してください。

## 1. 目的

- root crate (`bevy_app`) を Bevy の app shell に寄せる
- `bevy_app` の library は共有 Resource・公開 module・root re-export・`HellWorkersGamePlugin` を所有し、binary `main.rs` は platform/backend/window の起動設定だけを持つ
- 純粋ロジックや共有 model を責務ごとに別 crate へ置く
- `cargo check --workspace` を常に green に保ちながら段階的に分割する

## 2. 現在の workspace 構成

`Cargo.toml` の workspace member は以下です。

```text
crates/bevy_app
crates/visual_test
crates/hw_core
crates/hw_energy
crates/hw_world
crates/hw_logistics
crates/hw_jobs
crates/hw_familiar_ai
crates/hw_soul_ai
crates/hw_spatial
crates/hw_ui
crates/hw_visual
```

root `Cargo.toml`は`members = ["crates/*"]`、`default-members = ["crates/bevy_app"]`を使います。
内部crate間の直接依存は各`Cargo.toml`を正本とし、現在は次の通りです。

| crate | workspace内の直接依存 |
|:---|:---|
| `hw_core` | — |
| `hw_energy` | — |
| `hw_jobs` | `hw_core`, `hw_energy` |
| `hw_world` | `hw_core`, `hw_jobs` |
| `hw_spatial` | `hw_core`, `hw_jobs`, `hw_world` |
| `hw_logistics` | `hw_core`, `hw_jobs`, `hw_world`, `hw_spatial` |
| `hw_familiar_ai` | `hw_core`, `hw_energy`, `hw_jobs`, `hw_logistics`, `hw_world`, `hw_spatial` |
| `hw_soul_ai` | `hw_core`, `hw_energy`, `hw_jobs`, `hw_logistics`, `hw_world`, `hw_spatial` |
| `hw_ui` | `hw_core`, `hw_jobs`, `hw_logistics` |
| `hw_visual` | `hw_core`, `hw_spatial`, `hw_world` |
| `visual_test` | `hw_core`, `hw_visual`, `hw_world` |
| `bevy_app` | 全`hw_*` crate |

重要な原則:

- leaf crate から root crate (`bevy_app`) へ逆依存しない
- `hw_components` のような雑多な共通箱は作らない
- 型定義とその主要 `impl` は同じ crate に置く

### Bevy workspace features（root `Cargo.toml`）

workspace 共通の `bevy` 依存は `default-features = false` で必要 feature のみ列挙する。

| feature | 用途 |
|:---|:---|
| `system_clipboard` | `EditableText` の Ctrl+C/V 等。OS クリップボード連携（テキスト入力 UI: リネーム・検索・Dev PoC） |

境界ルール（最終整理反映）:

- UI は **2 層**に分離する（規範: [`crate-boundaries.md` §1.1](crate-boundaries.md#11-ui-2層構造hw_ui--bevy_appinterface)）:
  - **Widget 層 (`hw_ui`)**: ノード生成・テーマ・ViewModel→ノード同期・`UiIntent` 発行
  - **Adapter 層 (`bevy_app/src/interface/`)**: ECS→ViewModel、Presenter（thin sync）、`UiIntent` ハンドラ
- `hw_ui` は UI の構築・更新ロジックを集約し、`bevy_app` は shell/adapter とゲーム状態更新ハンドラを保持する。
- `bevy_app` → `hw_ui` は依存方向を維持し、`hw_ui` から `bevy_app` へ依存しない。
- `UiShell` 的な役割（Selection やカメラ・モード遷移）は `bevy_app` 側で管理する。`MainCamera`、`SelectedEntity` / `HoveredEntity` / `SelectionIndicator`、`UiNodeRegistry` / `UiSlot` / `UiMountSlot` / `UiRoot` のような**共有 UI 契約型**は `hw_core` が所有し、`hw_ui` は必要に応じて re-export する。`world_cursor_pos` のような UI 実装ヘルパーは `hw_ui::camera` に残す。

#### UI 2 層 — 機能別ファイル対応（Adapter = `bevy_app/src/interface/ui/`）

| 機能 | ViewModel | Presenter | Widget (`hw_ui`) |
|:---|:---|:---|:---|
| エンティティリスト | `list/view_model.rs`, `list/change_detection.rs` | `list/sync.rs` | `list/spawn`, `list/sync`, `list/visual`, `list/section_toggle` |
| タスクリスト | `panels/task_list/view_model.rs`, `panels/task_list/dirty.rs` | `panels/task_list/presenter.rs`, `panels/task_list/update.rs`, `panels/task_list/actions.rs` | `panels/task_list/types.rs`, `render.rs`, `interaction.rs` |
| 操作 → ゲーム | — | `interaction/intent_handler.rs`, `interaction/handlers/`, `interaction/intent_context.rs`（Stockpile editor intentをtyped domain requestへ変換） | `intents.rs`（型定義・発行元） |
| 情報パネル | `presentation/`（EntityInspectionQuery、`StockpileInspectionFields`構築） | `panels/info_panel` re-export + root wiring | `panels/info_panel/*`, `models/inspection/`（Stockpile editor modelを含む） |
| 結果通知 | save/load・task actionのroot outcome、`hw_logistics`のStockpile outcome | `notifications.rs`（安全な表示文言adapter） | `notifications/`（Message、reducer、有界履歴、UI） |
| 初期 UI ツリー | — | `setup/mod.rs`（`GameAssets` → `UiAssets`） | `setup/*` |

### `hw_ui`

役割:

- UI ノード生成・更新ロジックおよびウィジェット/レイアウトの本体を集約
- `UiAssets` トレイトを通じてアセット（フォント・アイコン）を抽象化
- ゲームエンティティ（DamnedSoul, Familiar 等）への直接依存を持たない

代表例（主要モジュール）:

- `setup/` — `UiAssets` trait, `setup_ui` fn（UI ツリー構築。bottom_bar / submenus / panels / entity_list / time_control / dialogs）。Modal/Pause は full-viewport `UiInputCapture` root、構造 root/slot は picking-transparent にする
- `components.rs` — MenuState, MenuButton, FamiliarListItem, SoulListItem、hover/captureを分離する `UiInputState` 等（`UiNodeRegistry` / `UiSlot` / `UiMountSlot` / `UiRoot` は `hw_core::ui_nodes` から re-export）
- `theme.rs` — `UiTheme` Resource（カラーパレット・フォントサイズ・スペーシング・サイズ定数）
- `intents.rs` — `UiIntent` enum（プレイヤー UI 操作メッセージ。Stockpile単一適用・範囲編集開始を含む）
- `text_input_intents.rs` — `TextInputIntent` enum（non-`Copy` テキスト確定イベント、例: `RenameSoul`）
- `widgets/text_field.rs` — 再利用可能 `spawn_text_field` ヘルパー、`TextFieldRole`
- `interaction/text_field.rs` — フォーカス枠・Enter/Escape・検索ライブ sync
  - `text_input_consumed_keyboard` は `InputFocusSystems::Dispatch` 前にリセットし、Enter/Escape 適用は dispatch 後に行う
  - 検索 sync の登録責務は `bevy_app` の entity list plugin 側が持ち、`EditableTextSystems` 後に値を読む
- `interaction/` — tooltip/dialog/hover_action/status_display システム群（FPS, speed, dream pool, area_edit_preview 等）
- `list/` — EntityListDirty, EntityListViewModel, EntityListNodeIndex, FamiliarSectionNodes, EntityListMinimizeState, EntityListResizeState, DragState, spawn（`spawn_familiar_section`, `spawn_soul_list_item_entity` 等）, sync（`sync_familiar_sections`, `sync_unassigned_souls`）, section_toggle（`entity_list_section_toggle_system`）, selection_focus, tree_ops, visual（apply_row_highlight, entity_list_visual_feedback_system）
- `panels/tooltip_builder/` — text_wrap, widgets (spawn_progress_bar 等), templates（Soul/Building/Resource/UiButton/Generic ツールチップ）
- `panels/info_panel/` — InfoPanelPinState, InfoPanelState, spawn_info_panel_ui, info_panel_system
- `panels/task_list/` — `TaskEntry`、status/reason、filter/sort、action capability/state、work_type_icon、
  render（focus rowとaction barをsibling生成）、pure UI interaction。ゲームowner判定やcomponent mutationは持たない
- `panels/menu.rs` — menu_visibility_system
- `models/inspection/` — EntityInspectionModel, EntityInspectionViewModel, SoulInspectionFields, StockpileInspectionFields
- `notifications/` — `UserFacingNotification`、`NotificationCenter`、2秒dedupe、4秒toast expiry、toast 3件／重要履歴64件のreducerとUI。ゲーム固有outcome型には依存しない
- `selection/` — SelectionIntent, cleanup_selection_references_system, typed placement validation / feedback / area plan API（`SelectedEntity` / `HoveredEntity` / `SelectionIndicator` は `hw_core` から re-export）
- `camera.rs` — `world_cursor_pos`（スクリーン座標→ワールド座標変換ユーティリティ。`MainCamera` は `hw_core` から re-export）
- `plugins/` — UiCorePlugin / UiEntityListPlugin / UiFoundationPlugin / UiInfoPanelPlugin / UiTooltipPlugin（fn ポインタ受け付けシェル）
- **`area_edit/`** — エリア選択・編集状態の純粋データ型（`AreaEditHandleKind`, `AreaEditOperation`, `AreaEditDrag`, `AreaEditSession`, `AreaEditHistory`, `AreaEditHistoryEntry`, `AreaEditClipboard`, `AreaEditPresets`）。`AreaEditClipboard` 等は `bevy_app/command/area_selection.rs` から直接 `pub use hw_ui::area_edit::*` として re-export。`AreaEditHandleKind` は `bevy_app/command/mod.rs` からも re-export。`area_edit/interaction.rs` に `detect_area_edit_operation`・`apply_area_edit_drag`・`cursor_icon_for_operation` の pure helper を所有（M1 移設済み）

ここに置かないもの:

- ゲームエンティティ（DamnedSoul, Familiar, Blueprint, Door 等）の ECS Query
- `BuildContext`, `ZoneContext`, `TaskContext`（app_contexts）への依存
- `GameAssets` の直接参照（`UiAssets` トレイト経由に抽象化する）
- `Res<GameAssets>` をシステム引数に取るシステム関数（Bevy の `Res<T>` はトレイトオブジェクト不可）
- PlayMode 遷移ロジック（`NextState<PlayMode>`）
- WorldMap / WorldMapWrite への依存

root 側の `bevy_app/src/interface/ui/` 残留（Adapter 層 — ViewModel / Presenter / Intent）:

| ファイル/モジュール | 層 | 残留理由 |
|:---|:---|:---|
| `list/view_model.rs`, `list/change_detection.rs` | ViewModel | `Familiar`, `DamnedSoul`, `AssignedTask` 等の ECS Query |
| `panels/task_list/view_model.rs`, `panels/task_list/dirty.rs` | ViewModel | `Designation`, producer diagnostics、owner marker 等のゲームクエリ |
| `presentation/` | ViewModel | `EntityInspectionQuery`（ゲームエンティティ 10+ 型のクエリ集約） |
| `list/sync.rs` | Presenter | `hw_ui::list::sync` への thin shell（`GameAssets` 注入） |
| `panels/task_list/presenter.rs`, `panels/task_list/update.rs` | Presenter | ViewModel → `hw_ui` render 橋渡し。`update.rs` は `Res<GameAssets>` 必須 |
| `panels/task_list/actions.rs` | Intent / Adapter | live capability再検証、owner別priority/cancel、`TaskActionOutcome`変換。`hw_ui`へゲーム型を逆依存させない |
| `interaction/intent_context.rs`, `interaction/handlers/`, `interaction/intent_handler.rs` | Intent | `BuildContext`, `ZoneContext`, `FamiliarOperation`, `TimeSpeed`, `WorldMapWrite` 等のゲーム依存 `UiIntent` 処理。Stockpile操作はtyped requestへ変換し、domain mutationは`hw_logistics`へ委譲 |
| `interaction/mode.rs` | Intent | `PlayMode` 遷移、`TaskMode`, `BuildingType` |
| `notifications.rs` | Presenter | `SaveLoadOutcome` / `StockpilePolicyChangeOutcome`をsafeな`UserFacingNotification`へexhaustiveに変換（task action adapterは`panels/task_list/actions.rs`） |
| `list/interaction.rs`, `list/interaction/navigation.rs` | Intent | 行クリック・Tab 巡回・target 付き `UiIntent` 発行（SectionToggle は hw_ui 側） |
| `list/drag_drop.rs` | Intent | `SquadManagementRequest`, `SoulIdentity`（`DragState` 型は hw_ui） |
| `panels/context_menu.rs` | Intent | `Familiar`, `DamnedSoul`, `Building`, `Door` の分類 |
| `vignette.rs` | Presenter | `TaskContext`（DreamPlanting モード判定） |

- `hw_visual` はビジュアルシステム全体を集約し、`GameAssets` には依存しない。`hw_visual::handles` の 8 Resource（`WallVisualHandles` 等）は root の `init_visual_handles` startup システムが `GameAssets` から注入する。`SoulTaskHandles` は `hw_core::visual`、`DoorVisualHandles` は `hw_world`、`ResourceItemVisualHandles` は `hw_logistics` が所有し、同じ startup 注入パターンを共有する。`terrain_visual` はアセット handle を持たず、source-aware WorldMap 同期と `TerrainChangedEvent` 発行だけを担う。visual marker (`FadeOut`, `WheelbarrowMovement`) は `hw_core::visual` に置き、`hw_familiar_ai` / `hw_soul_ai` / `hw_visual` の共有型として扱う。Dream 系 UI 連携で読む shared contract (`MainCamera`, `SelectedEntity`, `UiNodeRegistry`, `UiMountSlot` など) も `hw_core` 側に寄せ、`hw_visual` が `hw_ui` に直接依存しない構成へ整理済み。
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
- `selection_indicator::update_selection_indicator` — 選択エンティティを追従する黄色スプライト indicator（`SelectionIndicator` コンポーネントは `hw_core::selection` が所有。実装は `hw_visual`、登録は同フレーム反映のため root `Interface` フェーズで行う）

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
- `hw_jobs::tasks::AssignedTask`（`crates/hw_jobs/src/tasks/mod.rs`）— タスク実行バリアントの正本。`soul_ai::execute::task_execution::types`はhandler向けの選択的re-export
- `soul_ai::execute::task_execution::context::{TaskExecutionContext, TaskQueries, TaskAssignmentQueries, ConstructionSiteAccess}` — タスク実行/割り当て用 context・SystemParam
- `soul_ai::execute::task_execution::handler::{TaskHandler, run_task_handler, execute_haul_with_wheelbarrow}` — タスクハンドラトレイト・ディスパッチ
- `soul_ai::execute::task_execution::{gather, build, coat_wall, collect_bone, collect_sand, frame_wall, haul, haul_to_blueprint, haul_to_mixer, move_plant, pour_floor, refine, reinforce_floor, common}` — 各タスク種別実装
- `soul_ai::execute::task_execution::{haul_with_wheelbarrow, bucket_transport, transport_common}` — 輸送系タスク実装
- `soul_ai::execute::task_execution::stockpile_policy` — live `IncomingDeliveries` から committed / unreserved を区別し、通常搬送とmixed猫車batchを共通Stockpile evaluatorへ接続するruntime adapter
- `soul_ai::decide::work::auto_refine` — MudMixer の自動精製指定発行
- `soul_ai::decide::work::auto_build` — 資材完了 Blueprint への自動割り当て
- `soul_ai::decide::work::auto_build_diagnostics` — auto-build producer の latest-only coverage / reason snapshot
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
- `familiar_ai::decide::task_management::policy_score` — base worker score後にtransport / Familiarのscalar contributionを合成する共有no-clamp score helper
- `familiar_ai::decide::task_management::diagnostics` — internal typed rejection、Familiar-local 1票reducer、
  `FamiliarTaskCandidateDiagnostics`。UI表示に依存せず通常delegation cycleで置換publishする
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
- `familiar_ai/perceive/resource_sync.rs` — root perceive system。`SharedResourceCache` snapshot と `ReservationSignatureCache` を実ワールドから同期し、load 後に再構築する責務は root に残す

移設済み system の登録ルール:

- 実装本体を `hw_familiar_ai` / `hw_soul_ai` / `hw_visual` / `hw_jobs` へ移した system は、原則として所有 crate の Plugin が唯一の登録者になる。
- root 側の `pub use` / thin shell は互換パス維持と ordering 参照のために残してよいが、同じ system function を再登録してはいけない。
- root shell は `.after(...)` / `.before(...)` で移設済み system に順序制約を付けるだけにとどめる。二重登録すると Bevy 0.19 の schedule 初期化で `SystemTypeSet` が曖昧になり panic する。
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
- `lib.rs`: 共有 Resource、公開 module、root re-export、library unit testの入口
- `main.rs`: process設定の解釈と window / render / backend設定、`HellWorkersGamePlugin` の追加
- `plugins/game.rs`: production game resource / state / `GameSystemSet` chain と parent game plugin の一意な登録
- `input_actions/`: project-owned keyboard edge の唯一の resolver、pending/visible Modal/Pause capture、foreground UI gate、capture-start rollback
- `systems/save/`: persisted schema/transactionと、requestごとに全reset後1件だけ発行する`SaveLoadOutcome`

ここに残すもの:

- plugin 定義
- startup / visual / UI system
- ECS resource と shell system
- root 側の互換 re-export 層
- `GameAssets`・sprite spawn・root 固有 resource を伴う adapter
- `hw_world::WorldMapRead/Write`、pathfinding context、full-fat query を扱う root adapter system
- request 消費時に app 側状態を再検証して副作用を確定する adapter
- `plugins/logic.rs` の scheduling facade。特に command 系 chain、maintenance/spawn 系の非 chain 登録、floor/wall construction の phase chain、room detection の `.after(dream_tree_planting_system)` は root の唯一の ordering 契約として保持する

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
- `GameSettings`（`hw_core::settings`）— 永続化対象のゲーム設定 Resource（型定義のみ）。RON ロード/保存・反映・intent 処理は `bevy_app/systems/settings/`、設定画面 UI は `hw_ui`。write は bevy_app のみ（詳細: [docs/settings.md](settings.md)）

### `hw_energy`

役割:

- Soul Energy システム専用の型・定数・Relationship を集約するドメインクレート
- `hw_core` に依存しない独立 leaf crate（依存: `bevy` のみ）

代表例:

- `constants::{OUTPUT_PER_SOUL, DREAM_CONSUME_RATE_GENERATING, DREAM_GENERATE_FLOOR, OUTDOOR_LAMP_DEMAND, OUTDOOR_LAMP_EFFECT_RADIUS, SOUL_SPA_BONE_COST_PER_TILE, FATIGUE_RATE_GENERATING}`
- `components::{PowerGrid, PowerGenerator, PowerConsumer, Unpowered, YardPowerGrid}`
  - `PowerGrid` — Yard に 1 対 1 で存在する電力網エンティティ（generation / consumption / powered を保持）
  - `PowerGenerator` — SoulSpaSite に付与するサイト単位の発電集計コンポーネント（Phase 1b で使用開始）
  - `PowerConsumer` — 電力消費建物に付与。`#[require(Unpowered)]` で未接続時のデフォルトを停電側に設定
  - `Unpowered` — 停電マーカー。グリッド再計算で除去/再挿入される
  - `YardPowerGrid` — PowerGrid エンティティ上に付与。所属 Yard への逆参照
- `relationships::{GeneratesFor, GridGenerators, ConsumesFrom, GridConsumers}`
  - `GeneratesFor` — SoulSpaSite → PowerGrid（発電機グリッド登録）
  - `ConsumesFrom` — OutdoorLamp 等 → PowerGrid（消費者グリッド登録）

ここに置かないもの:

- `hw_core` 等の他 hw_* クレートへの依存
- Grid 再計算・ランプバフ等のシステムロジック（`bevy_app/src/systems/energy/` が担当）
- Soul Spa / OutdoorLamp の建物型定義（`hw_jobs::model::BuildingType` が所有）

仕様詳細: [soul_energy.md](soul_energy.md)

### `hw_world`

役割:

- world の純粋ロジック
- pathfinding, terrain, map helper, 座標変換
- room detection core（入力分類、flood-fill、validator、`RoomBounds`）
- AI helper が使用する read-only 空間トレイト

代表例:

- terrain / river / mapgen / regrowth
- spawn grid helper
- `WorldMap`
- `WorldMapRead`, `WorldMapWrite` — `WorldMap` access を system 境界で統一する `SystemParam`
- `world_to_grid`, `grid_to_world`
- nearest walkable / river query
- `AnchorLayout`, `GridRect` — `Site/Yard` と Yard 内固定物の pure data 契約。本番は `AnchorLayout::aligned_to_worldgen_seed` が `river::preview_river_min_y` を用い川南端より南へ Site 北辺をオフセット（`docs/world_layout.md`）
- `WorldMasks` — `site_mask`, `yard_mask`, `river_mask`, protection band, `river_centerline`
- `mapgen::mod.rs` / `mapgen::pipeline` — `mapgen` の公開 shell と WFC パイプライン本体。`generate_world_layout` は module root から再公開し、オーケストレーション実装は `pipeline.rs` に置く
- `generate_world_layout` — WFC ベースの地形生成エントリ（`river_mask` / `final_sand_mask` / `rock_field_mask` を確定後にソルバー実行し、resource 配置と retry/fallback を含めて最終 `GeneratedWorldLayout` を返す）
- `mapgen::wfc_adapter` — gridbugs `wfc` を局所化する adapter（`run_wfc`, `post_process_tiles`, `WorldConstraints` 等）
- `test_seeds` (`#[cfg(test)]`) — WFC 周辺テストの代表 seed 定義を共有する crate 内モジュール
- `room_detection::{build_detection_input, detect_rooms, room_is_valid_against_input}`
- `PathWorld` trait — `is_walkable` など通行判定 API（`WorldMap` の impl も `hw_world` が所有）
- `SpatialGridOps` trait — `get_nearby_in_radius` など空間グリッド read-only API（concrete resource の本体は `hw_spatial`）
- `Yard`, `Site`, `PairedYard`, `PairedSite`（zone 系コンポーネント）
- `zone_ops::identify_removal_targets` — 削除対象タイル + 孤立フラグメント特定（Flood Fill）
- `zone_ops::area_tile_size`, `rectangles_overlap_site`, `rectangles_overlap`, `expand_yard_area` — ゾーン geometry helper
- `terrain_visual::{ObstaclePositionIndex, obstacle_sync_system, TerrainChangedEvent}` — runtime marker の旧位置/source を index し、source-aware に WorldMap を同期する。自然物由来の最後の blocker を外した場合だけ terrain visual 更新を通知する
- `door_systems::{DoorVisualHandles, apply_door_state, evaluate_door_auto_open, soul_keeps_door_open}` — ドア通行状態/スプライト更新と1候補のpure判定。近傍index adapterは`hw_spatial`、`GameAssets`はrootから専用Resourceで注入する
- **`Room`, `RoomOverlayTile`（Component）** — Room ECS 型
- **`RoomTileLookup`, `RoomDetectionState`, `RoomValidationState`（Resource）** — Room 管理リソース
- **`DreamTreePlantingPlan`（`tree_planting.rs`）** — Dream 植林計画の純粋データ構造。ビルダー関数（`build_dream_tree_planting_plan`）は `GameAssets`/`DreamPool` 依存のため bevy_app に残留
- **`room_systems::{detect_rooms_system, validate_rooms_system, mark_room_dirty_from_building_changes_system, on_building_added, on_building_removed, on_door_added, on_door_removed, sync_room_overlay_tiles_system}`** — Room ECS adapter 層（検出・検証・dirty マーキング・オーバーレイ同期をすべて所有）。`bevy_app/src/systems/room/` は削除済みで、`plugins/logic.rs`・`plugins/visual.rs` が直接 import する

ここに置かないもの:

- root 固有アセットを前提にした初期 sprite spawn
- `GameAssets` への直接依存
- root 側の startup / plugin wiring / entity spawn facade
- `SpatialIndex<Tag>` storage、crate 所有 tag、標準 Transform updater は `hw_spatial` が保持する。9 個の concrete resource 名は type alias で、`ResourceItem` / `Stockpile` / `TransportRequest` の component 特化 wrapper は `hw_logistics` にあり `plugins/spatial` から登録する

補足:

- `hw_world` は WFC 移行のため `wfc` と `direction` に直接依存しているが、その利用点は `mapgen::wfc_adapter` に閉じ込める

### `hw_spatial`

役割:

- `SpatialIndex<Tag>` の共通storage、`hw_world::SpatialGridOps`のconcrete impl、標準Transform updater
- `GridData` と空間検索ヘルパの共通化、crate 所有 ZST tag
- 2D 空間スナップショットの初期化時の query 補助

ここに置くもの:

- `SpatialGrid`, `FamiliarSpatialGrid`, `BlueprintSpatialGrid`, `DesignationSpatialGrid`, `ResourceSpatialGrid`, `StockpileSpatialGrid`, `TransportRequestSpatialGrid`, `GatheringSpotSpatialGrid`, `FloorConstructionSpatialGrid`
- `SoulIndexTag` などの index tag と `SpatialIndex<Tag>`。Resource の Visibility と Gathering の center / Added-only policy は専用 updater として保持する
- `SpatialIndex<Tag>::generation()`。membershipまたは記録位置の実変更だけで進み、rootのtask availability revisionへ入力する
- `door_proximity` — Soul indexを使うdoor auto-open/close adapterと`DoorPerfMetrics`

ここに置かないもの:

- root `WorldMap` shell、`WorldMapRead/Write`、startup/wiring

### `hw_logistics`

役割:

- 物流の共有 model / helper
- transport request の完全な実行ロジック（producer / arbitration / plugin）
- GameAssets・UI に依存しない物流システムの集約

代表例:

- `ResourceItem`, `Wheelbarrow`, `Stockpile`, `StockpilePolicy`, `StockpileAcceptance`, `StockpilePolicyPatch`
- water / ground resource helper
- `TransportRequest*`, `ReceiverPolicyTier`, `TransportRequestPlugin`, `TransportRequestSet`
- transport metrics / state sync / lifecycle cleanup
- `ManualTransportCloseContext` / `close_manual_transport_request` — UI cancelとanchor cleanupが共用するowner close API
- `WheelbarrowArbitrationDiagnostics` — arbitration既存走査から公開するlatest-only typed outcome/header
- `SharedResourceCache`（タスク間リソース予約 cache。frame-local delta と reservation snapshot を分離して保持）
- `apply_reservation_op` / `apply_reservation_requests_system`（予約操作の反映 helper）
- **`LogisticsPlugin`**：`apply_reservation_requests_system` を `SoulAiSystemSet::Execute` に登録する Plugin（`src/plugin.rs`）
- `TileSiteIndex`（タイル→サイト逆引き）
- `construction_phase_transition` — index count/entity uniqueness/owner/stateを全検証してからfloor/wall phaseを原子的に進めるadapterと`ConstructionPerfMetrics`
- `construction_helpers::{ResourceItemVisualHandles, spawn_refund_items}` — `GameAssets` 依存を root 注入 Resource に抽象化した建設キャンセル共通 helper
- producer 全系（`blueprint`, `bucket`, `consolidation`, `mixer`, `task_area`, `wheelbarrow`, `floor_construction`, `wall_construction`, `provisional_wall` 等）
- `manual_haul_selector::{select_stockpile_anchor, find_existing_request}` — 手動 haul 選定アルゴリズム（`DesignationTargetQuery` 非依存）
- `stockpile_policy::{evaluate_stockpile_policy, StockpilePolicyInput, StockpileTransferPhase}` — producer / grant / assignment / executionが共有する唯一の方針判定
- `stockpile_policy_change::{StockpilePolicyChangeRequest, StockpilePolicyChangeOutcome, apply_stockpile_policy_change_requests_system}` — 単一・範囲編集のlive再検証とdomain mutation

補足:

- `apply_reservation_requests_system` は `hw_logistics::LogisticsPlugin` が `SoulAiSystemSet::Execute` に登録する。`ResourceReservationRequest` の `add_message`、`SharedResourceCache`、`ReservationSyncTimer`、`ReservationSignatureCache` の `init_resource` は root app shell が担当する。`profiling` feature時の `ReservationSyncPerfMetrics` も root が登録する。Entity を持つ signature cache と、その再構築を保証する同期 timer は root の load reset inventory に属する。

ここに置かないもの:

- `GameAssets` 依存の初期スポーン（`crates/bevy_app/src/systems/logistics/initial_spawn/` は root 残留。`mod.rs` が facade、`layout.rs` / `terrain_resources.rs` / `facilities.rs` / `report.rs` に責務分割）
- UI ロジスティクス表示

### `hw_jobs`

役割:

- jobs の共有 model
- building / blueprint / designation 系の基礎型
- `hw_core::visual_mirror` へ状態を写す軽量な sync system / observer 群（登録責務は root）

代表例:

- `BuildingType`, `Building`, `Blueprint`
- `Designation`, `Priority`, `TaskSlots`
- `PlayerIssuedDesignation` — 手動 Chop / Mine の保存可能なpositive provenance marker
- `diagnostics` — `TaskDiagnosticClass`、producer/coverage、fixed counter、input stamp/revisionの表示非依存共有契約
- `MudMixerStorage`
- `AssignedTask`（ワーカー実行中タスク状態 + 全フェーズ型）
- `TaskAssignmentRequest`（`hw_jobs::events`）
- `lifecycle::{ReservationSignature, collect_active_reservation_ops, active_reservation_signature}`（予約 operation の正規化と比較用 signature）
- `MovePlanned`（建物移動タスクの計画状態）
- `Door`, `DoorCloseTimer`
- `FloorConstructionSite`, `WallConstructionSite`（親 site component）
- `FloorTileBlueprint`, `WallTileBlueprint`（タイル単位建設状態）
- `FloorTileState`, `WallTileState`（建設フェーズ enum）
- `TargetFloorConstructionSite`, `TargetWallConstructionSite`
- `FloorConstructionCancelRequested`, `WallConstructionCancelRequested`
- `remove_tile_task_components`（designation 系コンポーネントの一括 remove helper）
- `visual_sync::{observers,sync}` — Gather/RestArea/Building/MudMixer/AssignedTask/Construction の visual mirror 同期関数群

ここに置かないもの:

- floor / wall constructionのcancel/completion、asset依存spawn、production ordering（rootが所有）
- building completion shell
- doorのindex candidate抽出（`hw_spatial`）とproduction登録（root）
- `add_systems` / `add_observer` の plugin 登録責務（`bevy_app/src/plugins/logic.rs` が担当）

## 4. どこに置くかの判断基準

### `hw_core` に置く

- 複数ドメインから参照される
- Bevy app shell から独立している
- 安定した基礎型として使いたい

### `hw_world` に置く

- world/map/pathfinding の純粋ロジック
- `WorldMap` を trait や引数で抽象化できる
- root 固有の `GameAssets` を直接参照しない world 系 system
- `GameAssets` の一部フィールドだけが必要な場合は、`DoorVisualHandles` のような専用 Resource を leaf crate 側で定義し、root の startup system が注入する

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
| `crate::plugins::spatial` | `SpatialPlugin` — `hw_spatial` / `hw_logistics` のグリッド更新を `GameSystemSet::Spatial` に登録する app-shell 入口（`systems/spatial` モジュールは廃止済み） |
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
- mapgen / regrowth の純粋ロジック（地形境界オーバーレイ `borders` / `terrain_border` は MS-3-4 で廃止済み）
- `WorldMapRead` / `WorldMapWrite` の `SystemParam`
- `obstacle_sync_system` のような source-aware WorldMap 同期 + 地形ビジュアル通知（`TerrainChangedEvent` → bevy_app で `TerrainIdMap` 更新）
- door state/sprite/WorldMap適用と1候補のpure開閉rule（`DoorVisualHandles`注入）。index candidate抽出は`hw_spatial`

`crates/bevy_app/src/world/map/spawn.rs`, `crates/bevy_app/src/world/regrowth.rs`, `crates/bevy_app/src/systems/logistics/initial_spawn/` は app shell です。地形スポーンは `spawn_map` が `WorldMap.tile_entities` に紐づく `Tile` 論理 anchor を登録し、`spawn_terrain_chunks` が `TerrainSurfaceMaterial` / `Terrain3dHandles` を使って chunk render entity を生成する構成になっています。これらは `GameAssets`, `Commands`, `Resource` を扱い、純粋ロジックと `WorldMap` access wrapper は `hw_world` から呼び出します。startup は `GeneratedWorldLayout` を root Resource に包んで 1 回だけ生成し、`TerrainFeatureMap` と `TerrainIdMap` をその snapshot から焼き、地形描画・初期木/岩・初期木材・猫車置き場・regrowth 初期化が同じ layout を共有します。

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
