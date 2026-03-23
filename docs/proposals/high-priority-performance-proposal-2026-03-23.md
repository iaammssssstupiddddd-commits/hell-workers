# パフォーマンス改善（優先度高）— Soul 経路・移譲到達判定・Idle 命令委譲

## メタ情報

| 項目 | 値 |
| --- | --- |
| ドキュメントID | `high-priority-performance-proposal-2026-03-23` |
| ステータス | `Draft` |
| 作成日 | `2026-03-23` |
| 最終更新日 | `2026-03-23` |
| 作成者 | `AI Agent (Cursor)` |
| 関連計画 | `TBD`（実装着手時に `docs/plans/` に分割可。`docs/plans/` は gitignore のためコミット対象外の場合あり） |
| 関連Issue/PR | `N/A` |

## 1. 背景と問題

- **現状**: Soul 数・使い魔数の増加に伴い、Actor/Logic フェーズの CPU 負荷が上がる。コード走査により、次の 3 領域が **優先度高** の改善候補として整理された。
- **問題**:
  1. **Soul 経路探索**（`hw_soul_ai::pathfinding_system`）: `DamnedSoul` を毎フレーム・二相（タスク優先 / アイドル）で全走査する。`MAX_PATHFINDS_PER_FRAME`（8）で A\* 回数は制限されるが、**ループ反復・再利用判定・クールダウン処理**は Soul 数に線形。
  2. **タスク移譲の到達判定**（`hw_world::can_reach_target` ← `hw_familiar_ai` の `reachable_with_cache`）: キャッシュミス時は `find_path` / `find_path_to_adjacent` が走り、**委譲ティックでスパイク**しうる。
  3. **Idle 命令時の委譲**: `familiar_task_delegation_system` で `allow_task_delegation || is_idle_command` により、`FamiliarCommand::Idle` の使い魔は **0.5 秒タイマーに関係なく毎フレーム** `TaskManager::delegate_task` が実行されうる（Yard 共有タスクの取りこぼし防止がコメント上の意図）。
- **なぜ今やるか**: スケール時のボトルネックを構造的に抑え、プロファイラ導入前でも「どこに投資するか」の合意形成に使う。

## 2. 目的（Goals）

- Soul 数が大きいセッションでも **フレーム時間の中央値・尾部（p99）** を改善する余地を作る。
- タスク移譲における **到達判定コスト**を削減または段階化し、スパイクを抑える。
- Idle 命令時の委譲について、**仕様を維持しつつ**不要な毎フレームフル委譲を減らす選択肢を整理する。

## 3. 非目的（Non-Goals）

- GPU / レンダリングパイプラインの最適化（別提案とする）。
- A\* 本体の完全置換（ナビメッシュ等）は本提案の必須スコープ外（別途評価）。
- セーブデータ形式の変更。

## 4. 提案内容（概要）

- **一言要約**: 優先度高 3 件を **計測前提**で切り分け、それぞれに「低リスクの定数・アルゴリズム調整」から「設計変更」まで段階的な施策を並べる。
- **主要な変更点**:
  - **P1 経路**: クエリ走査の打ち切り・公平性（ローテーション）、必要なら `MAX_PATHFINDS_PER_FRAME` の再検討（CPU と待ち時間のトレードオフを文書化）。
  - **P2 到達判定**: より安い到達可能性（連結成分・深さ上限付き探索等）の **段階的フォールバック**、またはキャッシュ戦略の見直し（誤判定リスクを設計で管理）。
  - **P3 Idle 委譲**: 軽い間引き（別タイマー）、前フレームとの差分がない場合の短絡、仕様上不可ならコメントと計測結果で「現状維持」を明示。
- **期待される効果**: 高負荷シナリオでの **Logic/Actor コスト削減**、および **委譲ティックのスパイク低減**（実測で検証）。

## 5. 詳細設計

### 5.1 P1 — Soul 経路探索（`pathfinding_system`）

| 項目 | 内容 |
| --- | --- |
| 現状 | `crates/hw_soul_ai/src/soul_ai/pathfinding/system.rs`。`MAX_PATHFINDS_PER_FRAME`（`hw_core::constants::ai`）とタスクフェーズ用の予約（`RESERVED_IDLE_PATHFINDS_PER_FRAME = 2`）。 |
| 施策例 | (a) `pathfind_count` が予算に達したら **内側ループを早期 `break`** できるか検討（同一フレームで後続 Soul の軽い分岐までスキップできる場合）。 (b) **開始インデックスのローテーション**で、常に同じ Soul が毎フレーム「先頭」にならないようにする。 (c) 定数調整は `docs` と `constants` に根拠を残す。 |
| リスク | 早期 `break` は「クールダウン消費」等の公平性・挙動に影響しうるため、シナリオテスト必須。 |

#### 評価結果（コードレビュー後）

