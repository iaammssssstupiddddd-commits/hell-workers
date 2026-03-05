# TaskExecution SystemParam 整理リファクタ実装計画

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `taskexecution-systemparam-refactor-plan-2026-03-05` |
| ステータス | `Draft` |
| 作成日 | `2026-03-05` |
| 最終更新日 | `2026-03-05` |
| 作成者 | `Codex` |
| 関連提案 | `N/A` |
| 関連Issue/PR | `N/A` |

## 1. 目的

- 解決したい課題: `task_execution/context.rs` に SystemParam と trait 実装が集中し、変更時の影響範囲が広い。
- 到達したい状態: Reservation/Designation/Storage/AssignmentRead を責務別に整理し、拡張時の競合リスクを下げる。
- 成功指標:
  - `context.rs` の責務分離が完了し可読性が向上。
  - `TaskQueries` 系 API の互換性を維持。
  - `cargo check` 成功。

## 2. スコープ

### 対象（In Scope）

- `src/systems/soul_ai/execute/task_execution/context.rs` の分割。
- `TaskReservationAccess` 実装重複の簡素化。
- `mod.rs` の再 export 整理。

### 非対象（Out of Scope）

- Task 実行仕様変更。
- クエリ内容（対象コンポーネント）の仕様変更。

## 3. 現状とギャップ

- 現状:
  - Query 型定義、SystemParam 集約、trait 実装が1ファイルで肥大化。
  - `TaskReservationAccess` 実装が3型でほぼ重複。
- 問題:
  - 追加コンポーネント対応時に差分が散らばる。
- 本計画で埋めるギャップ:
  - 型定義と振る舞い定義を分け、変更粒度を縮小する。

## 4. 実装方針（高レベル）

- 方針:
  - `context/` モジュール化し `access.rs`, `queries.rs`, `execution.rs` に分割。
  - trait 実装は共通 getter 経路を作り重複を削減。
- 設計上の前提:
  - 既存 callsite 互換のため public 型名は維持する。
  - `Deref/DerefMut` 契約は保持する。
- Bevy 0.18 APIでの注意点:
  - `#[derive(SystemParam)]` のライフタイム定義を崩さない。
  - Query mutability の変更を避ける。

## 5. マイルストーン

## M1: context モジュール分割

- 変更内容:
  - `context.rs` をディレクトリ化し型群を分割。
  - 既存 import パスを壊さない re-export を提供。
- 変更ファイル:
  - `src/systems/soul_ai/execute/task_execution/context.rs`（置換）
  - `src/systems/soul_ai/execute/task_execution/context/mod.rs`
  - `src/systems/soul_ai/execute/task_execution/context/access.rs`
  - `src/systems/soul_ai/execute/task_execution/context/queries.rs`
  - `src/systems/soul_ai/execute/task_execution/context/execution.rs`
- 完了条件:
  - [ ] 既存参照がコンパイル可能。
  - [ ] public API 互換を維持。
- 検証:
  - `cargo check`

## M2: TaskReservationAccess 重複削減

- 変更内容:
  - `TaskQueries/TaskAssignmentQueries/TaskUnassignQueries` の実装重複を共通化。
  - belongs_to 取得経路を一箇所に寄せる。
- 変更ファイル:
  - `src/systems/soul_ai/execute/task_execution/context/queries.rs`
- 完了条件:
  - [ ] trait 実装重複が削減される。
  - [ ] 既存 reservation writer の挙動が維持。
- 検証:
  - `cargo check`

## M3: docs 同期と保守コメントの最小追加

- 変更内容:
  - タスククエリ境界の説明を docs に反映（必要時）。
  - 非自明箇所にのみ短いコメント追加。
- 変更ファイル:
  - `docs/tasks.md`（必要時）
  - `docs/DEVELOPMENT.md`（必要時）
- 完了条件:
  - [ ] 新構成の責務境界が追える。
  - [ ] `cargo check` 成功。
- 検証:
  - `cargo check`

## 6. リスクと対策

| リスク | 影響 | 対策 |
| --- | --- | --- |
| SystemParam ライフタイム崩れ | コンパイルエラー | 分割は型移動のみから開始し挙動変更を後段化 |
| trait 共通化時の参照ミス | 予約操作の不整合 | 既存3実装のシグネチャ互換を diff で確認 |

## 7. 検証計画

- 必須:
  - `cargo check`
- 手動確認シナリオ:
  - Familiar の task assignment 発行。
  - Soul の task execute/unassign 時の reservation 更新。
- パフォーマンス確認（必要時）:
  - 不要（構造整理が目的）。

## 8. ロールバック方針

- どの単位で戻せるか:
  - M1（分割）と M2（trait整理）を分離して戻せる。
- 戻す時の手順:
  - 失敗段階のみ revert。
  - `cargo check` で安全確認。

## 9. AI引継ぎメモ（最重要）

### 現在地

- 進捗: `0%`
- 完了済みマイルストーン: なし
- 未着手/進行中: `M1`,`M2`,`M3`

### 次のAIが最初にやること

1. `context.rs` を「型定義」と「実装」に区別してマッピング。
2. M1（分割）だけ先行し `cargo check`。
3. M2 で trait 重複削減に入る。

### ブロッカー/注意点

- `TaskAssignmentQueries` の `Deref` 契約を壊さないこと。
- `TaskReservationAccess` の public trait 名を変えないこと。

### 参照必須ファイル

- `src/systems/soul_ai/execute/task_execution/context.rs`
- `src/systems/soul_ai/execute/task_execution/mod.rs`
- `docs/tasks.md`

### 最終確認ログ

- 最終 `cargo check`: `2026-03-05` / `not run`（計画書作成のみ）
- 未解決エラー: `N/A`

### Definition of Done

- [ ] 目的に対応するマイルストーンが全て完了
- [ ] 影響ドキュメントが更新済み
- [ ] `cargo check` が成功

## 10. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| `2026-03-05` | `Codex` | 初版作成 |
