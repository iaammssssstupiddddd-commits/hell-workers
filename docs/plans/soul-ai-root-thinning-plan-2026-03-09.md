# soul_ai を段階的に crate へ寄せて root を薄くする計画

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `soul-ai-root-thinning-plan-2026-03-09` |
| ステータス | `Draft (コード調査済み)` |
| 作成日 | `2026-03-09` |
| 最終更新日 | `2026-03-09` |
| 作成者 | `AI (Codex)` |
| 更新者 | `AI (Copilot)` |
| 関連提案 | `N/A` |
| 関連Issue/PR | `N/A` |

## 1. 目的

- **解決したい課題**: `src/systems/soul_ai` には、すでに `hw_ai` へ移せる Decide/Execute ロジックと、root にしか置けない adapter が混在している。さらに module tree に入っていない stale file も残っており、crate 境界の判断を難しくしている。
- **到達したい状態**: `hw_ai` には shared model と request/message だけで閉じる Soul AI ロジックを集約し、`hw_spatial` には shared model を対象にした空間グリッドを寄せる。root の `soul_ai` は plugin wiring、`GameAssets`/UI/Gizmo 依存、`PopulationManager` 依存、`task_execution` のような root-only adapter に縮退する。
- **成功指標**:
  - `auto_refine` と `auto_build` が `hw_ai` に移っている
  - `GatheringSpot` 系 shared model の置き場と `GatheringSpotSpatialGrid` の責務が下位 crate に揃っている
  - `src/systems/soul_ai` に残るファイルが root-only 契約を満たしている
  - stale file が削除され、`cargo check -p hw_ai` と `cargo check --workspace` が成功する

---

## 2. スコープ

### 対象（In Scope）

- `src/systems/soul_ai` の stale file 整理
- `decide/work/auto_refine.rs` の `hw_ai` への移設
- `decide/work/auto_build.rs` の `hw_ai` への移設
- `GatheringSpot` 系 shared model の下位 crate への移設
- `GatheringSpotSpatialGrid` の `hw_spatial` への移設
- `idle_behavior_decision_system` を `hw_ai` へ移すための前提整理
- `gathering_spawn` を pure logic と root adapter に分割する設計整理
- `docs/cargo_workspace.md`, `docs/soul_ai.md`, `src/systems/soul_ai/README.md`, `crates/hw_ai/README.md`, `crates/hw_spatial/README.md` の同期

### 非対象（Out of Scope）

- `src/systems/soul_ai/execute/task_execution/**` の全面移設
- `src/systems/soul_ai/helpers/work.rs::unassign_task` の移設
- `src/systems/soul_ai/decide/drifting.rs` / `src/systems/soul_ai/execute/drifting.rs` の移設
- `src/systems/soul_ai/visual/**` の `hw_ui` 化
- `PopulationManager` や `GameAssets` 自体の crate 移動
- gameplay アルゴリズムの変更

---

## 3. 現状調査結果

### 3.1 すでに `hw_ai` 側へ移っている領域

以下は root 側に shell だけが残っている、または `mod.rs` で inline re-export されている。

| 領域 | 実体 |
| --- | --- |
| `decide/escaping.rs` | `hw_ai::soul_ai::decide::escaping` |
| `decide/gathering_mgmt.rs` | `hw_ai::soul_ai::decide::gathering_mgmt` |
| `decide/idle_behavior/{exhausted_gathering,motion_dispatch,rest_area,rest_decision,task_override,transitions}` | `hw_ai::soul_ai::decide::idle_behavior::*` |
| `perceive/escaping.rs` | `hw_ai::soul_ai::perceive::escaping` |
| `update/vitals_influence.rs` | `hw_ai::soul_ai::update::vitals_influence` |
| `helpers/gathering.rs`, `helpers/gathering_positions.rs` | `hw_ai::soul_ai::helpers::*` |
| `execute/gathering_apply.rs` | `hw_ai::soul_ai::execute::gathering_apply` |
| `execute/escaping_apply`, `execute/idle_behavior_apply` | `src/systems/soul_ai/execute/mod.rs` で inline re-export |

### 3.2 stale file と重複の疑いが強いもの（コード調査で確認済み）

Rust では `mod.rs` 内の inline module 定義がファイルより優先されるため、以下 4 件は実際にコンパイル対象外になっている。

