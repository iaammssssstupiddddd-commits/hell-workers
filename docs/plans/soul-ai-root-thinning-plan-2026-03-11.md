# soul_ai Root Thinning Plan (Blocker Reassessment)

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `soul-ai-root-thinning-plan-2026-03-11` |
| ステータス | `Completed` |
| 作成日 | `2026-03-11` |
| 最終更新日 | `2026-03-11` |
| 作成者 | `AI (Copilot), AI (Codex)` |
| 関連提案 | `N/A` |
| 関連Issue/PR | `N/A` |
| 先行計画 | `docs/plans/archive/soul-ai-root-thinning-plan-2026-03-09.md` |

> この `Completed` は本計画のスコープ完了を意味する。  
> M1-M4 は実装、M5 は blocker の文書化であり、`task_execution` 全面移設や `unassign_task` crate 化の完了を意味しない。

## 1. 目的

- 解決したい課題: `src/systems/soul_ai` には、すでに下位 crate へ寄せられる実装と、本当に root に残すべき adapter が混在している。前回計画では `root 依存` の判定が広すぎ、再エクスポート越しの依存まで blocker 扱いしていた。
- 到達したい状態: `root-only` と `re-export 経由で見えているだけの依存` を区別し、先に剥がせる実装を `hw_ai` / `hw_visual` / `hw_jobs` へ段階的に寄せる。
- 成功指標:
  - `visual/*` が `hw_visual` に移る
  - `apply_task_assignment_requests_system` が root から外れる
  - `transport_common/lifecycle.rs` が root `soul_ai` 配下から外れる
  - `src/systems/soul_ai` に残る主要ファイルが「強い root blocker」を説明できる

## 2. スコープ

### 対象（In Scope）

- `src/systems/soul_ai/visual/*` の `hw_visual` への移設（M1）
- `src/systems/soul_ai/execute/task_execution/mod.rs` の assignment apply 層の `hw_ai` への移設（M2）
- `src/systems/soul_ai/execute/task_execution/transport_common/lifecycle.rs` の `hw_jobs` への移設（M3）
- `drifting` 系の `PopulationManager` 依存を event / root adapter に分離する設計と実装（M4）
- `task_execution` 全体を止めている真の blocker（construction site 型）の明文化（M5）
- 関連ドキュメントの同期

### 非対象（Out of Scope）

- `src/systems/soul_ai/execute/task_execution/**` の全面移設（construction site 型 crate 化が先決）
- `helpers/work.rs::unassign_task` の移設（`WorldMap` + `Visibility` + `WheelbarrowMovement` の多方面 blocker あり）
- `PopulationManager` や `GameAssets` 自体の crate 移動
- `FloorConstructionSite` / `WallConstructionSite` の今回中の crate 化

## 3. 型の実体所有者マップ（再確認済み）

前回計画での誤認を是正した調査結果。`crate::` パスは多くが再エクスポートであり、実体 crate は下表の通り。

