# Familiar AI のオーケストレーター分離・Plugin 移行計画

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `familiar-ai-adapter-plan-2026-03-13` |
| ステータス | `Draft（コード調査済み・具体化版）` |
| 作成日 | `2026-03-13` |
| 最終更新日 | `2026-03-13` |
| 作成者 | `Gemini Agent` → Copilot Agent がブラッシュアップ |
| 関連提案 | `docs/proposals/crate-boundaries-refactor-plan.md` |
| 関連Issue/PR | `N/A` |

## 1. 目的

- **解決したい課題**: Familiar AI の Decide Phase を担う2つの Bevy System（`familiar_ai_state_system` / `familiar_task_delegation_system`）が `bevy_app` で登録されており、`crate-boundaries.md §3.3`「自クレート内で完結するシステムは Leaf Plugin で登録する」原則に違反している。
- **到達したい状態**: 両システムが `hw_familiar_ai::FamiliarAiCorePlugin` で登録される。`bevy_app::FamiliarAiPlugin` は `FamiliarAiCorePlugin` の add_plugins と root 固有リソースの初期化のみを担う薄いアダプターになる。
- **成功指標**: `cargo check --workspace` が通過し、ゲーム内で使い魔の状態遷移・タスク委譲が従来通り動作すること。

## 2. 事前調査結果（コード実態）

### 2.1 依存関係マップ（調査済み）

```
bevy_app::FamiliarAiPlugin
  ├── hw_familiar_ai::FamiliarAiCorePlugin（add_plugins 済み）
  │     └── decide::following, perceive::state_detection, execute::state_apply 等を登録
  └── 追加で以下を直接 add_systems（← 本計画の対象）
        ├── decide::state_decision::familiar_ai_state_system
        └── decide::task_delegation::familiar_task_delegation_system
```

### 2.2 各ファイルのRoot依存ブロッカー（実調査結果）

#### `state_decision.rs` のブロッカー

| ブロッカー型 | 定義場所 | 実際の型 | 対応方針 |
| --- | --- | --- | --- |
| `FamiliarStateQuery<'w,'s>` | `bevy_app/systems/familiar_ai/helpers/query_types.rs` | 全フィールドが `hw_core`/`hw_jobs` 型 | `hw_familiar_ai::decide::query_types` へ移動 |
| `FamiliarSoulQuery<'w,'s>` | 同上 | 全フィールドが `hw_core`/`hw_jobs`/`hw_logistics` 型 | 同上 |
| `FamiliarDecideOutput<'w>` | `bevy_app/systems/familiar_ai/decide/mod.rs` | ラップする Event 型はすべて `hw_core::events` | `hw_familiar_ai::decide` モジュールへ移動 |

> **重要**: `DamnedSoul`・`IdleState`・`Path`・`Destination`・`Familiar`・`FamiliarOperation`・`ActiveCommand`・`TaskArea` はすべて `hw_core` 定義。`AssignedTask` は `hw_jobs` 定義。`SpatialGrid` は `hw_spatial` 定義。これらは **Root 依存ではない**。`hw_familiar_ai/Cargo.toml` は既に全依存を持つ。

#### `task_delegation.rs` のブロッカー

| ブロッカー型 | 定義場所 | 実際の型 | 対応方針 |
| --- | --- | --- | --- |
| `FamiliarTaskDelegationTimer` | `bevy_app/systems/familiar_ai/mod.rs` | Root 固有 Resource | `hw_familiar_ai` へ移動 |
| `FamiliarDelegationPerfMetrics` | 同上 | Root 固有 Resource | `hw_familiar_ai` へ移動 |
| `FamiliarTaskQuery<'w,'s>` | `bevy_app/systems/familiar_ai/helpers/query_types.rs` | 全フィールドが Leaf 型 | `hw_familiar_ai::decide::query_types` へ移動 |
| `WorldMapRead<'w>` | `hw_world` | Leaf 型 (`hw_world`) | **ブロッカーではない**（既に hw_familiar_ai dep） |
| `PathfindingContext` | `hw_world` | Leaf 型 | **ブロッカーではない** |

#### `familiar_processor.rs` のブロッカー

