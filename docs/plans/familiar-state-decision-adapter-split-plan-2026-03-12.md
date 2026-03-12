# Familiar State Decision Adapter Split Plan

root `state_decision.rs` に残る Decide orchestration を、`hw_ai` の pure outcome core と root の message adapter に分離するための計画。

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `familiar-state-decision-adapter-split-plan-2026-03-12` |
| ステータス | `Draft` |
| 作成日 | `2026-03-12` |
| 最終更新日 | `2026-03-12` |
| 作成者 | `AI (Codex)` |
| 関連提案 | `N/A` |
| 関連Issue/PR | `N/A` |

> コードサーベイ基準日: `2026-03-12`

## 1. 目的

- 解決したい課題: [`state_decision.rs`](/home/satotakumi/projects/hell-workers/src/systems/familiar_ai/decide/state_decision.rs) が `SpatialGrid` / full-fat query / `MessageWriter` を同時に抱え、使い魔 1 体分の状態判断と request 発行が 1 ファイルに混在している。
- 到達したい状態: `hw_ai` 側に「使い魔 1 体分の状態判断結果を返す core」を置き、root 側 [`state_decision.rs`](/home/satotakumi/projects/hell-workers/src/systems/familiar_ai/decide/state_decision.rs) は query/resource 取得と message 変換だけを担う thin adapter に縮退する。
- 成功指標:
  - `crates/hw_ai/src/familiar_ai/decide/state_decision.rs` が新設され、状態判断本体を所有する
  - root `state_decision.rs` から pure branching と inline message 構築の大半が除去される
  - `FamiliarDecideOutput` への書き込みが root adapter に限定される
  - `cargo check -p hw_ai` と `cargo check --workspace` が成功する

## 2. スコープ

### 対象（In Scope）

- `state_decision.rs` の core / adapter 分離
- `hw_ai::familiar_ai::decide` への `StateDecisionContext` / `StateDecisionOutcome` 追加
- root `helpers/query_types.rs` と `hw_ai::decide::query_types.rs` の境界整理
- `FamiliarDecideOutput` を使う request/event 変換の集約
- `docs/familiar_ai.md` / `docs/cargo_workspace.md` / `src/systems/familiar_ai/README.md` の境界説明更新

### 非対象（Out of Scope）

- `task_delegation.rs` / `familiar_processor.rs` の crate 移設
- `task_management` / pathfinding / `WorldMapRead` 周りの再設計
- `encouragement.rs` / `auto_gather_for_blueprint.rs` の同時整理
- gameplay アルゴリズム変更
- request/message 型の所有 crate 変更

## 3. 現状とギャップ

- 現状:
  - root [`state_decision.rs`](/home/satotakumi/projects/hell-workers/src/systems/familiar_ai/decide/state_decision.rs) は `FamiliarAiStateDecisionParams` で `SpatialGrid`、familiar/soul query、休憩関連 query、`FamiliarDecideOutput` をまとめて受け取る。
  - 同ファイル内で `Idle` command 時のリクルート、`Scouting` 継続、分隊管理、状態遷移、`SquadManagementRequest` / `FamiliarStateRequest` / `FamiliarAiStateChangedEvent` / `FamiliarIdleVisualRequest` 発行まで処理している。
  - `recruitment` / `scouting` / `supervising` / `state_handlers` / `finalize_state_transitions` はすでに `hw_ai` 側へ寄っているが、最後の orchestration 層だけが root に残っている。
- 問題:
  - request 発行の都合で pure decision と app shell が分離されておらず、`hw_ai` 境界が `state_decision` で止まっている。
  - `transmute_lens_filtered` と message 変換が同じ関数に混在し、 borrow 競合と責務の見通しが悪い。
  - 既存の `familiar-ai-root-thinning` 計画では `state_decision` は「root 縮退」候補だったが、outcome 変換層の設計が未整理のまま残っている。
- 本計画で埋めるギャップ:
  - state 判断を `hw_ai` 側の per-familiar core に寄せる
  - root 側には `SystemParam` と `MessageWriter` を扱う adapter のみ残す
  - 次段で `task_delegation` などを検討するときの境界を明確にする

## 4. 実装方針（高レベル）