| 型 / Resource | 実体ファイル | 実体 crate | root で使う際のパス |
| --- | --- | --- | --- |
| `DamnedSoul` | `crates/hw_core/src/soul.rs` | `hw_core` | `crate::entities::damned_soul::DamnedSoul` (re-export) |
| `GatheringSpot`, `GatheringVisuals` | `crates/hw_core/src/gathering.rs` | `hw_core` | `crate::systems::soul_ai::helpers::gathering::*` (re-export chain) |
| `GatheringParticipants`, `ParticipatingIn` | `crates/hw_core/src/relationships.rs` | `hw_core` | `crate::relationships::*` (re-export) |
| `RestingIn`, `RestAreaReservedFor`, `RestAreaCooldown` | `crates/hw_core/src/relationships.rs` / `soul.rs` | `hw_core` | `crate::entities::damned_soul::*` / `crate::relationships::*` |
| `DriftingState`, `IdleBehavior`, `DreamQuality`, `GatheringBehavior` | `crates/hw_core/src/soul.rs` | `hw_core` | `crate::entities::damned_soul::*` (re-export) |
| `Destination`, `Path` | `crates/hw_core/src/soul.rs` | `hw_core` | `crate::entities::damned_soul::*` (re-export) |
| `CommandedBy`, `WorkingOn`, `DeliveringTo` | `crates/hw_core/src/relationships.rs` | `hw_core` | `crate::relationships::*` (re-export) |
| `Familiar`, `ActiveCommand` | `crates/hw_core/src/familiar.rs` | `hw_core` | `crate::entities::familiar::components::*` (re-export) |
| `HoveredEntity` | `crates/hw_ui/src/selection/mod.rs` | `hw_ui` | 直接 `hw_ui::` 参照 |
| `MainCamera` | `crates/hw_ui/src/camera.rs` | `hw_ui` | 直接 `hw_ui::` 参照 |
| `OnGatheringLeft`, `OnTaskAssigned`, `OnSoulRecruited` | `crates/hw_core/src/events.rs` | `hw_core` | `crate::events::*` (re-export) |
| `GatheringSpotSpatialGrid` | `crates/hw_spatial/src/gathering.rs` | `hw_spatial` | `crate::systems::spatial::*` (re-export) |
| `IdleVisualSoulQuery`, `TaskAssignmentSoulQuery` | `crates/hw_ai/src/soul_ai/helpers/query_types.rs` | `hw_ai` | `crate::systems::soul_ai::helpers::query_types::*` (re-export) |
| `AssignedTask` | `crates/hw_jobs/src/assigned_task.rs` | `hw_jobs` | `crate::systems::soul_ai::execute::task_execution::*` (re-export) |
| `TaskAssignmentRequest` | `crates/hw_jobs/src/events.rs` | `hw_jobs` | `crate::events::TaskAssignmentRequest` (re-export) |
| `WorldMapRead` | `crates/hw_world/src/...` | `hw_world` | `crate::world::map::WorldMapRead` (re-export) |
| **`PopulationManager`** | **`src/entities/damned_soul/spawn.rs`** | **root** | **root-only** |
| **`FloorConstructionSite`** | **`src/systems/jobs/floor_construction/components.rs`** | **root** | **root-only** |
| **`WallConstructionSite`** | **`src/systems/jobs/wall_construction/components.rs`** | **root** | **root-only** |

> **重要**: `PopulationManager` / `FloorConstructionSite` / `WallConstructionSite` の 3 型のみが真の root-only。その他の `crate::` パスは調査済みの再エクスポート。

## 4. blocker 再評価

### 4.1 強い root blocker

| 対象 | 根拠（root-only 型） | 判定 |
| --- | --- | --- |
| `helpers/work.rs::unassign_task` | `WorldMapWrite`（hw_world 実体だが書き込みあり）+ `Visibility` + `hw_visual::haul::WheelbarrowMovement` を複合変更 | 強い |
| `task_execution/context/access.rs` | `StorageAccess` / `MutStorageAccess` が `FloorConstructionSite` / `WallConstructionSite` の Query を持つ。この 2 型は root-only | 強い |
### 4.2 中程度の blocker

| 対象 | 根拠（root-only 型） | 判定 | 対処案 |
| --- | --- | --- | --- |
| `decide/drifting.rs` | `PopulationManager::can_start_escape()` の読み取り + `start_escape_cooldown()` の副作用 | 中 | 操作 2 点を event 化し本体から切り離す |
| `execute/drifting.rs::despawn_at_edge_system` | `PopulationManager::total_escaped` のカウンタ更新 | 中 | `DriftingCompleted` event → root adapter で処理 |

### 4.3 blocker 根拠が弱いもの（今回移設対象 / 完了済み）