| ブロッカー型 | 定義場所 | 対応方針 |
| --- | --- | --- |
| `FamiliarDelegationContext` struct | `bevy_app/decide/familiar_processor.rs` | hw_familiar_ai へ移動（フィールドはすべて Leaf 型） |
| `ConstructionSiteAccess<'w,'s>` | `hw_soul_ai/soul_ai/execute/task_execution/context/access.rs` | **要検討**: `hw_familiar_ai` → `hw_soul_ai` 依存追加か、`ConstructionSiteAccess` を `hw_jobs` へ移動か |

> `hw_soul_ai` は `hw_familiar_ai` に依存していないため循環なし。ただし Leaf 間の密結合を避けるため `hw_jobs` への移動が望ましい。

### 2.3 `hw_familiar_ai/Cargo.toml` 現状

```toml
hw_core      = { path = "../hw_core" }      # ✅ DamnedSoul, FamiliarAiState 等
hw_jobs      = { path = "../hw_jobs" }      # ✅ AssignedTask
hw_logistics = { path = "../hw_logistics" } # ✅ Inventory, TileSiteIndex
hw_world     = { path = "../hw_world" }     # ✅ WorldMap, PathfindingContext
hw_spatial   = { path = "../hw_spatial" }   # ✅ SpatialGrid, DesignationSpatialGrid
```

M1（state_decision）は Cargo.toml 変更不要。M2 で `ConstructionSiteAccess` を `hw_jobs` に移動する。

## 3. スコープ

### 対象（In Scope）

1. `familiar_ai_state_system` を `hw_familiar_ai::FamiliarAiCorePlugin` で登録する
2. `familiar_task_delegation_system` を `hw_familiar_ai::FamiliarAiCorePlugin` で登録する
3. それに必要な型（`FamiliarStateQuery`/`FamiliarSoulQuery`/`FamiliarTaskQuery`/`FamiliarDecideOutput`/`FamiliarTaskDelegationTimer`/`FamiliarDelegationPerfMetrics`/`FamiliarDelegationContext`）を `hw_familiar_ai` へ移動する
4. **【M2前提】** `ConstructionSiteAccess` を `hw_soul_ai` から `hw_jobs` へ移動する（案Bの採用）

### 非対象（Out of Scope）

- Soul AI の登録移譲（別計画で実施）
- `GameSystemSet` / `FamiliarAiSystemSet` の定義変更

## 4. 実装方針

- `crate-boundaries.md §4.1 フェーズ2`「Leaf crate 間の依存のみで動くシステムの移動」に相当する作業。
- 型を動かす際は `bevy_app` 側に `pub use hw_familiar_ai::...` の re-export を一時的に残しコンパイルを通してから、次のステップで root 側の直接参照を削除する。
- 登録順序は `bevy_app::FamiliarAiPlugin` に記述された `.chain()` 制約を `FamiliarAiCorePlugin::build` 内で完全に再現する。

## 5. マイルストーン

### M1: `familiar_ai_state_system` の移譲


#### M1-1: `FamiliarStateQuery` / `FamiliarSoulQuery` の移動

- **変更ファイル（移動元）**: `crates/bevy_app/src/systems/familiar_ai/helpers/query_types.rs`
- **変更ファイル（移動先）**: `crates/hw_familiar_ai/src/familiar_ai/decide/query_types.rs`（既存ファイルに追記）
- **作業内容**:
  1. `FamiliarStateQuery` と `FamiliarSoulQuery` の type alias 定義を `hw_familiar_ai` の `query_types.rs` 末尾に追記
  2. import を `crate::*` から `hw_core::*`/`hw_jobs::*`/`hw_logistics::*` に書き換え
  3. `bevy_app` 側の `query_types.rs` に `pub use hw_familiar_ai::familiar_ai::decide::query_types::{FamiliarStateQuery, FamiliarSoulQuery};` を追加（既存参照を壊さない）
- **完了条件**: `cargo check --workspace` が通る

#### M1-2: `FamiliarDecideOutput` の移動

