# Soul Pathfinding 実行器分割リファクタ実装計画

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `pathfinding-executor-split-plan-2026-03-05` |
| ステータス | `Draft` |
| 作成日 | `2026-03-05` |
| 最終更新日 | `2026-03-05` |
| 作成者 | `Codex` |
| 関連提案 | `N/A` |
| 関連Issue/PR | `N/A` |

## 1. 目的

- 解決したい課題: `entities/damned_soul/movement/pathfinding.rs` の `pathfinding_system` が予算管理・再利用・fallback・失敗処理を一体で扱っている。
- 到達したい状態: 「探索対象抽出」「予算消費」「経路計算」「失敗後処理」の責務を分離し、挙動追跡しやすくする。
- 成功指標:
  - `pathfinding_system` 本体が orchestration に集中する。
  - 到達不能時の cleanup 契約が維持される。
  - `cargo check` 成功。

## 2. スコープ

### 対象（In Scope）

- `src/entities/damned_soul/movement/pathfinding.rs` の内部分割。
- 予算配分 (`RESERVED_IDLE_PATHFINDS_PER_FRAME`) の扱い整理。
- rest area fallback の責務切り出し。

### 非対象（Out of Scope）

- A* アルゴリズム変更。
- `world/pathfinding.rs` の探索仕様変更。

## 3. 現状とギャップ

- 現状:
  - 1システムで2フェーズループ（task優先/idle優先）と各種分岐を処理。
  - cleanup の副作用が深い条件分岐内に埋まっている。
- 問題:
  - 不具合調査時に分岐追跡コストが高い。
- 本計画で埋めるギャップ:
  - 意図単位の補助関数群へ分離し、回帰範囲を限定する。

## 4. 実装方針（高レベル）

- 方針:
  - worker 単位更新関数を導入し、budget 管理を明示 API 化。
  - `try_reuse_existing_path` / `try_rest_area_fallback_path` / `cleanup_unreachable_destination` の呼び出し順を固定。
- 設計上の前提:
  - 現行の path cooldown と unassign_task 契約を維持。
  - task優先→idle補助の予算配分設計を維持。
- Bevy 0.18 APIでの注意点:
  - `Query` の mutable borrow 範囲を拡大しすぎない。
  - `Commands` の side effect は frame 内順序依存を崩さない。

## 5. マイルストーン

## M1: 予算管理と worker 更新の分離

- 変更内容:
  - per-worker 更新処理を `process_worker_pathfinding`（仮称）へ抽出。
  - budget 判定を helper 化し、`pathfind_count` の更新点を明示。
- 変更ファイル:
  - `src/entities/damned_soul/movement/pathfinding.rs`
- 完了条件:
  - [ ] `pathfinding_system` のネストが縮小される。
  - [ ] pathfind 回数制限の挙動が維持される。
- 検証:
  - `cargo check`

## M2: fallback/cleanup の責務境界固定

- 変更内容:
  - rest area fallback と unreachable cleanup を別レイヤに切り出す。
  - idle と task の分岐ごとの差をコメントで明示。
- 変更ファイル:
  - `src/entities/damned_soul/movement/pathfinding.rs`
- 完了条件:
  - [ ] fallback/cleanup の入口条件が明文化される。
  - [ ] unassign 条件が現状維持。
- 検証:
  - `cargo check`

## M3: docs 追記と保守導線整備

- 変更内容:
  - pathfinding 実行境界の説明を必要に応じ docs へ反映。
  - 関連関数の意図コメントを追加（最小限）。
- 変更ファイル:
  - `src/entities/damned_soul/movement/pathfinding.rs`
  - `docs/architecture.md`（必要時）
- 完了条件:
  - [ ] 実行境界がコード/ドキュメントで一致する。
  - [ ] `cargo check` 成功。
- 検証:
  - `cargo check`

## 6. リスクと対策

| リスク | 影響 | 対策 |
| --- | --- | --- |
| 分割時に pathfind_count 更新位置がズレる | 探索回数過不足 | 更新箇所を helper 経由に一本化 |
| cleanup 条件の漏れ | 到達不能 Soul の停止/予約残留 | task/idle ケースを別シナリオで手動確認 |

## 7. 検証計画

- 必須:
  - `cargo check`
- 手動確認シナリオ:
  - 通常タスクで path 再利用→再探索が機能する。
  - rest area 予約時の fallback が機能する。
  - 到達不能時に cooldown と unassign が適用される。
- パフォーマンス確認（必要時）:
  - `cargo run -- --spawn-souls 500 --spawn-familiars 30 --perf-scenario`

## 8. ロールバック方針

- どの単位で戻せるか:
  - M1/M2/M3 のコミット単位で戻せる。
- 戻す時の手順:
  - 失敗した段階以降を revert。
  - `cargo check` と pathfinding 手動確認を再実施。

## 9. AI引継ぎメモ（最重要）

### 現在地

- 進捗: `0%`
- 完了済みマイルストーン: なし
- 未着手/進行中: `M1`,`M2`,`M3`

### 次のAIが最初にやること

1. `pathfinding_system` 内の条件分岐を段階ラベル化（task/idle/cooldown）。
2. 先に M1 だけ行って `cargo check`。
3. その後 fallback/cleanup の抽出へ進む。

### ブロッカー/注意点

- `unassign_task` 呼び出し条件を変えないこと。
- `PathCooldown` の付与/解除タイミングを維持すること。

### 参照必須ファイル

- `src/entities/damned_soul/movement/pathfinding.rs`
- `src/world/pathfinding.rs`
- `src/systems/soul_ai/helpers/work.rs`

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