| 対象 | 見かけの依存 | 実態 | 移設先 |
| --- | --- | --- | --- |
| `visual/idle.rs`, `visual/gathering.rs`, `visual/vitals.rs` | `crate::entities::damned_soul::*` など多数 | 全て hw_core / hw_ui / hw_spatial / hw_ai の再エクスポート | `hw_visual` |
| `execute/gathering_spawn.rs` の visual spawn helper | `GameAssets` / sprite spawn | `GatheringVisualHandles` を startup で注入し、spawn 本体を `hw_visual::soul::gathering_spawn::spawn_gathering_spot` へ抽出済み | `hw_visual` |
| `apply_task_assignment_requests_system` とその直下 helper 群 | `crate::events::*`, `crate::relationships::*` など | 全て hw_core / hw_jobs / hw_logistics / hw_ai の再エクスポート。root-only 型は不使用 | `hw_ai` |
| `transport_common/lifecycle.rs` | `AssignedTask`, `ResourceReservationOp` | 完全な純粋関数（Bevy 依存なし）。全型が hw_jobs / hw_core 実体 | `hw_jobs` |

## 5. crate 依存グラフ（移設先の前提確認）

調査済みの Cargo.toml に基づく。

| crate | hw_core | hw_jobs | hw_logistics | hw_world | hw_spatial | hw_ui | hw_visual |
| --- | --- | --- | --- | --- | --- | --- | --- |
| `hw_visual` | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | — |
| `hw_ai` | ✅ | ✅ | ✅ | ✅ | ✅ | ❌ | ❌ |
| `hw_jobs` | ✅ | — | ❌ | ❌ | ❌ | ❌ | ❌ |

> `hw_ai` は `hw_ui` に依存していないため、`HoveredEntity` / `MainCamera` を使う visual 系は `hw_ai` ではなく `hw_visual` に移す。

## 6. 実装方針（高レベル）

- 方針:
  - blocker 根拠が弱いものから順に剥がす（M1 → M2 → M3 → M4 → M5）。
  - 移設時のパス変換は「`crate::xxx` を `hw_xxx::...` に書き換えるだけ」で済む箇所が多い（上記型マップ参照）。
  - `task_execution` は「全面移設」ではなく apply 層・pure helper 層のように層を切り分けて移す。
  - `drifting` は event / adapter 分離を先に行い、本体移設は後段で行う。
- 設計上の前提:
  - `hw_visual` は `hw_core`, `hw_jobs`, `hw_logistics`, `hw_spatial`, `hw_world`, `hw_ui` に依存できる（実装済み）。
  - `hw_ai` は `hw_core`, `hw_jobs`, `hw_logistics`, `hw_world`, `hw_spatial` に依存できる（実装済み）。`hw_ui` には依存不可。
  - `GameAssets` 自体は root 所有でも、必要な sprite handle は専用 resource に変換して crate 側へ渡せる（既存パターン: `handles.rs`）。
- Bevy 0.18 APIでの注意点:
  - `MessageReader<T>` / `Commands` / `Query` を使う system 自体は crate 側へ置ける。問題になるのは型所有者と resource 所有者。
  - `SystemParam` の置き場は依存している component/resource の実体所有者に合わせる。

## 7. マイルストーン

### M1: soul visual を `hw_visual` へ移設する

**移設対象 system と配置先:**

| system 関数名 | 現在地 | 移設先 |
| --- | --- | --- |
| `idle_visual_system` | `src/systems/soul_ai/visual/idle.rs` | `crates/hw_visual/src/soul/idle.rs` |
| `gathering_visual_update_system` | `src/systems/soul_ai/visual/gathering.rs` | `crates/hw_visual/src/soul/gathering.rs` |
| `gathering_debug_visualization_system` | `src/systems/soul_ai/visual/gathering.rs` | `crates/hw_visual/src/soul/gathering.rs` |
| `familiar_hover_visualization_system` | `src/systems/soul_ai/visual/vitals.rs` | `crates/hw_visual/src/soul/vitals.rs` |

**依存型と移設後のインポートパス変換:**

