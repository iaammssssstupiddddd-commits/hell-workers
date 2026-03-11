# Familiar Task Management `hw_ai` 抽出 実装計画

Familiar の task management を `hw_ai` へ集約し、root 側を orchestration と construction bridge に縮退するための計画。M1-M5 は完了済み。

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `familiar-task-management-hw-ai-extraction-plan-2026-03-11` |
| ステータス | `Archived` |
| 作成日 | `2026-03-11` |
| 最終更新日 | `2026-03-12（archive化）` |
| 作成者 | `AI (Codex)` |
| 関連提案 | `docs/proposals/archive/familiar-task-management-hw-ai-extraction-proposal-2026-03-11.md` |
| 関連Issue/PR | `N/A` |
| 先行計画 | `docs/plans/archive/familiar-ai-root-thinning-plan-2026-03-09.md` |

## 1. 目的

- Familiar の task search / scoring / source selector / reservation shadow / assignment build を `hw_ai::familiar_ai::decide::task_management` に集約する。
- root 側には `familiar_task_delegation_system` の orchestration、`WorldMap` / pathfinding / concrete spatial grid 依存、construction site bridge だけを残す。
- `FamiliarTaskAssignmentQueries` を root の full query alias から切り離し、`hw_ai` 側所有の独立 query にする。

## 2. スコープ

### 対象

- `FamiliarStorageAccess` と `ConstructionSiteAccess` の責務分割
- `crates/hw_ai/src/familiar_ai/decide/task_management/` の新設と本体移設
- root `task_management` の thin bridge 化
- `task_delegation.rs` / `familiar_processor.rs` の adapter 整理
- `docs/cargo_workspace.md`, `docs/familiar_ai.md`, `docs/tasks.md`, plan / proposal の同期

### 非対象

- `familiar_task_delegation_system` 自体の `hw_ai` 移設
- `task_execution` 全体の同時移設
- `FloorConstructionSite` / `WallConstructionSite` 自体の crate 移設
- gameplay アルゴリズム変更

## 3. 最終境界

### `hw_ai` が所有するもの

- `familiar_ai::decide::task_management::context`
  - `FamiliarTaskAssignmentQueries`
  - `TaskAssignmentReadAccess`
  - `ReservationAccess`
- `task_finder` / `validator` / `task_assigner`
- `delegation::{TaskManager, assignment_loop, members}`
- `builders::{basic, haul, water}`
- `policy::{basic, floor, water}`
- `policy::haul::{blueprint, consolidation, demand, direct_collect, floor, lease_validation, mixer, provisional_wall, returns, source_selector, stockpile, wall, wheelbarrow}`

### root が所有するもの

- `src/systems/familiar_ai/decide/task_delegation.rs`
  - timer / perf metrics / pathfinding / `WorldMap` 依存を持つ orchestration
- `src/systems/familiar_ai/decide/familiar_processor.rs`
  - root query と `ConstructionSiteAccess` を `hw_ai` core に渡す adapter
- `src/systems/soul_ai/execute/task_execution/context/access.rs`
  - `ConstructionSiteAccess`
  - `ConstructionSitePositions` trait の実装
- `src/systems/familiar_ai/decide/task_management/mod.rs`
  - 互換パス維持用の thin bridge

### 補足

- `FloorConstructionSite` / `WallConstructionSite` は依然として root-only 型であり、そのため construction site Query は `ConstructionSiteAccess` に隔離している。
- `familiar_task_delegation_system` は concrete spatial grid と pathfinding を扱うため root 残留とした。

## 4. マイルストーン結果

### M1: Familiar 向け access を construction 依存から切り離す ✅

- `FamiliarStorageAccess` を導入し、Familiar 向け共通 query から construction site 依存を除去した。
- `FamiliarTaskAssignmentQueries` を `hw_ai` 所有の独立型に切り出し、root 側の重複定義を撤去した。
- `ConstructionSiteAccess` を別 `SystemParam` とし、trait 経由で `hw_ai` core に渡す形へ整理した。

### M2: `task_management` core モジュールを `hw_ai` に新設する ✅

- `IncomingDeliverySnapshot`、`ReservationShadow`、候補収集、validator、source selector など read-heavy core を `crates/hw_ai` へ移設した。
- root 側の対応モジュールは re-export / thin bridge に縮退した。

### M3: non-construction assignment build / policy を `hw_ai` へ移す ✅

- `TaskManager`、assignment loop、builders、basic/haul/water policy の本体を `hw_ai` に移設した。
- construction 系 policy も `ConstructionSitePositions` trait 越しに `hw_ai` 側へ統合し、root 側とのロジック重複を解消した。

### M4: root bridge と task delegation adapter を整理する ✅

- root `task_management/` は `mod.rs` だけを残す thin bridge へ縮退した。
- `task_delegation.rs` / `familiar_processor.rs` は root orchestration と adapter に専念する構成へ整理した。
- `ConstructionSiteAccess` の trait 実装だけを root 側 construction bridge として残した。

### M5: ドキュメント同期と回帰確認 ✅

- `docs/cargo_workspace.md`, `docs/familiar_ai.md`, `docs/tasks.md` を actual boundary に合わせて更新した。
- plan / proposal のメタ情報と引継ぎメモを完了状態へ更新した。
- `python scripts/update_docs_index.py` で `docs/plans/README.md` と `docs/proposals/README.md` を同期した。

## 5. 検証

- `cargo check -p hw_ai`
- `cargo check --workspace`
- `python scripts/update_docs_index.py`

## 6. 残リスクと次段の前提

- `ConstructionSiteAccess` を不要にするには `FloorConstructionSite` / `WallConstructionSite` の crate 移設が前提になる。
- runtime 挙動の追加確認が必要な場合は、floor / wall / provisional wall 搬入を優先して見る。
- 今後 `task_management` を変更する場合も、`FamiliarTaskAssignmentQueries` の定義元は `hw_ai` に固定する。

## 7. AI 引継ぎメモ

### 現在地

- 進捗: `100%`
- 完了済みマイルストーン: **M1-M5**
- 未着手/進行中: なし

### 次のAIが最初にやること

1. crate 境界をさらに動かす場合は `ConstructionSiteAccess` を残している理由を先に確認する。
2. Familiar task delegation の仕様変更時は `docs/familiar_ai.md` / `docs/tasks.md` / `docs/cargo_workspace.md` を同時更新する。
3. runtime 回帰が疑われる場合は floor / wall / provisional wall の搬入シナリオを優先確認する。

### Definition of Done

- [x] `hw_ai` に task management core が集約されている
- [x] root 側が orchestration と construction bridge に縮退している
- [x] 関連 docs と plan / proposal / index が同期している
- [x] `cargo check --workspace` が通っている

## 8. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| `2026-03-11` | `AI (Codex)` | 初版作成 |
| `2026-03-11` | `AI (Copilot)` | M1 実装完了反映 |
| `2026-03-12` | `AI (Codex)` | M2-M5 完了反映、最終境界と docs 同期内容を追記 |
| `2026-03-12` | `AI (Codex)` | `docs/plans/archive/` へ移動し、関連提案パスを archive に更新 |
