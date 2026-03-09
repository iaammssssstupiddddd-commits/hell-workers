# familiar_ai を hw_ai へ寄せて root を薄くする計画

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `familiar-ai-root-thinning-plan-2026-03-09` |
| ステータス | `Draft` |
| 作成日 | `2026-03-09` |
| 最終更新日 | `2026-03-09` |
| 作成者 | `AI (Codex)` |
| 関連提案 | `N/A` |
| 関連Issue/PR | `N/A` |

## 1. 目的

- **解決したい課題**: `familiar_ai` の純 AI ロジックが root に多く残っている。`WorldMap` / `Commands` / speech UI を必要としない処理まで root 側が抱えており、`hw_ai::FamiliarAiCorePlugin` があるのに crate 境界が薄くならない。
- **到達したい状態**: `hw_ai` に Familiar の状態機械・判断ヘルパ・需要供給計画・共有 `SystemParam` を集約し、root の `familiar_ai` は plugin wiring と root-only adapter に縮退する。残る root ファイルは「root-only 契約」（後述）を必ず満たす。
- **成功指標**:
  - `src/systems/familiar_ai/` に残るファイルが root-only 契約を満たす
  - `decide` 配下の純 AI ロジックが `hw_ai` へ移り、root 側は re-export または薄い wrapper になる
  - `cargo check -p hw_ai` と `cargo check --workspace` が成功する

---

## 2. スコープ

### 対象（In Scope）

- `FamiliarSoulQuery` を用途別に分割（M1）
- `decide/squad.rs`, `decide/supervising.rs`, `decide/scouting.rs`, `decide/state_handlers/*` の移設（M2）
- `familiar_processor.rs` 内 `finalize_state_transitions` / `process_squad_management` の移設（M2）
- `decide/recruitment.rs` の純ロジック部分を `SpatialGridOps` trait 経由で移設（M3）
- `decide/encouragement.rs` の decision 部分移設（M3）
- `decide/state_decision.rs` の root 側縮退（M3 後）
- `decide/auto_gather_for_blueprint/planning.rs`, `demand.rs`, `supply.rs` の移設（M4）
- `perceive/resource_sync.rs` の apply helper を `hw_logistics` へ移設（M5）
- 実装後の `docs/cargo_workspace.md`, `docs/familiar_ai.md`, `crates/hw_ai/README.md` の同期（M6）

### 非対象（Out of Scope）

- `decide/task_delegation.rs` と `decide/task_management/` の移設（WorldMap/PF/SG が深く絡む。別計画）
- `WorldMap` resource 本体の移設
- concrete SpatialGrid resource 本体の移設
- `execute/max_soul_apply.rs`, `execute/squad_apply.rs`, `execute/idle_visual_apply.rs` の `hw_ai` 化
- gameplay アルゴリズムの変更

---

## 3. 現状調査結果

### 3.1 hw_core に既に存在する型（移設ブロッカーではない）

以下の型は **すでに `hw_core`** に定義されており、`hw_ai` から直接利用可能。移設計画の「ブロッカー」にならない。

| 型 | hw_core パス | 備考 |
| --- | --- | --- |
| `FamiliarAiState` | `hw_core::familiar::FamiliarAiState` | 状態機械 enum |
| `Familiar` | `hw_core::familiar::Familiar` | |
| `FamiliarOperation` | `hw_core::familiar::FamiliarOperation` | `fatigue_threshold`, `max_controlled_soul` 等 |
| `ActiveCommand` | `hw_core::familiar::ActiveCommand` | |
| `DamnedSoul` | `hw_core::soul::DamnedSoul` | |
| `Destination` | `hw_core::soul::Destination` | |
| `Path` | `hw_core::soul::Path` | |
| `IdleState` | `hw_core::soul::IdleState` | `IdleBehavior` を含む |
| `StressBreakdown` | `hw_core::soul::StressBreakdown` | |
| `RestAreaCooldown` | `hw_core::soul::RestAreaCooldown` | |
| `CommandedBy` | `hw_core::relationships::CommandedBy` | |
| `Commanding` | `hw_core::relationships::Commanding` | |
| `ParticipatingIn` | `hw_core::relationships::ParticipatingIn` | |
| `RestingIn` | `hw_core::relationships::RestingIn` | |
| `TaskArea` | `hw_core::area::TaskArea` | |
| `SquadManagementRequest` | `hw_core::events::SquadManagementRequest` | |
| `FamiliarStateRequest` | `hw_core::events::FamiliarStateRequest` | |
| `FamiliarAiStateChangedEvent` | `hw_core::events::FamiliarAiStateChangedEvent` | |
`AssignedTask` は `hw_jobs`、`Inventory` は `hw_logistics` にあり、どちらも `hw_ai` の依存関係に含まれる。

