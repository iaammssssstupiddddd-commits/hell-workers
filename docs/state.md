# State管理システム

ゲームの操作モードをBevyのStatesシステムで一元管理します。

## PlayMode

プレイ中の操作モードを表すState。

| モード | 説明 | 遷移条件 |
|--------|------|----------|
| `Normal` | 通常操作（選択・移動） | デフォルト / Escキー |
| `BuildingPlace` | 建物配置中 | Buildボタンクリック |
| `ZonePlace` | ゾーン配置中 | Zoneボタンクリック |
| `TaskDesignation` | タスク指定中（伐採/採掘など） | Ordersメニュー選択 |

遷移: Normal ↔ BuildingPlace（Buildボタン/Esc）、Normal ↔ ZonePlace（Zoneボタン/Esc）、Normal ↔ TaskDesignation（Ordersメニュー/Esc）。

## コンテキストリソース

各モードの詳細情報を保持するリソース。

| リソース | 型 | 用途 |
|----------|-----|------|
| `BuildContext` | `Option<BuildingType>` | 配置する建物の種類 |
| `ZoneContext` | `Option<ZoneType>` | 配置するゾーンの種類 |
| `TaskContext` | `TaskMode` | タスクの詳細（伐採/採掘/運搬など） |

## TaskMode バリアント一覧

`src/systems/command/mod.rs`:

| バリアント | 用途 | ドラッグ開始位置 |
|:--|:--|:--|
| `None` | 通常モード（デフォルト） | — |
| `DesignateChop(Option<Vec2>)` | 伐採指示（矩形ドラッグ） | Some = ドラッグ中 |
| `DesignateMine(Option<Vec2>)` | 採掘指示（矩形ドラッグ） | Some = ドラッグ中 |
| `DesignateHaul(Option<Vec2>)` | 運搬指示（矩形ドラッグ） | Some = ドラッグ中 |
| `CancelDesignation(Option<Vec2>)` | 指示キャンセル（矩形ドラッグ） | Some = ドラッグ中 |
| `SelectBuildTarget` | 建築対象選択中 | — |
| `AreaSelection(Option<Vec2>)` | TaskArea 編集モード | Some = 新規矩形ドラッグ中 |
| `AssignTask(Option<Vec2>)` | 未割当タスクを Familiar に割り当て | Some = ドラッグ中 |
| `ZonePlacement(ZoneType, Option<Vec2>)` | Stockpile/Zone 配置 | Some = ドラッグ中 |
| `ZoneRemoval(ZoneType, Option<Vec2>)` | Zone 解除 | Some = ドラッグ中 |
| `FloorPlace(Option<Vec2>)` | 床エリア配置 | Some = ドラッグ中 |
| `WallPlace(Option<Vec2>)` | 壁ライン配置 | Some = ドラッグ中 |
| `DreamPlanting(Option<Vec2>)` | Dream 植林モード | Some = ドラッグ中 |

`Option<Vec2>` は `None` = 待機、`Some(pos)` = ドラッグ開始位置（進行中）を示す。

## TaskDesignation の補足（TaskArea 編集）

`PlayMode::TaskDesignation` で `TaskContext = TaskMode::AreaSelection(...)` のとき、TaskArea 専用の連続編集モードとして動作します。

### AreaSelection の状態
- `TaskMode::AreaSelection(None)`: 待機（新規ドラッグ開始 / 既存エリア直接編集）
- `TaskMode::AreaSelection(Some(start_pos))`: 新規矩形ドラッグ中

### 遷移ルール
- `Orders -> Area` で `TaskMode::AreaSelection(None)` に遷移
- 適用後はデフォルトで `TaskMode::AreaSelection(None)` を維持（連続編集）
- `Shift + 左ボタンリリース` で適用と同時に `PlayMode::Normal` へ復帰
- `Esc` で `PlayMode::Normal` へ復帰

### 入力補足
- Areaモード中の `Tab` / `Shift + Tab` は Familiar のみを循環対象にする
- `Ctrl + Z / Y`（および `Ctrl + Shift + Z`）で TaskArea の Undo/Redo を行う

## 共通仕様

### Escキーによるキャンセル

- 全モードでEscキーを押すと`Normal`に戻る
- **メニュー展開も同時に閉じる**（`MenuState::Hidden`）

### run_if条件

モード限定システムは `.run_if(in_state(PlayMode::BuildingPlace))` のようにステートでゲートする。`OnEnter` / `OnExit` でモード遷移時の初期化・クリーンアップを実装する。

## 関連ファイル

- `src/game_state.rs` - PlayMode、Context定義
- `src/main.rs` - State登録、OnEnter/OnExit
- `src/interface/selection/` - Escキーによるキャンセル処理
- `src/interface/ui/interaction/mod.rs` - ボタンによる状態遷移とモード表示更新
- `src/systems/logistics.rs` - zone_placement（ZoneContext使用）
