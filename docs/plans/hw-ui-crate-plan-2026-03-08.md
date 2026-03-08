# hw_ui crate 分離 実装計画

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `hw-ui-crate-plan-2026-03-08` |
| ステータス | `InProgress` |
| 作成日 | `2026-03-08` |
| 最終更新日 | `2026-03-08` |
| 作成者 | `AI` |
| 関連提案 | `docs/proposals/hw-ui-crate.md` |
| 関連Issue/PR | `N/A` |

## 1. 目的

- **解決したい課題**: `src/interface/` は 94 ファイルあり、`src/plugins/interface.rs` から root crate に直接組み込まれている。UI の変更でも root crate 全体の再コンパイルが走りやすく、依存境界も `mod` 単位に埋もれている。
- **到達したい状態**: `crates/hw_ui/` を追加し、UI の描画・レイアウト・汎用インタラクションを leaf crate へ寄せる。root crate は `InterfacePlugin` の shell、Presentation Model 構築、`WorldMap`/selection/ゲーム状態変更ハンドラに責務を絞る。
- **成功指標**:
  - `CARGO_HOME=/home/satotakumi/.cargo cargo check -p hw_ui` が成功する
  - `CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check` が成功する
  - `src/interface/ui/{plugins,setup,panels,list}` が `hw_ui` へ移動済み、または root 側 thin wrapper のみになる
  - `src/plugins/interface.rs` が `hw_ui` plugin 登録と root shell adapter 接続を行う薄い層（30行以下）になる

## 1.5. 実装戦略（実行順）

1. `M1` と `M2` は前提整備のため直列実行し、`TaskMode` / `TimeSpeed` の定義移動が完了してから `UiIntent` を本実装に進める。
2. `M3` を境界実装として先に確定し、`MenuAction` を `UiIntent` に置換した後に UI 資産の移設 (`M4` 以降) を進める。
3. `M4`/`M5` は `crates/hw_ui` の境界固定を優先し、`src/interface/ui/presentation` と `list/change_detection` の責務分離を順次反映する。
4. `M6`/`M7` では `Change` 順序とイベント順序の回帰を最優先し、再コンパイル・実動作確認を先行する。
5. `M8` はドキュメント・境界記述の最終整合として実施し、変更点を必ず `docs/` 側へ反映して完了とする。

### 運用ルール

- 各マイルストーン完了時点で `CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check` が通ることを前提条件とする。
- `hw_ui` 側に `crate::systems` / `crate::entities` / `crate::world` / `crate::app_contexts` 参照が混入した場合は、原因 file を即座にロールバック候補にし再設計する。
- 既存 plugin 登録順 (`UiFoundation → UiCore → UiTooltip → UiInfoPanel → UiEntityList`) を壊さない。

## 2. スコープ

### 対象（In Scope）

- `crates/hw_ui/` の新設と workspace 追加
- `TaskMode` / `TimeSpeed` の `hw_core` への移動（`hw_ui` が root 型に依存しないための前提条件）
- UI 基盤 (`theme`, `UiSlot`, `UiNodeRegistry`, `MenuState`, `LeftPanelMode`, setup, panels, list, tooltip, dialog) の crate 移動
- `MenuAction` を廃止し、`hw_ui` から root へ intent を渡す `UiIntent` 境界の新設
- `EntityInspectionModel` パターンを全パネルに拡張し、root adapter 化
- root 側の `InterfacePlugin` / adapter / shell の整理
- 関連ドキュメント更新（`docs/architecture.md`, `docs/cargo_workspace.md`, `docs/README.md`）

### 非対象（Out of Scope）

- UI デザイン変更
- `src/systems/visual/` の crate 分離
- `debug_spawn_system` の削除や挙動変更
- `WorldMap` resource / `WorldMapRead` / `WorldMapWrite` の移動
- `src/interface/selection/`（21ファイル）の crate 移動（WorldMap/unassign_task 依存が強い）
- `src/interface/ui/vignette.rs` の移動（`TaskContext` 依存のため root shell 残留）

## 3. 現状とギャップ

### 現状（コード調査済み）

**`src/plugins/interface.rs` の plugin 登録順（この順序は変更禁止）**:
```rust
app.add_plugins((
    UiFoundationPlugin,   // 1. theme / UiSlot / UiNodeRegistry 初期化
    UiCorePlugin,         // 2. setup / interaction の core
    UiTooltipPlugin,      // 3. tooltip ホバー系
    UiInfoPanelPlugin,    // 4. info panel / context menu
    UiEntityListPlugin,   // 5. entity list / drag-drop / resize
))
.add_systems(Update, debug_spawn_system ...); // 6. デバッグ専用 → root 残留
```

**`src/interface/ui/components.rs` の `MenuAction` — root 型依存一覧**:

