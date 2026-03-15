# タスクリストUI仕様

最終更新: 2026-03-08

## 概要
画面左側に表示される常駐パネルのモードの1つです（エンティティリストとタブ切替）。
現在の **Designation（仕事の指示）** の一覧を表示し、進捗状況を可視化します。

## 表示構成

### グループヘッダー
タスクは `WorkType` ごとにグループ化され、各グループにヘッダーが表示されます。

`[WorkTypeアイコン] [ラベル] ([件数])`

- アイコンは WorkType に対応（斧=Chop、ピッケル=Mine、ハンマー=Build、運搬=Haul系）
- テーマカラーで着色

### タスクアイテム
各アイテムは固定高さ（20px）の行で、以下の要素を横並びに配置します：

`[WorkTypeアイコン 16px] [説明テキスト 12px] [ワーカーカウント ×N]`

#### 1. WorkType アイコン (16px)
作業種別ごとに異なるアイコンとテーマカラーを表示：
- **Chop**: 斧アイコン / `chop` 色
- **Mine**: ピッケルアイコン / `mine` 色
- **Build**: ハンマーアイコン / `build` 色
- **Haul / HaulToMixer / WheelbarrowHaul**: 運搬アイコン / `haul` 色
- **GatherWater / HaulWaterToMixer**: 運搬アイコン / `water` 色
- **CollectSand**: ピッケルアイコン / `gather_default` 色
- **Refine**: ハンマーアイコン / `build` 色

#### 2. 説明テキスト (12px)
作業種別と対象エンティティに基づいて自動生成されます。
- **建築**: `Construct [BuildingType]` (例: `Construct Wall`)
- **採掘**: `Mine Rock`
- **伐採**: `Chop Tree`
- **運搬**: `Haul [Resource]` (手動), `Haul [Resource] to Mixer` (自動)
- **水汲み**: `Gather Water`

高優先度タスク (Priority >= 5) は `accent_ember` 色で強調表示されます。

#### 3. ワーカーカウント (10px)
作業員が割り当てられている場合のみ `×N` を `text_secondary` 色で表示します。

## ビジュアルフィードバック

エンティティリストと統一されたホバー・選択ハイライトを提供します。

### 背景色
- **デフォルト**: `list_item_default`
- **ホバー**: `list_item_hover`
- **選択中**: `list_item_selected`
- **選択中+ホバー**: `list_item_selected_hover`

### 選択ボーダー
ピン留めされたエンティティに対応するアイテムに左 3px の `list_selection_border` 色ボーダーを表示します。

## 更新タイミング

- `PreUpdate` で `detect_task_list_changed_components` → `detect_task_list_removed_components` → `update_task_list_state_system` を順序固定で実行します。
- `LeftPanelMode::TaskList` 中でも、無変更フレームではスナップショット再生成と子 UI の再構築を行いません。
- 再生成トリガーは、`Designation` とその表示内容に影響する関連コンポーネントの `Added` / `Changed` / `Removed`、および左パネルのタブ切替です。
- `TaskListDirty` は `state_dirty` / `list_dirty` / `summary_dirty` の 3 つの責務に分かれます。
- `state_dirty` は snapshot と summary の再計算要求、`list_dirty` は左パネル本文の再描画要求、`summary_dirty` は画面上部 summary の更新要求です。
- `TaskListState.snapshot` は最新観測済みデータを保持し、未描画の `pending` snapshot は持ちません。
- 左パネルを `TaskList` に切り替えたフレームは `mark_all()` で `state_dirty` / `list_dirty` を両方立て、最新スナップショットで再描画します（タスクデータが変わっていない場合も含む）。
- 画面上部の task summary は `TaskListState.summary_total` / `summary_high` を参照し、タスクリストと同じ dirty source を共有します。

## 実装アーキテクチャ
- `LeftPanelMode::TaskList` 時に表示
- `crates/bevy_app/src/interface/ui/panels/task_list/`：責務別に分割
  - `view_model.rs` - スナップショット生成と summary 集計（`TaskListState`, `TaskEntry`）
  - `presenter.rs` - WorkType → icon / label / description
  - `render.rs` - UI 再構築
  - `interaction.rs` - クリック、タブ、可視、ハイライト（`task_list_visual_feedback_system` 等）
  - `dirty.rs` - タスクリストと task summary の dirty source
  - `update.rs` - dirty gate 付きオーケストレーション、必要時のみ再描画
- `crates/bevy_app/src/interface/ui/plugins/info_panel.rs` が `PreUpdate` の dirty 検知と state 更新、`Update` の左パネル表示更新を束ねます。
- `crates/bevy_app/src/interface/ui/interaction/status_display/mode_panel.rs` が cached summary を読み、task summary 表示だけを差分更新します。
- `Designation` コンポーネントを持つエンティティをクエリし、関連コンポーネント（Blueprint, TransportRequest等）を参照して説明文を生成
- `task_list_visual_feedback_system` が `Interaction` と `InfoPanelPinState` を監視し、`ui/list::apply_row_highlight` でホバー・選択ハイライトを適用

## インタラクション
- **ホバー**: 背景色がハイライト
- **クリック**: カメラをそのタスク（対象エンティティ）の位置へ移動し、InfoPanel にピン留め
- **選択状態**: ピン留めされたエンティティに対応するアイテムに選択ボーダーと背景色が表示

## 関連ファイル（最終境界反映）

### `hw_ui` 側（実装本体）
- `crates/hw_ui/src/panels/task_list/types.rs` - `TaskEntry`, `TaskListDirty`
- `crates/hw_ui/src/panels/task_list/render.rs` - `rebuild_task_list_ui`
- `crates/hw_ui/src/panels/task_list/interaction.rs` - `task_list_click_system`, `task_list_visual_feedback_system`, `left_panel_tab_system`, `left_panel_visibility_system`
- `crates/hw_ui/src/panels/task_list/work_type_icon.rs` - WorkType → アイコン/カラー/ラベル変換
- `crates/hw_ui/src/panels/menu.rs` - `menu_visibility_system`

### root shell（adapter）
- `crates/bevy_app/src/interface/ui/panels/task_list/mod.rs` - hw_ui re-export + ゲーム固有モジュール統合
- `crates/bevy_app/src/interface/ui/panels/task_list/view_model.rs` - スナップショット生成と summary 集計（ゲームエンティティクエリ）
- `crates/bevy_app/src/interface/ui/panels/task_list/dirty.rs` - dirty 検知システム（Designation 等の Changed 監視）
- `crates/bevy_app/src/interface/ui/panels/task_list/update.rs` - dirty gate 付きオーケストレーション（`Res<GameAssets>` 依存のため root 残留）
- `crates/bevy_app/src/interface/ui/plugins/info_panel.rs` - task list の dirty 検知 / state 更新 / 左パネル system 登録
- `crates/bevy_app/src/interface/ui/interaction/status_display/mode_panel.rs` - task summary の cached 描画
