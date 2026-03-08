# エンティティリストUI仕様

最終更新: 2026-02-26

## 概要
画面左側のパネルで、使い魔とソウル一覧を表示します。  
表示内容は `EntityListViewModel` を経由して **100ms 間隔**で差分同期されます。

## パネル構成

### ヘッダー
- タイトル: `Entity List`
- 右側に最小化ボタン（`-` / `+`）
- 最小化時は本文 (`EntityListBody`) を非表示化し、パネル高さをヘッダー相当へ縮小

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
- `build_entity_list_view_model_system`
  - `current/previous` スナップショット構築
- `sync_entity_list_from_view_model_system`
  - 使い魔セクションを差分同期（追加/削除/折りたたみ/ヘッダーテキスト）
  - 未所属ソウル行をキー管理で差分更新（`EntityListNodeIndex.unassigned_rows`）
  - 表示順は `replace_children` でビュー順へ再整列

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

## 補助表示
- `Unassigned Souls` の内容がオーバーフローした時のみスクロールヒント表示
- `IgnoreScroll` によりヘッダー要素はスクロール対象から除外

## 入力ガード
- `UiInputBlocker` + `UiInputState` でUI上のワールド入力を抑止
- スクロール領域上のホイール入力はリスト優先

## 楽観的更新（体感改善）
- 使役数上限の `-` / `+` 操作時、`FamiliarOperation.max_controlled_soul` 更新直後に
  使い魔ヘッダー表示（`現在/最大`）を即時更新する
- 最終的な整合は通常の100ms差分同期と `FamiliarOperationMaxSoulChangedEvent` の処理で維持する

## 主な関連ファイル（最終境界反映）

### root shell（adapter）
- `src/interface/ui/list/mod.rs` - イベント受付、interaction/system 登録、`EntityListDirty` の橋渡し
- `src/interface/ui/list/change_detection.rs` - 変更検知トリガ（root に残置）
- `src/interface/ui/list_legacy/` - 互換のため残っている旧実装（今後整理対象）

### `hw_ui` 側（分離済み）
- `crates/hw_ui/src/list/models.rs` - ビューモデル型
- `crates/hw_ui/src/list/dirty.rs` - dirty リソース定義
- `crates/hw_ui/src/list/mod.rs` - `hw_ui` 対外エクスポート

### 境界横断
- `src/interface/ui/components.rs`, `src/interface/ui/theme.rs` は `hw_ui` API の再エクスポートシェルとして残す
- `src/interface/ui/setup/entity_list.rs`（初期構築）は root shell 経由で呼び出される
