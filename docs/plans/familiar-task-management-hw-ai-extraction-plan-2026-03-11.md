# Familiar Task Management `hw_ai` 抽出 実装計画

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `familiar-task-management-hw-ai-extraction-plan-2026-03-11` |
| ステータス | `In Progress` |
| 作成日 | `2026-03-11` |
| 最終更新日 | `2026-03-11（M1完了）` |
| 作成者 | `AI (Codex)` |
| 関連提案 | `docs/proposals/familiar-task-management-hw-ai-extraction-proposal-2026-03-11.md` |
| 関連Issue/PR | `N/A` |
| 先行計画 | `docs/plans/archive/familiar-ai-root-thinning-plan-2026-03-09.md` |

> **最終コードサーベイ（2026-03-11）**: コードベースを実際に確認した結果を本計画に反映済み。ファイル数・型依存・移設可否の判定は実ファイルに基づく。

## 1. 目的

- 解決したい課題: `src/systems/familiar_ai/decide/task_management/` の中核ロジックが root crate に残り、`hw_ai` と app shell の責務境界が `TaskAssignmentQueries` 依存で曖昧になっている。
- 到達したい状態: 候補収集、スコアリング、source selector、reservation shadow、assignment build の主処理を `hw_ai::familiar_ai::decide::task_management` へ寄せ、root 側は `familiar_task_delegation_system` と construction bridge と adapter に縮退する。
- 成功指標:
  - Familiar 向けの非 construction `task_management` 実装本体が `crates/hw_ai` に置かれている
  - `FloorConstructionSite` / `WallConstructionSite` 依存が Familiar 向け共通 Query から切り離されている
  - `cargo check -p hw_ai` と `cargo check --workspace` が成功する

## 2. スコープ

### 対象（In Scope）

- `TaskAssignmentQueries` / `StorageAccess` の Familiar 向け責務分割
- `crates/hw_ai/src/familiar_ai/decide/task_management/` の新設
- `task_finder` / `validator` / `delegation` / `task_assigner` / `builders` / `policy` のうち、shared 型だけで閉じる部分の移設
- `policy/floor.rs` と `policy/haul/{floor,wall,provisional_wall}.rs` のための root construction adapter 整備
- root `task_delegation.rs` / `familiar_processor.rs` の import 切替と thin adapter 化
- `docs/cargo_workspace.md`, `docs/familiar_ai.md`, `docs/tasks.md`, `crates/hw_ai/README.md` の同期

### 非対象（Out of Scope）

- `familiar_task_delegation_system` 自体の `hw_ai` 移設
- `task_execution` 全体の同時移設
- `unassign_task` の crate 化
- `FloorConstructionSite` / `WallConstructionSite` 自体の crate 移設
- gameplay アルゴリズムの変更や WorkType 追加

## 3. 現状とギャップ

### 3.1 実ファイル構成（コードサーベイ 2026-03-11）

`src/systems/familiar_ai/decide/task_management/` の全 35 ファイル:

| サブモジュール | ファイル | 移設可否 |
| --- | --- | --- |
| `mod.rs` | `IncomingDeliverySnapshot`、`FamiliarTaskAssignmentQueries` alias | ✅ 型定義を hw_ai へ |
| `task_assigner.rs` | `ReservationShadow`、`CachedSourceItem`、`SourceSelectorFrameCache`、`AssignTaskContext` | ✅ |
| `task_finder/` | `mod.rs`（`DelegationCandidate`、`ScoredDelegationCandidate`）、`score.rs`、`filter.rs` | ✅ |
| `validator/` | `mod.rs`、`resolver.rs`、`finder.rs`、`wheelbarrow.rs`、`reservation.rs` | ✅ |
| `delegation/` | `mod.rs`（`TaskManager`）、`assignment_loop.rs`、`members.rs` | ✅ |
| `builders/` | `mod.rs`、`basic.rs`、`haul.rs`、`water.rs` | ✅（`haul.rs` はコメント参照のみ、実型依存なし） |
| `policy/mod.rs`、`basic.rs`、`water.rs` | WorkType dispatcher 他 | ✅ |
| `policy/haul/` | `mod.rs`、`demand.rs`、`direct_collect.rs`、`source_selector.rs`、`stockpile.rs`、`blueprint.rs`、`mixer.rs`、`returns.rs`、`consolidation.rs`、`lease_validation.rs`、`wheelbarrow.rs` | ✅ |
| `policy/floor.rs` | `FloorConstructionSite` 依存（`queries.storage.floor_sites`） | ❌ root bridge 残留 |
| `policy/haul/floor.rs`、`wall.rs`、`provisional_wall.rs` | `floor_sites`/`wall_sites` フィールドへの直接アクセス | ❌ root bridge 残留 |