### 3.2 root-only 型（移設先で使えない）

| 型 | 定義場所 | 理由 |
| --- | --- | --- |
| `FamiliarVoice` | `src/entities/familiar/voice.rs` | speech/UI 依存（root のみ） |
| `SpeechHistory` | `src/systems/visual/speech/cooldown.rs` | speech/UI 依存（root のみ） |
| `WorldMap` / `WorldMapRead` | root | resource 本体は root 残留 |
| concrete `SpatialGrid` (Res) | `hw_spatial` を root で System Param として使うシステム | root wrapper 経由で抽象化が必要 |

### 3.3 現在 hw_ai へ移設済み（re-export 縮退済み）

| root ファイル | `hw_ai` 側 |
| --- | --- |
| `perceive/state_detection.rs` | `crates/hw_ai/src/familiar_ai/perceive/state_detection.rs` |
| `decide/following.rs` | `crates/hw_ai/src/familiar_ai/decide/following.rs` |
| `execute/state_apply.rs` | `crates/hw_ai/src/familiar_ai/execute/state_apply.rs` |
| `execute/state_log.rs` | `crates/hw_ai/src/familiar_ai/execute/state_log.rs` |

### 3.4 移設候補と難易度（コード調査結果）

#### 移設対象ファイル一覧（decide/）

| ファイル | 移設先 | 難易度 | root-only 依存 | 前提条件 |
| --- | --- | --- | --- | --- |
| `decide/squad.rs` | `hw_ai` | 🟢 Easy | `FamiliarSoulQuery`（root 型エイリアス）のみ | M1: narrow query 定義 |
| `decide/supervising.rs` | `hw_ai` | 🟢 Easy | 同上 + `AssignedTask`（hw_jobs 経由 OK） | M1: narrow query 定義 |
| `decide/scouting.rs` | `hw_ai` | 🟢 Easy | 同上 | M1: narrow query 定義 |
| `decide/state_handlers/mod.rs` | `hw_ai` | 🟢 Easy | `crate::systems::familiar_ai::FamiliarAiState` → `hw_core` | なし（単独で移設可） |
| `decide/state_handlers/idle.rs` | `hw_ai` | 🟢 Easy | `ActiveCommand`, `FamiliarCommand` → `hw_core` | なし（単独で移設可） |
| `decide/state_handlers/searching.rs` | `hw_ai` | 🟢 Easy | `supervising::move_to_center` 呼び出し | `supervising.rs` 移設後 |
| `decide/state_handlers/scouting.rs` | `hw_ai` | �� Easy | `scouting::FamiliarScoutingContext` 参照 | `scouting.rs` 移設後 |
| `decide/state_handlers/supervising.rs` | `hw_ai` | 🟢 Easy | `supervising::FamiliarSupervisingContext` 参照 | `supervising.rs` 移設後 |
| `familiar_processor.rs::finalize_state_transitions` | `hw_ai` | 🟢 Easy | なし（`FamiliarAiState` のみ = hw_core） | なし（単独で移設可） |
| `familiar_processor.rs::process_squad_management` | `hw_ai` | 🟡 Medium | `FamiliarSoulQuery`（root 型エイリアス） | M1: narrow query 定義 |
| `decide/recruitment.rs` のスコアリング部 | `hw_ai` | 🟡 Medium | `SpatialGrid` → `SpatialGridOps` trait に置換 | `SpatialGridOps` ジェネリック化（M3） |
| `decide/encouragement.rs` の decision 部 | `hw_ai` | 🟡 Medium | `SpatialGrid` → trait | M3 |
| `decide/state_decision.rs` | `hw_ai` or root 縮退 | 🔴 Hard | `FamiliarDecideOutput`（root SystemParam）、`SpatialGrid` concrete | M1 + M2 + M3 後 |
| `auto_gather_for_blueprint/planning.rs` | `hw_ai` | 🟡 Medium | なし（純ロジック） | M4 |
| `auto_gather_for_blueprint/demand.rs` | `hw_ai` | 🟡 Medium | marker/owner の置き場 | M4 |
| `auto_gather_for_blueprint/supply.rs` | `hw_ai` | 🟡 Medium | `AutoGatherDesignation` 移設要否 | M4 |
| `auto_gather_for_blueprint/helpers.rs` の純ヘルパ | `hw_ai` | 🟡 Medium | `is_reachable` は root 残留 | M4 |
| `perceive/resource_sync.rs` の apply helpers | `hw_logistics` | 🟡 Medium | `SharedResourceCache` update | M5 |