| Variant | 依存する root 型 | 分類 |
|---------|----------------|------|
| `SelectBuild(BuildingType)` | `crate::systems::jobs::BuildingType` | → `hw_jobs` 経由で hw_ui 参照可 |
| `SelectZone(ZoneType)` / `RemoveZone(ZoneType)` | `crate::systems::logistics::ZoneType` | → `hw_logistics` 経由で参照可 |
| `SelectArchitectCategory(Option<BuildingCategory>)` | `crate::systems::jobs::BuildingCategory` | → `hw_jobs` 経由で参照可 |
| `SelectTaskMode(TaskMode)` | `crate::systems::command::TaskMode` | **要 hw_core 移動** |
| `SetTimeSpeed(TimeSpeed)` | `crate::systems::time::TimeSpeed` | **要 hw_core 移動** |
| `InspectEntity(Entity)` / `ToggleDoorLock(Entity)` / `MovePlantBuilding(Entity)` | `Entity`（汎用） | そのまま参照可 |
| その他 (`Toggle*`, `Clear*`, `Open*`, `Close*`, etc.) | なし | そのまま移動可 |

**`src/interface/ui/presentation/mod.rs` の root 直接依存**:
```rust
use crate::entities::damned_soul::{DamnedSoul, Gender, IdleBehavior, IdleState};
use crate::entities::familiar::Familiar;
use crate::systems::jobs::Blueprint;
use crate::systems::soul_ai::execute::task_execution::AssignedTask;
use crate::systems::soul_ai::perceive::escaping::is_escape_threat_close;
use crate::systems::spatial::FamiliarSpatialGrid;
// + Inventory, Tree, Rock, Designation, Building, Stockpile, StoredItems,
//   MudMixerStorage, RestArea, RestAreaOccupants など 10 以上の root 型
```
→ `EntityInspectionQuery` SystemParam がこれらを全て直接 query している。ViewModel 化必須。

**`src/interface/ui/list/change_detection.rs` の root 直接依存**:
```rust
// detect_entity_list_changes が直接 query するコンポーネント:
Changed<DamnedSoul>, Added<DamnedSoul>, RemovedComponents<DamnedSoul>
Changed<AssignedTask>
Changed<SoulIdentity>
Changed<Familiar>, Added<Familiar>, RemovedComponents<Familiar>
Changed<FamiliarAiState>, Changed<FamiliarOperation>
Changed<Commanding>, Changed<CommandedBy>, RemovedComponents<CommandedBy>
Changed<SectionFolded>, Changed<UnassignedFolded>
```
→ change detection は root adapter に残し、`EntityListDirty` resource だけ `hw_ui` が消費する。

**`src/interface/selection/building_move/mod.rs` の重依存**:
```rust
use crate::world::map::{WorldMap, WorldMapWrite};
use crate::systems::soul_ai::helpers::work::unassign_task;
// + TaskUnassignQueries, AssignedTask variants (Refine, HaulToMixer, MovePlant, BucketTransport)
```
→ 今回スコープ外。root shell 残留。

### 問題まとめ

1. `MenuAction::SelectTaskMode` / `SetTimeSpeed` で参照する `TaskMode` / `TimeSpeed` が root crate にあり、`hw_ui` に直接持ち出せない。
2. `presentation/mod.rs` の `EntityInspectionQuery` が 10 以上の root domain 型に直接依存しており、`hw_ui` へそのまま移せない。
3. `list/change_detection.rs` が DamnedSoul / Familiar / AssignedTask の変化を直接検知しており、`hw_ui` の leaf crate 化を妨げている。
4. UI の描画とゲーム状態変更（door lock → WorldMapWrite、build/zone 選択 → context 変更）が `interaction/mod.rs` に混在している。

### 本計画で埋めるギャップ

- `TaskMode` / `TimeSpeed` を `hw_core` へ移動し（M2）、`hw_ui` の `UiIntent` から参照可能にする。
- Presentation Model を root adapter が生成し、`hw_ui` は resource 読み取りのみ行う形に整理する（M3）。
- `interaction/` の世界状態変更を root handler に委譲し、`hw_ui` は `UiIntent` event 発行のみにする（M5）。

## 4. 実装方針

- **基本方針**: full 移動を一気に狙わず、`hw_ui` を「表示と UI 入力の leaf crate」、root を「ゲーム状態 adapter / shell」とする段階分離で進める。
- **`hw_ui` の依存関係**:
  ```toml
  # crates/hw_ui/Cargo.toml
  [dependencies]
  bevy = { workspace = true }
  hw_core  = { path = "../hw_core"  }   # TaskMode, TimeSpeed, GameSystemSet, etc. (M2 後)
  hw_jobs  = { path = "../hw_jobs"  }   # BuildingType, BuildingCategory, Blueprint
  hw_logistics = { path = "../hw_logistics" }  # ZoneType, Stockpile
  ```
  `hw_world`, `hw_spatial`, `hw_ai` は **依存しない**（WorldMap, FamiliarSpatialGrid は ViewModel 経由）。