> **重要**: `builders/haul.rs:292` の `FloorConstructionSite` はコメント内の説明文のみ。実際の型インポートはなく、**移設可能**。

### 3.2 Construction site 依存の実際の所在

```
access.rs (StorageAccess)
  └── floor_sites: Query<(Transform, FloorConstructionSite, Option<TaskWorkers>)>   ← ここだけ
  └── wall_sites:  Query<(Transform, WallConstructionSite,  Option<TaskWorkers>)>   ← ここだけ
        ↓ フィールドアクセス経由
  policy/floor.rs          → queries.storage.floor_sites
  policy/haul/floor.rs     → queries.storage.floor_sites
  policy/haul/wall.rs      → queries.storage.wall_sites
  policy/haul/provisional_wall.rs → queries.storage.{floor_sites,wall_sites}
```

`TaskAssignmentQueries` は `storage: StorageAccess` を持つため、Familiar 側が `FamiliarTaskAssignmentQueries` alias でこの全体を取り込んでいる。

### 3.3 問題の整理

- `StorageAccess` に construction site Query が混在しているため、construction 無関係の 28 ファイルまで root 残留扱いになっている。
- `FamiliarTaskAssignmentQueries` が `TaskAssignmentQueries` の全 alias であり、construction field を含む `StorageAccess` を Familiar 側に強制している。
- docs 上は `hw_ai` が Familiar decide コアを持つ方針だが、`task_management` だけ例外として大きく残っている。

### 3.4 本計画で埋めるギャップ

- `StorageAccess` を「Familiar core で使う部分」と「construction 専用部分」に分割し、Familiar 向け `FamiliarStorageAccess` を新設する。
- `task_management` を「root construction bridge（4 ファイル）」と「AI core（31 ファイル）」に分け、順次 `hw_ai` へ移設する。
- root に残る責務を `WorldMap` / pathfinding / concrete spatial grid / construction bridge に限定する。

## 4. 実装方針（高レベル）

- 方針:
  - 先に Query 境界を整理し、その後 `task_management` を read-heavy な層から順に `hw_ai` へ移す。
  - `familiar_task_delegation_system` は root に残し、`TaskManager` とその配下モジュールを `hw_ai` から呼ぶ構成にする。
  - construction site 依存は `task_management` 本体に混ぜず、root 側の bridge / adapter として分離する。
- 設計上の前提:
  - `TaskArea`, `AssignedTask`, `TransportRequest`, `ResourceItem`, `Blueprint`, `MudMixerStorage`, `Stockpile` などは `hw_*` crate 側の実体を参照できる。
  - 真の root-only blocker は `FloorConstructionSite`, `WallConstructionSite`, `WorldMapRead`, `PathfindingContext`, concrete spatial grids である。
  - system 登録責務は引き続き root の `familiar_task_delegation_system` が持ち、移設後も二重登録しない。
- 期待される性能影響:
  - ランタイム挙動は原則不変。候補収集順、スコア式、予約反映経路は維持する。
  - 実行時性能の改善は主目的ではなく、compile 境界と保守性改善が中心。
  - Query 分割により Familiar 側の読み取り責務が明確になり、将来の system 並列性改善余地は増える。
- Bevy 0.18 API での注意点:
  - `SystemParam` は所有型の crate 境界に敏感なので、`TaskAssignmentQueries` の一括 alias を維持したままでは移設しない。
  - root 側 thin shell は ordering 参照のために残してよいが、同じ system function を再登録しない。

## 5. マイルストーン

## 5. マイルストーン

## M1: Familiar 向け access を construction 依存から切り離す ✅

- 変更内容:
  - `StorageAccess` から `floor_sites` / `wall_sites` を除いた `FamiliarStorageAccess` を新設する（または `FamiliarStorageAccess` struct を `access.rs` 内に追加して `floor_sites`/`wall_sites` を除く）。
  - `FamiliarTaskAssignmentQueries` を `TaskAssignmentQueries` の alias から独立型（`SystemParam` struct）に変更し、`storage` フィールドに `FamiliarStorageAccess` を使う。
  - `TaskAssignmentQueries` / `TaskQueries` は Soul 実行用として `StorageAccess`（construction 含む）を引き続き保持し、Soul 側の挙動を変えない。
  - `task_management/mod.rs` の `FamiliarTaskAssignmentQueries` alias を削除し、独立定義へ切替える。

  > **注**: `MutStorageAccess` も同様に `FamiliarMutStorageAccess` を検討するが、Familiar 側で mutable storage が必要かを先に確認すること。