- **変更ファイル（移動元）**: `crates/bevy_app/src/systems/familiar_ai/decide/mod.rs`（`FamiliarDecideOutput` struct 定義部分）
- **変更ファイル（移動先）**: `crates/hw_familiar_ai/src/familiar_ai/decide/mod.rs`
- **作業内容**:
  1. `FamiliarDecideOutput` と `#[derive(SystemParam)]` を `hw_familiar_ai::decide::mod` に移動
  2. `MessageWriter` の import 元を確認し `hw_familiar_ai` 側で解決（`hw_familiar_ai` は既に `bevy` dep あり）
  3. `bevy_app::decide::mod` に `pub use hw_familiar_ai::familiar_ai::decide::FamiliarDecideOutput;` を追加
- **完了条件**: `cargo check --workspace` が通る

#### M1-3: `familiar_ai_state_system` の移動と Plugin 登録

- **変更ファイル（移動元）**: `crates/bevy_app/src/systems/familiar_ai/decide/state_decision.rs`
- **変更ファイル（移動先）**: `crates/hw_familiar_ai/src/familiar_ai/decide/state_decision.rs`（既存ファイルを拡張）
- **作業内容**:
  1. `FamiliarAiStateDecisionParams` と `familiar_ai_state_system` を `hw_familiar_ai` の `state_decision.rs` 末尾に移動
  2. import を `crate::*` 参照から `hw_core::*` 等に修正
  3. `hw_familiar_ai::FamiliarAiCorePlugin::build` に以下を追加：
     ```rust
     .add_systems(
         Update,
         decide::state_decision::familiar_ai_state_system
             .in_set(FamiliarAiSystemSet::Decide),
     )
     ```
  4. `bevy_app::FamiliarAiPlugin` の Decide chain から `familiar_ai_state_system` を削除し、残る先頭の `blueprint_auto_gather_system` に `.after()` を追加して順序を明示する：
     ```rust
     // 変更前
     ((
         decide::state_decision::familiar_ai_state_system,
         decide::auto_gather_for_blueprint::blueprint_auto_gather_system,
         ApplyDeferred,
         decide::task_delegation::familiar_task_delegation_system,
         decide::encouragement::encouragement_decision_system,
     ).chain()).in_set(FamiliarAiSystemSet::Decide)

     // 変更後（state_decision は FamiliarAiCorePlugin 側に移動済みのため削除、
     //         chain 先頭に .after() を追加して順序保証）
     ((
         decide::auto_gather_for_blueprint::blueprint_auto_gather_system
             .after(hw_familiar_ai::familiar_ai::decide::state_decision::familiar_ai_state_system),
         ApplyDeferred,
         decide::task_delegation::familiar_task_delegation_system,
         decide::encouragement::encouragement_decision_system,
     ).chain()).in_set(FamiliarAiSystemSet::Decide)
     ```
  5. `bevy_app/src/systems/familiar_ai/decide/state_decision.rs` は空になるため削除（mod.rs から宣言も削除）
- **完了条件**: `cargo check --workspace` が通る

---

### M2: `familiar_task_delegation_system` の移譲

> **前提判断**: `ConstructionSiteAccess` の扱いについて、**案B（`hw_jobs` への移動）** を採用する。
> M2 に着手する最初のステップとして、これを実施する。

#### M2-0: `ConstructionSiteAccess` の `hw_jobs` への移動 (前提タスク)

- **変更ファイル（移動元）**: `crates/hw_soul_ai/src/soul_ai/execute/task_execution/context/access.rs`
- **変更ファイル（移動先）**: `crates/hw_jobs/src/construction.rs`
- **作業内容**:
  1. `ConstructionSiteAccess` 構造体と、その `ConstructionSitePositions` 実装を `hw_jobs/src/construction.rs` へ移動する。
  2. `hw_jobs/src/lib.rs` で `pub use` する。
  3. `bevy_app` および `hw_soul_ai` に残る `use` パスを `hw_jobs::construction::ConstructionSiteAccess` へ修正する。
- **完了条件**: `cargo check --workspace` が通る。

#### M2-1: リソース型の移動（`FamiliarTaskDelegationTimer` / `FamiliarDelegationPerfMetrics`）

