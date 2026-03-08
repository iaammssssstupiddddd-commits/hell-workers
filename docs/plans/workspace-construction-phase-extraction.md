# Floor/Wall 建設フェーズ列挙型の hw_jobs 抽出

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `workspace-construction-phase-extraction-2026-03-08` |
| ステータス | `Completed` |
| 作成日 | `2026-03-08` |
| 最終更新日 | `2026-03-08` |
| 作成者 | `AI` |
| 関連提案 | `N/A` |
| 関連Issue/PR | `N/A` |

## 1. 目的

- 解決したい課題: Floor/Wall 建設で使うフェーズ・状態型が `src/systems/jobs/*_construction/components.rs` に散在し、`hw_core::assigned_task` の `AssignedTask` バリアント（`ReinforceFloorTile`, `PourFloorTile`, `FrameWallTile`, `CoatWall`）と密接に関連するにもかかわらず crate 境界を越えて型安全に共有できない。
- 到達したい状態: `FloorConstructionPhase`, `FloorTileState`, `WallConstructionPhase`, `WallTileState` を `crates/hw_jobs` に移設し、`hw_core`/AI 側との型依存を薄くする。
- 成功指標: `cargo check --workspace` 成功、対象 enum が `hw_jobs::construction` から再利用可能、root 側は互換性を保った再エクスポート化。

## 2. スコープ

### 対象（In Scope）

- `FloorConstructionPhase`, `FloorTileState` を `crates/hw_jobs` に移動
- `WallConstructionPhase`, `WallTileState` を `crates/hw_jobs` に移動
- `src/systems/jobs/floor_construction/components.rs`、`src/systems/jobs/wall_construction/components.rs` の enum 重複定義を撤去し、`pub use hw_jobs::construction::{...}` に置換
- `hw_jobs` 側から `pub mod construction` として公開 API を整理

### 非対象（Out of Scope）

- `FloorConstructionSite` / `WallConstructionSite` の移動（`TaskArea` 依存のため root に維持）
- `FloorTileBlueprint` / `WallTileBlueprint` の移動（ゲーム固有の entity/state を持つため root に維持）
- `TargetFloorConstructionSite` / `TargetWallConstructionSite` の移動
- `*CancelRequested` マーカー系コンポーネントの移動

## 3. 現状とギャップ

- 現状:
  - `FloorConstructionPhase` / `FloorTileState` は `src/systems/jobs/floor_construction/components.rs`
  - `WallConstructionPhase` / `WallTileState` は `src/systems/jobs/wall_construction/components.rs`
  - `hw_core::assigned_task::ReinforceFloorPhase` 等は worker 観点の進捗 enum として別定義
- 問題:
  - フェーズ/状態が module 境界で分断され、型の意味対応を毎回 mapping する必要がある
  - root / core / jobs 間で import パスが増え、横断改修時に破綻しやすい
- 本計画で埋めるギャップ:
  - 建設フェーズ関連型を `hw_jobs` に集約して、crate 越えの参照を標準化
  - 将来の `AssignedTask` 側 enum 統合の前提を作る

## 4. 実装方針（高レベル）

- 方針: `hw_jobs` に `construction` モジュールを追加し、4 つの enum を純データ型として移設。
- 設計上の前提:
  - 本計画では `Component` derive は付けず、`Clone / Copy / Debug / PartialEq / Eq / Reflect` を維持する
  - Floor/Wall 側の site / blueprint struct は root 側で維持し、再エクスポートで既存 API を保つ
  - `hw_jobs` は既存依存で bevy への依存を持つため `Reflect` derive は追加で問題が出にくい

## 5. マイルストーン

### M1: hw_jobs に construction モジュール追加

- 変更内容:
  - `crates/hw_jobs/src/construction.rs` を新規作成
  - `FloorConstructionPhase`, `FloorTileState`, `WallConstructionPhase`, `WallTileState` を定義
  - `crates/hw_jobs/src/lib.rs` に `pub mod construction;` を追加
- 変更ファイル:
  - `crates/hw_jobs/src/construction.rs`（新規）
  - `crates/hw_jobs/src/lib.rs`
- 完了条件:
- [x] `hw_jobs::construction` から 4 つの enum が public として利用可能
- 検証:
  - `cargo check -p hw_jobs`

### M2: root コンポーネントでの再エクスポート化

