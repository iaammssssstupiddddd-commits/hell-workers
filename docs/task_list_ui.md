# タスクリストUI仕様

最終更新: 2026-02-13

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

## 実装アーキテクチャ
- `LeftPanelMode::TaskList` 時に表示
- `src/interface/ui/panels/task_list/`：責務別に分割
  - `view_model.rs` - スナップショット生成（`TaskListState`, `TaskEntry`）
  - `presenter.rs` - WorkType → icon / label / description
  - `render.rs` - UI 再構築
  - `interaction.rs` - クリック、タブ、可視、ハイライト（`task_list_visual_feedback_system` 等）
  - `update.rs` - オーケストレーション、差分検知・再描画のトリガー
- `Designation` コンポーネントを持つエンティティをクエリし、関連コンポーネント（Blueprint, TransportRequest等）を参照して説明文を生成
- `task_list_visual_feedback_system` が `Interaction` と `InfoPanelPinState` を監視し、`ui/list::apply_row_highlight` でホバー・選択ハイライトを適用

## インタラクション
- **ホバー**: 背景色がハイライト
- **クリック**: カメラをそのタスク（対象エンティティ）の位置へ移動し、InfoPanel にピン留め
- **選択状態**: ピン留めされたエンティティに対応するアイテムに選択ボーダーと背景色が表示