- **root shell に残す対象**（変更しない）:
  - `src/interface/selection/`（21ファイル）
  - `src/interface/camera.rs`
  - `src/interface/ui/vignette.rs`
  - `debug_spawn_system` in `src/plugins/interface.rs`
  - `src/interface/ui/presentation/mod.rs`（root adapter として残す）
  - `src/interface/ui/list/change_detection.rs`（root adapter として残す）
- **Bevy 0.18 注意点**:
  - `Changed<Interaction>` 読み取りシステムは既存 `chain` / 登録順を維持する。
  - Plugin 登録順 `UiFoundation → UiCore → UiTooltip → UiInfoPanel → UiEntityList` を崩さない。
  - `Event` / `Message` の `add_event` 呼び出しは plugin 側で集中管理（重複登録を避ける）。

### 4.1 採用する境界

| 区分 | 置き場所 | 代表型 |
| --- | --- | --- |
| UI テーマ・スロット・ノード registry・UI 専用 resource | `hw_ui` | `UiSlot`, `UiNodeRegistry`, `LeftPanelMode`, `MenuState`, `UiInputState`, `PlacementFailureTooltip` |
| UI 専用コンポーネント | `hw_ui` | `MenuButton`, `HoverActionOverlay`, `HoverTooltip`, `TooltipTemplate`, `InfoPanel`, `EntityListPanel`, etc. |
| UI セットアップ | `hw_ui` | `setup/` 全ファイル（bottom_bar, dialogs, entity_list, panels, submenus, time_control） |
| UI 表示用 ViewModel（生成は root） | root 生成 / `hw_ui` 消費 | `EntityInspectionModel`, `EntityListViewModel`, `SoulRowViewModel` |
| UI 入力の意図表明 | `hw_ui` → Event | `UiIntent`（新設）|
| UI ビジュアル補助 | `hw_ui` | `interaction/common.rs`, `interaction/hover_action.rs`, `interaction/dialog.rs`, tooltip 全体 |
| ゲーム状態変更 | root | `interaction/menu_actions.rs`（root handler へ）, `interaction/mode.rs`（root handler へ）, `door_lock_action_system` |
| camera/debug/placement/vignette shell | root | `camera.rs`, `selection/`, `vignette.rs`, `debug_spawn_system` |

### 4.2 `UiIntent` 設計

```rust
// crates/hw_ui/src/intents.rs
#[derive(Message, Copy, Clone, Debug)]
pub enum UiIntent {
    // ゲームモード選択
    SelectTaskMode(hw_core::TaskMode),       // ← TaskMode は hw_core へ移動後
    SetTimeSpeed(hw_core::TimeSpeed),        // ← TimeSpeed は hw_core へ移動後
    TogglePause,
    // 建築・ゾーン
    SelectBuild(hw_jobs::BuildingType),
    SelectZone(hw_logistics::ZoneType),
    RemoveZone(hw_logistics::ZoneType),
    SelectArchitectCategory(Option<hw_jobs::BuildingCategory>),
    SelectFloorPlace,
    SelectAreaTask,
    SelectDreamPlanting,
    // UI 操作
    ToggleArchitect,
    ToggleZones,
    ToggleOrders,
    ToggleDream,
    InspectEntity(Entity),
    ClearInspectPin,
    ToggleDoorLock(Entity),
    OpenOperationDialog,
    CloseDialog,
    AdjustFatigueThreshold(f32),
    AdjustMaxControlledSoul(isize),
    MovePlantBuilding(Entity),
}
```

Root 側 handler (`src/interface/ui/interaction/intent_handler.rs` を新設) が `UiIntent` を受け取り、`TaskContext`, `BuildContext`, `ZoneContext`, `TimeSpeed`, `WorldMapWrite` へ変換する。

### 4.3 採用しない案

- `TaskMode` / `TimeSpeed` を新規共有 crate に切り出す案: `hw_core` に追加すれば十分なため不要。
- `selection/` を M1〜M5 の完了条件に含める案: `WorldMapWrite` + `unassign_task` 依存が強く、独立 milestone として別計画化。

## 5. マイルストーン

### M1: `hw_ui` crate 骨格と workspace 追加

**目的**: コンパイルが通る最小 crate を作り、root から `HwUiPlugin` を登録できる状態にする。

**変更内容**:
1. `Cargo.toml`（workspace root）に `crates/hw_ui` を追加
2. `crates/hw_ui/Cargo.toml` を作成（依存: bevy, hw_core, hw_jobs, hw_logistics）
3. `crates/hw_ui/src/lib.rs` に空の `HwUiPlugin` を作成
   ```rust
   pub struct HwUiPlugin;
   impl Plugin for HwUiPlugin {
       fn build(&self, _app: &mut App) {}
   }
   ```
