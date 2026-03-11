# hw_ui クレート抽出 - Root を薄くする

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `hw-ui-extraction-2026-03-10` |
| ステータス | `Completed` |
| 作成日 | `2026-03-10` |
| 最終更新日 | `2026-03-11` |
| 作成者 | `AI` |
| 関連提案 | N/A |
| 関連Issue/PR | N/A |

## 1. 目的

- 解決したい課題: `src/interface/ui/` に大量のコードが残っており、root クレートが肥大化している
- 到達したい状態: root はゲームエンティティ依存の薄いアダプタ層のみとなり、UIロジック・レイアウト・ウィジェットは `hw_ui` に集約
- 成功指標: `list_legacy/` と `panels_legacy/` が削除され、root 側の `interface/ui/` にゲーム依存のないコードが残らない

## 2. スコープ

### 対象（In Scope）

- `src/interface/ui/` から `crates/hw_ui/` へのコード移動
- `UiSetupAssets` トレイトの `UiAssets` への改名・拡張（アイコン等のアセット抽象化）
- `include!` legacy パターンの解消
- `panels_legacy/`, `list_legacy/` の廃止

### 非対象（Out of Scope）

- `src/interface/selection/` の移動（別スコープ）
- 新機能の追加
- hw_ui の他プロジェクト再利用（当面このゲーム専用）

## 3. 現状とギャップ

### 現在の hw_ui クレート（抽出済み）

| モジュール | 主要な型 | LOC |
|---|---|---|
| `components.rs` | UiNodeRegistry, EntityListPanel, FamiliarListItem, SoulListItem, UiTooltip 他 50+ | 364 |
| `theme.rs` | UiTheme | 405 |
| `setup/` | UiSetupAssets trait, setup_ui fn | 1,483 |
| `interaction/` | tooltip, dialog, hover_action, status_display システム | ~870 |
| `list/` | EntityListViewModel, dirty | ~104 |
| `models/` | EntityInspectionModel, SoulInspectionFields | ~41 |
| `panels/menu.rs` | コンテキストメニュー基底 | 126 |
| `selection/` | **SelectedEntity, HoveredEntity** (✅ 既抽出済み), SelectionIntent, placement | ~674 |
| **合計** | | **~4,707 LOC** |

> **注意**: `SelectedEntity` と `HoveredEntity` は `crates/hw_ui/src/selection/mod.rs` に**既に定義済み**。
> 旧計画の M5「selection 関連型の移動」は**不要**（完了済み）。

### Root に残っている主なコード