### 3.5 root に残すべき領域

| 対象 | root 残留理由 |
| --- | --- |
| `decide/task_delegation.rs` | `WorldMapRead`, `PathfindingContext`, concrete SpatialGrid, perf cache を抱える |
| `decide/task_management/**/*` | `TaskAssignmentQueries` が `soul_ai::task_execution` context に直結 |
| `decide/auto_gather_for_blueprint.rs` / `actions.rs` | `Commands` と pathfinding による orchestration |
| `perceive/resource_sync.rs::sync_reservations_system` | `AssignedTask` lifecycle と removed detection に直結 |
| `execute/max_soul_apply.rs` / `squad_apply.rs` / `idle_visual_apply.rs` | speech/UI/`Commands` 依存 |

### 3.6 主なボトルネック（具体的）

- **`FamiliarSoulQuery`** が広すぎる（10 フィールド）。`squad.rs` / `supervising.rs` / `scouting.rs` は `Inventory`（hw_logistics）と `ParticipatingIn`（hw_core）を実際には使わない。この型エイリアスが root にある限り、これらのファイルを `hw_ai` へ移せない。
- **`FamiliarStateQuery`** が `FamiliarVoice` と `SpeechHistory`（root-only）を含む。`state_decision.rs` ではこれらを `_voice_opt` / `_history_opt` として無視しているにも関わらず、query の型に含まれているため `hw_ai` へ移すと root-only 型への依存が発生する。
- **`FamiliarDecideOutput`**（root SystemParam）は `state_decision.rs` のみが `SystemParam` として直接所持する。M2 では `squad` / `scouting` / `supervising` から direct な `MessageWriter` 依存を外し、root adapter が outcome を request message に変換する構成を採った。
- **`state_decision.rs`** が `SpatialGrid` を `Res` として直接参照する。これは M3 で recruitment ロジックが hw_ai に移ればほぼ不要になるはず。

---

## 4. 実装方針

- **root**: Bevy app shell と adapter（システム登録・query 結合・resource 取得）
- **`hw_ai`**: Familiar の判断ロジック（state machine、context struct、helper 関数群）
- **`hw_logistics`**: 物流共有 helper（`SharedResourceCache` update など）

### 4.1 root-only 契約

実装完了後、`src/systems/familiar_ai/` に残るファイルは次のいずれかを満たすこと。

1. `Commands` による entity orchestration を行う
2. `WorldMapRead/Write` または `PathfindingContext` を直接扱う
3. concrete SpatialGrid resource を `Res<T>` として直接扱う
4. `FamiliarVoice` / `SpeechHistory` など speech/UI/visual 依存を持つ
5. `soul_ai::task_execution` の context / query 群に直接依存する

このいずれにも当てはまらないロジックは `hw_ai` または `hw_logistics` に寄せる。

