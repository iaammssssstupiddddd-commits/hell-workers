# ui — UI システム統合（ルートクレート）

## 役割

`hw_ui` クレートのプラグイン群をゲーム固有の ECS コンポーネントに結びつけるアダプタ層。
ゲームエンティティ（`Familiar`, `DamnedSoul`, `Building` 等）への依存が不可避な処理はここに残し、
純粋 UI ロジックは `hw_ui` に置く。

## ディレクトリ構成

| ディレクトリ/ファイル | 内容 |
|---|---|
| `mod.rs` | 全サブモジュールの公開 API |
| `components.rs` | UI コンポーネント共通定義（`hw_ui` からの re-export 含む） |
| `theme.rs` | スタイリング定数（色・フォント・サイズ） |
| `vignette.rs` | 画面周辺ヴィネットエフェクト |
| `plugins/` | UI プラグイン登録 |
| `setup/` | UI 要素の初期スポーン・`UiAssets` アダプタ実装 |
| `panels/` | 情報パネル（ゲーム固有 + hw_ui re-export） |
| `list/` | エンティティリスト（ゲーム固有実装 + hw_ui re-export） |
| `presentation/` | UI ビルダー・プレゼンテーション層 |
| `interaction/` | マウス・キーボード入力ハンドラ |

## setup/ ディレクトリ

| ファイル | 内容 |
|---|---|
| `mod.rs` | `GameAssets` が `UiAssets` を実装するアダプタ |

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
| `context_menu.rs` | コンテキストメニュー（ゲーム固有: Familiar/Soul/Building 分岐） |
| `info_panel/` | `hw_ui` re-export + ゲーム固有ビューモデル |
| `task_list/` | `hw_ui` re-export + ゲーム固有 dirty/view_model/update |
| `tooltip_builder/` | `hw_ui` re-export |

## list/ ディレクトリ

| ファイル/ディレクトリ | 内容 |
|---|---|
| `mod.rs` | 公開 API（hw_ui re-export + ゲーム固有） |
| `spawn.rs` / `spawn/` | リストノードのスポーン |
| `sync.rs` / `sync/` | リスト内容の同期 |
| `view_model.rs` | リストビューモデル（ゲーム固有クエリ） |
| `change_detection.rs` | 変化検出 |
| `dirty.rs` | ダーティフラグ（`hw_ui::list::EntityListDirty` re-export） |
| `drag_drop.rs` | ドラッグ&ドロップ（`DragState` は hw_ui、システムはここ） |
| `interaction.rs` / `interaction/` | リストアイテムのインタラクション（navigation はここ残留） |
| `selection_focus.rs` | `hw_ui::list` re-export |
| `tree_ops.rs` | `hw_ui::list::clear_children` re-export |

## ルート残留の理由

| ファイル/システム | 残留理由 |
|---|---|
| `list/interaction/navigation.rs` | `Res<TaskContext>`（ルート定義型）に依存 |
| `panels/task_list/update.rs` | `Res<GameAssets>` — Bevy は `Res<dyn Trait>` 不可 |
| `panels/context_menu.rs` | `Familiar`/`DamnedSoul`/`Building` ECS クエリ |
| `list/drag_drop.rs` (system) | `DamnedSoul`/`SoulIdentity` ECS クエリ |