- **(a) 早期 `break` → 不要（現状実装済み）**
  - `process_worker_pathfinding` 内で `if *pathfind_count >= budget { return; }` が既に機能している。
  - 外側ループはクールダウンカウントダウン（`cooldown.remaining_frames -= 1`）のために全 Soul を継続走査する必要があり、A* のみ早期リターンする現実装は正しい。追加変更不要。

- **(b) ローテーション → 妥当・計画書作成済み**
  - 計画書: `docs/plans/soul-pathfinding-rotation.md`

- **(c) 定数コメント → ローテーション実装時に合わせて対応**

### 5.2 P2 — 移譲到達判定（`can_reach_target` / `reachable_with_cache`）

| 項目 | 内容 |
| --- | --- |
| 現状 | `crates/hw_world/src/pathfinding/mod.rs` の `can_reach_target`。`assignment_loop.rs` の `reachable_with_cache` と `ReachabilityFrameCache`（`task_delegation.rs`）。 |
| 施策例 | (a) **粗い到達判定**（例: 連結成分 ID）を先に試し、失敗時のみ A\* 系へ。 (b) `TASK_DELEGATION_TOP_K` や距離上限との組み合わせで **呼び出し回数上限**を再確認。 (c) `FamiliarDelegationPerfMetrics::reachable_with_cache_calls` を活用し、改善前後を比較。 |
| リスク | 粗い判定は **誤った割り当て／取りこぼし**につながる。ゲーム不変条件（`docs/invariants.md`）との整合を確認すること。 |

#### 評価結果（コードレビュー後）→ **保留（現状維持）**

- `reachable_with_cache` はすでに `(worker_grid, target_grid)` をキーとするキャッシュを実装済み。WorldMap 変更時即時クリア・60 フレーム毎安全クリアも機能している。
- `TASK_DELEGATION_TOP_K = 24` および `MAX_ASSIGNMENT_DIST_SQ`（60 タイル相当）により候補数・距離のフィルタも実装済み。
- **(a) 連結成分による粗い判定**は効果が見込めるが、WorldMap 更新との同期・データ構造追加・誤判定リスクが高く、**プロファイラまたは `FamiliarDelegationPerfMetrics` でキャッシュミス頻度が支配的と確認されるまで着手しない**。
- 着手判断: `reachable_with_cache_calls` が高負荷シナリオで継続的に高止まりしているかを計測してから再評価。

### 5.3 P3 — Idle 命令時の委譲頻度（`allow_task_delegation || is_idle_command`）

| 項目 | 内容 |
| --- | --- |
| 現状 | `crates/hw_familiar_ai/src/familiar_ai/decide/task_delegation.rs`（コメント: Yard 共有タスク）。 |
| 施策例 | (a) Idle 専用の **軽い間隔**（別 `Timer` Resource）で `delegate_task` のフル実行を間引く。 (b) **候補集合のハッシュ／件数が前フレームと同一**ならスキップ（WorldMap 変更時は無効化）。 (c) 仕様上毎フレーム必須なら、提案書に **「変更しない」** と理由を明記。 |
| リスク | Yard / 共有タスクの **検知遅延**、取りこぼし。 |

#### 評価結果（コードレビュー後）→ **不要（仕様上変更不可）**

コードコメントに明記されているとおり、`FamiliarCommand::Idle` の使い魔が毎フレームタスク委譲を行うのは **Yard 共有タスクを取りこぼさないための仕様要件**である。

```rust
// Yard 共有タスクは TaskArea 非依存で拾える要件のため、
// Idle command でも委譲処理自体は実行する。
allow_task_delegation: allow_task_delegation || is_idle_command,
```

- (a) 間引きタイマーを導入すると Yard タスクの検知遅延が発生し、ゲームプレイへの影響が大きい。
- (b) 候補ハッシュによるスキップは WorldMap 更新以外の変化（新タスクのスポーン等）を見逃すリスクがある。
- **(c) を採用: 現状維持。変更しない。**

もし将来 Idle 使い魔が多数存在する場面でボトルネックが計測された場合は、Yard タスクの変更通知イベント化（差分駆動）を別提案として検討すること。

### 5.4 変更対象（想定）

- `crates/hw_soul_ai/src/soul_ai/pathfinding/system.rs`
- `crates/hw_core/src/constants/ai.rs`（定数変更時）
- `crates/hw_world/src/pathfinding/mod.rs`（到達判定戦略）
- `crates/hw_familiar_ai/src/familiar_ai/decide/task_management/delegation/assignment_loop.rs`
- `crates/hw_familiar_ai/src/familiar_ai/decide/task_delegation.rs`
- `docs/familiar_ai.md` / `docs/architecture.md`（挙動変更時）

### 5.5 データ/コンポーネント/API 変更