- 変更ファイル:
  - `src/systems/soul_ai/execute/task_execution/context/access.rs`（`FamiliarStorageAccess` + `ConstructionSiteAccess` 追加）
  - `src/systems/soul_ai/execute/task_execution/context/queries.rs`（`FamiliarTaskAssignmentQueries` 独立型追加、Deref/DerefMut/TaskReservationAccess 実装）
  - `src/systems/soul_ai/execute/task_execution/context/mod.rs`（re-export 更新）
  - `src/systems/familiar_ai/decide/task_management/mod.rs`（alias 削除、独立型 re-export に変更）
  - `src/systems/familiar_ai/decide/task_management/policy/haul/floor.rs`（`queries.storage.floor_sites` → `queries.construction_sites.floor_sites`）
  - `src/systems/familiar_ai/decide/task_management/policy/haul/wall.rs`（`queries.storage.wall_sites` → `queries.construction_sites.wall_sites`）
- 完了条件:
  - [x] `FamiliarStorageAccess` が `FloorConstructionSite` / `WallConstructionSite` を参照しない
  - [x] `FamiliarTaskAssignmentQueries` が独立 `SystemParam` struct である
  - [x] `policy/floor.rs`・`policy/haul/{floor,wall,provisional_wall}.rs` は既存の `TaskAssignmentQueries`（または `StorageAccess`）を引き続き参照できる
  - [x] `cargo check --workspace` が通る
- 検証:
  - `CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check --workspace` → **Finished（エラーなし、2026-03-11）**

  > **実装メモ**: `FamiliarTaskAssignmentQueries` に `construction_sites: ConstructionSiteAccess` フィールドを追加し、`policy/haul/floor.rs` と `policy/haul/wall.rs` のアクセスパスを `queries.construction_sites.*` に変更することで、`FamiliarStorageAccess` と構造体の全フィールド変更を最小限に抑えた。

## M2: `task_management` core モジュールを `hw_ai` に新設する

- 変更内容:
  - `crates/hw_ai/src/familiar_ai/decide/task_management/` を新設し、shared data type から移設を開始する。
  - **移設対象**（construction 非依存・M1 後に型が確定するもの）:
    - `mod.rs` → `IncomingDeliverySnapshot`（`FamiliarTaskAssignmentQueries` は M1 後に hw_ai へ再 export）
    - `task_assigner.rs` → `ReservationShadow`、`CachedSourceItem`、`SourceSelectorFrameCache`、`AssignTaskContext`
    - `task_finder/mod.rs` → `DelegationCandidate`、`ScoredDelegationCandidate`
    - `task_finder/score.rs`、`task_finder/filter.rs`
    - `validator/mod.rs`、`validator/resolver.rs`、`validator/finder.rs`、`validator/wheelbarrow.rs`、`validator/reservation.rs`（5 ファイル）
    - `policy/haul/demand.rs`、`direct_collect.rs`、`source_selector.rs`（read-heavy core）
  - `crates/hw_ai/src/familiar_ai/decide/mod.rs` に `pub mod task_management;` を追加する。
  - root 側の対応ファイルは hw_ai 実装への re-export または thin wrapper に縮退する。

- 変更ファイル（hw_ai 追加）:
  - `crates/hw_ai/src/familiar_ai/decide/mod.rs`
  - `crates/hw_ai/src/familiar_ai/decide/task_management/mod.rs`
  - `crates/hw_ai/src/familiar_ai/decide/task_management/task_assigner.rs`
  - `crates/hw_ai/src/familiar_ai/decide/task_management/task_finder/{mod,score,filter}.rs`
  - `crates/hw_ai/src/familiar_ai/decide/task_management/validator/{mod,resolver,finder,wheelbarrow,reservation}.rs`
  - `crates/hw_ai/src/familiar_ai/decide/task_management/policy/haul/{demand,direct_collect,source_selector}.rs`