| ファイル | shadowing している inline module | 備考 |
| --- | --- | --- |
| `src/systems/soul_ai/decide/separation.rs` | `decide/mod.rs`: `pub mod separation { pub use hw_ai::soul_ai::decide::separation::*; }` | 旧版は `WorldMapRead` を使用。hw_ai 正式版は `Res<WorldMap>` に更新済み |
| `src/systems/soul_ai/execute/escaping_apply.rs` | `execute/mod.rs`: `pub mod escaping_apply { pub use hw_ai::soul_ai::execute::escaping_apply::*; }` | `EscapeRequest` を処理する完全実装が残存するが未参照 |
| `src/systems/soul_ai/execute/idle_behavior_apply.rs` | `execute/mod.rs`: `pub mod idle_behavior_apply { pub use hw_ai::soul_ai::execute::idle_behavior_apply::*; }` | `IdleBehaviorRequest` を処理する完全実装が残存するが未参照 |
| `src/systems/soul_ai/helpers/query_types.rs` | `helpers/mod.rs`: `pub mod query_types { pub use hw_ai::soul_ai::helpers::query_types::*; }` | `AutoBuildSoulQuery` など 8 種の型エイリアスが残存するが未参照 |

> **重要**: 4 ファイルを削除しても `cargo check --workspace` には影響しない（参照されていないため）。削除前後に上記 inline module が `mod.rs` に残っていることを `rg` で確認すること。

### 3.3 そのまま下位 crate に寄せやすい候補（調査結果を反映）

| ファイル | 目標 crate | 実際に使っている root 型 | 真の状態 |
| --- | --- | --- | --- |
| `src/systems/soul_ai/decide/work/auto_refine.rs` | `hw_ai` | なし（下記 §3.5 参照）| **即移設可能** |
| `src/systems/soul_ai/decide/work/auto_build.rs` | `hw_ai` | なし（下記 §3.5 参照）| **即移設可能** |
| `src/systems/soul_ai/decide/idle_behavior/mod.rs` (`idle_behavior_decision_system`) | `hw_ai` | `WorldMapRead`（root wrapper）, `GatheringSpotSpatialGrid`（M3 後に解決）| **M3 完了後に移設可能** |
| `src/systems/soul_ai/execute/gathering_spawn.rs` の判定ロジック | `hw_ai` | `GameAssets`（visual spawn 部分のみ）| **M3 完了後にロジック分離可能** |
| `src/systems/spatial/gathering.rs` | `hw_spatial` | `GatheringSpot`（現在 hw_ai にある）| **M3 の GatheringSpot 移設で解決** |

### 3.4 主なブロッカー

1. **`GatheringSpotSpatialGrid` が hw_spatial に入れない問題**  
   `GatheringSpot` は現在 `hw_ai::soul_ai::helpers::gathering` にある。  
   `hw_spatial` が `hw_ai` に依存すると循環依存になるため、`GatheringSpot` を `hw_core` に降ろしてから `hw_spatial` に `gathering.rs` を追加する必要がある。

2. **`idle_behavior_decision_system` の `WorldMapRead` 依存**  
   `WorldMapRead` は root の `src/world/map/access.rs` にある SystemParam wrapper。  
   hw_ai 版 `separation.rs` はすでに `Res<hw_world::WorldMap>` に書き直されており、  
   `idle_behavior_decision_system` も同様に `WorldMapRead` → `Res<WorldMap>` に置換することで hw_ai へ移せる。

3. **`gathering_spawn_system` が `GameAssets` に依存**  
   判定ロジック（`GatheringReadiness` の tick・発生判定）と visual 生成（`GameAssets` を使うスプライト生成）が同居している。  
   `GatheringSpawnRequest` メッセージを新設し、pure 判定を hw_ai が emit・root adapter が consume する構造にする。

4. **`docs/cargo_workspace.md` の記述が現コードとズレている**  
   `GatheringSpot` をまだ root 固有型として扱っている（実際は hw_ai にある）。M3 で hw_core に移した時点で一緒に修正する。

### 3.5 auto_refine・auto_build の依存型解決（コード調査結果）

初版計画では「shared 型だけで閉じる」と記述したが、具体的な型ごとに確認した結果を示す。

**`auto_refine.rs` の全 import → 実際の定義元**

| `crate::` パス | 実際の定義元 | hw_ai から使う方法 |
| --- | --- | --- |
| `entities::familiar::{ActiveCommand, FamiliarCommand}` | `hw_core::familiar` | `hw_core::familiar::*` |
| `events::{DesignationOp, DesignationRequest}` | `hw_core::events` | `hw_core::events::*` |
| `relationships::{StoredItems, TaskWorkers}` | `hw_core::relationships` | `hw_core::relationships::*` |
| `systems::command::TaskArea` | `hw_core::area::TaskArea` | `hw_core::area::TaskArea` |
| `systems::jobs::{Designation, MudMixerStorage, WorkType}` | `hw_jobs::{Designation, MudMixerStorage}`, `hw_core::jobs::WorkType` | `hw_jobs::*`, `hw_core::jobs::WorkType` |
| `systems::logistics::{ResourceType, Stockpile}` | `hw_core::logistics::ResourceType`, `hw_logistics::zone::Stockpile` | `hw_core::logistics::ResourceType`, `hw_logistics::zone::Stockpile` |
| `soul_ai::execute::task_execution::AssignedTask` | **`hw_jobs::AssignedTask`**（`types.rs` は `pub use hw_jobs::assigned_task::*`） | `hw_jobs::AssignedTask` |
| `soul_ai::execute::task_execution::move_plant::MovePlanned` | **`hw_jobs::MovePlanned`**（`move_plant.rs` は `pub use hw_jobs::MovePlanned`） | `hw_jobs::MovePlanned` |
| `hw_core::constants::MUD_MIXER_REFINE_PRIORITY` | `hw_core::constants` | そのまま |