### 4.2 SpatialGrid の取り扱い方針

`hw_ai` 内コードで空間グリッドを参照する場合は、concrete 型（`SpatialGrid`）ではなく `hw_world::SpatialGridOps` トレイトを使うこと。

```rust
// hw_ai 内：NG
use crate::systems::spatial::SpatialGrid;
fn try_recruit(grid: &SpatialGrid, ...) { ... }

// hw_ai 内：OK（soul_ai/helpers/gathering_positions.rs の実例と同じパターン）
use hw_world::SpatialGridOps;
fn try_recruit<G: SpatialGridOps>(grid: &G, ...) { ... }
```

root 側 wrapper が concrete `SpatialGrid` を取得してから `hw_ai` ヘルパーへ渡す。

### 4.3 移設パターン

移設の基本形：

```
hw_ai 側:
  - Context struct の定義
  - ロジック関数の実装（Context を受け取る）

root 側:
  - System 関数（Bevy SystemParam を受け取り、Context を組み立てて hw_ai 関数を呼ぶ）
  - または re-export のみ
```

`crates/hw_ai/src/familiar_ai/decide/following.rs` が既存の移設済み実例。`use bevy::prelude::*` と `use hw_core::*` のみで閉じている。

### 4.4 FamiliarSoulQuery の分割方針（M1 の核心）

```rust
// 現在（root、10 フィールド）
pub type FamiliarSoulQuery<'w, 's> = Query<(
    Entity, &'static Transform, &'static DamnedSoul,
    &'static mut AssignedTask, &'static mut Destination, &'static mut Path,
    &'static IdleState,
    Option<&'static mut Inventory>,   // squad/supervising/scouting では未使用
    Option<&'static CommandedBy>,
    Option<&'static ParticipatingIn>, // squad/supervising では未使用
), Without<Familiar>>;

// 分割案（hw_ai または hw_core に定義できる minimal 版）
// Squad 検証・release に必要なフィールドのみ
pub type SoulSquadQuery<'w, 's> = Query<(
    Entity, &'static DamnedSoul, Option<&'static CommandedBy>,
), Without<Familiar>>;

// Supervising/Scouting に必要なフィールドのみ
pub type SoulMovementQuery<'w, 's> = Query<(
    Entity, &'static Transform, &'static DamnedSoul,
    &'static AssignedTask, &'static mut Destination, &'static mut Path,
    &'static IdleState, Option<&'static CommandedBy>,
), Without<Familiar>>;
```

既存の `FamiliarSoulQuery` は互換のために root に残し、タスク委譲・recruitment の full-fat 版として使い続ける（Out of Scope の領域）。

---

## 5. マイルストーン

### M1: Query と型エイリアスの整理

**目的**: M2 以降の移設を可能にする最小前提作業。

- **変更内容**:
  - `helpers/query_types.rs`：`FamiliarStateQuery` から `FamiliarVoice` と `SpeechHistory` を除外し、speech 依存のない `FamiliarStateDecisionQuery` を作成する（または `Option` を削除）
  - `helpers/query_types.rs`：squad/supervising/scouting 用の narrow query を追加定義する
  - 既存の `FamiliarSoulQuery` と `FamiliarStateQuery` は root 互換のために残す

- **変更ファイル**:
  - `src/systems/familiar_ai/helpers/query_types.rs`（narrow query 追加）
  - `src/systems/familiar_ai/decide/state_decision.rs`（speech フィールドの `_voice_opt` / `_history_opt` を query から除去）

- **完了条件**:
  - [x] `FamiliarStateQuery` に speech/UI 型が含まれない（または除外バリアントが存在する）
  - [x] squad / supervising / scouting に必要な最小フィールドの query 型が存在する
  - [x] `cargo check --workspace` が通る（M1 完了 2026-03-09）

- **検証**:
  ```bash
  CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check --workspace
  ```

---

### M2: pure familiar ロジックを hw_ai へ移す