4. `src/plugins/interface.rs` に `use hw_ui::HwUiPlugin;` と `app.add_plugins(HwUiPlugin)` を追加
5. `docs/cargo_workspace.md` に `hw_ui` の依存関係を記載

**変更ファイル**:
- `Cargo.toml`
- `crates/hw_ui/Cargo.toml` （新規）
- `crates/hw_ui/src/lib.rs` （新規）
- `src/plugins/interface.rs`
- `docs/cargo_workspace.md`

**完了条件**:
- [ ] `CARGO_HOME=/home/satotakumi/.cargo cargo check -p hw_ui` が通る
- [ ] `CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check` が通る
- [ ] root から `HwUiPlugin` が登録されている

**検証コマンド**:
```bash
CARGO_HOME=/home/satotakumi/.cargo cargo check -p hw_ui
CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check
```

---

### M2: `TaskMode` / `TimeSpeed` を `hw_core` へ移動

**目的**: `UiIntent` が root 型に依存せず `hw_core` の型だけで完結できるようにする。`MenuAction` の廃止前提。

**変更内容**:
1. `src/systems/command.rs` の `TaskMode` enum を `crates/hw_core/src/` の適切な module へ移動
   - 移動先候補: `hw_core::game_state` または新設 `hw_core::modes`
   - root 側は `pub use hw_core::TaskMode;` で re-export して既存 import を壊さない
2. `src/systems/time.rs` の `TimeSpeed` enum を `crates/hw_core/src/` へ移動
   - 同様に root 側は re-export
3. `hw_core` を使う全 crate（hw_ai, hw_spatial 等）が `TaskMode`/`TimeSpeed` を参照していれば import パスを更新

**変更ファイル**:
- `crates/hw_core/src/` （新 module または既存 module への追記）
- `src/systems/command.rs` （移動元 + re-export に変更）
- `src/systems/time.rs` （移動元 + re-export に変更）
- `hw_ai` / `hw_spatial` など、TaskMode/TimeSpeed を直接使っているファイル

**完了条件**:
- [ ] `TaskMode` / `TimeSpeed` が `hw_core` に定義されている
- [ ] root 側が re-export しており、既存コードのコンパイルが通る
- [ ] `CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check` が通る

**注意**:
- `TaskMode::DreamPlanting(_)` には内部データがある。型構造を確認してから移動する。
- `src/interface/ui/vignette.rs` が `crate::systems::command::TaskMode` を参照しているため、re-export によって影響を受けないか確認する。

---

### M3: `UiIntent` event と root intent handler の新設

**目的**: `MenuAction` を廃止し、`hw_ui` が `UiIntent` event を発行、root が処理するパターンを確立する。

**変更内容**:
1. `crates/hw_ui/src/intents.rs` を新設（「4.2 UiIntent 設計」の enum を実装）
2. `crates/hw_ui/src/lib.rs` に `app.add_message::<UiIntent>()` を追加
3. `src/interface/ui/interaction/intent_handler.rs` を新設
   - `fn handle_ui_intent(mut events: MessageReader<UiIntent>, mut task_ctx: ResMut<TaskContext>, ...)` を実装
   - `MenuAction` の各分岐を `UiIntent` の match 分岐として書き直す
4. `src/interface/ui/plugins/core.rs` に `ui_interaction_system`, `handle_ui_intent` などを `GameSystemSet::Interface` で登録
5. `src/interface/ui/components.rs` から `MenuAction` enum を削除し、`UiIntent` を `hw_ui` から re-import
6. `src/interface/ui/interaction/menu_actions.rs` を `UiIntent` event 送信に書き換え
7. `src/plugins/messages.rs` から `UiIntent` の重複登録を除去し、`hw_ui` 側 `add_message` のみで管理

**変更ファイル**:
- `crates/hw_ui/src/intents.rs` （新規）
- `crates/hw_ui/src/lib.rs`
- `src/interface/ui/interaction/intent_handler.rs` （新規）
- `src/interface/ui/interaction/menu_actions.rs`
- `src/interface/ui/components.rs`
- `src/interface/ui/plugins/core.rs`
- `src/plugins/messages.rs`（重複登録の確認）

**完了条件**:
- [x] `MenuAction` enum が削除されている（または `UiIntent` への型エイリアスのみ）
- [x] `hw_ui` 側の interaction が `UiIntent` event を発行する形へ更新
- [x] root handler が `UiIntent` を受け取り `PlayMode`/`BuildContext`/`TaskContext`/`ZoneContext`/時間制御を更新している
- [x] `CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check` は、`bevy_app` 側の確認で通過
- [ ] `cargo run` で UI 操作が全件再検証されている

---

### M4: UI 基盤型と setup の `hw_ui` 移動

**目的**: `UiSlot`, `UiNodeRegistry`, `MenuState`, テーマ、セットアップ系を `hw_ui` leaf crate に集約する。