- 変更ファイル（root 縮退）:
  - `src/systems/familiar_ai/decide/task_management/mod.rs`
  - `src/systems/familiar_ai/decide/task_management/task_assigner.rs`
  - `src/systems/familiar_ai/decide/task_management/task_finder/*.rs`
  - `src/systems/familiar_ai/decide/task_management/validator/*.rs`
- 完了条件:
  - [ ] `collect_scored_candidates` と候補検証の主処理が `hw_ai` 実装を参照する
  - [ ] root 側の対応モジュールが re-export か thin wrapper のみ
  - [ ] `cargo check -p hw_ai` が通る
- 検証:
  - `CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check -p hw_ai`
  - `CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check --workspace`

## M3: non-construction assignment build / policy を `hw_ai` へ移す

- 変更内容:
  - 以下を `hw_ai` へ移す:
    - `delegation/{mod,assignment_loop,members}.rs`（`TaskManager` 含む）
    - `task_assigner.rs`（`assign_task_to_worker` 含む）
    - `builders/{mod,basic,haul,water}.rs`（`haul.rs` の FloorConstructionSite はコメント参照のみで実型依存なし）
    - `policy/{mod,basic,water}.rs`
    - `policy/haul/{mod,blueprint,stockpile,mixer,returns,consolidation,lease_validation,wheelbarrow}.rs`（11 ファイル）
  - `policy/floor.rs`、`policy/haul/{floor,wall,provisional_wall}.rs` は **この段階では root bridge に残し**、共通 interface 越しに呼ぶ。
- 変更ファイル（hw_ai 追加）:
  - `crates/hw_ai/src/familiar_ai/decide/task_management/delegation/{mod,assignment_loop,members}.rs`
  - `crates/hw_ai/src/familiar_ai/decide/task_management/task_assigner.rs`
  - `crates/hw_ai/src/familiar_ai/decide/task_management/builders/{mod,basic,haul,water}.rs`
  - `crates/hw_ai/src/familiar_ai/decide/task_management/policy/{mod,basic,water}.rs`
  - `crates/hw_ai/src/familiar_ai/decide/task_management/policy/haul/{mod,blueprint,stockpile,mixer,returns,consolidation,lease_validation,wheelbarrow}.rs`
- 変更ファイル（root 縮退）:
  - `src/systems/familiar_ai/decide/task_management/delegation/*.rs`
  - `src/systems/familiar_ai/decide/task_management/task_assigner.rs`
  - `src/systems/familiar_ai/decide/task_management/builders/*.rs`
  - `src/systems/familiar_ai/decide/task_management/policy/{mod,basic,water}.rs`
  - `src/systems/familiar_ai/decide/task_management/policy/haul/{mod,blueprint,stockpile,mixer,returns,consolidation,lease_validation,wheelbarrow}.rs`
- 完了条件:
  - [ ] Basic / haul / water の割り当て build が `hw_ai` 実装を経由する
  - [ ] 予約 shadow と source selector の経路が root から複製されていない
  - [ ] floor / wall / provisional 以外の policy 実体が root に残っていない
- 検証:
  - `CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check -p hw_ai`
  - `CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check --workspace`

## M4: root construction bridge と task delegation adapter を整理する

- 変更内容:
  - root `task_management` を construction bridge（4 ファイル）と re-export のみに縮退する。
  - **root 残留確定ファイル**:
    - `policy/floor.rs`（`FloorConstructionSite` 依存）
    - `policy/haul/floor.rs`（`queries.storage.floor_sites` 使用）
    - `policy/haul/wall.rs`（`queries.storage.wall_sites` 使用）
    - `policy/haul/provisional_wall.rs`（`floor_sites`/`wall_sites` 使用）
  - これら 4 ファイルの呼び出し経路を `hw_ai::familiar_ai::decide::task_management::policy` の construction bridge trait 越しに明文化する。
  - `src/systems/familiar_ai/decide/task_delegation.rs` と `familiar_processor.rs` を新しい `hw_ai::familiar_ai::decide::task_management` API に合わせて更新する。
- 変更ファイル:
  - `src/systems/familiar_ai/decide/task_management/mod.rs`（re-export のみに縮退）
  - `src/systems/familiar_ai/decide/task_management/policy/floor.rs`（construction bridge として残留）
  - `src/systems/familiar_ai/decide/task_management/policy/haul/{floor,wall,provisional_wall}.rs`（同上）
  - `src/systems/familiar_ai/decide/task_delegation.rs`
  - `src/systems/familiar_ai/decide/familiar_processor.rs`
