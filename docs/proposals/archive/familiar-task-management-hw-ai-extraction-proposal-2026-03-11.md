# Familiar Task Management `hw_ai` 抽出提案

Familiar の task management を `hw_ai` へ移し、root を app shell と construction bridge に寄せるための提案。関連計画の M1-M5 として実装済み。

## メタ情報

| 項目 | 値 |
| --- | --- |
| ドキュメントID | `familiar-task-management-hw-ai-extraction-proposal-2026-03-11` |
| ステータス | `Archived` |
| 作成日 | `2026-03-11` |
| 最終更新日 | `2026-03-12（archive化）` |
| 作成者 | `Codex` |
| 関連計画 | `docs/plans/archive/familiar-task-management-hw-ai-extraction-plan-2026-03-11.md` |
| 関連Issue/PR | `N/A` |

## 1. 背景と問題

- `src/systems/familiar_ai/decide/task_management/` には、候補収集、優先度評価、搬送元選定、予約影反映、`AssignedTask` 構築など、使い魔 AI の中核ロジックがまとまっていた。
- 責務は `hw_ai` 向きであるにもかかわらず、root 側 `TaskAssignmentQueries` への依存を通じて `bevy_app` に残留していた。
- その結果、Familiar decide の責務境界が読みにくく、root crate が厚くなっていた。

## 2. 目的（Goals）

- Familiar の task management core を `hw_familiar_ai::familiar_ai::decide::task_management` に揃える。
- root 側には orchestration、plugin wiring、construction site bridge だけを残す。
- `TaskAssignmentQueries` 依存のうち、真に root-only な部分を `ConstructionSiteAccess` に隔離する。

## 3. 非目的（Non-Goals）

- `task_execution` 全体の同時移設
- `unassign_task` の crate 化
- `FloorConstructionSite` / `WallConstructionSite` の今回中の移設
- UI / visual 系の同時整理

## 4. 提案内容（概要）

- 一言要約:
  `src/systems/familiar_ai/decide/task_management/` を `hw_ai` へ集約し、root 側を thin adapter に縮小する。
- 実装結果:
  - `crates/hw_ai/src/familiar_ai/decide/task_management/` に query / task finder / validator / task assigner / builders / policy / delegation を集約した。
  - `FamiliarTaskAssignmentQueries` は `hw_ai` 所有の独立 query になり、root 側の重複定義を撤去した。
  - root 側は `task_delegation.rs` / `familiar_processor.rs` / `ConstructionSiteAccess` bridge を保持する構成へ整理した。

## 5. 詳細設計

### 5.1 仕様

- 候補収集、スコアリング、source selector、reservation shadow、assignment build の挙動は維持する。
- `familiar_task_delegation_system` は root 所有のままにし、`WorldMap` / pathfinding / concrete spatial grid と `ConstructionSiteAccess` を束ねて `hw_ai` core を呼ぶ。
- `FloorConstructionSite` / `WallConstructionSite` の Query は `ConstructionSiteAccess` に隔離し、`ConstructionSitePositions` trait 経由で `hw_ai` に渡す。

### 5.2 変更対象

- `crates/hw_ai/src/familiar_ai/decide/task_management/`
- `src/systems/familiar_ai/decide/task_management/mod.rs`
- `src/systems/familiar_ai/decide/task_delegation.rs`
- `src/systems/familiar_ai/decide/familiar_processor.rs`
- `src/systems/soul_ai/execute/task_execution/context/access.rs`
- `src/systems/soul_ai/execute/task_execution/context/mod.rs`
- `docs/cargo_workspace.md`
- `docs/familiar_ai.md`
- `docs/tasks.md`

### 5.3 データ / API 変更

- 追加:
  - `FamiliarStorageAccess`
  - `ConstructionSiteAccess`
  - `ConstructionSitePositions` trait 経由の bridge
- 変更:
  - `FamiliarTaskAssignmentQueries` を `hw_ai` 側定義に統一
  - root `task_management/` を thin bridge に縮退
- 削除:
  - root 側の task management 実装本体

## 6. 代替案と比較