| 型 | 移設前（root re-export） | 移設後（実体 crate） |
| --- | --- | --- |
| `DamnedSoul` | `crate::entities::damned_soul::DamnedSoul` | `hw_core::soul::DamnedSoul` |
| `GatheringSpot`, `GatheringVisuals` | `crate::systems::soul_ai::helpers::gathering::*` | `hw_core::gathering::*` |
| `GatheringParticipants`, `ParticipatingIn` | `crate::relationships::*` | `hw_core::relationships::*` |
| `HoveredEntity` | (直接 `hw_ui::`) | `hw_ui::selection::HoveredEntity` |
| `MainCamera` | (直接 `hw_ui::`) | `hw_ui::camera::MainCamera` |
| `Familiar`, `ActiveCommand` | `crate::entities::familiar::components::*` | `hw_core::familiar::*` |
| `CommandedBy` | `crate::relationships::CommandedBy` | `hw_core::relationships::CommandedBy` |
| `GatheringSpotSpatialGrid` | `crate::systems::spatial::*` | `hw_spatial::gathering::GatheringSpotSpatialGrid` |
| `IdleVisualSoulQuery` | `crate::systems::soul_ai::helpers::query_types::*` | `hw_ai::soul_ai::helpers::query_types::*` |
| `GATHERING_LEAVE_RADIUS`, `GATHERING_ARRIVAL_RADIUS_BASE` | `crate::systems::soul_ai::helpers::gathering::*` | `hw_core::constants::*` (要確認) |

> `hw_visual` は `hw_ai` に依存できない（循環回避）。`IdleVisualSoulQuery` を使う場合は `hw_visual::Cargo.toml` への `hw_ai` 追加が必要かどうかを事前確認すること。代替として query alias を `hw_visual` 内で再定義する。

**変更ファイル:**

- `src/systems/soul_ai/visual/idle.rs` → 内容を `crates/hw_visual/src/soul/idle.rs` へ移動後に削除
- `src/systems/soul_ai/visual/gathering.rs` → 内容を `crates/hw_visual/src/soul/gathering.rs` へ移動後に削除
- `src/systems/soul_ai/visual/vitals.rs` → 内容を `crates/hw_visual/src/soul/vitals.rs` へ移動後に削除
- `src/systems/soul_ai/visual/mod.rs` → 削除（または空の thin re-export に変更）
- `crates/hw_visual/src/soul.rs` → submodule 構成（`mod idle; mod gathering; mod vitals;`）に変更
- `crates/hw_visual/src/lib.rs` → 上記 system を Plugin 登録に追加
- `src/plugins/visual.rs` → 移設済み system の登録行を削除（hw_visual 側 Plugin に委譲）

**`src/plugins/visual.rs` で削除する行（現状確認済み）:**
```
// 以下の system 登録を削除:
idle_visual_system                      (chain の途中)
familiar_hover_visualization_system     (chain の途中)
gathering_visual_update_system          (chain の途中)
gathering_debug_visualization_system    (chain の途中)
```

**完了条件:**
- [x] root `visual/idle.rs`, `visual/gathering.rs`, `visual/vitals.rs` が削除または thin re-export になっている
- [x] `src/plugins/visual.rs` から上記 4 system の登録行が消えている
- [x] `crates/hw_visual/src/lib.rs` の Plugin build で上記 4 system が登録されている
- [x] `cargo check --workspace` が成功する

---

### M2: assignment apply 層を `hw_ai` へ移設する

**移設対象関数と配置先 (`crates/hw_ai/src/soul_ai/execute/task_assignment_apply.rs` を新規作成):**

| 関数名 | 役割 | root-only 依存 |
| --- | --- | --- |
| `apply_task_assignment_requests_system` | system 本体 | **なし**（全依存型は hw_* 実体） |
| `normalize_worker_idle_state` | idle 状態正規化 | **なし**（`Visibility` は Bevy 型、他は hw_core） |
| `prepare_worker_for_task_apply` | ワーカー準備（リレーション付与） | **なし**（`CommandedBy`, `WorkingOn` は hw_core） |
| `worker_can_receive_assignment` | 割り当て可否チェック | **なし**（`AssignedTask`, `IdleState` は hw_*） |
| `apply_assignment_state` | AssignedTask/Destination/Path 更新 | **なし** |
| `apply_assignment_reservations` | リソース予約適用 | **なし**（`SharedResourceCache` は hw_logistics） |
| `attach_delivering_to_relationship` | DeliveryTo リレーション付与 | **なし**（`DeliveringTo` は hw_core） |
| `trigger_task_assigned_event` | `OnTaskAssigned` event 発行 | **なし**（hw_core イベント） |