**目的**: `squad.rs`, `supervising.rs`, `scouting.rs`, `state_handlers/*`, `finalize_state_transitions` を hw_ai へ移設。M1 完了後に実施。

- **変更内容**:

  1. **`decide/squad.rs`**:
     - `SquadManager` struct と全メソッドを `crates/hw_ai/src/familiar_ai/decide/squad.rs` へ移動
     - import を `crate::*` から `hw_core::*` + `hw_jobs::*` + narrow query 型へ変更
     - root 側は `pub use hw_ai::familiar_ai::decide::squad::SquadManager;` re-export に縮退

  2. **`decide/supervising.rs`**:
     - `FamiliarSupervisingContext` struct と `supervising_logic` / `move_to_center` を `hw_ai` へ移動
     - import: `hw_core::familiar::FamiliarAiState`, `hw_core::soul::{Destination, Path}`, `hw_core::area::TaskArea`, `hw_core::constants::TILE_SIZE`
     - root 側は re-export に縮退

  3. **`decide/scouting.rs`**:
     - `FamiliarScoutingContext` struct と `scouting_logic` を `hw_ai` へ移動
     - import: `hw_core::relationships::CommandedBy`, `hw_core::relationships::ParticipatingIn`, narrow soul query, pure outcome struct
     - root 側は re-export に縮退

  4. **`decide/state_handlers/*`**:
     - `mod.rs`（`StateTransitionResult`）: `hw_core::familiar::FamiliarAiState` のみ依存→そのまま hw_ai へ
     - `idle.rs`: `hw_core::familiar::{ActiveCommand, FamiliarCommand}` + `hw_core::soul::{Destination, Path}` + `hw_core::familiar::FamiliarAiState` → hw_ai へ
     - `searching.rs`: `supervising::move_to_center` 呼び出し → `supervising.rs` 移設後に hw_ai へ
     - `scouting.rs`: `scouting::FamiliarScoutingContext` 参照 → `scouting.rs` 移設後に hw_ai へ
     - `supervising.rs`: `supervising::FamiliarSupervisingContext` 参照 → `supervising.rs` 移設後に hw_ai へ
     - root 側は re-export に縮退

  5. **`familiar_processor.rs::finalize_state_transitions`**:
     - `hw_ai::familiar_ai::decide::helpers` 等に移動（`FamiliarAiState` + `Entity` のみ依存）
     - root 側の `familiar_processor.rs` は re-export または直接 call に変更

  6. **`familiar_processor.rs::process_squad_management`**:
     - `FamiliarSquadContext` struct と関数を hw_ai へ移動し、narrow query を受けて `SquadManagementOutcome` を返す
     - root 側は re-export または直接 call に変更

- **変更ファイル**:
  - `src/systems/familiar_ai/decide/squad.rs`（re-export に縮退）
  - `src/systems/familiar_ai/decide/supervising.rs`（re-export に縮退）
  - `src/systems/familiar_ai/decide/scouting.rs`（re-export に縮退）
  - `src/systems/familiar_ai/decide/state_handlers/mod.rs`（re-export に縮退）
  - `src/systems/familiar_ai/decide/state_handlers/idle.rs`（re-export に縮退）
  - `src/systems/familiar_ai/decide/state_handlers/searching.rs`（re-export に縮退）
  - `src/systems/familiar_ai/decide/state_handlers/scouting.rs`（re-export に縮退）
  - `src/systems/familiar_ai/decide/state_handlers/supervising.rs`（re-export に縮退）
  - `src/systems/familiar_ai/decide/familiar_processor.rs`（純ロジック部を移設、delegation 部は残留）
  - `crates/hw_ai/src/familiar_ai/decide/squad.rs`（新規）
  - `crates/hw_ai/src/familiar_ai/decide/supervising.rs`（新規）
  - `crates/hw_ai/src/familiar_ai/decide/scouting.rs`（新規）
  - `crates/hw_ai/src/familiar_ai/decide/state_handlers/` 配下（新規 4 ファイル）
  - `crates/hw_ai/src/familiar_ai/decide/helpers.rs`（新規、finalize_state_transitions 等）
  - `crates/hw_ai/src/familiar_ai/mod.rs`（plugin 登録範囲を拡張）

