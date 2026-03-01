# タスク割り当てビルダー共通化 実装計画

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `assignment-builder-unification-plan-2026-03-01` |
| ステータス | `Completed` |
| 作成日 | `2026-03-01` |
| 最終更新日 | `2026-03-01` |
| 作成者 | `Codex` |
| 関連提案 | `N/A` |
| 関連Issue/PR | `N/A` |

## 1. 目的

- 解決したい課題: タスク割り当てビルダーで `AssignedTask` 構築・予約生成・`submit_assignment` 呼び出しが重複しており、修正漏れリスクが高い。
- 到達したい状態: 各ビルダー関数は「差分パラメータの定義」に集中し、共通処理は1箇所で管理される。
- 成功指標:
  - `builders/basic.rs` / `builders/haul.rs` / `builders/water.rs` の重複ブロックを削減。
  - 予約適用経路と `WorkType` 指定が既存挙動と一致。
  - `cargo check` が成功。

## 2. スコープ

### 対象（In Scope）

- `task_management/builders` 内の共通化。
- `submit_assignment` 呼び出し直前までの共通テンプレート化。
- 予約オペレーション生成の責務整理。

### 非対象（Out of Scope）

- `AssignedTask` の仕様変更。
- タスク探索ポリシー（candidate 選定）のロジック変更。
- 予約データ構造（`ResourceReservationOp`）自体の変更。

## 3. 現状とギャップ

- 現状: 各 `issue_*` が同型の流れを重複実装している。
- 問題:
  - 1箇所修正時に他箇所へ同時反映が必要。
  - 新規タスク追加で boilerplate が増える。
  - 予約漏れや `WorkType` 不整合の温床になる。
- 本計画で埋めるギャップ: 1つの共通 API で割り当て要求を生成し、差分のみ各ビルダーで持つ構造にする。

## 4. 実装方針（高レベル）

- 方針:
  - 共通入力を `AssignmentSpec`（仮称）へ集約。
  - `build_reservation_ops_*` ヘルパーを導入して予約生成を再利用。
  - 既存の `submit_assignment` を最終出口として維持。
- 設計上の前提:
  - 既存のイベント/予約適用順序は変更しない。
  - `AssignedTask` バリアントの生成タイミングは現状維持。
- Bevy 0.18 APIでの注意点:
  - `SystemParam` 型を跨ぐ参照のライフタイム崩壊を避けるため、共通化は純粋関数中心で行う。

## 5. マイルストーン

## M1: 共通化インターフェース導入

- 変更内容:
  - `builders` 内に共通 spec/ヘルパーを追加。
  - 既存関数の引数セットに合う最小 API を定義。
- 変更ファイル:
  - `src/systems/familiar_ai/decide/task_management/builders/mod.rs`
  - `src/systems/familiar_ai/decide/task_management/builders/*.rs`
  - `docs/tasks.md`（必要時）
- 完了条件:
  - [x] 共通 API が追加される
  - [x] 既存呼び出し側の置換準備ができる
- 検証:
  - `cargo check`

## M2: basic/haul/water の移行

- 変更内容:
  - `basic.rs` / `haul.rs` / `water.rs` の重複呼び出しを共通 API に置換。
  - `reserve source/mixer` の分岐をヘルパーで統一。
- 変更ファイル:
  - `src/systems/familiar_ai/decide/task_management/builders/basic.rs`
  - `src/systems/familiar_ai/decide/task_management/builders/haul.rs`
  - `src/systems/familiar_ai/decide/task_management/builders/water.rs`
- 完了条件:
  - [x] 3ファイルで同型 boilerplate が削減される
  - [x] 予約オペレーション内容が既存と一致
- 検証:
  - `cargo check`

## M3: 仕様同期とフォローアップ

- 変更内容:
  - 命名/責務コメントを整理。
  - `docs/tasks.md` の実装境界が変わる場合は更新。
- 変更ファイル:
  - `src/systems/familiar_ai/decide/task_management/builders/mod.rs`
  - `docs/tasks.md`（必要時）
  - `docs/DEVELOPMENT.md`（必要時）
- 完了条件:
  - [x] 共通化後の責務境界が文書化される
  - [x] `cargo check` が通る
- 検証:
  - `cargo check`

## 6. リスクと対策

| リスク | 影響 | 対策 |
| --- | --- | --- |
| 予約生成の差分を吸収しきれない | 実行時の予約競合 | 先に現行 `reservation_ops` を一覧化し、1件ずつ移行して比較する |
| 共通 API が過剰抽象化になる | 可読性低下 | 「3箇所以上で重複する処理のみ共通化」をルール化する |
| WorkType の紐付けミス | 誤イベント発火 | `issue_*` ごとの `WorkType` を移行チェックリストで照合する |

## 7. 検証計画

- 必須:
  - `cargo check`
- 手動確認シナリオ:
  - Gather/Haul/Water 系の割り当てが発行されること。
  - `ReservedForTask` / mixer destination 予約が維持されること。
- パフォーマンス確認（必要時）:
  - `cargo run -- --spawn-souls 500 --spawn-familiars 30 --perf-scenario`

## 8. ロールバック方針

- どの単位で戻せるか:
  - `builders` モジュール単位でロールバック可能。
- 戻す時の手順:
  - 共通 API 導入コミットを revert。
  - その後 `cargo check` で整合確認。

## 9. AI引継ぎメモ（最重要）

### 現在地

- 進捗: `100%`
- 完了済みマイルストーン: `M1` / `M2` / `M3`
- 未着手/進行中: なし

### 次のAIが最初にやること

1. 必要に応じて `builders` へ新規 `issue_*` を追加する際、`AssignmentSpec` + 予約ヘルパーを再利用する。
2. 新規タスク追加時は `WorkType` と `reservation_ops` の整合をレビューする。
3. 変更後に `cargo check` を実行する。

### ブロッカー/注意点

- `WheelbarrowDestination::Mixer` の予約は item type 推定を含むため分岐を残す必要がある。

### 参照必須ファイル

- `src/systems/familiar_ai/decide/task_management/builders/mod.rs`
- `src/systems/familiar_ai/decide/task_management/builders/basic.rs`
- `src/systems/familiar_ai/decide/task_management/builders/haul.rs`
- `src/systems/familiar_ai/decide/task_management/builders/water.rs`
- `docs/DEVELOPMENT.md`

### 最終確認ログ

- 最終 `cargo check`: `2026-03-01` / `pass` (`cargo check --target-dir /tmp/hell-workers-target`)
- 未解決エラー: なし（計画作成時点）

### Definition of Done

- [x] 目的に対応するマイルストーンが全て完了
- [x] 影響ドキュメントが更新済み
- [x] `cargo check` が成功

## 10. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| `2026-03-01` | `Codex` | 初版作成 |
| `2026-03-01` | `Codex` | 実装完了に合わせてステータス・進捗・DoDを更新 |
