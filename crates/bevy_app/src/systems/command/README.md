# command — プレイヤーコマンド処理

## 役割

プレイヤーが Familiar に与えるコマンド（タスクエリア設定・ゾーン配置・タスク指定）を処理するシステム群。
UI からの入力を受け取り、`Designation` や `TaskArea` コンポーネントを生成・変更する。

## crate 境界

このモジュールは **shell + visual + ECS apply** のみを担う。

| 責務 | 所有先 |
|---|---|
| pure geometry helper（`wall_line_area`, `area_from_center_and_size` 等） | `hw_core::area` |
| タスクモードのドラッグ開始座標取得（`get_drag_start`） | `hw_core::area` |
| ゾーン連結判定 / 削除対象特定（`identify_removal_targets`） | `hw_world::zone_ops` |
| ゾーン geometry helper（`area_tile_size`, `rectangles_overlap*`, `expand_yard_area`） | `hw_world::zone_ops` |
| 手動 haul 選定アルゴリズム（`select_stockpile_anchor`, `find_existing_request`） | `hw_logistics::manual_haul_selector` |
| 入力 orchestration / camera / window 依存処理 | root（本モジュール） |
| ECS `Commands` / `WorldMapWrite` を使うゾーン適用処理 | root（本モジュール） |
| visual spawn / indicator 更新 | root（本モジュール） |
| `AreaEditSession` / `AreaEditHistory` 等の Resource 型 | root（本モジュール） |

## 主要ファイル

| ファイル | 内容 |
|---|---|
| `mod.rs` | 公開 API（crate 所有 helper と shell system に分けてコメント整理済み） |
| `assign_task.rs` | `assign_task_system` — クリックによるタスク指定 |
| `input.rs` | `familiar_command_input_system` — Familiar コマンド入力処理 |
| `indicators.rs` | タスクエリア・指定インジケーターの同期 |
| `visualization.rs` | コマンド状態の視覚フィードバック |

## area_selection/ ディレクトリ

タスクエリアのドラッグ選択・編集機能。

| ファイル | 内容 |
|---|---|
| `apply.rs` | エリア選択の確定（ECS apply） |
| `cancel.rs` | エリア選択のキャンセル |
| `cleanup.rs` | エリア選択後のクリーンアップ |
| `cursor.rs` | カーソル位置の追跡 |
| `geometry.rs` | root 側 UI/camera 依存 helper。pure helper は `hw_core::area` に移設済み（re-export で公開） |
| `input.rs` / `input/press.rs` / `input/drag.rs` / `input/transitions.rs` / `input/release/` | エリア選択入力処理（press・drag・releaseをフェーズ別サブモジュールに分割済み） |
| `indicator.rs` | エリア選択ビジュアル（`GameAssets` + mesh/material spawn、root 残留） |
| `manual_haul.rs` | 手動運搬の指定。選定アルゴリズムは `hw_logistics::manual_haul_selector` を呼ぶ thin adapter |
| `queries.rs` | `DesignationTargetQuery` 型定義 |
| `shortcuts.rs` | キーボードショートカット |
| `state.rs` | `AreaEditSession`, `AreaEditHistory`, `AreaEditClipboard`, `AreaEditPresets` Resource 型 |

## zone_placement/ ディレクトリ

ストックパイル・ヤードゾーンの配置・削除。

| ファイル | 内容 |
|---|---|
| `placement.rs` | `zone_placement_system` — ゾーン配置（ECS apply）。バリデーション helper は `hw_world::zone_ops` を呼ぶ |
| `removal.rs` | `zone_removal_system` — ゾーン削除（ECS apply） |
| `removal_preview.rs` | `ZoneRemovalPreviewState` — 削除プレビュー。連結判定は `hw_world::identify_removal_targets` を使用 |

## TaskArea コンポーネント

```rust
TaskArea { bounds: AreaBounds }  // Familiar が管轄するエリア
```

`TaskArea` は `hw_core::area` に定義され、`mod.rs` で re-export されている。
`count_positions_in_area` / `overlap_summary_from_areas` / `wall_line_area` / `get_drag_start` / `area_from_center_and_size`
も同様に `hw_core::area` 由来の pure helper を `mod.rs` 経由で公開している。
`TaskAreaIndicator` コンポーネントで視覚的インジケーターエンティティと紐付けられる。