**`auto_build.rs` の全 import → 実際の定義元**

| `crate::` パス | 実際の定義元 | hw_ai から使う方法 |
| --- | --- | --- |
| `entities::damned_soul::StressBreakdown` | `hw_core::soul::StressBreakdown` | `hw_core::soul::StressBreakdown` |
| `entities::familiar::{ActiveCommand, FamiliarCommand}` | `hw_core::familiar` | `hw_core::familiar::*` |
| `events::TaskAssignmentRequest` | `hw_jobs::events::TaskAssignmentRequest` | `hw_jobs::events::TaskAssignmentRequest` |
| `relationships::{Commanding, ManagedBy, TaskWorkers}` | `hw_core::relationships` | `hw_core::relationships::*` |
| `systems::command::TaskArea` | `hw_core::area::TaskArea` | `hw_core::area::TaskArea` |
| `systems::jobs::{Blueprint, Designation, Priority, TaskSlots, WorkType}` | `hw_jobs::{Blueprint, Designation, Priority, TaskSlots}`, `hw_core::jobs::WorkType` | `hw_jobs::*` |
| `soul_ai::execute::task_execution::types::{AssignedTask, BuildData, BuildPhase}` | **`hw_jobs::assigned_task::*`** | `hw_jobs::{AssignedTask, BuildData, BuildPhase}` |
| `soul_ai::helpers::query_types::AutoBuildSoulQuery` | `hw_ai::soul_ai::helpers::query_types` | `crate::soul_ai::helpers::query_types::AutoBuildSoulQuery` |
| `soul_ai::helpers::work as helpers` | root `helpers/work.rs` だが `is_soul_available_for_work` は `hw_ai::soul_ai::helpers::work` にも存在 | `crate::soul_ai::helpers::work as helpers` |
| `systems::spatial::BlueprintSpatialGrid` | `hw_spatial::BlueprintSpatialGrid` | `hw_spatial::BlueprintSpatialGrid` |

**結論: `auto_refine` / `auto_build` の root-only 型への依存はゼロ**。`AssignedTask`・`MovePlanned` はすでに `hw_jobs` にあり、root `task_execution/types.rs` はただの re-export。`hw_ai` の `Cargo.toml` はすでに `hw_core`, `hw_jobs`, `hw_logistics`, `hw_spatial` に依存しており、追加 crate 依存なしで移設できる。

---

## 4. 実装方針（高レベル）

### 4.1 root-only 契約

今後 `src/systems/soul_ai` に残してよいのは、次のいずれかを満たすものだけとする。

| 残留条件 | 例 |
| --- | --- |
| `GameAssets` や sprite/entity spawn に直接依存する | `gathering_spawn` の visual adapter |
| UI / camera / gizmo 依存 | `visual/gathering.rs`, `visual/vitals.rs` |
| `PopulationManager` や root 固有 resource を直接読む | `decide/drifting.rs`, `execute/drifting.rs` |
| `task_execution` の full-fat query / unassign 副作用を持つ | `execute/task_execution/**`, `helpers/work.rs` |

逆に、shared model・shared events・`hw_world::WorldMap`・`hw_spatial` の resource だけで閉じるものは `hw_ai` または `hw_spatial` へ寄せる。

### 4.2 target 構成

```text
crates/hw_core
  └─ gathering shared model
     - GatheringSpot
     - GatheringObjectType
     - gathering constants / helper

crates/hw_spatial
  └─ GatheringSpotSpatialGrid

crates/hw_ai
  └─ soul_ai
     - decide/work/{auto_build, auto_refine}
     - decide/idle_behavior/mod.rs
     - execute/gathering_spawn core logic

src/systems/soul_ai
  └─ root shell / adapter
     - plugin wiring
     - execute/task_execution/**
     - execute/drifting.rs
     - helpers/work.rs
     - visual/**
     - gathering visual spawn adapter
```

### 4.3 設計上の前提（コード調査で確認済み）