- 変更内容:
  - `src/systems/jobs/floor_construction/components.rs`
    - `FloorConstructionPhase`/`FloorTileState` の定義を削除し、`pub use hw_jobs::construction::{FloorConstructionPhase, FloorTileState};` を追加
  - `src/systems/jobs/wall_construction/components.rs`
    - `WallConstructionPhase`/`WallTileState` の定義を削除し、`pub use hw_jobs::construction::{WallConstructionPhase, WallTileState};` を追加
  - `src/systems/jobs/mod.rs` の re-export 経路を確認し、既存利用側の import 変更を最小化
- 変更ファイル:
  - `src/systems/jobs/floor_construction/components.rs`
  - `src/systems/jobs/wall_construction/components.rs`
  - `src/systems/jobs/mod.rs`（必要時）
- 完了条件:
- [x] root 側で enum 定義が 0 件
- [x] `cargo check` 通過で既存 import が壊れていない
- 検証:
  - `cargo check`

### M3: docs と型関係の明文化（任意）

- 変更内容:
  - `docs/architecture.md` に `AssignedTask` 側の worker フェーズ (`ReinforceFloorPhase` など) と tile 状態 (`FloorTileState`/`WallTileState`) の対応方針を明文化
  - 統合は次フェーズとし、本計画では「分離したまま責務を明示」に留める
- 変更ファイル:
  - `docs/architecture.md`
- 完了条件:
  - [x] 2 系統の enum の関係が文書化されている
- 検証:
  - N/A（ドキュメントのみ）

## 6. リスクと対策

| リスク | 影響 | 対策 |
| --- | --- | --- |
| `Reflect` derive の import/feature 事情 | 中 | `hw_jobs` の既存 bevy 依存を使って derive を維持し、`rustfmt` と `cargo check -p hw_jobs` で確認 |
| `pub use` 置換漏れ | 中 | M2 終了時点で `cargo check` と `rg "FloorConstructionPhase|WallConstructionPhase|FloorTileState|WallTileState"` で旧定義の残存を検出 |
| AssignedTask 型との見落とし | 低 | M3 で型関係を明文化し、実装統合は別計画に分離 |

## 7. 検証計画

- 必須:
  - `cargo check -p hw_jobs`
  - `cargo check`
- 手動確認シナリオ:
  - Floor/Wall 建設タスクの割り当て→進捗更新→完了までの主要経路が成立すること
- パフォーマンス確認: 不要（型移動のみ）

## 8. ロールバック方針

- どの単位で戻せるか:
  - M1, M2, M3 を個別 revert 可能
- 戻す時の手順:
  - `git revert` で対象 commit を戻す（M2 を戻す場合は再エクスポート変更を同時に戻す）

## 9. AI引継ぎメモ（最重要）

### 現在地

- 進捗: `100%`
- 完了済みマイルストーン: `M1, M2, M3`
- 未着手/進行中: `M4`（今回は該当なし）

### 次のAIが最初にやること

1. `src/systems/jobs/floor_construction/components.rs` の enum 定義を切り分け対象として確認
2. `src/systems/jobs/wall_construction/components.rs` の enum 定義を切り分け対象として確認
3. `crates/hw_jobs/src/construction.rs` を新規作成し、4 つの enum を移設
4. `docs/architecture.md` に型分離責務を追記

### ブロッカー/注意点

- `FloorConstructionSite` / `WallConstructionSite` は `TaskArea` 依存のため、site struct は移動しない
- `FloorTileState` 系は「worker の進捗」とは別の概念。`hw_core::assigned_task::*Phase` へは安易な統合をしない

### 参照必須ファイル

- `src/systems/jobs/floor_construction/components.rs` — 現在の `FloorConstructionPhase` / `FloorTileState` 定義
- `src/systems/jobs/wall_construction/components.rs` — 現在の `WallConstructionPhase` / `WallTileState` 定義
- `crates/hw_core/src/assigned_task.rs` — `AssignedTask` 側 phase の現状
- `crates/hw_jobs/src/lib.rs` — `construction` module 追加先
- `docs/architecture.md` — 方針明文化先

### 最終確認ログ

- 最終 `cargo check`: `2026-03-08 / success`
- 未解決エラー: N/A

### Definition of Done

- [x] `hw_jobs::construction` に対象 enum が追加されている
- [x] root 側は enum の再エクスポートのみになっている
- [x] `cargo check` と `cargo check -p hw_jobs` が成功している
- [x] `docs/architecture.md` に型対応の関係が記述されている

## 10. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| `2026-03-08` | `AI` | 初版作成 |
| `2026-03-08` | `AI` | M1/M2/M3 実装完了、`hw_jobs::construction` 追加と root 再エクスポート化、`docs/architecture.md` 追記 |