| カテゴリ | 実装場所 | LOC | 依存の性質 |
|---|---|---|---|
| **tooltip_builder** (text_wrap, widgets, templates, mod) | `panels_legacy/tooltip_builder/` | ~534 | GameAssets のみ → UiAssets で解決可能 |
| **info_panel** (state, model, layout, update) | `panels_legacy/info_panel/` | ~696 | GameAssets のみ → UiAssets で解決可能 |
| **list 汎用** (resize, minimize, tree_ops) | `list_legacy/` | ~366 | hw_ui 型のみ → そのまま移動可能 |
| **visual.rs** | `list_legacy/interaction/visual.rs` | 115 | DragState (root), SelectedEntity (hw_ui ✓) |
| **task_list render+interaction** | `panels_legacy/task_list/render.rs` + `interaction.rs` | ~284 | WorkType (hw_core), InfoPanelPinState (M3 後に解決) |
| **navigation.rs** | `list_legacy/interaction/navigation.rs` | 137 | **TaskContext (root app_contexts) — root に残す** |
| **drag_drop.rs** | `list_legacy/drag_drop.rs` | 199 | DamnedSoul, SoulIdentity — root に残す |
| **interaction.rs** | `list_legacy/interaction.rs` | 221 | FamiliarOperation, FamiliarAiState — root に残す |
| **spawn/** | `list_legacy/spawn/` | ~447 | Familiar, DamnedSoul — root に残す |
| **sync/** | `list_legacy/sync/` | ~592 | Familiar ビューモデル構築 — root に残す |
| **context_menu.rs** | `panels_legacy/context_menu.rs` | 273 | DamnedSoul, Familiar, Blueprint — root に残す |
| **task_list view_model** | `panels/task_list/view_model.rs` | 171 | Blueprint, Designation 等 — root に残す |

**Root に残るゲーム依存コード（移動不可）: ~2,500 LOC**
**抽出候補: ~2,000 LOC (tooltip_builder, info_panel, list 汎用, visual, task_list render+interaction)**

### 依存分類の原則

**hw_ui に移動可能** = ゲームエンティティ型（DamnedSoul, Familiar, Blueprint 等）への直接依存がない

**Root に残す** = ゲームエンティティの ECS Query や `app_contexts` (BuildContext, ZoneContext, **TaskContext** 等) に依存

## 4. 実装方針

### アセット抽象化

`UiSetupAssets` トレイトを `UiAssets` に改名・拡張し、全ウィジェット・レイアウト系で必要なアセットを提供する。
拡張は各 M のタイミングで段階的に行う。

```rust
// crates/hw_ui/src/assets.rs (または setup/mod.rs に統合)
pub trait UiAssets {
    // 既存（UiSetupAssets から継続）
    fn font_ui(&self) -> &Handle<Font>;
    fn font_familiar(&self) -> &Handle<Font>;
    fn icon_arrow_down(&self) -> &Handle<Image>;
    fn glow_circle(&self) -> &Handle<Image>;

    // M1 で追加（info_panel 用）
    fn icon_stress(&self) -> &Handle<Image>;
    fn icon_fatigue(&self) -> &Handle<Image>;
    fn icon_male(&self) -> &Handle<Image>;
    fn icon_female(&self) -> &Handle<Image>;

    // M5 で追加（task_list アイコン用）
    fn icon_axe(&self) -> &Handle<Image>;
    fn icon_pick(&self) -> &Handle<Image>;
    fn icon_hammer(&self) -> &Handle<Image>;
    fn icon_haul(&self) -> &Handle<Image>;
    fn icon_bone_small(&self) -> &Handle<Image>;
}
```

Root 側の `GameAssets` が `UiAssets` を実装する。

### legacy 解消

`include!` パターンを廃止し、コードを移動先（hw_ui または root 直下）に直接配置する。
`list_legacy/` と `panels_legacy/` を削除。

## 5. マイルストーン

### M1: UiAssets トレイト拡張 + tooltip_builder 全体抽出

tooltip_builder はゲームエンティティへの直接依存がなく、UiAssets 抽象化で GameAssets 依存が解消できる。
`UiSetupAssets` の改名と拡張もここで行い、後続 M の基盤を作る。

**移動対象:**
| ファイル | 行数 | 現在の依存 | 対策 |
|---|---|---|---|
| `panels_legacy/tooltip_builder/text_wrap.rs` | 61 | なし | そのまま移動 |
| `panels_legacy/tooltip_builder/widgets.rs` | 180 | `GameAssets.font_ui`, `TooltipHeader/Body/ProgressBar` | `UiAssets` トレイト経由 + 既に hw_ui の型 |
| `panels_legacy/tooltip_builder/mod.rs` | 45 | `GameAssets`, `EntityInspectionModel`, `UiTooltip` | `UiAssets` + 既に hw_ui の型 |
| `panels_legacy/tooltip_builder/templates.rs` | 246 | `GameAssets`, `EntityInspectionModel`, `UiTooltip` | `UiAssets` + 既に hw_ui の型 |

**変更内容:**
1. `UiSetupAssets` → `UiAssets` にリネーム、info_panel 用アイコンメソッド追加
2. `tooltip_builder/` を `crates/hw_ui/src/panels/tooltip_builder/` に移動
3. 全ファイルの `GameAssets` を `&dyn UiAssets` に差し替え
4. Root 側は `pub use hw_ui::panels::tooltip_builder::*` で re-export

**変更ファイル:**
- `crates/hw_ui/src/setup/mod.rs` — `UiSetupAssets` → `UiAssets` リネーム、メソッド追加
- `crates/hw_ui/src/panels/` — `tooltip_builder` モジュール追加
- `src/interface/ui/setup/mod.rs` — トレイト名対応、新メソッド実装
- `src/interface/ui/panels/tooltip_builder/` → 削除（直接 re-export に変更）
- `src/interface/ui/panels_legacy/tooltip_builder/` → 削除

**完了条件:**
- [ ] `cargo check` 成功
- [ ] `panels_legacy/tooltip_builder/` 削除済み
- [ ] `UiSetupAssets` という名前が codebase に残っていない

---

### M2: list 汎用ロジック抽出 (resize, minimize, tree_ops, selection_focus)

ゲームエンティティ依存がゼロ。使用する型はすべて hw_ui 内に定義済み。

**`selection_focus.rs` について:**
`list_legacy/selection_focus.rs`（32 行）は `use bevy::prelude::*;` のみをインポートする。
関数本体で `crate::interface::camera::MainCamera` と `crate::interface::selection::SelectedEntity` を参照しているが、
いずれも `pub use hw_ui::camera::MainCamera` および `pub use hw_ui::selection::SelectedEntity` の re-export であり、
実質的に hw_ui 型のみへの依存。hw_ui 内に移動すれば直接参照に置き換えられる。

**移動対象:**
| ファイル | 行数 | 現在の依存 | 対策 |
|---|---|---|---|
| `list_legacy/tree_ops.rs` | 24 | Bevy のみ | そのまま移動 |
| `list_legacy/resize.rs` | 255 | `EntityListPanel`, `UiTheme`, `EntityListMinimizeState` | 全て hw_ui 内の型 |
| `list_legacy/minimize.rs` | 87 | `EntityListPanel`, `EntityListBody`, `UiTheme` | 全て hw_ui 内の型 |
| `list_legacy/selection_focus.rs` | 32 | hw_ui 型のみ（re-export 経由） | hw_ui 内で直接参照に変更 |

**変更内容:**
1. `crates/hw_ui/src/list/` に `tree_ops.rs`, `resize.rs`, `minimize.rs`, `selection_focus.rs` 追加
2. `EntityListMinimizeState`, `EntityListResizeState` を hw_ui で定義（現在 root にある）
3. `selection_focus.rs` 内の `crate::interface::*` パスを `crate::*`（hw_ui 内参照）に変更
4. Root 側は re-export + プラグイン登録で使用

**変更ファイル:**
- `crates/hw_ui/src/list/mod.rs` — モジュール追加
- `src/interface/ui/list/resize.rs` → re-export
- `src/interface/ui/list/minimize.rs` → re-export
- `src/interface/ui/list/tree_ops.rs` → re-export
- `src/interface/ui/list/selection_focus.rs` → re-export
- `src/interface/ui/list_legacy/resize.rs` → 削除
- `src/interface/ui/list_legacy/minimize.rs` → 削除
- `src/interface/ui/list_legacy/tree_ops.rs` → 削除
- `src/interface/ui/list_legacy/selection_focus.rs` → 削除

**完了条件:**
- [ ] `cargo check` 成功
- [ ] `EntityListMinimizeState`, `EntityListResizeState` は hw_ui で定義
- [ ] `list_legacy/selection_focus.rs` 削除済み

---

### M3: info_panel 抽出 (state, model, layout, update)

info_panel は hw_ui の型（UiSlot, InfoPanelNodes, UiNodeRegistry）と `UiAssets` のみに依存する。
M1 で UiAssets に icon_stress/fatigue/male/female が追加されているため、layout.rs と update.rs の依存がすべて解消する。

**移動対象:**
| ファイル | 行数 | 現在の依存 | 対策 |
|---|---|---|---|
| `panels_legacy/info_panel/state.rs` | 13 | なし（Resource のみ） | そのまま移動 |
| `panels_legacy/info_panel/model.rs` | 48 | `EntityInspectionModel` (hw_ui) | そのまま移動 |
| `panels_legacy/info_panel/layout.rs` | 341 | `GameAssets` (font, icon_stress, icon_fatigue) | `UiAssets` トレイト経由（M1 で追加済み） |
| `panels_legacy/info_panel/update.rs` | 283 | `GameAssets` (icon_male, icon_female) | `UiAssets` トレイト経由（M1 で追加済み） |

**変更内容:**
1. `crates/hw_ui/src/panels/info_panel/` を作成
2. 全4ファイルを移動、`GameAssets` を `&dyn UiAssets` に差し替え
3. `InfoPanelPinState`, `InfoPanelState` は hw_ui で定義
4. Root 側は re-export のみ

**変更ファイル:**
- `crates/hw_ui/src/panels/` — `info_panel` モジュール追加
- `src/interface/ui/panels/info_panel/` → re-export
- `src/interface/ui/panels_legacy/info_panel/` → 削除

**完了条件:**
- [ ] `cargo check` 成功
- [ ] `panels_legacy/info_panel/` 削除済み
- [ ] `InfoPanelPinState` は hw_ui で定義

---

### M4: DragState 型を hw_ui に移動 + visual.rs 抽出

**背景と判断:**

`list_legacy/interaction/visual.rs`（115 行）の依存:
- `SoulListItem`, `FamiliarListItem` → hw_ui ✓
- `SelectedEntity` → hw_ui ✓（既に抽出済み）
- `UiTheme` → hw_ui ✓
- `DragState` → **現在 root の `list_legacy/drag_drop.rs` に定義**

`DragState` は `Option<Entity>`, `Timer` のみを持つ純粋な Resource でゲーム型への依存がない。
`drag_drop.rs` のシステムロジック（DamnedSoul, SoulIdentity 依存）は root に残したまま、
`DragState` 型のみ hw_ui に移動すれば visual.rs が hw_ui に移動できる。

**`navigation.rs` は Root に残す:**
`list_legacy/interaction/navigation.rs`（137 行）は `Res<TaskContext>` を使用しており、
`TaskContext` は root の `app_contexts.rs` にあるゲーム固有の Resource である。
（`TaskMode` 自体は hw_core にあるが、`TaskContext` wrapper は root に残すべき）
→ navigation.rs は "Root に残すもの" に追加。

**移動対象:**
| 対象 | 行数 | 現在の依存 | 対策 |
|---|---|---|---|
| `DragState` 型定義 | ~15 | `Option<Entity>`, `Timer` のみ | hw_ui に移動 |
| `list_legacy/interaction/visual.rs` | 115 | hw_ui 型のみ（DragState 移動後） | hw_ui に移動 |

**変更内容:**
1. `DragState` を `crates/hw_ui/src/list/` に移動（または `components.rs` に統合）
2. `drag_drop.rs` のシステムはそのまま root に残し、`use hw_ui::list::DragState` で参照
3. `visual.rs` を `crates/hw_ui/src/list/visual.rs` に移動

**変更ファイル:**
- `crates/hw_ui/src/list/` — `DragState`, `visual.rs` 追加
- `src/interface/ui/list_legacy/drag_drop.rs` — DragState 定義を削除、hw_ui から import
- `src/interface/ui/list_legacy/interaction/visual.rs` → 削除（hw_ui に移動）

**完了条件:**
- [ ] `cargo check` 成功
- [ ] `DragState` は hw_ui で定義
- [ ] `visual.rs` は hw_ui に移動済み

---

### M5: task_list 汎用部分の抽出 (TaskEntry, render, interaction)

**背景:**

- `task_list/render.rs`（145 行）は現在 `presenter::get_work_type_icon()` を呼ぶが、
  `presenter.rs` はゲーム型（Blueprint, BonePile, TransportRequest 等）に強依存。
- ただし `WorkType` 自体は `hw_core::jobs::WorkType` として定義済みで hw_ui からアクセス可能。
- アイコンマッピング（WorkType → icon, color）は UI の責務であり、hw_ui 側に置ける。
- `TaskEntry` は `entity: Entity`, `description: String`, `priority: u32`, `worker_count: usize` のみを持つ純粋な ViewModel 型。

**移動対象:**

| ファイル/型 | 行数 | 現在の依存 | 対策 |
|---|---|---|---|
| `TaskEntry` 型（view_model.rs より分離） | ~8 | なし（純粋データ） | hw_ui に移動 |
| `panels_legacy/task_list/render.rs` | 145 | `WorkType` (hw_core), `GameAssets.font_ui`, `TaskListItem` (hw_ui), `presenter::get_work_type_icon` | work_type_icon を hw_ui に移動後に解決 |
| `panels_legacy/task_list/interaction.rs` | 139 | `InfoPanelPinState` (M3 で hw_ui 移動済み), `TaskListItem` (hw_ui) | M3 完了後に移動可能 |
| `TaskListDirty` Resource | ~5 | 純粋データ型 | hw_ui に移動 |

**`update.rs` は root に残す:**
`panels_legacy/task_list/update.rs`（47 行）は `task_list_update_system` という Bevy システム関数で、
`game_assets: Res<crate::assets::GameAssets>` をシステム引数に取る。
Bevy の `Res<T>` はトレイトオブジェクトを使えないため、`Res<dyn UiAssets>` にできない。
M5 完了後は `render::rebuild_task_list_ui` が hw_ui から公開されるため、
root 側の `update.rs` は `use hw_ui::panels::task_list::render::rebuild_task_list_ui` を呼ぶ形に変わる。
**update.rs 自体は root に残し、hw_ui の render を呼び出すアダプタとして機能する。**

**変更内容:**
1. `crates/hw_ui/src/panels/task_list/` を作成
2. `UiAssets` に task_list アイコンメソッドを追加（`icon_axe`, `icon_pick`, `icon_hammer`, `icon_haul`, `icon_bone_small`）
3. `work_type_icon(wt: &WorkType, assets: &dyn UiAssets, theme: &UiTheme) -> (Handle<Image>, Color)` を hw_ui に追加
4. `TaskEntry` を hw_ui に移動（root 側では hw_ui から re-export）
5. `render.rs` を hw_ui に移動（`presenter` への依存を `work_type_icon` に置き換え）
6. `interaction.rs` を hw_ui に移動（M3 完了が前提）
7. `TaskListDirty` を hw_ui に移動
8. Root 側 `view_model.rs` のシステム（Blueprint 等に依存）は root に残す

**変更ファイル:**
- `crates/hw_ui/src/panels/task_list/` — TaskEntry, TaskListDirty, render, interaction, work_type_icon 追加
- `src/interface/ui/setup/mod.rs` — UiAssets の task_list アイコン実装追加
- `src/interface/ui/panels/task_list/` — re-export に変更
- `src/interface/ui/panels_legacy/task_list/render.rs` → 削除
- `src/interface/ui/panels_legacy/task_list/interaction.rs` → 削除

**完了条件:**
- [ ] `cargo check` 成功
- [ ] `panels_legacy/task_list/` から render.rs と interaction.rs が削除済み
- [ ] `TaskEntry`, `TaskListDirty` は hw_ui で定義
- [ ] `TaskListState` のシステム部分（Blueprint 依存）は root に残っている

---

### M6: legacy ディレクトリ完全削除

M1-M5 完了後、以下を削除:
- `src/interface/ui/list_legacy/` — 残ったファイル（navigation.rs, drag_drop.rs, interaction.rs, spawn/, sync/）を root 直下の `list/` に直接統合し legacy ディレクトリを廃止
- `src/interface/ui/panels_legacy/` — 残ったファイル（context_menu.rs, presenter.rs 等）を root 直下の `panels/` に直接統合し legacy ディレクトリを廃止
- 残存する `include!` マクロを全て除去

**作業内容:**
1. `list_legacy/*.rs` の実体ファイルを `list/` 直下に移動（`include!` 解消）
2. `panels_legacy/*.rs` の実体ファイルを `panels/` 直下に移動
3. `include!` を使っていた 1-line シム `*.rs` ファイルを削除
4. `panels/task_list/mod.rs` の `#[path = "../../panels_legacy/task_list/xxx.rs"]` パターンを通常の `mod` 宣言に書き換え（`include!` 同等のパス迂回を解消）
5. `list_legacy/`, `panels_legacy/` ディレクトリを削除

**完了条件:**
- [ ] `cargo check` 成功
- [ ] `list_legacy/`, `panels_legacy/` が存在しない
- [ ] `include!` マクロが `src/interface/ui/` から消滅
- [ ] `#[path = ".."]` によるクロスディレクトリ参照が消滅

---

## Root に残すもの（移動しない）

以下はゲームエンティティへの直接的な ECS 依存があり、root に残す:

| ファイル | 理由 |
|---|---|
| `interaction/intent_handler.rs` | BuildContext, ZoneContext, FamiliarOperation 等 |
| `interaction/mode.rs` | PlayMode 遷移, TaskMode, BuildingType 等 |
| `interaction/menu_actions.rs` | 薄いディスパッチだが UiIntent のバリアント数に1:1対応 |
| `interaction/mod.rs` の各システム | ui_keyboard_shortcuts (app_contexts), door_lock (Door, WorldMap), move_plant (MoveContext), update_operation_dialog (Familiar) |
| `presentation/` | EntityInspectionQuery（ゲームエンティティ10+ 型のクエリ集約）|
| `list/change_detection.rs` | DamnedSoul, Familiar, AssignedTask 等の Changed 監視 |
| `list/view_model.rs` | Familiar, DamnedSoul, FamiliarAiState からビューモデル構築 |
| `list/sync/` | ゲームエンティティ → UI ノード同期 |
| `list/spawn/` | FamiliarSection, SoulRow のスポーン（ゲーム構造依存） |
| `list/drag_drop.rs` | SquadManagementRequest, SoulIdentity（DragState 型のみ hw_ui へ分離） |
| `list/interaction.rs` | FamiliarOperation, FamiliarAiState, SquadManagementRequest |
| `list/interaction/navigation.rs` | **TaskContext (app_contexts) への Res 依存**（TaskMode は hw_core にあるが TaskContext wrapper は root） |
| `panels/context_menu.rs` | Familiar, DamnedSoul, Building, Door の分類 |
| `panels/task_list/view_model.rs` | Designation, Blueprint, WorkType 等のゲームクエリ |
| `panels/task_list/dirty.rs` の detect systems | ゲームコンポーネントの Changed 監視 |
| `panels/task_list/presenter.rs` | Blueprint, BonePile, TransportRequest 等の description 生成 |
| `panels/task_list/update.rs` | `Res<GameAssets>` をシステム引数に取るため hw_ui 移動不可。M5 後は hw_ui の render を呼ぶアダプタになる |
| `vignette.rs` | TaskContext (DreamPlanting モード判定) |

## 6. リスクと対策

| リスク | 影響 | 対策 |
|---|---|---|
| `UiAssets` トレイトの肥大化 | アセット追加のたびにトレイト修正が必要 | 最小限のメソッドに絞る。将来的に Resource ベースに切り替え検討 |
| `include!` 解消時のパス変更ミス | コンパイルエラー | M ごとに `cargo check` で即時検証 |
| hw_ui の hw_core 以外の依存追加 | クレート間依存の複雑化 | hw_jobs, hw_logistics の型は既に依存済み。新規依存は追加しない |
| M5 で `render.rs` が `presenter` に依存したまま | 移動できない | `get_work_type_icon` のみ hw_ui 側に先に移す |
| M5 で `interaction.rs` が M3 未完了で詰まる | 移動できない | M3 → M5 の順序を厳守 |

## 7. 検証計画

- 必須: 各 M 完了時に `CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check`
- 手動確認: ゲーム起動してUIの表示・操作が正常であること
  - エンティティリストの展開/折りたたみ/リサイズ
  - ツールチップの表示
  - Info パネルの表示/ピン
  - タスクリスト切り替え

## 8. ロールバック方針

- 各 M は独立してコミット可能
- 依存順序: M1 → M3 → M5（M1 の UiAssets が M3 の前提、M3 の InfoPanelPinState が M5 の前提）
- M2 と M4 は M1 と独立して並行実施可能
- 問題発生時は該当 M のコミットを revert

## 9. AI引継ぎメモ

### 現在地

- 進捗: `100%`
- 完了済みマイルストーン: M1-M6 全て完了（2026-03-11）
- 未着手: なし

### 次のAIが最初にやること

1. `docs/plans/hw-ui-crate-extraction.md` を読む
2. M1 から着手: `UiSetupAssets` → `UiAssets` リネーム + アイコンメソッド追加 → `tooltip_builder/` 移動
3. 各 M 完了ごとに `cargo check`

### ブロッカー/注意点

- `include!` は相対パスで解決されるため、移動時にモジュールパスの整合性に注意
- `panels_legacy/` のファイルは `include!` 経由でのみ参照されている（直接 mod 宣言されていない）
- `panels/task_list/mod.rs` は `#[path = "../../panels_legacy/task_list/xxx.rs"]` で panels_legacy を参照している（`include!` でなく Rust の mod パス上書き機能）
- **`navigation.rs` は `TaskContext` 依存があるため hw_ui に移動できない** — root に残す
- **`SelectedEntity`/`HoveredEntity` は既に hw_ui にある** — M5（旧計画）は不要
- **`update.rs`（task_list）は `Res<GameAssets>` システム引数のため hw_ui に移動できない** — root に残し、M5 後は hw_ui の render を呼ぶアダプタとして使う
- `menu_actions.rs` は `MenuAction` = `UiIntent` のエイリアスで、1:1 のディスパッチ。将来的に hw_ui 側で `ui_interaction_system` を提供すれば不要になる可能性がある
- M5 の `render.rs` 移動は `presenter::get_work_type_icon` を hw_ui の `work_type_icon` に置き換える前提

### 参照必須ファイル

- `crates/hw_ui/src/lib.rs` — hw_ui の公開 API
- `crates/hw_ui/src/setup/mod.rs` — UiSetupAssets トレイト（UiAssets にリネーム対象）
- `src/interface/ui/mod.rs` — root 側の UI モジュール構成
- `src/interface/ui/plugins/` — システム登録の全体像
- `src/app_contexts.rs` — TaskContext 定義（navigation.rs が参照）

### 期待される最終状態

**Root (`src/interface/ui/`) に残るもの:**
- `mod.rs` — re-export のみ
- `interaction/` — intent_handler, mode, menu_actions, mod.rs のゲーム固有システム
- `presentation/` — EntityInspectionQuery + ビルダー
- `list/` — change_detection, view_model, sync/, spawn/, drag_drop (ロジック), interaction, navigation（ゲーム依存部分のみ）
- `panels/` — context_menu, task_list/view_model, task_list/presenter, task_list/dirty（ゲーム依存部分のみ）
- `vignette.rs`
- `plugins/` — プラグイン登録（システム登録コールバック）
- `setup/` — GameAssets → UiAssets アダプタ

**hw_ui に移動完了:**
- `panels/tooltip_builder/` (text_wrap, widgets, templates, mod)
- `list/` (resize, minimize, tree_ops, **selection_focus**, DragState, visual)
- `panels/info_panel/` (layout, model, state, update)
- `panels/task_list/` (TaskEntry, TaskListDirty, render, interaction, work_type_icon)

### Definition of Done

- [ ] M1-M6 が全て完了
- [ ] `list_legacy/`, `panels_legacy/` が削除済み
- [ ] `include!` マクロと `#[path = ".."]` クロスディレクトリ参照が `src/interface/ui/` から消滅
- [ ] `UiSetupAssets` という名前が codebase に残っていない
- [ ] `cargo check` が成功
- [ ] ゲーム起動で UI 動作確認済み
