# 情報パネルUI仕様

最終更新: 2026-03-08

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
  - `update_entity_inspection_view_model_system` が `EntityInspectionViewModel` resource を更新
  - パネル側は描画責務に限定
- `InfoPanelState` で前回モデルを保持し、同一内容の再描画を抑制
- `Update` では `update_entity_inspection_view_model_system` → `info_panel_system` の順に固定し、selection / pin / entity 消滅の反映が 1 フレーム遅れないようにします。
- `info_panel_system` は `menu_visibility_system` の後、`update_mode_text_system` の前で実行されます。

## デザイン仕様（現行）
- 幅: `260px`（`min 200 / max 400`）
- 背景: セマンティックグラデーション
- 外枠: `panel_border_width` + `panel_corner_radius`
- セクションディバイダー: `Status / Current Task / Inventory`

## 関連ファイル（最終境界反映）

### `hw_ui` 側（実装本体）
- `crates/hw_ui/src/panels/info_panel/` - `InfoPanelState`, `InfoPanelPinState`, `spawn_info_panel_ui`, `info_panel_system`
- `crates/hw_ui/src/panels/menu.rs` - `menu_visibility_system`

### root shell（adapter）
- `src/interface/ui/panels/info_panel/mod.rs` - `hw_ui::panels::info_panel` の re-export
- `src/interface/ui/plugins/info_panel.rs` - ViewModel producer / consumer の順序固定と plugin wiring
- `src/interface/ui/presentation/` - `EntityInspectionModel` / `ViewModel` 構築（ゲームエンティティクエリ）
- `src/interface/ui/panels/context_menu.rs` - `Inspect (Pin)` メニュー
- `src/interface/ui/interaction/menu_actions.rs` - `InspectEntity` / `ClearInspectPin`
- `src/interface/ui/components.rs` - `UiSlot` / `InfoPanelPinState` / 再エクスポート
