# hw_ui — UI コンポーネント・インタラクション

## 役割

ゲーム UI 全体（パネル・リスト・ダイアログ・ツールチップ・建設配置プレビュー）のセットアップと入力処理を担うクレート。
`UiRoot` / `UiMountSlot` / `UiNodeRegistry` の仕組みにより、UI要素をスロット単位で管理する。

## ディレクトリ構成

| ディレクトリ/ファイル | 内容 |
|---|---|
| `lib.rs` | `HwUiPlugin` — 全 UI プラグインの登録 |
| `intents.rs` | `UiIntent` — ユーザー操作の意図メッセージ型 |
| `theme.rs` | スタイリング・テーマ定数 |
| `components.rs` | UI コンポーネントレジストリ・共有ユーティリティ |
| `camera.rs` | UI / ワールドビューのカメラシステム |
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
| `mod.rs` | `UiAssets` trait — セットアップに必要なフォント・アイコンの抽象化 |
| `bottom_bar.rs` | 下部コントロールバー |
| `time_control.rs` | 時間速度・一時停止 UI |
| `panels.rs` | 情報パネル・メニュー |
| `entity_list.rs` | エンティティ一覧 UI |
| `dialogs.rs` | ダイアログボックス |
| `submenus.rs` | サブメニュー階層 |

### list/ ディレクトリ

| ファイル | 内容 |
|---|---|
| `mod.rs` | 公開 API |
| `dirty.rs` | `EntityListDirty` フラグ |
| `drag_state.rs` | `DragState` — ドラッグ中エンティティの状態型 |
| `minimize.rs` | `EntityListMinimizeState` + 最小化トグルシステム |
| `resize.rs` | `EntityListResizeState` + リサイズシステム |
| `selection_focus.rs` | `focus_camera_on_entity`, `select_entity_and_focus_camera` |
| `tree_ops.rs` | `clear_children` — ツリー操作ユーティリティ |
| `visual.rs` | `apply_row_highlight`, `entity_list_visual_feedback_system` |
| `models.rs` | リスト向けデータモデル |

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
| `tooltip/` | ツールチップ (target, layout, fade) |

### selection/ ディレクトリ

| ファイル | 内容 |
|---|---|
| `intent.rs` | `SelectionIntent` 型 |
| `placement.rs` | 建設配置バリデーション・ジオメトリ計算 |

## UI スロット構造

```
UiRoot
  └── UiMountSlot (LeftPanel / RightPanel / Bottom / Overlay / TopRight / TopLeft)
        └── 各 UI コンポーネント
```

`UiNodeRegistry` が各スロット/エンティティの安定したマッピングを保持し、更新システムから参照される。

## アセット抽象化（UiAssets）

`setup/mod.rs` の `UiAssets` trait により、セットアップ関数がゲーム固有の `GameAssets` に直接依存しない設計になっている。
ルートクレートで `GameAssets: UiAssets` を実装し、`&dyn UiAssets` として渡す。

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
| `Res<GameAssets>` を引数にするシステム | `task_list/update.rs` — Bevy は `Res<dyn Trait>` 不可 |
| ゲーム状態遷移 (`PlayMode`) | ルートクレートの責務 |
| `app_contexts` 型 | `TaskContext` 等 — ルートクレート定義 |

## 依存クレート

- `hw_core`, `hw_jobs`, `hw_logistics`, `bevy`