- `AssignedTask` の実体は `crates/hw_jobs/src/assigned_task.rs` にある。root `task_execution/types.rs` は `pub use hw_jobs::assigned_task::*` の 1 行のみ
- `MovePlanned` の実体は `hw_jobs` にある。`move_plant.rs` は `pub use hw_jobs::MovePlanned` の 1 行のみ
- `StressBreakdown` は `hw_core::soul` にある（root `entities/damned_soul` は re-export）
- `TaskAssignmentRequest` は `hw_jobs::events`、`DesignationRequest` は `hw_core::events`
- `hw_ai` の `Cargo.toml` はすでに `hw_core`, `hw_jobs`, `hw_logistics`, `hw_spatial`, `hw_world` に依存しており、**M2 には追加 crate 依存が不要**
- `SoulAiCorePlugin` は `hw_core::system_sets::SoulAiSystemSet` を使えるため、Decide 系 system の登録先を `hw_ai` に広げられる

### 4.4 Bevy 0.18 API での注意点

- `MessageWriter<T>` / `MessageReader<T>` を使う system はそのまま `hw_ai` に置ける。message 登録自体は root の `MessagesPlugin` が持つ
- `WorldMapRead` は root wrapper。`hw_ai` では `Res<WorldMap>` を使う（hw_ai 版 `separation.rs` が先行事例）
- `Added<GatheringSpot>` による grid 更新は維持し、`Changed<GatheringSpot>` による過剰発火を避ける（既存コメントに明記済み）

---

## 5. マイルストーン

### M1: stale file を整理し、root-only 契約を固定する

- **変更内容**:
  - module tree に入っていない stale file を削除する
  - `src/systems/soul_ai/README.md` に root-only 契約を明記する
  - `mod.rs` で inline re-export している箇所と file shell の使い分けを統一する

- **変更ファイル**:
  - `src/systems/soul_ai/decide/separation.rs`
  - `src/systems/soul_ai/execute/escaping_apply.rs`
  - `src/systems/soul_ai/execute/idle_behavior_apply.rs`
  - `src/systems/soul_ai/helpers/query_types.rs`
  - `src/systems/soul_ai/README.md`

- **完了条件**:
  - [ ] stale file 4 件が削除されている
  - [ ] `src/systems/soul_ai` に残す条件が README で明文化されている
  - [ ] module tree から外れた `soul_ai` ファイルが残っていない

- **検証**:
  ```bash
  cargo check --workspace
  rg -n "pub mod separation \\{|pub mod query_types \\{|pub mod escaping_apply \\{|pub mod idle_behavior_apply \\{" src/systems/soul_ai
  ```

---

### M2: `auto_refine` と `auto_build` を `hw_ai` へ移す

- **変更内容**:
  1. `crates/hw_ai/src/soul_ai/decide/work/` ディレクトリを新規作成
  2. `auto_refine.rs` を hw_ai に移設し、import を以下のように書き換える:
     - `crate::entities::familiar::*` → `hw_core::familiar::*`
     - `crate::events::{DesignationOp, DesignationRequest}` → `hw_core::events::*`
     - `crate::relationships::{StoredItems, TaskWorkers}` → `hw_core::relationships::*`
     - `crate::systems::command::TaskArea` → `hw_core::area::TaskArea`
     - `crate::systems::jobs::{Designation, MudMixerStorage}` → `hw_jobs::{Designation, MudMixerStorage}`
     - `crate::systems::logistics::{ResourceType, Stockpile}` → `hw_core::logistics::ResourceType`, `hw_logistics::zone::Stockpile`
     - `crate::systems::soul_ai::execute::task_execution::AssignedTask` → `hw_jobs::AssignedTask`
     - `crate::systems::soul_ai::execute::task_execution::move_plant::MovePlanned` → `hw_jobs::MovePlanned`
  3. `auto_build.rs` を hw_ai に移設し、import を以下のように書き換える:
     - `crate::entities::damned_soul::StressBreakdown` → `hw_core::soul::StressBreakdown`
     - `crate::entities::familiar::*` → `hw_core::familiar::*`
     - `crate::events::TaskAssignmentRequest` → `hw_jobs::events::TaskAssignmentRequest`
     - `crate::relationships::{Commanding, ManagedBy, TaskWorkers}` → `hw_core::relationships::*`
     - `crate::systems::command::TaskArea` → `hw_core::area::TaskArea`
     - `crate::systems::jobs::{Blueprint, Designation, Priority, TaskSlots}` → `hw_jobs::*`
     - `crate::systems::soul_ai::execute::task_execution::types::{AssignedTask, BuildData, BuildPhase}` → `hw_jobs::{AssignedTask, BuildData, BuildPhase}`
     - `crate::systems::soul_ai::helpers::query_types::AutoBuildSoulQuery` → `crate::soul_ai::helpers::query_types::AutoBuildSoulQuery`
     - `crate::systems::soul_ai::helpers::work as helpers` → `crate::soul_ai::helpers::work as helpers`
     - `crate::systems::spatial::BlueprintSpatialGrid` → `hw_spatial::BlueprintSpatialGrid`
  4. `crates/hw_ai/src/soul_ai/decide/mod.rs` に `pub mod work;` を追加
  5. `SoulAiCorePlugin::build()` の `SoulAiSystemSet::Decide` ブロックに以下を追加:
     ```rust
     decide::work::auto_refine::mud_mixer_auto_refine_system,
     decide::work::auto_build::blueprint_auto_build_system,
     ```
  6. root `src/systems/soul_ai/decide/work/auto_refine.rs` → re-export shell:
     ```rust
     pub use hw_ai::soul_ai::decide::work::auto_refine::*;
     ```
  7. root `src/systems/soul_ai/decide/work/auto_build.rs` → re-export shell:
     ```rust
     pub use hw_ai::soul_ai::decide::work::auto_build::*;
     ```
  8. root `src/systems/soul_ai/mod.rs` から `decide::work::auto_refine` / `decide::work::auto_build` の `add_systems` 呼び出しを削除する