- **変更ファイル（移動元）**: `crates/bevy_app/src/systems/familiar_ai/mod.rs`
- **変更ファイル（移動先）**: `crates/hw_familiar_ai/src/familiar_ai/mod.rs` または `crates/hw_familiar_ai/src/familiar_ai/decide/resources.rs`（新規）
- **作業内容**:
  1. `FamiliarTaskDelegationTimer` と `FamiliarDelegationPerfMetrics` の struct 定義を移動
  2. `FAMILIAR_TASK_DELEGATION_INTERVAL` は既に `hw_core::constants` にあるため import のみ変更
  3. `bevy_app` 側に `pub use` re-export を追加（`init_resource` 等を壊さない）
  4. `FamiliarAiCorePlugin` で `init_resource::<FamiliarTaskDelegationTimer>()` / `init_resource::<FamiliarDelegationPerfMetrics>()` を追加
  5. `bevy_app::FamiliarAiPlugin` の対応 `init_resource` 行を削除
- **完了条件**: `cargo check --workspace` が通る

#### M2-2: `FamiliarTaskQuery` / `FamiliarDelegationContext` の移動

- **変更ファイル（移動元）**:
  - `crates/bevy_app/src/systems/familiar_ai/helpers/query_types.rs`（FamiliarTaskQuery）
  - `crates/bevy_app/src/systems/familiar_ai/decide/familiar_processor.rs`（FamiliarDelegationContext）
- **変更ファイル（移動先）**: `crates/hw_familiar_ai/src/familiar_ai/decide/query_types.rs` と `crates/hw_familiar_ai/src/familiar_ai/decide/delegation_context.rs`（新規）
- **作業内容**:
  1. `FamiliarTaskQuery` を hw_familiar_ai query_types へ移動
  2. `FamiliarDelegationContext` struct と `process_task_delegation_and_movement` 関数を hw_familiar_ai へ移動（hw_world, hw_spatial 型を直接使用）
  3. `bevy_app/decide/familiar_processor.rs` に re-export を追加し、残留コメントのみを保持

#### M2-3: `familiar_task_delegation_system` の移動と Plugin 登録

- **変更ファイル（移動元）**: `crates/bevy_app/src/systems/familiar_ai/decide/task_delegation.rs`
- **変更ファイル（移動先）**: `crates/hw_familiar_ai/src/familiar_ai/decide/task_delegation.rs`（新規）
- **作業内容**:
  1. `FamiliarAiTaskDelegationParams` と `familiar_task_delegation_system` を hw_familiar_ai へ移動
  2. `ReachabilityFrameCache` リソース定義も hw_familiar_ai へ移動
  3. `FamiliarAiCorePlugin::build` の `familiar_ai_state_system` 登録に `.before()` を追加し、`task_delegation` を `.after()` で後続させる：
     ```rust
     // FamiliarAiCorePlugin 側（M1-3 からの変更点：.before() を追加）
     .add_systems(
         Update,
         (
             decide::state_decision::familiar_ai_state_system,
             ApplyDeferred,
             decide::task_delegation::familiar_task_delegation_system,
         )
             .chain()
             .in_set(FamiliarAiSystemSet::Decide),
     )
     ```
     > **注意**: `FamiliarAiPlugin` 側に残る `blueprint_auto_gather_system` は M1-3 時点で `familiar_ai_state_system` との `.after()` 関係を設定済みのため、`task_delegation` → `encouragement` との順序は `FamiliarAiPlugin` 側の chain がそのまま保証する。
  4. `bevy_app::FamiliarAiPlugin` から `task_delegation` の add_systems 行・`ReachabilityFrameCache` の `init_resource` 行を削除する。`blueprint_auto_gather → ApplyDeferred → encouragement` の chain は残留させ、先頭の `after(familiar_ai_state_system)` は維持する（M1-3 で設定済み）
- **完了条件**: `cargo check --workspace` が通る

## 6. リスクと対策

