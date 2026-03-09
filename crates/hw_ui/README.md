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
| `list/` | エンティティリスト描画・更新 |
| `models/` | UI データモデル（エンティティ詳細ビューモデル等） |
| `panels/` | 情報パネル・メニュー |
| `interaction/` | マウス・キーボード入力処理（下表） |
| `selection/` | 建設配置プレビュー・エンティティ選択（下表） |

### setup/ ディレクトリ

| ファイル | 内容 |
|---|---|
| `bottom_bar.rs` | 下部コントロールバー |
| `time_control.rs` | 時間速度・一時停止 UI |
| `panels.rs` | 情報パネル・メニュー |
| `entity_list.rs` | エンティティ一覧 UI |
| `dialogs.rs` | ダイアログボックス |
| `submenus.rs` | サブメニュー階層 |

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

## 入力ゲーティング

`UiInputState.pointer_over_ui` が UI 上にポインターがあるかどうかを共有フラグとして管理する。
カメラ操作・ゲーム入力処理はこのフラグを確認してUI上でのクリックを無視する。

## 依存クレート

- `hw_core`, `hw_jobs`, `hw_logistics`, `bevy`