**依存型と移設後のインポートパス変換:**

| 旧パス（root re-export） | 新パス（実体 crate 直接参照） |
| --- | --- |
| `crate::events::{OnGatheringLeft, OnSoulRecruited, OnTaskAssigned}` | `hw_core::events::{OnGatheringLeft, OnSoulRecruited, OnTaskAssigned}` |
| `crate::events::TaskAssignmentRequest` | `hw_jobs::events::TaskAssignmentRequest` |
| `crate::systems::soul_ai::helpers::query_types::TaskAssignmentSoulQuery` | `crate::soul_ai::helpers::query_types::TaskAssignmentSoulQuery` (hw_ai 内パス) |
| `crate::relationships::{ParticipatingIn, RestingIn, RestAreaReservedFor, CommandedBy, WorkingOn, DeliveringTo}` | `hw_core::relationships::*` |
| `crate::entities::damned_soul::{DamnedSoul, IdleState, RestAreaCooldown, DriftingState}` | `hw_core::soul::{DamnedSoul, IdleState, RestAreaCooldown, DriftingState}` |
| `crate::world::map::WorldMapRead` | (この関数では直接不使用—要確認) |

> **注意**: `normalize_worker_idle_state` の引数に `With<DamnedSoul>` が含まれるが `DamnedSoul` は hw_core なので問題なし。

**変更ファイル:**

- `src/systems/soul_ai/execute/task_execution/mod.rs` → `apply_task_assignment_requests_system` と直下 helper 群を削除し、`hw_ai` から再エクスポート
- `crates/hw_ai/src/soul_ai/execute/task_assignment_apply.rs` → **新規作成**（移設した関数群）
- `crates/hw_ai/src/soul_ai/execute/mod.rs` → `pub mod task_assignment_apply;` 追加
- `crates/hw_ai/src/soul_ai/mod.rs` → `SoulAiCorePlugin::build` の Execute フェーズに `execute::task_assignment_apply::apply_task_assignment_requests_system` を登録
- `src/plugins/logic.rs` (または該当 plugin) → root 側の登録行を削除

**完了条件:**
- [x] root `task_execution/mod.rs` に `apply_task_assignment_requests_system` 本体が残っていない
- [x] `hw_ai::SoulAiCorePlugin` の Execute フェーズで登録されている
- [x] root 側の二重登録が削除され、ordering 参照のみが残っている
- [x] `cargo check --workspace` が成功する

---

### M3: `transport_common/lifecycle.rs` を `hw_jobs` へ移設する

**移設対象関数:**

| 関数名 | シグネチャ（概略） | 純粋性 |
| --- | --- | --- |
| `collect_active_reservation_ops` | `(task: &AssignedTask, resolve_wheelbarrow_item_type: impl FnMut(Entity, ResourceType) -> ResourceType) -> Vec<ResourceReservationOp>` | ✅ 完全純粋（Bevy 依存なし） |
| `collect_release_reservation_ops` | 同上シグネチャ | ✅ 完全純粋 |

**移設先: `crates/hw_jobs/src/lifecycle.rs`（新規作成）**

理由: 両関数は `AssignedTask`（hw_jobs 実体）と `ResourceReservationOp`（hw_logistics 実体）のみに依存する pure helper であり、AI 実装層（hw_ai）の関心事ではなくタスク型ライブラリ（hw_jobs）の責務に近い。

> ただし `ResourceReservationOp` が `hw_logistics` 実体の場合、`hw_jobs` は `hw_logistics` に依存できないため `hw_ai` が移設先になる可能性がある。**実装前に `hw_logistics::ResourceReservationOp` の定義 crate と `hw_jobs/Cargo.toml` の依存リストを確認すること。**

**呼び出し元のパス変更:**