- **変更ファイル**:
  - `crates/hw_ai/src/soul_ai/decide/mod.rs` ← `pub mod work;` 追加
  - `crates/hw_ai/src/soul_ai/decide/work/mod.rs` ← 新規
  - `crates/hw_ai/src/soul_ai/decide/work/auto_refine.rs` ← 新規（移植）
  - `crates/hw_ai/src/soul_ai/decide/work/auto_build.rs` ← 新規（移植）
  - `crates/hw_ai/src/soul_ai/mod.rs` ← `SoulAiCorePlugin` に system 登録追加
  - `src/systems/soul_ai/decide/work/auto_refine.rs` ← shell に縮退
  - `src/systems/soul_ai/decide/work/auto_build.rs` ← shell に縮退
  - `src/systems/soul_ai/mod.rs` ← 重複 system 登録の削除

- **完了条件**:
  - [ ] `crates/hw_ai/src/soul_ai/decide/work/{auto_refine,auto_build}.rs` が存在する
  - [ ] root 側 `decide/work/*.rs` が re-export shell（1〜3 行）に縮退している
  - [ ] root `soul_ai/mod.rs` から `mud_mixer_auto_refine_system` / `blueprint_auto_build_system` の `add_systems` が消えている
  - [ ] `cargo check -p hw_ai` が成功する
  - [ ] `cargo check --workspace` が成功する

- **検証**:
  ```bash
  CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check -p hw_ai
  CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check --workspace
  # system が hw_ai に移ったことを確認
  rg "mud_mixer_auto_refine_system\|blueprint_auto_build_system" crates/hw_ai/src/
  # root から add_systems が消えたことを確認
  rg "mud_mixer_auto_refine_system\|blueprint_auto_build_system" src/systems/soul_ai/mod.rs
  ```

---

### M3: gathering shared model を下位 crate に揃え、`GatheringSpotSpatialGrid` を `hw_spatial` へ移す

- **現状の型の所在**:
  - `GatheringSpot`, `GatheringObjectType`, `GatheringUpdateTimer`, 各種定数 → `crates/hw_ai/src/soul_ai/helpers/gathering.rs`
  - `GatheringSpotSpatialGrid` + 更新 system → `src/systems/spatial/gathering.rs`（root）
  - root `src/systems/soul_ai/helpers/gathering.rs` → `pub use hw_ai::soul_ai::helpers::gathering::*;` の 1 行 re-export

- **変更内容（順序が重要）**:
  1. `crates/hw_core/src/gathering.rs` を新規作成し、`GatheringSpot`, `GatheringObjectType`, `GatheringUpdateTimer`, 全 gathering 定数/helper を移植する
  2. `crates/hw_core/src/lib.rs` に `pub mod gathering;` を追加し public export する
  3. `crates/hw_ai/src/soul_ai/helpers/gathering.rs` を以下の re-export shell に縮退する:
     ```rust
     pub use hw_core::gathering::*;
     ```
     （root の `src/systems/soul_ai/helpers/gathering.rs` は `pub use hw_ai::soul_ai::helpers::gathering::*` のまま変更不要）
  4. `crates/hw_spatial/src/gathering.rs` を新規作成。`GatheringSpot` を `hw_core::gathering` から import し `GatheringSpotSpatialGrid` + `update_gathering_spot_spatial_grid_system` を定義する
  5. `crates/hw_spatial/src/lib.rs` に `pub mod gathering;` と以下を追加:
     ```rust
     pub use gathering::{GatheringSpotSpatialGrid, update_gathering_spot_spatial_grid_system};
     ```
  6. root `src/systems/spatial/gathering.rs` を以下の re-export shell に縮退する:
     ```rust
     pub use hw_spatial::gathering::*;
     ```
  7. `src/systems/spatial/mod.rs` の `pub use gathering::...` が `hw_spatial` 経由でも解決できることを確認し、必要に応じて調整する
  8. `src/plugins/startup/mod.rs` の `init_resource::<GatheringSpotSpatialGrid>()` は root 経由の re-export のままで動作するため変更不要

