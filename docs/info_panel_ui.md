# 情報パネルUI仕様

最終更新: 2026-03-30

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

### 電力発電施設（Soul Spa）
`SoulSpaSite` を持つエンティティは `append_soul_spa_model()` で追記される。

| フェーズ | 表示内容 |
|:---|:---|
| Constructing | `Status: Constructing (搬入済み/必要数)` |
| Operational | `Status: Operational` / `Active: N/M souls` / `Output: X.XW` / `Grid: gen/con [POWERED\|BLACKOUT]` |

- `Active` は `PowerGenerator.current_output / output_per_soul` から算出
- Grid 行は `GeneratesFor` でグリッドに接続されている場合のみ表示

### 電力消費施設（Outdoor Lamp 等）
`PowerConsumer` を持つエンティティは `append_power_consumer_model()` で追記される。

| 項目 | 表示 |
|:---|:---|
| 需要と稼働状態 | `Demand: X.XW [ACTIVE]` または `Demand: X.XW [UNPOWERED]` |
| グリッド情報 | `Grid: gen/con [POWERED\|BLACKOUT]`（`ConsumesFrom` 接続時のみ） |

- `ACTIVE` = `Unpowered` コンポーネントなし
- `UNPOWERED` = `Unpowered` コンポーネントあり（`#[require(Unpowered)]` または停電時に付与）

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

### `build_model()` の呼び出し順序

```rust
build_soul_model || build_blueprint_model || build_familiar_model
    || build_item_model || build_tree_model || build_rock_model
append_soul_spa_model      // SoulSpaSite: 発電情報
append_building_model      // Building 汎用情報
append_power_consumer_model // PowerConsumer: 需要・稼働状態
append_designation_model   // Designation: タスク情報
```

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
- `crates/bevy_app/src/interface/ui/panels/mod.rs` - `hw_ui::panels::info_panel` の re-export と `context_menu_system` の公開
- `crates/bevy_app/src/interface/ui/mod.rs` - app shell 側の UI facade として `InfoPanelPinState` / `InfoPanelState` / `info_panel_system` を明示 re-export
- `crates/bevy_app/src/interface/ui/plugins/info_panel.rs` - ViewModel producer / consumer の順序固定と plugin wiring
- `crates/bevy_app/src/interface/ui/presentation/` - `EntityInspectionModel` / `ViewModel` 構築（ゲームエンティティクエリ）
  - `mod.rs` — `EntityInspectionQuery` SystemParam（クエリ定義）
  - `builders.rs` — 各 `build_*` / `append_*` メソッド実装
- `crates/bevy_app/src/interface/ui/panels/context_menu.rs` - `Inspect (Pin)` メニュー
- `crates/bevy_app/src/interface/ui/interaction/menu_actions.rs` - `InspectEntity` / `ClearInspectPin`
- `crates/bevy_app/src/interface/ui/setup/mod.rs` - `spawn_info_panel_ui` を `hw_ui` 実装へ委譲する setup adapter

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
- `crates/bevy_app/src/interface/ui/panels/mod.rs` - `hw_ui::panels::info_panel` の re-export と `context_menu_system` の公開
- `crates/bevy_app/src/interface/ui/mod.rs` - app shell 側の UI facade として `InfoPanelPinState` / `InfoPanelState` / `info_panel_system` を明示 re-export
- `crates/bevy_app/src/interface/ui/plugins/info_panel.rs` - ViewModel producer / consumer の順序固定と plugin wiring
- `crates/bevy_app/src/interface/ui/presentation/` - `EntityInspectionModel` / `ViewModel` 構築（ゲームエンティティクエリ）
- `crates/bevy_app/src/interface/ui/panels/context_menu.rs` - `Inspect (Pin)` メニュー
- `crates/bevy_app/src/interface/ui/interaction/menu_actions.rs` - `InspectEntity` / `ClearInspectPin`
- `crates/bevy_app/src/interface/ui/setup/mod.rs` - `spawn_info_panel_ui` を `hw_ui` 実装へ委譲する setup adapter