- 方針:
  - `hw_ai` に「1 familiar を入力すると pure outcome を返す API」を追加する。
  - root `state_decision.rs` は `for familiar in q_familiars.iter_mut()` のループを維持しつつ、各分岐の中身を `hw_ai` の関数へ委譲する。
  - request 発行は outcome を root 側 helper が変換する構成に統一する。
- 設計上の前提:
  - リクルート判定は引き続き `SpatialGridOps` 越しに行い、`hw_ai` 内で concrete `SpatialGrid` を持たない。
  - `FamiliarDecideOutput` は root 所有のままとし、`hw_ai` には渡さない。
  - `determine_transition_reason` は root adapter で最終状態が確定した後に呼ぶ。
- Bevy 0.18 APIでの注意点:
  - `QueryLens::transmute_lens_filtered` の借用寿命を伸ばしすぎず、1 分岐ごとに narrow query を作ってすぐ消費する。
  - `SystemParam` の所有場所を分けても二重登録しない。system 登録は引き続き root `FamiliarAiPlugin` が担当する。
  - `MessageWriter` の書き込み順を変えると挙動差分になり得るため、既存の `state_request -> state_changed_event` などの順序を維持する。

## 5. マイルストーン

## M1: `hw_ai` に state decision core を追加する

- 変更内容:
  - `crates/hw_ai/src/familiar_ai/decide/state_decision.rs` を新設する。
  - `StateDecisionContext`、`IdleCommandDecisionOutcome`、`AssignedCommandDecisionOutcome`、または同等の outcome 型を定義する。
  - `SquadManagementRequest` 相当の操作は message ではなく pure enum/list で返す。
  - 既存 `recruitment` / `state_handlers` / `helpers` を組み合わせて 1 familiar 分の判断ロジックを `hw_ai` 側へ移す。
- 変更ファイル:
  - `crates/hw_ai/src/familiar_ai/decide/state_decision.rs`
  - `crates/hw_ai/src/familiar_ai/decide/mod.rs`
  - `crates/hw_ai/src/familiar_ai/decide/query_types.rs`
- 完了条件:
  - [ ] `hw_ai` 側に per-familiar state decision API が存在する
  - [ ] `hw_ai` 側が `MessageWriter` に依存しない
  - [ ] `SpatialGridOps` ベースで recruitment 分岐を実行できる
- 検証:
  - `cargo check -p hw_ai`

## M2: root `state_decision.rs` を thin adapter に縮退する

- 変更内容:
  - root [`state_decision.rs`](/home/satotakumi/projects/hell-workers/src/systems/familiar_ai/decide/state_decision.rs) から branching 本体を削り、`hw_ai` の outcome を `FamiliarDecideOutput` へ変換する helper 群に置き換える。
  - `write_add_member_request` / `write_release_requests` と state changed event 生成を adapter 専用 helper に整理する。
  - 必要なら `helpers/query_types.rs` に state decision 専用の narrow query alias を追加し、`transmute_lens_filtered` の呼び先を明示する。
- 変更ファイル:
  - `src/systems/familiar_ai/decide/state_decision.rs`
  - `src/systems/familiar_ai/decide/mod.rs`
  - `src/systems/familiar_ai/helpers/query_types.rs`
- 完了条件:
  - [ ] root `state_decision.rs` が query/resource 取得と outcome 変換に集中している
  - [ ] `FamiliarDecideOutput` への write は root adapter からのみ行われる
  - [ ] 既存の request 発行順序と state transition reason 計算が維持される
- 検証:
  - `cargo check --workspace`

## M3: 境界ドキュメントを同期する

- 変更内容:
  - `docs/familiar_ai.md` に `state_decision` が root adapter であり、判断本体は `hw_ai::familiar_ai::decide::state_decision` にあることを追記する。
  - `src/systems/familiar_ai/README.md` と `docs/cargo_workspace.md` の Familiar AI 境界説明を actual boundary に合わせて更新する。
  - `python scripts/update_docs_index.py` を実行し、plan index を同期する。
- 変更ファイル:
  - `docs/familiar_ai.md`
  - `src/systems/familiar_ai/README.md`
  - `docs/cargo_workspace.md`
  - `docs/plans/README.md`
- 完了条件:
  - [ ] `state_decision` の所有境界が docs 上で一貫している
  - [ ] 新規計画書が `docs/plans/README.md` に反映されている
  - [ ] 実装後の root-only 理由が文章で説明できる