- **変更ファイル**:
  - `crates/hw_core/src/gathering.rs` ← 新規（model 移植）
  - `crates/hw_core/src/lib.rs` ← `pub mod gathering;` 追加
  - `crates/hw_ai/src/soul_ai/helpers/gathering.rs` ← shell に縮退
  - `crates/hw_spatial/src/gathering.rs` ← 新規（grid 追加）
  - `crates/hw_spatial/src/lib.rs` ← export 追加
  - `src/systems/spatial/gathering.rs` ← shell に縮退
  - `docs/cargo_workspace.md` ← `GatheringSpot` が `hw_core` に移ったことを反映

- **完了条件**:
  - [ ] `GatheringSpot` が `hw_core::gathering` に定義されている
  - [ ] `hw_spatial` が `GatheringSpotSpatialGrid` を所有している
  - [ ] `hw_ai → hw_spatial` の依存方向が維持されており、`hw_spatial → hw_ai` の逆依存がない
  - [ ] root から `crate::systems::soul_ai::helpers::gathering::GatheringSpot` のパスで引き続きアクセスできる（re-export chain による）
  - [ ] `cargo check -p hw_core` / `-p hw_spatial` / `-p hw_ai` が成功する

- **検証**:
  ```bash
  CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check -p hw_core
  CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check -p hw_spatial
  CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check -p hw_ai
  CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check --workspace
  # GatheringSpot が hw_core に定義されていることを確認
  rg "pub struct GatheringSpot" crates/
  # hw_spatial から hw_ai への依存がないことを確認
  rg "hw_ai" crates/hw_spatial/Cargo.toml
  ```

---

### M4: `idle_behavior` と `gathering_spawn` を pure logic と adapter に分ける

- **前提**: M3 完了後に着手すること（`GatheringSpotSpatialGrid` が `hw_spatial` に移っていることが必要）

- **変更内容**:

  #### idle_behavior_decision_system の hw_ai 移設
  1. `idle_behavior_decision_system` で使っている `WorldMapRead` を `Res<hw_world::WorldMap>` に置換する（hw_ai 版 `separation.rs` が先行事例）
  2. `GatheringSpotSpatialGrid` は M3 後に `hw_spatial` 経由で参照可能なので変更不要
  3. `IdleDecisionSoulQuery`, `IdleBehaviorRequest`, `IdleBehaviorOperation` はすでに hw_ai / hw_core にある
  4. `RestArea` は `hw_jobs` にある
  5. 関数本体（394 行）を `crates/hw_ai/src/soul_ai/decide/idle_behavior/mod.rs` へ移植し、`pub fn idle_behavior_decision_system` を export する
  6. `SoulAiCorePlugin::build()` の Decide セットに system を追加する
  7. root `src/systems/soul_ai/decide/idle_behavior/mod.rs` は re-export shell に縮退する:
     ```rust
     pub use hw_ai::soul_ai::decide::idle_behavior::*;
     ```
  8. root `src/systems/soul_ai/mod.rs` から `idle_behavior_decision_system` の `add_systems` を削除する

  #### gathering_spawn_system の分割
  1. 現在 `gathering_spawn.rs` で `GameAssets` に依存しているのは以下の2点のみ:
     - `game_assets.gathering_card_table.clone()`
     - `game_assets.gathering_campfire.clone()`
     - `game_assets.gathering_barrel.clone()`
  2. `hw_core::events` に `GatheringSpawnRequest { pos: Vec2, object_type: GatheringObjectType, nearby_souls: usize }` を追加する
  3. hw_ai 側: `GatheringReadiness` の tick と「発生判定 + `GatheringSpawnRequest` の emit」だけを担うシステムを作成する
  4. root 側: `GatheringSpawnRequest` を受け取り、`GameAssets` を使って aura + object sprite を spawn するアダプターを残す
  5. `GatheringSpawnRequest` を `MessagesPlugin` に登録する

- **変更ファイル**:
  - `crates/hw_ai/src/soul_ai/decide/idle_behavior/mod.rs` ← `idle_behavior_decision_system` 本体を追加（394 行移植）
  - `crates/hw_ai/src/soul_ai/mod.rs` ← system 登録追加
  - `crates/hw_core/src/events.rs` ← `GatheringSpawnRequest` 追加
  - `crates/hw_ai/src/soul_ai/execute/gathering_spawn.rs` ← 新規（pure 判定 + request emit）
  - `src/systems/soul_ai/decide/idle_behavior/mod.rs` ← shell に縮退
  - `src/systems/soul_ai/execute/gathering_spawn.rs` ← visual spawn adapter のみ残す
  - `src/systems/soul_ai/mod.rs` ← 重複 system 登録削除
  - `src/plugins/messages.rs` ← `GatheringSpawnRequest` 登録

