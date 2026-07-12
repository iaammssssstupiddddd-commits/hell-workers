# エンティティリストUI仕様

最終更新: 2026-07-10

## 概要
画面左側のパネルで、使い魔とソウル一覧を表示します。  
表示内容は `EntityListViewModel` を経由して **100ms 間隔**で差分同期されます。

## パネル構成

### ヘッダー
- タイトル: `Entity List`
- 右側に最小化ボタン（`-` / `+`）
- 最小化時は本文 (`EntityListBody`) を非表示化し、パネル高さをヘッダー相当へ縮小

### 検索バー（`EntityListSearchRow`）
- ヘッダー直下に `Search` ラベル + `EditableText` フィールド
- ライブフィルタ: 入力のたびに `EntityListSearchState.query` を更新し、VM 構築時に部分一致で絞り込み
- 対象: 使い魔配下 Soul 行・未所属 Soul 行（名前 `SoulIdentity.name`、`str::contains` のそのまま部分一致）
- 最大 64 文字。Enter 確定は不要（検索は常時反映）
- `Escape`: 検索フィールド本体と `EntityListSearchState.query` を同時にクリアし、同フレームのゲーム側 Escape 処理へは伝播しない
- 検索中に `SoulIdentity.name` が変わった場合は structure dirty となり、結果へ即時に入る/消える

### 本文 (`EntityListBody`)
- `FamiliarListContainer`（使い魔セクションの親）
- `Unassigned Souls` セクション（折りたたみ可能）
- `Scroll: Mouse Wheel` ヒント（必要時のみ表示）

### 使い魔セクション
- 左: 折りたたみボタン（▼/▶）
- 中央: 使い魔選択ボタン
- 右: 使役数上限の増減ボタン（`-` / `+`）
- 表示形式: `{名前} ({現在/最大}) [{AIステート}]`
- 展開時: 配下ソウル行を表示

### ソウル行
- 性別アイコン（`male.png` / `female.png`）
- 名前（ストレス色に連動）
- 疲労アイコン + 疲労%
- ストレスアイコン + ストレス%（高ストレス時は太字）
- Dream値（整数表示、`dream == 0` のとき枯渇色）
- タスクアイコン（Idle/Chop/Mine/Haul/Build 等）

## サイズとリサイズ
- 初期高さ: `420px`
- 最小高さ: `220px`
- 高さスナップ: `20px` 刻み
- 上下端から `10px` 以内でドラッグすると縦リサイズ開始
- 上端/下端どちらもドラッグ可能
- カーソルは `NsResize` に切り替え
- 最小化中はリサイズ無効

## 同期方式（実装）

`EntityListDirty` を **structure dirty / value dirty** に分離し、行の増減・並び替え（重い再構築）と
バイタル値の更新（軽量なテキスト差し替え）を別 system・別 run_if で処理する。

- `build_entity_list_view_model_system`（run_if: `needs_structure_sync() || needs_value_sync_only()`）
  - `current/previous` スナップショット構築。構造・値どちらの変化でも VM を作り直す
- `sync_entity_list_from_view_model_system`（run_if: **`needs_structure_sync()` のみ**）
  - 使い魔セクションを差分同期（追加/削除/折りたたみ/ヘッダーテキスト）
  - 未所属ソウル行をキー管理で差分更新（`EntityListNodeIndex.unassigned_rows`）
  - 表示順は `replace_children` でビュー順へ再整列
  - 行の生成時に値も設定するため、構造変化フレームでは値行 system は走らせない
- `sync_entity_list_value_rows_system`（run_if: **`needs_value_sync_only()` のみ**）
  - 既存行の `Text` / `TextColor` / `TextFont` / `ImageNode` を in-place 更新
  - **代入前に現値と比較し、変化した項目だけ書き込む**（`get_mut` の DerefMut を避けることで、値が変わらない vitals 更新で `Changed` が立って UI が再レイアウトされるのを防ぐ）
  - 二つの run_if は排他（structure 変化フレームは全再構築側が値も含めて処理する）
- 検索入力の同期は `EditableText` の pending edit 適用後（`EditableTextSystems` 後）の `PostUpdate` で行う
  - 検索値が変わった場合のみ `EntityListDirty::mark_structure()` を立てる
  - `last_applied` は同じ検索文字列で毎フレーム structure dirty を立て続けないための適用済み値

## インタラクション

### マウス
- 行ホバー: ハイライト
- 行クリック: 選択 + カメラフォーカス
- 選択中行: 左ボーダー + 選択色
- 未所属ソウル領域でホイール: 縦スクロール
- ソウル行長押し（0.2秒）でドラッグ開始
- 使い魔行へドロップで配属リクエスト送信
- 使い魔行の `-` / `+` クリック: 使役数上限を変更（`1..=8`）

### キーボード
- `Tab`: 次の候補を選択
- `Shift + Tab`: 前の候補を選択
- `TaskArea` 編集モード中（`TaskMode::AreaSelection`）は、`Tab/Shift+Tab` の循環対象を **Familiar のみ** に制限
- テキスト入力フォーカス中（検索バー・リネーム等）は `Tab` 巡回を含むゲーム keybind を抑止（`UiInputState::text_input_blocks_keybinds`）

## 補助表示
- `Unassigned Souls` の内容がオーバーフローした時のみスクロールヒント表示
- `IgnoreScroll` によりヘッダー要素はスクロール対象から除外