**変更内容**:
1. `src/interface/ui/theme.rs` → `crates/hw_ui/src/theme.rs` へ移動
2. `src/interface/ui/components.rs` の UI ローカル型を `crates/hw_ui/src/components.rs` へ移動
   - 移動対象: `UiSlot`, `UiNodeRegistry`, `LeftPanelMode`, `MenuState`, `UiInputState`, `PlacementFailureTooltip`
   - 移動対象: UI コンポーネント (`MenuButton`, `HoverActionOverlay`, `HoverTooltip`, etc.)
   - root 側は `pub use hw_ui::components::*;` で互換 re-export
3. `src/interface/ui/setup/` の全ファイル（6ファイル）を `crates/hw_ui/src/setup/` へ移動
   - `bottom_bar.rs`, `dialogs.rs`, `entity_list.rs`, `panels.rs`, `submenus.rs`, `time_control.rs`
   - setup 内の `use crate::` を `use hw_jobs::` / `use hw_logistics::` / `use hw_core::` / `use hw_ui::` に変換
4. `src/interface/ui/plugins/foundation.rs` の setup 呼び出し部分を `hw_ui` plugin 内に移動
5. `crates/hw_ui/src/lib.rs` に `HwUiFoundationPlugin` を実装

**変更ファイル**:
- `crates/hw_ui/src/theme.rs` （移動先）
- `crates/hw_ui/src/components.rs` （移動先）
- `crates/hw_ui/src/setup/` （移動先、6ファイル）
- `crates/hw_ui/src/plugins/foundation.rs` （新規、UiFoundationPlugin 相当）
- `src/interface/ui/theme.rs` （削除または re-export に変更）
- `src/interface/ui/components.rs` （re-export のみに縮小）
- `src/interface/ui/setup/` （削除または re-export に変更）
- `src/interface/ui/plugins/foundation.rs` （hw_ui を呼び出す wrapper に縮小）

**完了条件**:
- [x] `UiSlot`, `UiNodeRegistry`, `MenuState` が `hw_ui` 由来になっている
- [x] `hw_ui` 側の setup に `use crate::systems::command::TaskMode` / `use crate::systems::time::TimeSpeed` が残っていない
- [x] `UiFoundationPlugin` 相当の機能が `hw_ui` から提供されている
- [x] `CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check` が通る

---

### M5: Presentation Model の root adapter 化

**目的**: `hw_ui` の panels/list が domain component を直接 query せず、root が生成した ViewModel resource のみを読む構造にする。

**変更内容**:

**5-a: EntityInspectionModel の ViewModel 化**
1. `src/interface/ui/presentation/mod.rs` を root adapter として維持しつつ、出力を `EntityInspectionViewModel` resource に書き出すよう変更
   - `EntityInspectionQuery` SystemParam はそのまま root 側に残す
   - `EntityInspectionViewModel` resource を新設し、root adapter が毎フレーム更新
2. `crates/hw_ui/src/models/inspection.rs` に `EntityInspectionViewModel` の定義を移動（root が書き込み、hw_ui が読み取り）

**5-b: EntityList の ViewModel 分離**
1. `src/interface/ui/list/change_detection.rs` は root 側に残す（DamnedSoul 等の直接 Changed を検知するため）
2. `src/interface/ui/list/dirty.rs` の `EntityListDirty` resource を `hw_ui` に移動（hw_ui が消費、root が書き込む）
3. `src/interface/ui/list/mod.rs` の `EntityListViewModel`, `SoulRowViewModel`, `FamiliarRowViewModel` 等を `crates/hw_ui/src/list/models.rs` へ移動

**変更ファイル**:
- `crates/hw_ui/src/models/` （新規ディレクトリ）
- `crates/hw_ui/src/models/inspection.rs` （EntityInspectionViewModel）
- `crates/hw_ui/src/list/models.rs` （EntityListViewModel など）
- `crates/hw_ui/src/list/dirty.rs` （EntityListDirty: hw_ui 定義に変更）
- `src/interface/ui/presentation/mod.rs` （resource 書き出しに変更）
- `src/interface/ui/list/change_detection.rs` （root 残留、dirty resource への write を hw_ui crate のものに変更）
- `src/interface/ui/list/dirty.rs` （hw_ui から re-import に変更）

**完了条件**:
- [ ] `hw_ui` 内の panels/list が `DamnedSoul`, `Familiar`, `Blueprint`, `AssignedTask`, `FamiliarSpatialGrid` を直接 query しない
- [x] root adapter が ViewModel resource を生成している
- [x] `EntityListDirty` が `hw_ui` 定義になっており、root が書き込み hw_ui が読む
- [ ] 差分更新（structure_dirty / value_dirty）が維持されている
- [ ] `CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check` が通る
- [ ] `cargo run` でリスト表示・Info Panel が正常に動作する

---

### M6: Panels / List / Tooltip plugin の `hw_ui` 移動