- **完了条件**:
  - [ ] `idle_behavior_decision_system` の本体が `hw_ai` に移っている
  - [ ] root `decide/idle_behavior/mod.rs` が re-export shell（1〜3 行）になっている
  - [ ] `gathering_spawn_system` が pure 判定（hw_ai）と visual adapter（root）に分離されている
  - [ ] root 側 `gathering_spawn.rs` に `GameAssets` 非依存の判定ロジックが残っていない
  - [ ] `cargo check -p hw_ai` と `cargo check --workspace` が成功する

- **検証**:
  ```bash
  CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check -p hw_ai
  CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check --workspace
  # idle_behavior が hw_ai に移ったことを確認
  rg "idle_behavior_decision_system" crates/hw_ai/src/
  # root shell が 1 行になっていることを確認
  wc -l src/systems/soul_ai/decide/idle_behavior/mod.rs
  ```

---

### M5: 境界ドキュメントを同期し、残留 root ファイルを監査する

- **変更内容**:
  - `docs/cargo_workspace.md`, `docs/soul_ai.md`, `src/systems/soul_ai/README.md`, `crates/hw_ai/README.md`, `crates/hw_spatial/README.md` を更新する
  - `docs/README.md` と `docs/plans/README.md` の索引を同期する
  - `src/systems/soul_ai` に残ったファイルが root-only 契約を満たすか最終監査する

- **変更ファイル**:
  - `docs/cargo_workspace.md`
  - `docs/soul_ai.md`
  - `docs/README.md`
  - `docs/plans/README.md`
  - `src/systems/soul_ai/README.md`
  - `crates/hw_ai/README.md`
  - `crates/hw_spatial/README.md`

- **完了条件**:
  - [ ] ドキュメント記述と実コードの crate 境界が一致している
  - [ ] `src/systems/soul_ai` に残るファイルが root-only 契約で説明できる
  - [ ] 計画書のステータスを `Completed` か `Archived` に更新できる状態になっている

- **検証**:
  ```bash
  python scripts/update_docs_index.py
  cargo check --workspace
  ```

---

## 6. リスクと対策

| リスク | 影響 | 対策 |
| --- | --- | --- |
| `GatheringSpot` の移動先を誤る | `hw_spatial` との依存方向が壊れる | `hw_core` への移設順序（`hw_core` → `hw_spatial` → `hw_ai` re-export）を厳守する |
| M3 で `hw_ai/helpers/gathering.rs` を shell にした後、hw_ai 内の import が壊れる | `-p hw_ai` のビルドエラー | shell 化の直後に `cargo check -p hw_ai` を実行し、`hw_core::gathering` からの型参照が解決できるか確認する |
| `WorldMapRead` 置換が `idle_behavior` 以外の system にも波及する | M4 のスコープが広がる | M4 では `idle_behavior_decision_system` のみ `Res<WorldMap>` に切り替え、他の `WorldMapRead` 利用箇所は別計画に切り出す |
| `SoulAiCorePlugin` の system 登録順が崩れる | AI 挙動が変わる | root `soul_ai/mod.rs` と hw_ai の `SoulAiCorePlugin` で `SoulAiSystemSet::Decide` 内の相対順序（after 制約）を同じにする |
| stale file 削除で見えない参照を落とす | ビルドエラー | 削除前に `rg` で各ファイル名を全ソースから参照検索し、inline module の shadow が有効であることを確認する |
| `GatheringSpawnRequest` 追加で `MessagesPlugin` 登録漏れ | runtime panic | `hw_core::events` に追加後、`src/plugins/messages.rs` に登録を追加し、`cargo check` を必ず実行する |

---

## 7. 検証計画

- 必須:
  - `cargo check -p hw_ai`
  - `cargo check -p hw_spatial`
  - `cargo check --workspace`
- 手動確認シナリオ:
  - 資材が揃った Blueprint に Soul が自動で割り当てられる
  - MudMixer に材料が揃うと `Refine` designation が自動発行される
  - idle Soul が集まったときに gathering が発生し、参加・離脱・休憩遷移が維持される
  - 既存の escape / gathering apply / idle behavior apply が re-export 経由でも動作する
- パフォーマンス確認（必要時）:
  - `GatheringSpotSpatialGrid` 移設後に grid 更新頻度が増えていないか確認する

---

## 8. ロールバック方針

