# command — プレイヤーコマンド処理

## 役割

プレイヤーが Familiar に与えるコマンド（タスクエリア設定・ゾーン配置・タスク指定）を処理するシステム群。
UI からの入力を受け取り、`Designation` や `TaskArea` コンポーネントを生成・変更する。

## 主要ファイル

| ファイル | 内容 |
|---|---|
| `mod.rs` | `TaskArea` コンポーネント定義・公開 API |
| `assign_task.rs` | `assign_task_system` — クリックによるタスク指定 |
| `input.rs` | `familiar_command_input_system` — Familiar コマンド入力処理 |
| `indicators.rs` | タスクエリア・指定インジケーターの同期 |
| `visualization.rs` | コマンド状態の視覚フィードバック |

## area_selection/ ディレクトリ

タスクエリアのドラッグ選択・編集機能。

| ファイル | 内容 |
|---|---|
| `apply.rs` | エリア選択の確定 |
| `cancel.rs` | エリア選択のキャンセル |
| `cleanup.rs` | エリア選択後のクリーンアップ |
| `cursor.rs` | カーソル位置の追跡 |
| `geometry.rs` | エリア形状計算 |
| `input.rs` | エリア選択入力処理 |
| `indicator.rs` | エリア選択ビジュアル |
| `manual_haul.rs` | 手動運搬の指定 |
| `queries.rs` | エリア選択クエリ |
| `shortcuts.rs` | キーボードショートカット |
| `state.rs` | `AreaEditSession`, `AreaEditHistory` 等の状態管理 |

## zone_placement/ ディレクトリ

ストックパイル・ヤードゾーンの配置・削除。

| ファイル | 内容 |
|---|---|
| `placement.rs` | `zone_placement_system` — ゾーン配置 |
| `removal.rs` | `zone_removal_system` — ゾーン削除 |
| `removal_preview.rs` | `ZoneRemovalPreviewState` — 削除プレビュー |
| `connectivity.rs` | ゾーン連結性チェック |

## TaskArea コンポーネント

```rust
TaskArea { bounds: AreaBounds }  // Familiar が管轄するエリア
```

`TaskAreaIndicator` コンポーネントで視覚的インジケーターエンティティと紐付けられる。
