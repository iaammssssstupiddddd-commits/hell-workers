# タスクリストUI仕様

最終更新: 2026-02-11

## 概要
画面右側に表示される常駐パネルのモードの1つです（Info Panelとタブ切替）。
現在の **Designation（仕事の指示）** の一覧を表示し、進捗状況や優先度を可視化します。

## 表示構成

### 表示内容
以下の情報をリスト形式で表示します：
- **タスク内容**: `Construct Wall`, `Mine Rock`, `Haul Wood` など、具体的な作業内容
- **ステータス**: 割り当て状況と優先度

### 表示フォーマット
`[優先度] [ステータス] [タスク内容]`

#### 1. 優先度 (Priority)
タスクの重要度を示します。
- **高優先**: `★[P:n]` (Priority >= 5) - MudMixerへの自動供給や緊急タスク
- **低優先**: `▼[P:n]` (Priority < 0) - 後回しにしてよいタスク
- **通常**: `[P:n]` - 一般的な指示

#### 2. ステータス (Status)
タスクへの人員割り当て状況を示します。
- **待機中**: `[WAIT]` - 誰も割り当てられていない状態
- **実行中**: `[RUN:n]` - n 人の作業員がそのタスクに従事している状態

#### 3. タスク内容 (Description)
作業種別 (`WorkType`) と対象エンティティ (`Designation` のターゲット) に基づいて自動生成されます。
- **建築**: `Construct [BuildingType]` (例: `Construct Wall`)
- **採掘**: `Mine Rock`
- **伐採**: `Chop Tree`
- **運搬**: `Haul [Resource]` (手動), `Haul [Resource] to Mixer` (自動)
- **水汲み**: `Gather Water`

## 実装アーキテクチャ
- `RightPanelMode::TaskList` 時に表示
- `src/interface/ui/panels/task_list/update.rs` で `TaskListState` を用いて差分検知を行い、変更がある場合のみ再描画します。
- `Designation` コンポーネントを持つエンティティをクエリし、関連コンポーネント（Blueprint, TransportRequest等）を参照して説明文を生成します。

## インタラクション
- **リストクリック**: カメラをそのタスク（対象エンティティ）の位置へ移動し、詳細情報（Info Panel）を表示します。