- どの単位で戻せるか:
  - M1: stale file 削除と README 更新単位
  - M2: `auto_refine` / `auto_build` の移設単位
  - M3: gathering model / spatial grid 移設単位
  - M4: `idle_behavior` 移設と `gathering_spawn` 分割単位
- 戻す時の手順:
  1. 直前マイルストーンの commit を単位に revert する
  2. `docs/cargo_workspace.md` と `src/systems/soul_ai/README.md` を同時に戻す
  3. `cargo check --workspace` で境界不整合がないことを確認する

---

## 9. AI引継ぎメモ（最重要）

### 現在地

- 進捗: `0%`（コード調査済み、実装未着手）
- 完了済みマイルストーン: `なし`
- 調査で確認済みの重要事実:
  - `AssignedTask` と `MovePlanned` はすでに `hw_jobs` にある（root は re-export のみ）
  - `StressBreakdown` は `hw_core::soul` にある
  - `auto_refine` / `auto_build` は root-only 型への依存がゼロ（**今すぐ M2 に着手できる**）
  - `GatheringSpot` は `hw_ai::soul_ai::helpers::gathering` にある（root ではない）
  - stale file 4 件は inline module に shadow されて既にコンパイル対象外

### 次のAIが最初にやること

1. **M1（推奨 30 分）**: 4 ファイルを削除するだけ。`cargo check --workspace` が通ることを確認
2. **M2（推奨 1〜2 時間）**: §3.5 の import 変換表を見ながら hw_ai に `decide/work/` を作成する。`cargo check -p hw_ai` が通ることを確認してから root shell 化・system 登録削除を行う
3. **M3（推奨 2〜3 時間）**: `hw_core/src/gathering.rs` 新規作成 → hw_ai shell 化 → hw_spatial 追加 → root shell 化 の順に実施し、各 crate 単位で `cargo check` を通す

### ブロッカー/注意点（調査確認済み）

- `auto_refine` / `auto_build` に root-only 型の依存は存在しない。旧来の計画にあった「共有型のみ」という記述は正しく、追加 crate 依存も不要
- `docs/cargo_workspace.md` の「GatheringSpot は root 型」という説明は誤り（現在は `hw_ai` にある）
- `idle_behavior/mod.rs` は hw_ai 版（10 行）と root 版（394 行）が別物。hw_ai 版はまだ sub-module を宣言するだけのスタブ
- `task_execution` は大きく、今回の計画に混ぜると root 薄化の初手が重くなりすぎる（Out of Scope 維持）
- M4 の `WorldMapRead` 置換は `idle_behavior_decision_system` のみに限定し、他への波及を防ぐ

### 参照必須ファイル

- `docs/cargo_workspace.md`
- `docs/soul_ai.md`
- `src/systems/soul_ai/README.md`
- `src/systems/soul_ai/mod.rs`（system 登録の現状確認）
- `src/systems/soul_ai/decide/work/auto_build.rs`（import 変換の起点）
- `src/systems/soul_ai/decide/work/auto_refine.rs`（import 変換の起点）
- `src/systems/soul_ai/decide/idle_behavior/mod.rs`（M4 移設対象・394 行）
- `src/systems/soul_ai/execute/gathering_spawn.rs`（M4 分割対象）
- `src/systems/spatial/gathering.rs`（M3 対象・現状確認）
- `crates/hw_ai/src/soul_ai/mod.rs`（`SoulAiCorePlugin` の system 登録）
- `crates/hw_ai/src/soul_ai/helpers/gathering.rs`（M3 shell 化の起点）
- `src/systems/soul_ai/decide/work/auto_refine.rs`
- `src/systems/soul_ai/decide/idle_behavior/mod.rs`
- `src/systems/soul_ai/execute/gathering_spawn.rs`
- `src/systems/spatial/gathering.rs`
- `crates/hw_ai/src/soul_ai/mod.rs`

### 最終確認ログ

- 最終 `cargo check`: `未実施（docs 作成のみ）`
- 未解決エラー: `N/A`

### Definition of Done

- [ ] `M1` から `M5` まで完了している
- [ ] `src/systems/soul_ai` に残るファイルが root-only 契約で説明できる
- [ ] 影響ドキュメントが更新済み
- [ ] `cargo check --workspace` が成功している

## 10. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| `2026-03-09` | `AI (Codex)` | 初版作成 |
| `2026-03-09` | `AI (Copilot)` | コード実調査に基づきブラッシュアップ。§3.2 stale file 確認済み記録、§3.5 依存型解決テーブル（auto_refine/auto_build の全 import → 実定義元）、M2 詳細手順（import 変換リスト・変更ファイル一覧）、M3 変更順序明示、M4 具体的分割手順・GatheringSpawnRequest 設計、リスク表更新、AI引継ぎメモ拡充 |