| ファイル | 旧パス | 新パス |
| --- | --- | --- |
| `src/systems/familiar_ai/perceive/resource_sync.rs` | `crate::systems::soul_ai::execute::task_execution::transport_common::lifecycle` | `hw_jobs::lifecycle` （または `hw_ai::soul_ai::...`） |
| `src/systems/soul_ai/helpers/work.rs` | 同上 | 同上 |

**変更ファイル:**

- `src/systems/soul_ai/execute/task_execution/transport_common/lifecycle.rs` → 削除（または thin re-export に変更）
- `crates/hw_jobs/src/lifecycle.rs` → **新規作成**（または `hw_ai` 側に作成）
- `crates/hw_jobs/src/lib.rs` → `pub mod lifecycle;` 追加
- `src/systems/familiar_ai/perceive/resource_sync.rs` → import パス変更
- `src/systems/soul_ai/helpers/work.rs` → import パス変更

**完了条件:**
- [x] `collect_active_reservation_ops` / `collect_release_reservation_ops` が root `soul_ai` 配下にない
- [x] `resource_sync.rs` と `helpers/work.rs` が同じ新パスを参照している
- [x] `cargo check --workspace` が成功する

---

### M4: drifting の `PopulationManager` 依存を event/adapter に分離する

**現状の依存箇所（調査済み）:**

| ファイル | 使用箇所 | 操作内容 |
| --- | --- | --- |
| `src/systems/soul_ai/decide/drifting.rs` L72 | `population.can_start_escape()` | 読み取り（bool 返却） |
| `src/systems/soul_ai/decide/drifting.rs` L134 | `population.start_escape_cooldown()` | 書き込み（cooldown 開始） |
| `src/systems/soul_ai/execute/drifting.rs` L179 | `population.total_escaped += 1` | 書き込み（脱出カウンタ更新） |

**分離方針（event 化の具体案）:**

```
1. decide/drifting.rs の changes:
   - population.can_start_escape() → Res<PopulationManager> を残しつつ
     start_escape_cooldown() の呼び出しを DriftingEscapeStarted イベント発行に変える
   - DriftingEscapeStarted を hw_core::events に追加（PopulationManager 非依存の空イベント）

2. execute/drifting.rs の changes:
   - population.total_escaped += 1 → SoulEscaped イベント発行に変える
   - SoulEscaped を hw_core::events に追加

3. root adapter（新規または既存 plugin）:
   - on_drifting_escape_started: DriftingEscapeStarted を受信 → PopulationManager::start_escape_cooldown() 呼出
   - on_soul_escaped: SoulEscaped を受信 → population.total_escaped += 1
```

**event 化後の system 引数（drifting_decision_system）:**

```
Before: Res<Time>, Commands, ResMut<DriftingDecisionTimer>, ResMut<PopulationManager>, Query<...>
After:  Res<Time>, Commands, ResMut<DriftingDecisionTimer>, Res<PopulationManager>, Query<...>, EventWriter<DriftingEscapeStarted>
```

> `can_start_escape()` は読み取りのみなので `Res<PopulationManager>` として残す。書き込みを event 化することで hw_ai 側への依存排除が可能になる。

**変更ファイル:**

- `crates/hw_core/src/events.rs` → `DriftingEscapeStarted`, `SoulEscaped` event を追加
- `src/systems/soul_ai/decide/drifting.rs` → `ResMut<PopulationManager>` を `Res<PopulationManager>` に変更、`start_escape_cooldown()` を event 発行に変更
- `src/systems/soul_ai/execute/drifting.rs` → `total_escaped` 更新を `SoulEscaped` event 発行に変更
- `src/plugins/...` (root adapter) → `on_drifting_escape_started` / `on_soul_escaped` observer/system を登録

**完了条件:**
- [x] `drifting_decision_system` が `ResMut<PopulationManager>` を持たない（`Res<>` に格下げ）
- [x] `despawn_at_edge_system` が `ResMut<PopulationManager>` を持たない（event 発行に変更）
- [x] `PopulationManager` への書き込みは root adapter のみ
- [x] `cargo check --workspace` が成功する