## 入力ガード
- `UiInputBlocker` + `UiInputState` でUI上のワールド入力を抑止
- スクロール領域上のホイール入力はリスト優先
- `EditableText` フォーカス中は `text_input_focused` / `text_input_consumed_keyboard` によりゲーム側 `ButtonInput<KeyCode>` ショートカット（WASD パン、B/Z メニュー、Space/1-4 時間制御、Tab 巡回、Ctrl+C/V/Z/Y 等）を抑止
- Escape でフォーカス解除した同フレームは `text_input_consumed_keyboard` latch によりゲーム Escape 処理へ伝播しない

## 楽観的更新（体感改善）
- 使役数上限の `-` / `+` 操作時、`FamiliarOperation.max_controlled_soul` 更新直後に
  使い魔ヘッダー表示（`現在/最大`）を即時更新する
- Entity List の `-` / `+` と operation dialog の `-` / `+` はどちらも `UiIntent` を経由し、
  `handle_ui_intent` dispatcher から `interaction/handlers/familiar_settings.rs` の共通経路へ委譲されて `FamiliarOperation` 更新・ヘッダー即時更新・`FamiliarOperationMaxSoulChangedEvent` 発行を行う
- 最終的な整合は通常の100ms差分同期と `FamiliarOperationMaxSoulChangedEvent` の処理で維持する

## 主な関連ファイル（最終境界反映）

### root shell（adapter）
- `crates/bevy_app/src/interface/ui/list/mod.rs` - イベント受付、interaction/system 登録
- `crates/bevy_app/src/interface/ui/list/view_model.rs` - ゲームエンティティ → ビューモデル変換（検索フィルタ含む）
- `crates/bevy_app/src/interface/ui/list/change_detection.rs` - 変更検知トリガ（DamnedSoul/Familiar Changed 監視、検索中の `SoulIdentity` 変更は structure dirty）
- `crates/bevy_app/src/interface/ui/list/sync.rs` - `sync_entity_list_from_view_model_system` / `sync_entity_list_value_rows_system`（hw_ui sync helpers の thin shell）
- `crates/bevy_app/src/interface/ui/plugins/entity_list.rs` - 検索 sync を `EditableTextSystems` 後に登録
- `crates/bevy_app/src/interface/ui/list/drag_drop.rs` - ドラッグ&ドロップシステム（`DragState` 型は hw_ui）
- `crates/bevy_app/src/interface/ui/list/interaction.rs`, `interaction/navigation.rs` - 行クリック・Tab 巡回・target 付き `UiIntent` 発行（`FamiliarOperation` 直接更新は行わない）
- `crates/bevy_app/src/interface/ui/interaction/intent_handler.rs` - `UiIntent` dispatcher
- `crates/bevy_app/src/interface/ui/interaction/intent_context.rs` - `UiIntent` 処理が共有する `SystemParam` / query 集約
- `crates/bevy_app/src/interface/ui/interaction/handlers/familiar_settings.rs` - dialog/list button 共通の `FamiliarOperation` 更新、即時ヘッダー更新、`FamiliarOperationMaxSoulChangedEvent` 発行

### `hw_ui` 側（移設済み）
- `crates/hw_ui/src/list/models.rs` - ビューモデル型・`EntityListNodeIndex`・`FamiliarSectionNodes`
- `crates/hw_ui/src/list/spawn.rs` - `spawn_familiar_section`, `spawn_soul_list_item_entity` 等（`dyn UiAssets` 経由）
- `crates/hw_ui/src/list/sync.rs` - `sync_familiar_sections`, `sync_unassigned_souls`（`dyn UiAssets` 経由）
- `crates/hw_ui/src/list/section_toggle.rs` - `entity_list_section_toggle_system`（折りたたみ純UI操作）
- `crates/hw_ui/src/list/dirty.rs` - `EntityListDirty` リソース定義
- `crates/hw_ui/src/list/drag_state.rs` - `DragState` 型
- `crates/hw_ui/src/list/minimize.rs` - `EntityListMinimizeState` + 最小化トグルシステム
- `crates/hw_ui/src/list/resize.rs` - `EntityListResizeState` + リサイズシステム
- `crates/hw_ui/src/list/selection_focus.rs` - `focus_camera_on_entity`, `select_entity_and_focus_camera`
- `crates/hw_ui/src/list/tree_ops.rs` - `clear_children`
- `crates/hw_ui/src/list/visual.rs` - `apply_row_highlight`, `entity_list_visual_feedback_system`
- `crates/hw_ui/src/list/search.rs` - `EntityListSearchState`、検索 sync system
- `crates/hw_ui/src/widgets/text_field.rs` - 検索バー用 `spawn_text_field`
- `crates/hw_ui/src/interaction/text_field.rs` - フォーカス枠・Enter/Escape・検索ライブ sync（Escape 検索クリア含む）
- `crates/hw_ui/src/list/mod.rs` - `hw_ui` 対外エクスポート
- `crates/hw_ui/src/setup/mod.rs` - `UiAssets` トレイト（`icon_arrow_right`, `icon_idle`, `font_soul_name` 含む）

### 境界横断
- `crates/hw_ui/src/components.rs`, `crates/hw_ui/src/theme.rs` は `hw_ui` API の再エクスポートシェルとして残す
- `crates/hw_ui/src/setup/entity_list.rs`（初期構築・検索バー行 spawn）は root shell 経由で呼び出される