- **追加**: ローテーション用の `Resource`、Idle 用タイマー、連結成分キャッシュ等は **施策確定後**に記載。
- **変更**: 定数・関数シグネチャは施策ごとに最小限。
- **削除**: なし（本 Draft 時点）。

## 6. 代替案と比較

| 案 | 採否 | 理由 |
| --- | --- | --- |
| 計測なしで定数だけ変更 | 保留 | トレードオフ（待ち時間 vs CPU）がプレイに直結するため、可能ならプロファイラまたはメトリクスで裏取り。 |
| 経路探索を完全イベント駆動のみに | 不採用（単独） | アイドル中の継続移動・再探索が必要で、イベントのみでは不足。 |
| 到達判定を距離ヒューリスティックのみに置換 | 保留 | 高速だが誤判定リスクが高い。段階的フォールバックが現実的。 |

## 7. 影響範囲

- **ゲーム挙動**: Soul の出発タイミング、タスク割り当ての公平性、Yard 周りの反応性。
- **パフォーマンス**: Logic/Actor の ms 削減が主目的。
- **UI/UX**: 間接的（カクつき低減）。
- **セーブ互換**: データ構造は原則変更しない想定。
- **既存ドキュメント更新**: 定数・挙動変更時は `docs/familiar_ai.md` 等を更新。

## 8. リスクと対策

| リスク | 影響 | 対策 |
| --- | --- | --- |
| 経路の公平性・クールダウン処理の退行 | 中 | `cargo check` に加え、Soul 多数・建設中シナリオの手動確認。 |
| 到達判定の誤り | 高 | 不変条件確認、ログ、`reachable_with_cache_calls` の比較。 |
| Yard 共有タスクの取りこぼし | 高 | Idle 委譲変更時は該当シナリオを明示的にテスト。 |

## 9. 検証計画

- `CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check`
- 手動確認: Soul 多数スポーン、タスク割り当て、Yard・Idle 命令使い魔、建設中の委譲。
- 計測: Linux ネイティブで高負荷シナリオ（例: `--spawn-souls`）、外部プロファイラまたは `FamiliarDelegationPerfMetrics` のダンプ／可視化（`docs/proposals/archive/scaling_performance_bottlenecks.md` の優先アクションと整合）。

## 10. ロールアウト/ロールバック

- **導入**: P1 → P2 → P3 の順で小さな PR に分割することを推奨（依存がなければ並行も可）。
- **ロールバック**: 定数・分岐単位で `git revert` 可能にする。

## 11. 未解決事項（Open Questions）

- [ ] プロファイラで **実際のホットスポット順位**は P1 / P2 / P3 のどれか（着手順の確定）。
- [ ] Idle 委譲を毎フレームにしている箇所が、計測上 **支配的か**。
- [ ] 連結成分等の粗い到達判定を **WorldMap 更新とどう同期**するか。

## 12. AI引継ぎメモ（最重要）

### 現在地

- 進捗: `0%`（提案書のみ）
- 直近で完了したこと: 優先度高 3 領域の走査・本ドキュメント作成
- 現在のブランチ/前提: `master` 相当、`cargo check` 緑を維持すること

### 次のAIが最初にやること

1. `docs/invariants.md` と `docs/familiar_ai.md` §7 を読み、変更可能範囲を確認する。
2. 可能ならプロファイラまたは `FamiliarDelegationPerfMetrics` で **ベースライン**を取る。
3. P1（経路）から小さな変更＋検証、または Open Questions の結果に応じて順序変更。

### ブロッカー/注意点

- Bevy 0.18 API を推測で変更しない（プロジェクトルール）。
- `docs/plans/` は gitignore のため、永続メモは `docs/` 側へ反映するか、ユーザーが計画をコミットする運用を確認。

### 参照必須ファイル

- `crates/hw_soul_ai/src/soul_ai/pathfinding/system.rs`
- `crates/hw_core/src/constants/ai.rs`
- `crates/hw_world/src/pathfinding/mod.rs`
- `crates/hw_familiar_ai/src/familiar_ai/decide/task_delegation.rs`
- `crates/hw_familiar_ai/src/familiar_ai/decide/task_management/delegation/assignment_loop.rs`
- `docs/familiar_ai.md`
- `docs/invariants.md`

### 完了条件（Definition of Done）

- [x] 提案内容がレビュー可能な粒度で記述されている
- [x] リスク・影響範囲・検証計画が埋まっている
- [ ] 実装へ進む場合の `docs/plans/...` が明記されている（実装 PR 時に追加で可）

## 13. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| `2026-03-23` | `AI Agent (Cursor)` | 初版作成（優先度高 3 領域の提案書） |
| `2026-03-23` | `AI Agent (Copilot)` | コードレビューにより各提案を評価。P1-b → 計画書 `docs/plans/soul-pathfinding-rotation.md` 作成。P1-a・P2・P3 に「不要/保留」理由を追記。 |
