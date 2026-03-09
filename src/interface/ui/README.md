# ui — UI システム統合

## 役割

ゲーム UI 全体（パネル・リスト・ツールチップ・インタラクション・ヴィネット）のルートクレート統合。
`hw_ui` クレートのプラグイン群をここで組み合わせ、ゲーム固有の UI を構成する。

## ディレクトリ構成

| ディレクトリ/ファイル | 内容 |
|---|---|
| `mod.rs` | 全サブモジュールの公開 API |
| `components.rs` | UI コンポーネント共通定義 |
| `theme.rs` | スタイリング定数（色・フォント・サイズ） |
| `vignette.rs` | 画面周辺ヴィネットエフェクト |
| `plugins/` | UI プラグイン登録 |
| `setup/` | UI 要素の初期スポーン |
| `panels/` | 情報パネル |
| `list/` | エンティティリスト（最新実装） |
| `list_legacy/` | エンティティリスト（旧実装・移行中） |
| `panels_legacy/` | パネル（旧実装・移行中） |
| `presentation/` | UI ビルダー・プレゼンテーション層 |
| `interaction/` | マウス・キーボード入力ハンドラ |

## interaction/ ディレクトリ

| ファイル/ディレクトリ | 内容 |
|---|---|
| `mod.rs` | 公開 API |
| `common.rs` | 共有インタラクションユーティリティ |
| `dialog.rs` | ダイアログ操作 |
| `hover_action.rs` | ホバーエフェクト |
| `intent_handler.rs` | `UiIntent` メッセージ処理 |
| `menu_actions.rs` | メニューアクション処理 |
| `mode.rs` | UI モード管理 |
| `status_display.rs` | ステータス表示エントリポイント |
| `status_display/` | ステータスバー描画（runtime, dream bar, mode panel） |
| `tooltip/` | ツールチップ（target, layout, fade） |

## panels/ ディレクトリ

| ファイル/ディレクトリ | 内容 |
|---|---|
| `mod.rs` | 公開 API |
| `context_menu.rs` | コンテキストメニュー |
| `info_panel/` | エンティティ詳細パネル（layout, model, state, update） |
| `task_list/` | タスク一覧パネル |
| `tooltip_builder/` | ツールチップコンテンツ生成 |

## list/ ディレクトリ

| ファイル | 内容 |
|---|---|
| `mod.rs` | 公開 API |
| `spawn.rs` | リストノードのスポーン |
| `sync.rs` | リスト内容の同期 |
| `view_model.rs` | リストビューモデル |
| `change_detection.rs` | 変化検出 |
| `dirty.rs` | ダーティフラグ管理 |
| `interaction/` | リストアイテムのインタラクション |
| `tree_ops.rs` | ツリー操作ユーティリティ |
| `drag_drop.rs` | ドラッグ&ドロップ |
| `minimize.rs` | リスト最小化 |
| `resize.rs` | リストリサイズ |
| `selection_focus.rs` | 選択フォーカス管理 |