---

### M5: task_execution 全面移設 blocker を明文化する

**blocker の実体（調査済み）:**

`src/systems/soul_ai/execute/task_execution/context/access.rs` が `StorageAccess` / `MutStorageAccess` として下記 root-only 型の Query を保持している:

```rust
// StorageAccess（行89-110）
floor_sites: Query<(&FloorConstructionSite, &TaskWorkers)>
wall_sites:  Query<(&WallConstructionSite, &TaskWorkers)>

// MutStorageAccess（行157-178）
floor_sites: Query<(&mut FloorConstructionSite, &TaskWorkers)>
wall_sites:  Query<(&mut WallConstructionSite, &TaskWorkers)>
```

`FloorConstructionSite` は `src/systems/jobs/floor_construction/components.rs`、`WallConstructionSite` は `src/systems/jobs/wall_construction/components.rs` に root-only で定義されている。

**ドキュメント化の内容:**

- `FloorConstructionSite` / `WallConstructionSite` が `hw_jobs` へ移設されるまで `context/access.rs` は root に残る必要があること
- 移設が可能になる前提条件（これらの型を crate 化する別計画）
- `task_execution` 全面移設を行う次計画のための前提条件リスト

**変更ファイル:**

- `docs/cargo_workspace.md` → `FloorConstructionSite` / `WallConstructionSite` の crate 化待ち状態を記載
- `docs/soul_ai.md` → `task_execution` 深部の移設不能理由と次計画の前提条件を記載

**完了条件:**
- [x] `task_execution` を今すぐ移せない理由が docs に明文化されている
- [x] 次計画（construction site 型 crate 化）の前提条件が明確になっている

---

## 8. リスクと対策

| リスク | 影響 | 対策 |
| --- | --- | --- |
| M1: `hw_visual` が `hw_ai` に依存できず `IdleVisualSoulQuery` を使えない | idle_visual_system の Query 型を再定義する必要が生じる | 事前に Cargo.toml の依存方向を確認し、必要なら query alias を hw_visual 内で再定義する |
| M2: `normalize_worker_idle_state` の `Visibility` 操作が hw_ai では不自然 | hw_ai に visual 依存が混入する | Bevy の `Visibility` は標準型なので依存追加不要。問題なし |
| M3: `ResourceReservationOp` が hw_logistics 実体で hw_jobs から参照できない | lifecycle.rs を hw_jobs に置けない | `hw_ai/src/soul_ai/transport/lifecycle.rs` に変更する。実装前に依存グラフを確認する |
| M4: drifting を一気に本体移設しようとして `Res<PopulationManager>` で詰まる | 作業が中断する | M4 は「書き込み event 化」のみ。本体移設は別計画 |
| M5 以降: construction site 型 crate 化が他計画で遅れる | task_execution 全面移設が長期停滞する | 本計画内では明文化のみ。移設は別計画に完全委譲する |

## 9. 検証計画

**必須（各マイルストーン完了ごと）:**
```bash
CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check
```

**手動確認シナリオ:**

| シナリオ | 対応マイルストーン |
| --- | --- |
| Soul の idle 色変化・集会 visual・使い魔 hover line が従来どおり表示される | M1 |
| Familiar が発行した `TaskAssignmentRequest` が従来どおり Soul に適用される | M2 |
| タスク割り当て後の reservation sync が壊れない | M2, M3 |
| unassign/task 完了後の reservation 解放が正常に機能する | M3 |
| drifting 中の Soul が edge 到達で従来どおり脱走し、escaped カウントが正常に増加する | M4 |

**パフォーマンス確認（M1 完了後）:**
- Visual set の system 数増減と実行順序が従来と同一であること

## 10. ロールバック方針

- マイルストーン単位で独立して revert 可能（各 M は前の M に依存するが、コード変更は局所的）
- 戻す時の手順:
  1. 直近マイルストーン単位で `git revert` または `git checkout -- <files>` する
  2. `docs/cargo_workspace.md` と `docs/soul_ai.md` を同時に戻す
  3. `CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check` を再実行する