- **完了条件**:
  - [x] 上記ファイルが root から消えるか re-export のみになる
  - [x] root 側で pure state machine helper を直実装していない
  - [x] `crates/hw_ai/src/familiar_ai/decide/` に squad / supervising / scouting / state_handlers が揃う

- **検証**:
  ```bash
  CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check -p hw_ai
  CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check --workspace
  ```

---

### M3: SpatialGrid 依存の判断を adapter + core に分割する

**目的**: `recruitment.rs` と `encouragement.rs` の純ロジック部を hw_ai へ。SpatialGridOps trait を使う。

- **変更内容**:

  1. **`decide/recruitment.rs`** のスコアリング・選定部:
     - `score_recruit(soul_pos, fam_pos, ...)` → `hw_ai` へ（pure math のみ）
     - `try_immediate_recruit` と `start_scouting` を `hw_ai` へ、引数を `&impl SpatialGridOps` に変更:
       ```rust
       // hw_ai 内:
       use hw_world::SpatialGridOps;
       pub fn try_immediate_recruit<G: SpatialGridOps>(
           grid: &G, ...,
       ) -> Option<Entity> { ... }
       ```
     - root 側は `Res<SpatialGrid>` を取得し、 `RecruitmentManager::try_immediate_recruit(&*spatial_grid, ...)` と呼ぶ adapter に縮退

  2. **`decide/encouragement.rs`**:
     - 対象選定ロジック（スコアリング・フィルタリング部）を hw_ai へ
     - `SpatialGrid` 読み取りは root wrapper が担当
     - `EncouragementCooldown` コンポーネント: hw_core または hw_ai に移設し、`register_type` を hw_ai plugin に移動
     - root 側は `Time` / `SpatialGrid` / `MessageWriter<EncouragementRequest>` を束ねる adapter に縮退

  3. **`decide/state_decision.rs`**:
     - M2 完了後は squad/scouting/supervision ロジックを hw_ai に委譲している状態になる
     - M3 で recruitment も hw_ai に移ったら、system 本体をさらに薄くできる
     - `SpatialGrid` を `Res` として持つのは root system の責務のため、system function 自体は root 残留でも OK

  4. **`familiar_processor.rs::process_recruitment`**:
     - Context struct から `spatial_grid: &'a SpatialGrid` を `spatial_grid: &'a impl SpatialGridOps` へ変更し hw_ai へ移設

- **変更ファイル**:
  - `src/systems/familiar_ai/decide/recruitment.rs`
  - `src/systems/familiar_ai/decide/encouragement.rs`
  - `src/systems/familiar_ai/decide/state_decision.rs`
  - `src/systems/familiar_ai/decide/familiar_processor.rs`
  - `crates/hw_ai/src/familiar_ai/decide/recruitment.rs`（新規）
  - `crates/hw_ai/src/familiar_ai/decide/encouragement.rs`（新規）

- **完了条件**:
  - [x] `hw_ai` が concrete `SpatialGrid` resource を直接要求せずに recruitment / encouragement を扱える
  - [x] root 側の残留コードが候補抽出（`Res<SpatialGrid>` 取得）と adapter 呼び出しに限定される
  - [x] `EncouragementCooldown` 移設後に `register_type` が plugin 登録箇所で維持されている

- **検証**:
  ```bash
  CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check -p hw_ai
  CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check --workspace
  ```

---

### M4: auto gather の純計画層を hw_ai へ移す

- **変更内容**:
  - `auto_gather_for_blueprint/planning.rs`, `demand.rs`, `supply.rs` を `hw_ai` へ移す
  - `helpers.rs` の純ヘルパ（`drop_amount_for_resource`, `resolve_owner`, `div_ceil_u32` 等）を移す。`is_reachable`（PathfindingContext 依存）は root 側に残す
  - `AutoGatherDesignation` を hw_core / hw_jobs に移設するか、hw_ai 内で定義する（依存 crate の境界を確認してから決定）