**目的**: UI 描画・同期・イベント処理の実装を `hw_ui` に集約する。

**変更内容**:
1. `src/interface/ui/plugins/{core,info_panel,entity_list,tooltip}.rs` → `crates/hw_ui/src/plugins/` へ移動
2. `src/interface/ui/panels/` → `crates/hw_ui/src/panels/` へ移動
   - `context_menu.rs`, `menu.rs`, `info_panel/`, `task_list/`, `tooltip_builder/`
   - 各ファイルの `use crate::` を ViewModel resource / hw_ui internal / hw_jobs / hw_logistics に変換
3. `src/interface/ui/list/` の UI 描画系 → `crates/hw_ui/src/list/` へ移動
   - `spawn.rs`, `drag_drop.rs`, `minimize.rs`, `resize.rs`, `selection_focus.rs`, `tree_ops.rs`
   - `interaction/navigation.rs`, `interaction/visual.rs`
   - `spawn/familiar_section.rs`, `spawn/soul_row.rs`
   - `sync/familiar.rs`, `sync/unassigned.rs`
4. `src/plugins/interface.rs` を「`HwUiPlugin` 登録 + root adapter/shell 登録」のみに縮小
   - 目標: 30行以下

**変更ファイル**:
- `crates/hw_ui/src/plugins/` （4ファイル移動）
- `crates/hw_ui/src/panels/` （5+ ファイル移動）
- `crates/hw_ui/src/list/` （10+ ファイル移動）
- `src/interface/ui/plugins/` （wrapper または削除）
- `src/interface/ui/panels/` （wrapper または削除）
- `src/interface/ui/list/` （change_detection.rs 残留、他は wrapper または削除）
- `src/plugins/interface.rs` （薄い shell に縮小）

**完了条件**:
- [ ] info panel / entity list / tooltip の plugin 実装が `hw_ui` 側にある
- [ ] `src/interface/ui/{plugins,panels,list}` が wrapper または削除済み（`change_detection.rs` 除く）
- [ ] `src/plugins/interface.rs` が 30行以下
- [ ] `CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check` が通る
- [ ] `cargo run` で全 UI が正常動作する

---

### M7: Interaction の `hw_ui` 移動と root handler 整理

**目的**: `hw_ui` 側の interaction が `UiIntent` 発行のみとなり、root handler がゲーム状態を変更する境界を確定する。

**変更内容**:

**hw_ui へ移動するもの（UI ビジュアルのみ）**:
- `interaction/common.rs` → `crates/hw_ui/src/interaction/common.rs`（BackgroundColor 更新のみ）
- `interaction/hover_action.rs` → `crates/hw_ui/src/interaction/hover_action.rs`（overlay 位置更新）
- `interaction/dialog.rs` → `crates/hw_ui/src/interaction/dialog.rs`（OperationDialog 開閉 UI）
- `interaction/tooltip/` 全4ファイル → `crates/hw_ui/src/interaction/tooltip/`
- `interaction/status_display/` 全3ファイル → `crates/hw_ui/src/interaction/status_display/`（M3 完了後にデータは ViewModel から読む）

**root に残すもの（ゲーム状態変更）**:
- `interaction/intent_handler.rs`（M3 で新設済み） → root 残留
- `interaction/mode.rs` → root handler として残留（PlayMode/TaskContext/BuildContext/ZoneContext 変更）

**`arch_category_action_system` と `move_plant_building_action_system`**:
- 両システムは M3 の `UiIntent` handler 内に統合

**`door_lock_action_system`**:
- `WorldMapWrite` を使うため root に残留
- `UiIntent::ToggleDoorLock(entity)` → root handler で処理

**Changed<Interaction> 順序維持**:
- `common.rs` → `menu_actions.rs` → `mode.rs` → `arch_category.rs` の既存 `chain` を崩さないこと
- 移動後も同じ順序で plugin に登録すること

**変更ファイル**:
- `crates/hw_ui/src/interaction/` （移動先）
- `src/interface/ui/interaction/common.rs` （wrapper または削除）
- `src/interface/ui/interaction/dialog.rs` （wrapper または削除）
- `src/interface/ui/interaction/hover_action.rs` （wrapper または削除）
- `src/interface/ui/interaction/tooltip/` （wrapper または削除）
- `src/interface/ui/interaction/status_display/` （wrapper または削除）
- `src/interface/ui/interaction/mode.rs` （root 残留）
- `src/interface/ui/interaction/menu_actions.rs` （root 残留）
- `src/plugins/interface.rs`
- `src/plugins/messages.rs`

**完了条件**:
- [ ] `hw_ui` 側の interaction system に `WorldMapWrite`, `TaskContext`, `BuildContext`, `ZoneContext` が残っていない
- [ ] `door_lock_action_system` が root handler 内に存在する
- [ ] `Changed<Interaction>` 系システムの登録順が既存と同じ
- [ ] `CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check` が通る
- [ ] `cargo run` で全インタラクションが正常動作する

