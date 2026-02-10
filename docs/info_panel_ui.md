# 情報パネルUI仕様

最終更新: 2026-02-07

## 概要
画面右側に表示される常駐パネルです。  
`SelectedEntity` またはピン留め中エンティティを参照し、変更時は差分更新のみ行います。  
対象がない場合は `display: none` で非表示です。

## 表示ルール

### 参照優先順位
- `InfoPanelPinState.entity` があればそれを優先（ピン表示）
- ピンが無ければ `SelectedEntity`
- ピン対象が消滅した場合は自動でピン解除し、選択対象へフォールバック

### ピン操作
- 右クリックコンテキストメニューの `Inspect (Pin)` でピン設定
- パネル右上 `Unpin` ボタンで解除
- `Unpin` ボタンはピン中のみ表示

## 表示対象

### ソウル
- ヘッダー（名前）
- 性別アイコン
- ステータス
  - Motivation
  - Stress
  - Fatigue
- Current Task
- Inventory
- 共通テキスト（補助情報）

### 使い魔
- ヘッダー（名前）
- 共通テキスト（タイプ、指揮関連パラメータ）
- ソウル専用ステータス列は非表示

### その他
- Blueprint / Building / Resource / Tree / Rock / Designation などを
  `EntityInspectionModel` の共通テキストとして表示

## 実装アーキテクチャ
- `UiNodeRegistry`（`UiSlot -> Entity`）経由でノード参照
- `Query::get_mut(entity)` で対象ノードのみ更新
- 表示データは `presentation` 層で構築
  - `build_entity_inspection_model` が `EntityInspectionModel` を生成
  - パネル側は描画責務に限定
- `InfoPanelState` で前回モデルを保持し、同一内容の再描画を抑制

## デザイン仕様（現行）
- 幅: `260px`（`min 200 / max 400`）
- 背景: セマンティックグラデーション
- 外枠: `panel_border_width` + `panel_corner_radius`
- セクションディバイダー: `Status / Current Task / Inventory`

## 関連ファイル
- `src/interface/ui/panels/info_panel/` - パネル生成と差分更新
- `src/interface/ui/presentation/` - `EntityInspectionModel` 構築
- `src/interface/ui/panels/context_menu.rs` - `Inspect (Pin)` メニュー
- `src/interface/ui/interaction/menu_actions.rs` - `InspectEntity` / `ClearInspectPin`
- `src/interface/ui/components.rs` - `UiSlot` / `InfoPanelPinState` 関連