- **変更ファイル**:
  - `src/systems/familiar_ai/decide/auto_gather_for_blueprint.rs`
  - `src/systems/familiar_ai/decide/auto_gather_for_blueprint/helpers.rs`
  - `src/systems/familiar_ai/decide/auto_gather_for_blueprint/planning.rs`
  - `src/systems/familiar_ai/decide/auto_gather_for_blueprint/demand.rs`
  - `src/systems/familiar_ai/decide/auto_gather_for_blueprint/supply.rs`
  - `src/systems/familiar_ai/decide/auto_gather_for_blueprint/actions.rs`
  - `crates/hw_ai/src/familiar_ai/decide/auto_gather_for_blueprint/*`（新規）

- **完了条件**:
  - [x] root 側は `Commands` / pathfinding orchestration のみを持つ
  - [x] 需要供給計画ロジックが `hw_ai` に集約される

- **検証**:
  ```bash
  CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check -p hw_ai
  CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check --workspace
  ```

---

### M5: `SharedResourceCache` 周辺 helper を hw_logistics に寄せる

- **変更内容**:
  - `perceive/resource_sync.rs` の `apply_reservation_op` と `apply_reservation_requests_system` を `hw_logistics` へ移す
  - `sync_reservations_system` は root に残し、`task_execution` lifecycle 連携だけを担当させる

- **変更ファイル**:
  - `src/systems/familiar_ai/perceive/resource_sync.rs`
  - `crates/hw_logistics/src/` 内の新規または既存 resource cache helper

- **完了条件**:
  - [ ] `SharedResourceCache` 更新 helper が `hw_logistics` に置かれる
  - [ ] `resource_sync.rs` の root 残留部分が reservation 再構築に限定される

- **検証**:
  ```bash
  CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check -p hw_logistics
  CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check --workspace
  ```

---

### M6: root shell の整理と docs 同期

- **変更内容**:
  - `src/systems/familiar_ai/mod.rs` の登録対象を root-only システムへ絞る
  - `crates/hw_ai/src/familiar_ai/mod.rs` の plugin 登録範囲を拡張する（M2/M3 で移設したシステムを追加）
  - `docs/cargo_workspace.md`, `docs/familiar_ai.md`, `crates/hw_ai/README.md`, `docs/README.md` を実態に合わせて更新する

- **変更ファイル**:
  - `src/systems/familiar_ai/mod.rs`
  - `crates/hw_ai/src/familiar_ai/mod.rs`
  - `docs/cargo_workspace.md`
  - `docs/familiar_ai.md`
  - `docs/README.md`
  - `crates/hw_ai/README.md`

- **完了条件**:
  - [ ] `src/systems/familiar_ai/` に残るファイルが root-only 契約を満たす
  - [ ] `docs/cargo_workspace.md` の `hw_ai` 代表例に squad/supervising/scouting/recruitment が記載されている

- **検証**:
  ```bash
  CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check --workspace
  ```

---

## 6. リスクと対策

| リスク | 影響 | 対策 |
| --- | --- | --- |
| narrow query を増やしすぎて borrow 競合が増える | 高 | 用途別に SystemParam レベルで分ける。実際に必要なフィールドだけ持たせる |
| `SpatialGridOps` trait の generic 化で API が複雑になる | 中 | soul_ai/helpers/gathering_positions.rs の実例パターンを踏襲する |
| `EncouragementCooldown` 移設後に `register_type` が漏れる | 中 | M3 で plugin 登録箇所を同時に更新する |
| `state_decision` と `task_delegation` の責務が再び混ざる | 中 | `FamiliarDelegationContext` は root 残留、state machine だけを切り出す |
| auto gather の `AutoGatherDesignation` の所在が曖昧になる | 中 | `AutoGatherDesignation` の owner と責務を M4 で明文化する |
| state_handlers が parent module（squad 等）の移設より先に移ってしまう | 低 | 移設順序を守る：parent module 移設 → state_handlers 移設 |