- 検証:
  - `python scripts/update_docs_index.py`
  - `cargo check --workspace`

## 6. リスクと対策

| リスク | 影響 | 対策 |
| --- | --- | --- |
| outcome 型が細かすぎて root adapter が逆に複雑化する | 責務分離しても見通しが悪い | message 1件単位ではなく「state change」「squad ops」「idle visual」のまとまりで返す |
| `transmute_lens_filtered` の借用範囲が変わり Query 競合を起こす | `cargo check` 失敗、実装停滞 | narrow query 構築は helper 内に閉じず、root system 内で短寿命に保つ |
| request 発行順や state reason 計算順が変わる | runtime 挙動差分 | 既存の `old_state` / `next_state` / writer 呼び出し順を golden path として維持する |
| `state_decision` の core 化で `task_delegation` との責務境界が曖昧になる | 次段リファクタが再び混線する | 本計画では `task_delegation` を明示的に非対象とし、`state_decision` のみ切り離す |

## 7. 検証計画

- 必須:
  - `cargo check -p hw_ai`
  - `cargo check --workspace`
- 手動確認シナリオ:
  - `Idle` command の Familiar が近傍 Soul を即時リクルートできる
  - 遠方 Soul に対して `Scouting` へ遷移し、到達後に AddMember request を発行できる
  - 分隊満員時に `Supervising` へ遷移し、空きが出たとき `SearchingTask` へ戻る
  - 疲労解放で `ReleaseMember { reason: Fatigued }` が従来どおり発行される
- パフォーマンス確認（必要時）:
  - `familiar_ai_state_system` の 1 フレームあたり処理量に明確な増加がないこと

## 8. ロールバック方針

- どの単位で戻せるか:
  - M1: `hw_ai` への新規 file 追加のみを戻す
  - M2: root adapter 化だけを戻す
  - M3: docs 同期だけを戻す
- 戻す時の手順:
  - マイルストーンごとにコミットを分ける
  - 問題が出た場合は最後に成功していた milestone まで戻し、message 変換順と query 借用の差分を再確認する

## 9. AI引継ぎメモ（最重要）

### 現在地

- 進捗: `0%`
- 完了済みマイルストーン: なし
- 未着手/進行中:
  - M1-M3 全て未着手

### 次のAIが最初にやること

1. [`src/systems/familiar_ai/decide/state_decision.rs`](/home/satotakumi/projects/hell-workers/src/systems/familiar_ai/decide/state_decision.rs) を読み、Idle command 分岐と non-Idle 分岐の outcome 形を先に固定する。
2. `crates/hw_ai/src/familiar_ai/decide/recruitment.rs` と `helpers.rs` を見て、既存 pure helper で賄える部分と新規 core に残す分岐を切り分ける。
3. root adapter 側で保持すべき writer 順序と `determine_transition_reason` 呼び出し位置をメモしてから編集に入る。

### ブロッカー/注意点

- `state_decision` は `Idle` command とそれ以外で分岐構造がかなり異なるため、1 つの巨大 outcome 型に詰め込みすぎないこと。
- `task_delegation` を巻き込むとスコープが膨らむ。今回の計画では触れない。
- docs 上は「root adapter が request message を発行し、`hw_ai` は pure outcome を返す」という原則に合わせる。

### 参照必須ファイル

- `docs/familiar_ai.md`
- `docs/cargo_workspace.md`
- `src/systems/familiar_ai/README.md`
- `src/systems/familiar_ai/decide/state_decision.rs`
- `src/systems/familiar_ai/decide/mod.rs`
- `crates/hw_ai/src/familiar_ai/decide/recruitment.rs`
- `crates/hw_ai/src/familiar_ai/decide/helpers.rs`

### 最終確認ログ

- 最終 `cargo check`: `2026-03-12 / not run (planning only)`
- 未解決エラー:
  - なし（計画作成のみ）

### Definition of Done

- [ ] M1-M3 が完了している
- [ ] `state_decision` の core と adapter の責務がコードと docs の両方で一致している
- [ ] `cargo check -p hw_ai` と `cargo check --workspace` が成功している

## 10. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| `2026-03-12` | `AI (Codex)` | 初版作成 |