- 完了条件:
  - [ ] root `task_management` 配下の実装本体が construction bridge（4 ファイル）と adapter のみに限定される
  - [ ] `familiar_task_delegation_system` は従来通り root 所有のまま `hw_ai` core を呼ぶ
  - [ ] 二重登録や root / crate 側の実装重複がない
- 検証:
  - `CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check --workspace`
  - Familiar の blueprint / haul / water / construction 系割り当てを手動確認

## M5: ドキュメント同期と回帰確認

- 変更内容:
  - `docs/cargo_workspace.md`, `docs/familiar_ai.md`, `docs/tasks.md`, `crates/hw_ai/README.md`, 必要なら `src/systems/familiar_ai/README.md` を更新する。
  - proposal と plan の相互参照を同期する。
  - root 残留ファイル（construction bridge 4 点）の理由を docs へ明文化する。
  - `crates/hw_ai/README.md` の `decide/` ディレクトリ構造に `task_management/` を追記する。
- 変更ファイル:
  - `docs/cargo_workspace.md`
  - `docs/familiar_ai.md`
  - `docs/tasks.md`
  - `crates/hw_ai/README.md`
  - `docs/proposals/familiar-task-management-hw-ai-extraction-proposal-2026-03-11.md`
- 完了条件:
  - [ ] crate 境界説明と Familiar task delegation の責務分割が docs に反映されている
  - [ ] plan/proposal/index の参照切れがない
  - [ ] `CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check --workspace` が通る
- 検証:
  - `CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check --workspace`
  - `python scripts/update_docs_index.py`（存在する場合）

## 6. リスクと対策

| リスク | 影響 | 対策 |
| --- | --- | --- |
| `TaskQueries` / `TaskAssignmentQueries` 分割時に Soul 実行側の Query 構成を壊す | `task_execution` が広範囲に壊れる | M1 では Familiar 向け access 抽出を優先し、Soul 側の public API は互換維持する |
| construction bridge と core 側で需要計算や source 選定が二重実装になる | floor / wall / provisional のみ仕様がずれる | 共通ロジックは `hw_ai` に寄せ、root には construction 固有 Query 解決だけを残す |
| root と `hw_ai` に同名 module が並び、呼び出し経路が不明瞭になる | 将来の保守で再び root が肥大化する | root 側は `pub use` / thin wrapper のみとし、README と docs に所有者を明記する |
| reservation shadow / incoming snapshot の経路が変わり回帰する | 二重割り当てや過剰搬送が起きる | M2/M3 完了ごとに blueprint / haul / water シナリオで手動確認し、既存 debug metric を比較する |

## 7. 検証計画

- 必須:
  - `cargo check -p hw_ai`
  - `cargo check --workspace`
- 手動確認シナリオ:
  - Familiar が `TaskArea` 内の blueprint / haul / water 系タスクを従来通り選定できる
  - 予約競合時に同一資源へ二重割り当てしない
  - floor / wall / provisional wall の construction 系が移設途中でも候補から脱落しない
  - `ManagedTasks` と yard 共有タスクの発見条件が変わらない
- パフォーマンス確認:
  - `FamiliarDelegationPerfMetrics` の `source_selector_calls` / `reachable_with_cache_calls` が大きく悪化していないことを確認する

## 8. ロールバック方針

- どの単位で戻せるか:
  - M1 の access 分割
  - M2 の read-heavy core 移設
  - M3 の assignment build / policy 移設
  - M4 の root adapter 整理
- 戻す時の手順:
  - 各マイルストーンを独立 commit に分ける
  - まず `task_delegation.rs` / `familiar_processor.rs` の import を前段 API に戻し、その後 `hw_ai` 側追加モジュールを差し戻す
  - construction bridge と core の両方を同時に戻さず、境界変更の commit から逆順に戻す

## 9. AI引継ぎメモ（最重要）

### 現在地

- 進捗: `M1 完了（20%）`
- 完了済みマイルストーン: **M1**
- 未着手/進行中: M2 の read-heavy core 移設

### 次のAIが最初にやること

1. `crates/hw_ai/src/familiar_ai/decide/mod.rs` に `pub mod task_management;` を追加する。
2. `crates/hw_ai/src/familiar_ai/decide/task_management/` ディレクトリを新設し、`mod.rs` に `IncomingDeliverySnapshot` を移植する。
3. `task_assigner.rs` の `ReservationShadow`・`CachedSourceItem`・`SourceSelectorFrameCache`・`AssignTaskContext` を hw_ai へ移す（`FamiliarTaskAssignmentQueries` は M1 で独立型になったので使用可能）。
4. `task_finder/{mod,score,filter}.rs` と `validator/{5 ファイル}` を hw_ai へ移す。

