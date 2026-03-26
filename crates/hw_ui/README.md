# hw_ui — UI コンポーネント・インタラクション

## 役割

ゲーム UI 全体（パネル・リスト・ダイアログ・ツールチップ・建設配置プレビュー）のセットアップと入力処理を担うクレート。
`UiRoot` / `UiMountSlot` / `UiNodeRegistry` などの shared UI contract は `hw_core::ui_nodes` が所有し、`hw_ui` はそれを re-export しながら UI 要素をスロット単位で管理する。

## ディレクトリ構成

| ディレクトリ/ファイル | 内容 |
|---|---|
| `lib.rs` | `HwUiPlugin` — 全 UI プラグインの登録 |
| `intents.rs` | `UiIntent` — ユーザー操作の意図メッセージ型（Entity List 用の Familiar 指定 variant を含む） |
| `theme.rs` | スタイリング・テーマ定数 |
| `components.rs` | UI コンポーネントレジストリ・共有ユーティリティ |
| `camera.rs` | `world_cursor_pos`（スクリーン座標→ワールド座標変換 utility。`MainCamera` は `hw_core::camera` から re-export） |
| `area_edit/` | TaskArea 編集モード（インタラクション・状態管理） |
| `setup/` | UI 要素の初期スポーン（下表） |
| `plugins/` | UI システムの Bevy 登録（下表） |
| `list/` | エンティティリスト共通ロジック（下表） |
| `models/` | UI データモデル（エンティティ詳細ビューモデル等） |
| `panels/` | 情報パネル・タスクリスト・メニュー（下表） |
| `interaction/` | マウス・キーボード入力処理（下表） |
| `selection/` | 建設配置プレビュー・エンティティ選択（下表） |

### setup/ ディレクトリ

| ファイル | 内容 |
|---|---|
| `mod.rs` | `UiAssets` trait と `setup_ui` の公開 re-export を持つ root shell |
| `bottom_bar.rs` | 下部コントロールバー |
| `time_control.rs` | 時間速度・一時停止 UI |
| `panels.rs` | 情報パネル・メニュー |
| `entity_list.rs` | エンティティ一覧 UI |
| `dialogs.rs` | ダイアログボックス |
| `submenus.rs` | サブメニュー階層 |
| `root.rs` | UI ルート構築と `setup_ui` 実装本体 |

### list/ ディレクトリ

| ファイル | 内容 |
|---|---|
| `mod.rs` | 公開 API |
| `dirty.rs` | `EntityListDirty` フラグ |
| `drag_state.rs` | `DragState` — ドラッグ中エンティティの状態型 |
| `minimize.rs` | `EntityListMinimizeState` + 最小化トグルシステム |
| `resize.rs` | `EntityListResizeState` + リサイズシステム |
| `section_toggle.rs` | `entity_list_section_toggle_system` — セクション折りたたみの純UI操作 |
| `selection_focus.rs` | `focus_camera_on_entity`, `select_entity_and_focus_camera` |
| `spawn.rs` | Familiar セクション / Soul 行の UI ノード生成 helper |
| `sync.rs` | Familiar / Unassigned Soul の差分同期 helper |
| `tree_ops.rs` | `clear_children` — ツリー操作ユーティリティ |
| `visual.rs` | `apply_row_highlight`, `entity_list_visual_feedback_system` |
| `models.rs` | リスト向けデータモデル、`EntityListNodeIndex`、`FamiliarSectionNodes` |

### panels/ ディレクトリ

| ファイル/ディレクトリ | 内容 |
|---|---|
| `mod.rs` | 公開 API |
| `menu.rs` | `menu_visibility_system` |
| `info_panel/` | エンティティ詳細パネル（`InfoPanelState`, `InfoPanelPinState`, `info_panel_system`, `spawn_info_panel_ui`） |
| `task_list/` | タスク一覧パネル（`TaskEntry`, `TaskListDirty`, render/interaction/work_type_icon） |
| `tooltip_builder/` | ツールチップコンテンツ生成（widgets, text_wrap, templates） |

### interaction/ ディレクトリ

| ファイル/ディレクトリ | 内容 |
|---|---|
| `common.rs` | 共有インタラクションユーティリティ |
| `dialog.rs` | ダイアログ操作 |
| `hover_action.rs` | ホバーエフェクト |
| `status_display/` | ステータスバー描画 (runtime, dream bar, mode panel) |
| `tooltip/` | ツールチップ (`mod.rs` が共有型/re-export、`system.rs` が `hover_tooltip_system` 本体、`target`/`layout`/`fade` が補助) |

### selection/ ディレクトリ

| ファイル | 内容 |
|---|---|
| `intent.rs` | `SelectionIntent` 型 |
| `placement.rs` | 建設配置バリデーション・ジオメトリ計算 |

`selection/` は `SelectedEntity` / `HoveredEntity` / `SelectionIndicator` を `hw_core::selection` から re-export し、
despawn 後の参照掃除を行う `cleanup_selection_references_system` と配置判定 helper を持つ。

## UI スロット構造

```
UiRoot
  └── UiMountSlot (LeftPanel / RightPanel / Bottom / Overlay / TopRight / TopLeft)
        └── 各 UI コンポーネント
```

`UiNodeRegistry`（実体は `hw_core::ui_nodes`）が各スロット/エンティティの安定したマッピングを保持し、更新システムから参照される。

## アセット抽象化（UiAssets）

`setup/mod.rs` の `UiAssets` trait により、セットアップ関数がゲーム固有の `GameAssets` に直接依存しない設計になっている。`setup_ui` の実装本体は `setup/root.rs` に置き、`mod.rs` は trait と公開面の root shell にとどめる。
ルートクレートで `GameAssets: UiAssets` を実装し、`&dyn UiAssets` として渡す。

Entity List の `spawn` / `sync` helper もこの trait を利用し、`font_soul_name`、`icon_arrow_right`、`icon_idle` を含むフォント・アイコン供給を root adapter に委譲する。

```rust
// ルートクレート側でのアダプタ実装例
impl UiAssets for GameAssets {
    fn font_ui(&self) -> Handle<Font> { self.font_ui.clone() }
    // ...
}
```

## 入力ゲーティング

`UiInputState.pointer_over_ui` が UI 上にポインターがあるかどうかを共有フラグとして管理する。
カメラ操作・ゲーム入力処理はこのフラグを確認してUI上でのクリックを無視する。

## ここに置かないもの

| 理由 | 例 |
|---|---|
| ゲームエンティティ ECS クエリ | `DamnedSoul`, `Familiar` に触れるシステム |
| ゲーム固有 ViewModel 構築 | `Familiar` / `AssignedTask` から `EntityListViewModel` を組み立てる処理 |
| `Res<GameAssets>` を引数にするシステム | `task_list/update.rs` — Bevy は `Res<dyn Trait>` 不可 |
| ゲーム状態遷移 (`PlayMode`) | ルートクレートの責務 |
| `app_contexts` 型 | `TaskContext` 等 — ルートクレート定義 |
| world-space 選択表示の描画 | `update_selection_indicator` — 実装は `hw_visual`、root `Interface` フェーズで登録 |

## 依存クレート

- `hw_core`, `hw_jobs`, `hw_logistics`, `bevy`