---

### M8: Root shell の最終整理と docs 同期

**目的**: root に残る UI shell を最小化し、全ドキュメントを最終境界に同期させる。

**変更内容**:
1. root の UI 残留対象を整理・コメント化:
   - `src/interface/selection/`（21ファイル）: WorldMap 依存のため残留
   - `src/interface/camera.rs`: Input set に属すため残留
   - `src/interface/ui/vignette.rs`: TaskContext 依存のため残留
   - `src/interface/ui/presentation/mod.rs`: root adapter として残留
   - `src/interface/ui/list/change_detection.rs`: root adapter として残留
2. `docs/architecture.md` に hw_ui crate の位置づけと依存関係を追記
3. `docs/cargo_workspace.md` に hw_ui の依存グラフを更新
4. `docs/entity_list_ui.md`, `docs/info_panel_ui.md`, `docs/task_list_ui.md` の実装パス情報を更新
5. `docs/README.md` の参照一覧を更新
6. `docs/proposals/hw-ui-crate.md` に「本計画での決定事項」と「follow-up 課題（selection 分離）」を記録

**変更ファイル**:
- `docs/architecture.md`
- `docs/cargo_workspace.md`
- `docs/entity_list_ui.md`
- `docs/info_panel_ui.md`
- `docs/task_list_ui.md`
- `docs/README.md`
- `docs/proposals/hw-ui-crate.md`

**完了条件**:
- [ ] `hw_ui` と root shell の境界が docs に明記されている
- [ ] selection 分離が follow-up として docs に記録されている
- [ ] `docs/README.md` の参照が最新化されている
- [ ] `CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check` が通る

## 6. リスクと対策

| リスク | 影響 | 対策 |
| --- | --- | --- |
| `MenuAction` の削除漏れで `hw_ui` に root 型依存が残る | 高 | M3 完了後に `grep -r "crate::systems::command\|crate::systems::time" crates/hw_ui/` がヒットゼロであることを確認する |
| `TaskMode::DreamPlanting` の内部データ型が `hw_core` 移動で壊れる | 高 | 移動前に `DreamPlanting` が参照する型（DreamTask 等）も同時に移動するか、hw_core から re-export できる型だけに限定する |
| ViewModel 化で更新量が増え、UI 全再構築が戻る | 高 | `EntityListDirty`（structure_dirty / value_dirty の 2 段階）を維持し、`hw_ui` 側の sync が dirty 確認を先に行うことを守る |
| `Changed<Interaction>` 登録順変更で ボタン反応が壊れる | 高 | 既存の `.chain()` / `.after()` / `.before()` を全てコピーし、移動後の plugin 登録順を変えない。M6〜M7 完了後に全ボタン動作を手動確認する |
| selection 系との暗黙依存（`SelectedEntity`, `HoveredEntity` 等）を `hw_ui` が直接触ってしまう | 中 | `SelectedEntity`, `HoveredEntity` は root で定義・管理し、`hw_ui` は `UiIntent::InspectEntity(entity)` 経由でのみ参照する |
| `Entity` を含む `UiIntent` の対象が intent 処理時に despawn 済みになる | 中 | root handler で `query.get(entity).is_ok()` を確認し、invalid entity は no-op で処理する |
| `hw_ui` の `add_message::<UiIntent>()` と root 側の重複 Message 登録 | 低 | `src/plugins/messages.rs` から `UiIntent` の登録を削除し、`hw_ui` 側の `add_message` に一本化する |

## 7. 検証計画

**コンパイル確認（全 milestone 必須）**:
```bash
CARGO_HOME=/home/satotakumi/.cargo cargo check -p hw_ui
CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check
```

**依存関係クリーン確認（M3・M4・M5 完了後）**:
```bash
# hw_ui に root crate 型への直接依存が残っていないことを確認
rg -n "crate::systems|crate::entities|crate::world|crate::app_contexts" crates/hw_ui/src/
# → ヒット 0 件であること
```

**手動確認シナリオ（M6・M7 完了後）**:
1. Info Panel の表示・ピン留め・解除
2. Entity List の表示更新、DamnedSoul 追加/削除時の更新、ドラッグ&ドロップ、リサイズ
3. Tooltip / context menu のホバー表示とフェード
4. Bottom bar の submenu 開閉（Architect / Zones / Orders / Dream）
5. Time control（pause, 1x, 2x, 4x）と速度ボタンのハイライト
6. Build / Zone / Task の UI 操作が root handler 経由でゲーム状態に反映されること
7. `selection/` 系（建物配置、床・壁配置、建物移動）が既存どおり動作
8. door lock トグルが WorldMap に反映されること
9. Operation dialog（FatigueThreshold / MaxControlledSoul 変更）
10. debug spawn（`P`, `O` キー）が壊れていない