---

## 7. 検証計画

- 必須:
  - `CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check -p hw_ai`
  - `CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check -p hw_logistics`
  - `CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check --workspace`
- 手動確認シナリオ:
  - Familiar が `Idle` / `SearchingTask` / `Scouting` / `Supervising` を従来どおり遷移する
  - 近傍リクルート、遠方スカウト、監督追従、激励、Blueprint 自動 gather が回帰していない
- パフォーマンス確認（必要時）:
  - `cargo run -- --spawn-souls 500 --spawn-familiars 30 --perf-scenario`

---

## 8. ロールバック方針

- M1〜M6 を独立コミット単位で戻せるようにする
- root 側は移設直後しばらく re-export / thin wrapper を維持し、差し戻しを容易にする
- 戻す時の手順:
  1. 直近の移設マイルストーン単位で revert
  2. root 側の thin wrapper を一時的に実装へ戻す
  3. `cargo check --workspace` で整合確認

---

## 9. AI 引継ぎメモ（最重要）

### 現在地

- 進捗: `M5 着手前`
- 完了済みマイルストーン: M1（Query 型整理）、M2（pure familiar ロジック hw_ai へ移設）、M3（SpatialGrid 依存の判断を adapter + core に分割）、M4（auto gather の純計画層を hw_ai へ移す）
- 未着手/進行中: M5 から着手

### 次の AI が最初にやること

1. `perceive/resource_sync.rs` の apply helpers を確認する
2. `SharedResourceCache` 更新 helper を `hw_logistics` に移設する
3. root の `resource_sync.rs` は reservation 再構築のみを持つよう縮退する

### ブロッカー/注意点

- `hw_ai` へ concrete `SpatialGrid` / `WorldMapRead` を直接持ち込まない（`SpatialGridOps` trait を使う）
- `task_management` をこの計画の前半で無理に動かさない
- `SpeechHistory` と `FamiliarVoice` は root-only execute 側へ閉じ込める
- `FamiliarDecideOutput`（root SystemParam）は移設不要。M2 以降の hw_ai ロジックは outcome を返し、message 発行は root adapter に残す
- state_handlers は parent module（supervising/scouting）の移設後に移すこと
- `CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target` プレフィックスを cargo コマンドに付けること（ユーザーと root の CARGO_HOME が異なるため）

### 参照必須ファイル

- `docs/cargo_workspace.md`
- `docs/familiar_ai.md`
- `src/systems/familiar_ai/mod.rs`
- `src/systems/familiar_ai/helpers/query_types.rs`
- `src/systems/familiar_ai/decide/state_decision.rs`
- `src/systems/familiar_ai/decide/familiar_processor.rs`
- `src/systems/familiar_ai/perceive/resource_sync.rs`
- `crates/hw_ai/src/familiar_ai/mod.rs`
- `crates/hw_ai/src/familiar_ai/decide/following.rs`（移設済み実例）
- `crates/hw_ai/src/soul_ai/helpers/gathering_positions.rs`（SpatialGridOps ジェネリック化の実例）

### 最終確認ログ

- 最終 `cargo check`: `2026-03-09 M3 完了時点で cargo check -p hw_ai / cargo check --workspace 実行済み。エラー・警告ゼロ`
- 未解決エラー: `N/A`

### Definition of Done

- [ ] `familiar_ai` の純 AI ロジックが `hw_ai` へ移動済み
- [ ] root 側に残るファイルが root-only 契約を満たす
- [ ] `SharedResourceCache` 近傍 helper が `hw_logistics` に整理されている
- [ ] 影響ドキュメントが更新済み
- [ ] `CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check --workspace` が成功

---

## 10. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| `2026-03-09` | `AI (Codex)` | 初版作成 |
| `2026-03-09` | `AI (Copilot)` | コード調査結果を反映し具体化。型の hw_core 所在を確認、MessageWriter が Bevy 型であることを確認、FamiliarSoulQuery 分割方針・SpatialGridOps 活用方針・state_handlers 移設順序を追加 |
