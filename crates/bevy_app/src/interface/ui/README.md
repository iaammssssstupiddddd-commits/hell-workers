# ui — UI システム統合（ルートクレート）

## 役割

`hw_ui` クレートのプラグイン群をゲーム固有の ECS コンポーネントに結びつけるアダプタ層。
ゲームエンティティ（`Familiar`, `DamnedSoul`, `Building` 等）への依存が不可避な処理はここに残し、
純粋 UI ロジックは `hw_ui` に置く。

## ディレクトリ構成

| ディレクトリ/ファイル | 内容 |
|---|---|
| `mod.rs` | app shell 側の正規 UI facade。外部に必要なシンボルだけを明示 re-export |
| `vignette.rs` | 画面周辺ヴィネットエフェクト |
| `plugins/` | UI プラグイン登録 |
| `setup/` | UI 要素の初期スポーン・`UiAssets` アダプタ実装 |
| `panels/` | 情報パネル facade / task_list / tooltip_builder |
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
| `hover_action.rs` | ホバーエフェクト |
| `intent_handler.rs` | `UiIntent` dispatcher |
| `intent_context.rs` | `UiIntent` 処理共通の `SystemParam` / query 集約 |
| `handlers/` | `UiIntent` 種別ごとの実処理（general / familiar_settings / mode_selection / mode_toggle） |
| `menu_actions.rs` | メニューアクション処理 |
| `mode.rs` | UI モード管理 |
| `systems.rs` | インタラクションシステム |
| `status_display.rs` | ステータス表示エントリポイント |
| `status_display/` | ステータスバー描画（runtime, dream bar, mode panel） |
| `tooltip/` | ツールチップ（target, layout, fade） |

## panels/ ディレクトリ

| ファイル/ディレクトリ | 内容 |
|---|---|
| `mod.rs` | 公開 API |
| `context_menu.rs` | コンテキストメニュー（ゲーム固有: Familiar/Soul/Building 分岐） |
| `mod.rs` 内の `hw_ui::panels::info_panel` re-export | `InfoPanelState`, `InfoPanelPinState`, `spawn_info_panel_ui`, `info_panel_system` を facade として公開 |
| `task_list/` | `hw_ui` re-export + ゲーム固有 dirty/view_model/update |
| `tooltip_builder/` | `hw_ui` re-export |

## list/ ディレクトリ

| ファイル/ディレクトリ | 内容 |
|---|---|
| `mod.rs` | 公開 API（hw_ui re-export + ゲーム固有） |
| `sync.rs` | `sync_entity_list_from_view_model_system` / `sync_entity_list_value_rows_system`（`hw_ui::list::sync` の thin shell） |
| `view_model.rs` | リストビューモデル（ゲーム固有クエリ） |
| `change_detection.rs` | 変化検出 |
| `dirty.rs` | ダーティフラグ |
| `drag_drop.rs` | ドラッグ&ドロップ（`DragState` は hw_ui、システムはここ） |
| `interaction.rs` / `interaction/` | リストアイテムのゲーム側インタラクション（SectionToggle は hw_ui 側へ移設済み。`+/-` は target 付き `UiIntent` を発行し、navigation はここ残留） |
| `selection_focus.rs` | camera focus helper |

## 公開方針

- `crate::interface::ui` を root 側の正規入口とし、外部から使うシンボルだけを `mod.rs` で明示 re-export する
- `hw_ui::components::*` / `hw_ui::theme::*` の wildcard 再公開は行わない
- deep path の thin shell を減らし、`panels/mod.rs` や `list/mod.rs` の facade へ集約する

## ルート残留の理由

| ファイル/システム | 残留理由 |
|---|---|
| `list/sync.rs` | `Res<GameAssets>` を受けて `hw_ui::list::sync::*` を呼ぶ thin shell |
| `list/view_model.rs` | `Familiar` / `DamnedSoul` / `AssignedTask` / `FamiliarAiState` などゲーム固有 ECS Query に依存 |
| `interaction/intent_context.rs`, `interaction/handlers/`, `interaction/intent_handler.rs` | `UiIntent::AdjustMaxControlledSoul*` を含むゲーム固有 `UiIntent` を処理する root adapter。`intent_handler.rs` は dispatcher のみで、`FamiliarOperation` 更新や `PlayMode` / `TimeSpeed` / `WorldMapWrite` 依存は `intent_context.rs` と `handlers/` 側に残留 |
| `list/interaction/navigation.rs` | `Res<TaskContext>`（ルート定義型）に依存 |
| `panels/task_list/update.rs` | `Res<GameAssets>` — Bevy は `Res<dyn Trait>` 不可 |
| `panels/context_menu.rs` | `Familiar`/`DamnedSoul`/`Building` ECS クエリ |
| `list/drag_drop.rs` (system) | `DamnedSoul`/`SoulIdentity` ECS クエリ |