| リスク | 影響 | 具体的な確認ポイント | 対策 |
| --- | --- | --- | --- |
| 登録順序の喪失（1フレーム遅れ） | 高 | `bevy_app/mod.rs` の `.chain()` 内順序: `state_decision → auto_gather_for_blueprint → ApplyDeferred → task_delegation → encouragement` | plugin をまたぐ chain は `.chain()` では組めないため、`blueprint_auto_gather_system` に `.after(familiar_ai_state_system)` を付与し（M1-3）、M2-3 で `FamiliarAiCorePlugin` 側の chain に `task_delegation` を追加する |
| `FamiliarSoulQuery` の `transmute_lens_filtered` 呼び出し | 中 | `state_decision.rs` 内の `q_souls.transmute_lens_filtered::<...>()` | lens の型パラメータも hw 型のみ使用しているため問題なし（調査済み） |
| `ConstructionSiteAccess` の循環依存 | 高 | `hw_familiar_ai` → `hw_soul_ai` 依存追加 | 案Bで `hw_jobs` へ移動してから M2 に着手 |
| perf_metrics リセットロジックの分散 | 低 | `task_delegation.rs` 末尾の log_interval_secs リセット処理 | hw_familiar_ai 移動時にそのまま移植（ロジック変更なし） |

## 7. 検証計画

- **ステップ確認**: 各 M1-x / M2-x 完了後に必ず実行
  ```bash
  CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check --workspace
  ```
- **手動確認シナリオ**（全マイルストーン完了後）:
  1. 使い魔が `Idle → Scouting → Supervising` の状態遷移を行うこと
  2. 使い魔が「木を伐る」「物を運ぶ」タスクを Soul に正しく委任できること
  3. ハイロードシナリオ（`--spawn-souls 500 --spawn-familiars 30`）でクラッシュや明らかなフレーム落ちがないこと

## 8. ロールバック方針

- M1-1/M1-2/M1-3 を各々 1 コミットとし、失敗時は該当コミットを `git revert`
- M2 は前提タスク（ConstructionSiteAccess 移動）を含めて独立ブランチで実施

## 9. AI引継ぎメモ（最重要）

### 現在地

- 進捗: `33%`
- 完了済みマイルストーン: M1
- 未着手/進行中: M2（前提タスク待ち）

### 次のAIが最初にやること

1. M2 に着手する前に、**前提タスクである「`ConstructionSiteAccess` の `hw_jobs` への移動」を先に行う**こと。
2. その後、M2-1（リソース型の移動）に着手する。

### ブロッカー/注意点

- **`ConstructionSiteAccess`（M2 前提）**: M2 着手前に案B（`hw_jobs` 移動）を必ず実施すること。
- **`MessageWriter` の import**: `hw_familiar_ai` では既に `task_management/context.rs` で `MessageWriter` を使用しているので import 解決方法はそちらを参照。
- **chain 順序**: 実際の順序は `state_decision → auto_gather_for_blueprint → ApplyDeferred → task_delegation → encouragement`。plugin をまたぐため `.chain()` で一括管理できない。M1-3 で `blueprint_auto_gather_system.after(familiar_ai_state_system)` を設定し、M2-3 で `FamiliarAiCorePlugin` 側の chain に `task_delegation` を追加することで順序を維持する（M1-3/M2-3 の手順コメントを参照）。

### 参照必須ファイル

- `docs/crate-boundaries.md`（§3.3, §4.1 フェーズ2）
- `crates/hw_familiar_ai/src/familiar_ai/decide/task_management/context.rs`（MessageWriter の使用例）
- `crates/bevy_app/src/systems/familiar_ai/mod.rs`（現在の登録順序の正）

### 最終確認ログ

- 最終 `cargo check`: N/A（着手前）
- 未解決エラー: なし

### Definition of Done

- [ ] `familiar_ai_state_system` が `FamiliarAiCorePlugin` で登録されている
- [ ] `familiar_task_delegation_system` が `FamiliarAiCorePlugin` で登録されている
- [ ] `bevy_app::FamiliarAiPlugin` は `add_plugins(FamiliarAiCorePlugin)` と root 固有 Resource/Plugin の初期化のみになっている
- [ ] `cargo check --workspace` が成功
- [ ] 手動確認シナリオが通過

## 10. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| `2026-03-13` | Gemini Agent | 初版ドラフト作成 |
| `2026-03-13` | Copilot Agent | コード実調査に基づき全面ブラッシュアップ（ブロッカー型・具体的手順・Cargo.toml 状況を追記） |