## 11. AI引継ぎメモ（最重要）

### 現在地

- 進捗: `100%`（本計画スコープ完了）
- 完了済みマイルストーン: M1・M2・M3・M4・M5 すべて完了
- 完了済み:
  - **M1**: `idle_visual_system`, `gathering_visual_update_system`, `gathering_debug_visualization_system`, `familiar_hover_visualization_system` を `hw_visual::soul::*` へ移設。root `visual/*.rs` は thin re-export。`HwVisualPlugin` で登録済み。
  - **M2**: `apply_task_assignment_requests_system` と直下 helper 群を `hw_ai::soul_ai::execute::task_assignment_apply` へ移設。root `task_execution/mod.rs` は re-export のみ。`SoulAiCorePlugin` で登録済み。
  - **M3**: `collect_active_reservation_ops` / `collect_release_reservation_ops` を `hw_jobs::lifecycle` へ移設。root `transport_common/lifecycle.rs` は thin re-export。
  - **M4**: `drifting_decision_system` の `ResMut<PopulationManager>` を `Res<>` に格下げ、`DriftingEscapeStarted` イベント発行に変更。`despawn_at_edge_system` の `total_escaped` 書き込みを `SoulEscaped` イベント発行に変更。root adapter (`adapters.rs`) が両イベントを受信して `PopulationManager` を更新。
  - **M5**: `docs/soul_ai.md` と `docs/cargo_workspace.md` に `FloorConstructionSite` / `WallConstructionSite` が blocker である理由と次計画の前提条件を明文化済み。
  - **追加（計画外）**: `choose_drift_edge`, `is_near_map_edge`, `random_wander_target`, `drift_move_target` の 4 pure 関数を `hw_ai::soul_ai::helpers::drifting` へ抽出。root drifting 側は hw_ai の関数を呼び出すのみ。
- 未完了（別計画へ委譲）:
  - `task_execution` 深部は `FloorConstructionSite` / `WallConstructionSite` が blocker（別計画で construction site 型を hw_jobs に移設後に対応）
  - `unassign_task` は `Commands`・root 型依存のため root 残留

### 次計画で扱う課題

1. `FloorConstructionSite` / `WallConstructionSite` を `hw_jobs` へ移設する（`TaskArea` 依存の除去が前提）。
2. 移設後、`task_execution/context/access.rs` を hw_ai へ移設できるかを検討する。
3. `unassign_task` の root 残留を別途依存分離するかを検討する。

### ブロッカー/注意点

- `FloorConstructionSite` / `WallConstructionSite` は root 所有（`src/systems/jobs/*/components.rs`）。`task_execution/context/access.rs` を動かすには crate 化が必要で、それは別計画。
- `WorldMapRead`, `PathfindingContext` は root blocker ではない。これを理由に移設を止めない。

### 最終確認ログ

- 最終 `cargo check --workspace`: `2026-03-11` / `pass` / 警告ゼロ・エラーゼロ
- 未解決エラー: なし

### Definition of Done

- [x] M1–M4 が完了している（M5 はドキュメント化のみ）
- [x] 境界ドキュメント（`docs/cargo_workspace.md`, `docs/soul_ai.md`）が現状に同期している
- [x] crate README と root README が現状に同期している
- [x] `cargo check --workspace` が成功

## 12. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| `2026-03-11` | `AI (Copilot)` | M1-M5 完了確認。AI引継ぎメモを 0%→100% に修正。drifting ヘルパー抽出（計画外）を記録 |
| `2026-03-11` | `AI (Codex)` | 本計画スコープ完了を反映。M1-M4 実装 + M5 blocker 文書化までを Completed とし、全面移設は別計画である旨を明記 |
| `2026-03-11` | `AI (Codex)` | 初版作成 |
| `2026-03-10` | `AI (Copilot)` | コード実態調査に基づきブラッシュアップ。型所有者マップ・パス変換表・具体的変更ファイルリスト・event化の設計案を追加 |