| 案 | 採否 | 理由 |
| --- | --- | --- |
| `task_management` を `hw_ai` に段階移設する | 採用 | 責務が最も素直で、root shell 方針と一致する。 |
| 現状のまま root に置く | 不採用 | crate 境界方針に反し、AI 実装が root に滞留する。 |
| `task_execution` 全体を先に移す | 不採用 | blocker が強く、今回のスコープを超える。 |

## 7. 影響範囲

- ゲーム挙動:
  - 原則変更なし。割り当て結果と予約挙動を維持する。
- パフォーマンス:
  - 実行時性能改善が主目的ではなく、compile 境界と保守性改善が中心。
- UI / UX:
  - 直接影響なし。
- 既存ドキュメント:
  - `docs/cargo_workspace.md`, `docs/familiar_ai.md`, `docs/tasks.md`, 関連 plan / proposal index を更新対象とした。

## 8. リスクと対策

| リスク | 影響 | 対策 |
| --- | --- | --- |
| construction site 依存が `hw_ai` へ漏れる | crate 境界が再び崩れる | `ConstructionSiteAccess` に隔離し、trait 経由で渡す |
| root と `hw_ai` に query 定義が二重化する | 型ズレと保守コスト増 | `FamiliarTaskAssignmentQueries` の定義元を `hw_ai` の 1 箇所に固定 |
| plugin 登録が二重化する | schedule 初期化時の不整合 | system 実装の所有 crate を唯一の登録元にする |

## 9. 検証計画

- `cargo check -p hw_ai`
- `cargo check --workspace`
- floor / wall / provisional wall の搬入シナリオを優先した runtime 確認

## 10. ロールアウト / ロールバック

- 導入手順:
  1. Familiar 向け query を construction 依存から切り離す
  2. `task_management` core を `hw_ai` に移す
  3. root を thin bridge / adapter に整理する
  4. docs と index を同期する
- ロールバック:
  - query 境界変更 → core 移設 → adapter 整理の逆順で戻す

## 11. 未解決事項（Open Questions）

- [x] `TaskAssignmentQueries` の最小分割単位をどう定義するか
  - `FamiliarTaskAssignmentQueries` を `hw_ai` 側の独立 query とし、Soul 側 full query と分離した。
- [ ] `FloorConstructionSite` / `WallConstructionSite` を `hw_jobs` 側へ寄せるべきか
  - 今回は見送り。`ConstructionSiteAccess` bridge を維持する。
- [x] `TaskArea` 依存は adapter で十分か
  - 現時点では root orchestration + `hw_ai` core の分担で十分。

## 12. AI 引継ぎメモ（最重要）

### 現在地

- 進捗: `100%`
- 直近で完了したこと:
  - task management 実装本体の `hw_ai` 集約
  - root 側 thin bridge / adapter 化
  - 関連 docs の同期
- 現在の前提:
  - 実装本体は `crates/hw_ai/src/familiar_ai/decide/task_management/` にある

### 次のAIが最初にやること

1. 追加の crate 境界変更を行う場合は `ConstructionSiteAccess` を残している理由を確認する。
2. Familiar task delegation の仕様変更時は `docs/familiar_ai.md` / `docs/tasks.md` / `docs/cargo_workspace.md` を同時更新する。

### ブロッカー / 注意点

- `ConstructionSiteAccess` は `FloorConstructionSite` / `WallConstructionSite` の root-only Query を抱えている。
- `task_execution` 側の全面移設は今回のスコープ外。
- `FamiliarTaskAssignmentQueries` の定義元は `hw_ai` に固定し、root に重複定義を戻さないこと。

### 完了条件（Definition of Done）

- [x] 提案内容がレビュー可能な粒度で記述されている
- [x] リスク・影響範囲・検証計画が埋まっている
- [x] 実装計画と最終 docs が同期している

## 13. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| `2026-03-11` | `Codex` | 初版作成 |
| `2026-03-12` | `Codex` | 実装完了に合わせてステータス・未解決事項・AI 引継ぎメモを更新 |
| `2026-03-12` | `Codex` | `docs/proposals/archive/` へ移動し、関連計画パスを archive に更新 |