**パフォーマンス確認（M8 完了後）**:
```bash
CARGO_HOME=/home/satotakumi/.cargo cargo build --timings
# UI ファイル変更時に hw_ui のみ再コンパイルされることを確認
```

## 8. ロールバック方針

- **各 milestone を独立した git commit / branch** として管理し、`cargo check` green を常に維持する。
- root 側 re-export を段階的に削除することで、中間状態でも root が既存 API を提供できる。
- 問題のある milestone は `git revert` 1コマンドで戻せる粒度を維持する。
- ロールバック時は `docs/cargo_workspace.md` と `docs/architecture.md` を同時に戻す。

## 9. AI引継ぎメモ（最重要）

### 現在地

- 進捗: `65%`（M4 完了、M5-5a 完了）
- 完了済みマイルストーン: M1, M2, M3, M4, M5(5-a)
- 未着手: M5-5b, M6 〜 M8

### 次のAIが最初にやること

1. **M5**: `Presentation Model` の root adapter 化を完了させる
2. **M6**: `src/interface/ui/panels/` と `src/interface/ui/list/` の移設
3. **M7**: `interaction` の残置と root handler の最終整理
4. **M8**: docs 最終同期、shell 収束

### ブロッカー/注意点（コード調査済み）

| 問題 | 場所 | 対策 |
|------|------|------|
| `MenuAction::SelectTaskMode` / `SetTimeSpeed` が root 型依存 | `src/interface/ui/components.rs` L100-124 | M2 で `TaskMode`/`TimeSpeed` を hw_core 移動後に UiIntent へ置換 |
| `EntityInspectionQuery` が 10+ root 型を直接 query | `src/interface/ui/presentation/mod.rs` L36-87 | M5 で ViewModel resource に書き出す root adapter に変換 |
| `detect_entity_list_changes` が DamnedSoul/Familiar/AssignedTask を直接 Changed 検知 | `src/interface/ui/list/change_detection.rs` | root adapter として残留、`EntityListDirty` resource だけ hw_ui に移動 |
| `building_move/mod.rs` が `WorldMapWrite` + `unassign_task` に依存 | `src/interface/selection/building_move/mod.rs` | 今回スコープ外 |
| `vignette.rs` が `TaskContext` 依存 | `src/interface/ui/vignette.rs` | root shell 残留 |
| `door_lock_action_system` が `WorldMapWrite` を使用 | `src/interface/ui/interaction/mod.rs` L227-258 | `UiIntent::ToggleDoorLock` → root handler で処理 |

### 参照必須ファイル（実装前に必ず読む）

- `docs/proposals/hw-ui-crate.md` — 設計選択肢の原典
- `docs/cargo_workspace.md` — 現在の crate 依存グラフ
- `src/plugins/interface.rs` — plugin 登録順（変更禁止）
- `src/interface/ui/components.rs` — MenuAction 全 variant
- `src/interface/ui/presentation/mod.rs` — EntityInspectionQuery の全依存
- `src/interface/ui/list/change_detection.rs` — changed detection の全依存
- `src/interface/ui/plugins/mod.rs` — 5 plugin の公開構造
- `src/interface/ui/interaction/mod.rs` — ui_interaction_system の全ゲーム状態変更

### 最終確認ログ

- 最終 `cargo check`: `2026-03-08` / `pass`
  - `cargo check -p bevy_app@0.1.0`（`UiIntent` + `handle_ui_intent` 統合、`GameSystemSet::Interface` 再適用まで）
- 未解決エラー: なし

### Definition of Done

- [ ] `crates/hw_ui` が存在し、`CARGO_HOME=/home/satotakumi/.cargo cargo check -p hw_ui` が成功
- [ ] `hw_ui` に `use crate::systems` / `use crate::entities` / `use crate::world` / `use crate::app_contexts` が一切含まれない
- [ ] root `InterfacePlugin` が `HwUiPlugin` + root adapter/shell 登録のみの 30行以下になっている
- [ ] `src/interface/ui/{plugins,setup,panels,list}` が `hw_ui` 側へ移動済み（`change_detection.rs` 除く）
- [ ] docs が最終境界に同期されている
- [ ] `CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check` が成功

## 10. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| `2026-03-08` | `AI` | 初版作成 |
| `2026-03-08` | `AI` | コード調査に基づいてブラッシュアップ: MenuAction 全依存一覧、EntityInspectionQuery 全依存一覧、UiIntent 設計、M1〜M8 の具体的変更ファイル・検証コマンド・完了条件を追加 |
| `2026-03-08` | `AI` | M1〜M3 実装結果を反映（`UiIntent` 実装 + `Message` 化、root handler 整備、`UiCorePlugin` の set 化） |
| `2026-03-08` | `AI` | M4 を実装完了（setup の移設、`UiFoundationPlugin` の実装移管、`EntityListDirty` を hw_ui 定義へ移行） |