### 移設可否マップ（コードサーベイ結果）

| ファイル | hw_ai 移設 | 理由 |
| --- | --- | --- |
| `mod.rs`（IncomingDeliverySnapshot） | ✅ | hw_* 型のみ |
| `task_assigner.rs` | ✅ | construction site 型依存なし |
| `task_finder/{mod,score,filter}.rs` | ✅ | 読み取り・スコアリングのみ |
| `validator/{5 ファイル}` | ✅ | construction site 型依存なし |
| `delegation/{3 ファイル}` | ✅ | 本体はロジック抽象 |
| `builders/{mod,basic,water}.rs` | ✅ | construction site 型依存なし |
| `builders/haul.rs` | ✅ | FloorConstructionSite はコメント内のみ（実型依存なし） |
| `policy/{mod,basic,water}.rs` | ✅ | WorkType dispatcher |
| `policy/haul/{demand,direct_collect,source_selector}.rs` | ✅ | read-heavy core |
| `policy/haul/{blueprint,stockpile,mixer,returns,consolidation,lease_validation,wheelbarrow}.rs` | ✅ | construction site 型依存なし |
| `policy/floor.rs` | ❌ root 残留 | `queries.storage.floor_sites` 直接アクセス |
| `policy/haul/floor.rs` | ❌ root 残留 | `queries.storage.floor_sites` 直接アクセス |
| `policy/haul/wall.rs` | ❌ root 残留 | `queries.storage.wall_sites` 直接アクセス |
| `policy/haul/provisional_wall.rs` | ❌ root 残留 | `floor_sites`/`wall_sites` 両方アクセス |

### ブロッカー/注意点

- `familiar_task_delegation_system` 自体は `WorldMapRead` / `PathfindingContext` / concrete spatial grids を使うため root 残留前提。
- `FamiliarTaskAssignmentQueries` の alias を先に解消しないと M2 以降で広範囲エラーが発生する（M1 を compile-first で進める理由）。
- `builders/haul.rs` の FloorConstructionSite はコメント参照のみで実際の型インポートはない。**移設可能**。
- M1 で `MutStorageAccess` の扱いも確認すること（Familiar 側で mutable storage が必要か確認）。

### 参照必須ファイル

- `docs/proposals/familiar-task-management-hw-ai-extraction-proposal-2026-03-11.md`
- `docs/cargo_workspace.md`
- `docs/familiar_ai.md`
- `docs/tasks.md`
- `src/systems/soul_ai/execute/task_execution/context/access.rs`（`StorageAccess` / `FamiliarStorageAccess` 定義先）
- `src/systems/soul_ai/execute/task_execution/context/queries.rs`（`TaskAssignmentQueries` / `FamiliarTaskAssignmentQueries` 定義先）
- `src/systems/familiar_ai/decide/task_management/mod.rs`
- `src/systems/familiar_ai/decide/task_delegation.rs`
- `src/systems/familiar_ai/decide/familiar_processor.rs`
- `crates/hw_ai/src/familiar_ai/decide/mod.rs`（`task_management` を追加する先）
- `crates/hw_ai/README.md`

### 最終確認ログ

- 最終 `cargo check`: `2026-03-11` / **Finished（エラーなし）— M1 完了**
- 未解決エラー: なし（計画書作成時点では未実装）

### Definition of Done

- [ ] M1-M5 の方針がレビュー可能な粒度で固まっている
- [ ] root に残る責務（construction bridge 4 ファイル + delegation system）と `hw_ai` へ移す責務（31 ファイル）が衝突なく定義されている
- [ ] 関連 docs と proposal の参照関係が同期されている
- [ ] 実装完了時に `CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check --workspace` が通る段取りが明記されている

## 10. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| `2026-03-11` | `AI (Codex)` | 初版作成 |
| `2026-03-11` | `AI (Copilot)` | M1 実装完了: `FamiliarStorageAccess`・`ConstructionSiteAccess`・`FamiliarTaskAssignmentQueries` 追加、`policy/haul/{floor,wall}.rs` のアクセスパスを `queries.construction_sites.*` に更新、`cargo check --workspace` 成功 |
