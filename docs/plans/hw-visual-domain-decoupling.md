# hw_visual ドメイン分離：ミラーコンポーネント実装計画

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `hw-visual-domain-decoupling-plan-2026-03-14` |
| ステータス | `Draft` |
| 作成日 | `2026-03-14` |
| 最終更新日 | `2026-03-15` |
| 作成者 | Claude |
| 関連提案 | `docs/proposals/hw-visual-domain-decoupling.md` |

---

## 1. 目的

- **解決したい課題**: `hw_visual` が `hw_jobs` / `hw_logistics` を直接依存している
- **到達したい状態**: `hw_visual/Cargo.toml` から `hw_jobs` / `hw_logistics` 依存が消え（または Out of Scope 型のみになり）、`hw_core::visual_mirror` の型のみを参照する
- **成功指標**: `cargo check -p hw_visual` が通り、hw_visual の hw_jobs/hw_logistics 参照が本計画スコープ内型に限り 0 になる

---

## 2. スコープ

### 対象（In Scope）

| hw_visual ファイル | 依存型 | 置き換え先 |
|---|---|---|
| `gather/resource_highlight.rs` | `Designation`, `Rock`, `Tree` | `GatherHighlightMarker` ミラー |
| `dream/particle.rs` | `RestArea` | `RestAreaVisual` ミラー |
| `haul/wheelbarrow_follow.rs` | `Wheelbarrow` | `WheelbarrowMarker` ミラー |
| `haul/carrying_item.rs` | `Inventory`, `ResourceItem` | `InventoryItemVisual` ミラー |
| `gather/worker_indicator.rs` | `AssignedTask`, `GatherPhase`, `WorkType` | `SoulTaskVisualState` ミラー |
| `blueprint/worker_indicator.rs` | `AssignedTask`, `BuildPhase` | `SoulTaskVisualState` ミラー |
| `soul/idle.rs` | `AssignedTask` | `SoulTaskVisualState` ミラー |
| `soul/mod.rs` | `AssignedTask` + 6 Phase 型 | `SoulTaskVisualState` ミラー |
| `blueprint/{mod,effects,material_display,progress_bar}.rs` | `Blueprint` | `BlueprintVisualState` ミラー |
| `wall_connection.rs`（Blueprint 部分のみ） | `Blueprint` | `BlueprintVisualState` ミラー |
| `floor_construction.rs` | `FloorConstructionSite`, `FloorTileBlueprint`, `FloorTileState`, `FloorConstructionPhase` | `FloorTileVisualMirror` + `FloorSiteVisualState` ミラー |
| `wall_construction.rs` | `WallConstructionSite`, `WallTileBlueprint`, `WallTileState`, `WallConstructionPhase` | `WallTileVisualMirror` + `WallSiteVisualState` ミラー |

### 非対象（Out of Scope）

以下の型は本計画では扱わない。これらが残る 3 ファイルは M4 で依存が残ることを文書化する。

| ファイル | 残る依存型 | 理由 |
|---|---|---|
| `mud_mixer.rs` | `AssignedTask`, `RefinePhase`, `MudMixerStorage` | 別提案 |
| `tank.rs` | `Building`, `BuildingType`, `Stockpile` | 別提案 |
| `wall_connection.rs`（Building 部分） | `Building`, `BuildingType` | 別提案 |

---

## 3. 現状の依存箇所（全網羅）

### hw_jobs からのインポート（19 行）

| ファイル | インポートされている型 |
|---|---|
| `gather/resource_highlight.rs:8` | `Designation`, `Rock`, `Tree` |
| `gather/worker_indicator.rs:15` | `AssignedTask`, `GatherPhase`, `WorkType` |
| `blueprint/mod.rs:12` | `Blueprint` |
| `blueprint/effects.rs:13` | `Blueprint` |
| `blueprint/material_display.rs:10` | `Blueprint` |
| `blueprint/progress_bar.rs:17` | `Blueprint` |
| `blueprint/worker_indicator.rs:10` | `AssignedTask`, `BuildPhase` |
| `dream/particle.rs:10` | `RestArea` |
| `mud_mixer.rs:5-7` | `AssignedTask`, `RefinePhase`, `mud_mixer::MudMixerStorage`（Out of Scope）|
| `tank.rs:6` | `Building`, `BuildingType`（Out of Scope）|
| `wall_connection.rs:3` | `Blueprint`（In Scope）+ `Building`, `BuildingType`（Out of Scope）|
| `soul/idle.rs:9` | `AssignedTask` |
| `soul/mod.rs:21` | `AssignedTask`, `CoatWallPhase`, `FrameWallPhase`, `GatherPhase`, `HaulPhase`, `PourFloorPhase`, `ReinforceFloorPhase` |
| `floor_construction.rs:5-6` | `construction::{FloorConstructionPhase, FloorTileBlueprint}`, `FloorConstructionSite`, `FloorTileState` |
| `wall_construction.rs:5-6` | `construction::{WallConstructionPhase, WallTileBlueprint}`, `WallConstructionSite`, `WallTileState` |

### hw_logistics からのインポート（3 行）

| ファイル | インポートされている型 |
|---|---|
| `haul/carrying_item.rs:10` | `types::{Inventory, ResourceItem}` |
| `haul/wheelbarrow_follow.rs:10` | `types::Wheelbarrow` |
| `tank.rs:7` | `zone::Stockpile`（Out of Scope）|

---

## 4. 実装方針

- **ドメイン型は移動しない**: `Designation`/`Tree`/`Rock`/`Wheelbarrow` 等は hw_jobs/hw_logistics に残す
- **hw_core に `visual_mirror` サブモジュールを作成**: hw_visual が必要とするビジュアル情報のみを持つコンポーネントを定義する
- **`Changed<T>` / `Added<T>` / Observer 駆動で同期**: 毎フレーム全件走査ではなく変化時のみ更新する
- **hw_visual は `hw_core::visual_mirror` の型のみを Query する**

**システム登録箇所（重要）**: hw_jobs は Plugin を持たず、hw_logistics もトップレベルの Plugin を持たない（`TransportRequestPlugin` はサブプラグインのみ）。両クレートの同期システムと Observer はすべて `bevy_app/src/plugins/logic.rs` の適切なシステムセットに追加する。

---

## 5. マイルストーン

### M1: hw_core::visual_mirror の骨格作成とグループ A ミラー化

**変更内容**: `GatherHighlightMarker`/`RestAreaVisual`/`WheelbarrowMarker`/`InventoryItemVisual` を追加し、hw_jobs/hw_logistics 側で Observer 同期、hw_visual 側クエリ書き換え

**変更ファイル**:
```
crates/hw_core/src/visual_mirror/mod.rs          (新規)
crates/hw_core/src/visual_mirror/gather.rs       (新規)
crates/hw_core/src/visual_mirror/logistics.rs    (新規)
crates/hw_core/src/lib.rs                        (pub mod visual_mirror 追加)
crates/bevy_app/src/entities/damned_soul/mod.rs  (InventoryItemVisual 登録)
crates/bevy_app/src/plugins/logic.rs             (Observer 登録: on_designation_added/removed, on_rest_area_added, on_wheelbarrow_added)
                                                  (System 登録: sync_inventory_item_visual_system)
crates/hw_visual/src/gather/resource_highlight.rs
crates/hw_visual/src/dream/particle.rs
crates/hw_visual/src/haul/wheelbarrow_follow.rs
crates/hw_visual/src/haul/carrying_item.rs
```

**実装詳細**:

```rust
// crates/hw_core/src/visual_mirror/gather.rs
use bevy::prelude::*;

/// hw_jobs::Designation + (Tree|Rock) エンティティを hw_visual がハイライト表示するためのマーカー。
/// hw_jobs の Observer (on_designation_added/removed) が attach/detach する。
#[derive(Component)]
pub struct GatherHighlightMarker;

/// hw_jobs::RestArea エンティティを hw_visual が DreamParticle 対象にするためのミラー。
/// capacity は dream/particle.rs が直接読むため必須。
/// hw_jobs の Observer (on_rest_area_added) が attach する。
#[derive(Component)]
pub struct RestAreaVisual {
    pub capacity: usize,
}
```

```rust
// crates/hw_core/src/visual_mirror/logistics.rs
use bevy::prelude::*;
use crate::logistics::ResourceType;

/// hw_logistics::Wheelbarrow エンティティを hw_visual が型フィルタするためのマーカー。
/// hw_logistics の Observer (on_wheelbarrow_added) が attach する。
#[derive(Component)]
pub struct WheelbarrowMarker;

/// Soul エンティティが今何を持っているかを hw_visual が表示するためのミラー。
/// hw_logistics の sync_inventory_item_visual_system (Changed<Inventory>) が同期する。
///
/// **注意**: hw_visual/src/haul/components.rs に同名の `CarryingItemVisual` が存在する（Soul ではなく
/// ビジュアルアイコン entity 側のコンポーネント）。混同を避けるため hw_core 側は `InventoryItemVisual` と命名。
#[derive(Component, Default)]
pub struct InventoryItemVisual {
    /// None = 何も持っていない
    pub resource_type: Option<ResourceType>,
}
```

**Observer 実装（bevy_app/src/plugins/logic.rs で登録）**:

```rust
// Observer を登録する場所: bevy_app/src/plugins/logic.rs の build() 内
app.add_observer(on_designation_added);
app.add_observer(on_designation_removed);
app.add_observer(on_rest_area_added);
app.add_observer(on_wheelbarrow_added);

// Observer 本体: bevy_app/src/systems/jobs/ に配置するか、
// hw_jobs/src/ 内に visual_sync.rs を作成して bevy_app から呼ぶ
// （hw_jobs は fn を公開し、bevy_app で登録する形が正しい）

// hw_jobs/src/visual_sync.rs (新規)
use bevy::prelude::*;
use hw_core::visual_mirror::gather::{GatherHighlightMarker, RestAreaVisual};
use crate::model::{Designation, RestArea, Rock, Tree};

pub fn on_designation_added(
    trigger: Trigger<OnAdd, Designation>,
    mut commands: Commands,
    q: Query<(), Or<(With<Tree>, With<Rock>)>>,
) {
    // Tree/Rock エンティティのみにマーカーを追加（全 Designation に追加しない）
    if q.contains(trigger.target()) {
        commands.entity(trigger.target()).try_insert(GatherHighlightMarker);
    }
}

pub fn on_designation_removed(
    trigger: Trigger<OnRemove, Designation>,
    mut commands: Commands,
) {
    commands.entity(trigger.target()).remove::<GatherHighlightMarker>();
}

pub fn on_rest_area_added(
    trigger: Trigger<OnAdd, RestArea>,
    mut commands: Commands,
    q: Query<&RestArea>,
) {
    if let Ok(rest_area) = q.get(trigger.target()) {
        commands.entity(trigger.target()).try_insert(RestAreaVisual {
            capacity: rest_area.capacity,
        });
    }
}
```

```rust
// hw_logistics/src/visual_sync.rs (新規)
use bevy::prelude::*;
use hw_core::visual_mirror::logistics::{InventoryItemVisual, WheelbarrowMarker};
use crate::types::{Inventory, ResourceItem, Wheelbarrow};

pub fn on_wheelbarrow_added(trigger: Trigger<OnAdd, Wheelbarrow>, mut commands: Commands) {
    commands.entity(trigger.target()).try_insert(WheelbarrowMarker);
}

/// Inventory の変化時に InventoryItemVisual を同期する
pub fn sync_inventory_item_visual_system(
    mut q: Query<(&Inventory, &mut InventoryItemVisual), Or<(Changed<Inventory>, Added<Inventory>)>>,
    q_items: Query<&ResourceItem>,
) {
    for (inventory, mut visual) in q.iter_mut() {
        visual.resource_type = inventory
            .0
            .and_then(|item_entity| q_items.get(item_entity).ok())
            .map(|item| item.0);
    }
}
```

**Soul スポーン時に `InventoryItemVisual::default()` を追加**:
`bevy_app/src/entities/damned_soul/spawn.rs` の `commands.spawn((... AssignedTask::default(), ...))` バンドルに追加。

**hw_visual 側の書き換え**:

`gather/resource_highlight.rs`:
```rust
// Before
use hw_jobs::{Designation, Rock, Tree};
Query<(Entity, &Sprite), (With<Designation>, Or<(With<Tree>, With<Rock>)>, Without<ResourceVisual>)>
// After
use hw_core::visual_mirror::gather::GatherHighlightMarker;
Query<(Entity, &Sprite), (With<GatherHighlightMarker>, Without<ResourceVisual>)>

// update_resource_visual_system: Option<&Designation> → Option<&GatherHighlightMarker>
// cleanup_resource_visual_system: Without<Designation> → Without<GatherHighlightMarker>
```

`dream/particle.rs`:
```rust
// Before
use hw_jobs::RestArea;
Query<Entity, (With<RestArea>, Without<DreamVisualState>)>
Query<(Entity, &Transform, &RestArea, ...)>
// After
use hw_core::visual_mirror::gather::RestAreaVisual;
Query<Entity, (With<RestAreaVisual>, Without<DreamVisualState>)>
Query<(Entity, &Transform, &RestAreaVisual, ...)>  // capacity は rest_area_visual.capacity で参照
```

`haul/wheelbarrow_follow.rs`:
```rust
// Before: With<Wheelbarrow>
// After:  With<WheelbarrowMarker>
use hw_core::visual_mirror::logistics::WheelbarrowMarker;
```

`haul/carrying_item.rs`:
```rust
// Before
use hw_logistics::types::{Inventory, ResourceItem};
Query<(Entity, &Transform, &Inventory), ...>, q_items: Query<&ResourceItem>
// After
use hw_core::visual_mirror::logistics::InventoryItemVisual;
Query<(Entity, &Transform, &InventoryItemVisual), ...>
// inventory.0 の代わりに visual.resource_type を使用
```

**完了条件**:
- `hw_core::visual_mirror` モジュールが存在し上記 4 型が公開されている
- `gather/resource_highlight.rs`/`dream/particle.rs`/`haul/wheelbarrow_follow.rs`/`haul/carrying_item.rs` が hw_jobs/hw_logistics をインポートしていない
- `cargo check` 成功

---

### M2: SoulTaskVisualState ミラー化（AssignedTask の分離）

**変更内容**: `SoulTaskVisualState` を追加し、`Changed<AssignedTask>` + `Added<AssignedTask>` で同期。hw_visual の AssignedTask 依存を全て置き換え

**変更ファイル**:
```
crates/hw_core/src/visual_mirror/task.rs         (新規)
crates/hw_core/src/visual_mirror/mod.rs          (task モジュール追加)
crates/bevy_app/src/entities/damned_soul/spawn.rs (SoulTaskVisualState::default() 追加)
crates/bevy_app/src/plugins/logic.rs             (sync システム登録)
crates/hw_jobs/src/visual_sync.rs                (sync_soul_task_visual_system 追加)
crates/hw_visual/src/soul/mod.rs
crates/hw_visual/src/soul/idle.rs
crates/hw_visual/src/gather/worker_indicator.rs
crates/hw_visual/src/blueprint/worker_indicator.rs
```

**実装詳細**:

```rust
// crates/hw_core/src/visual_mirror/task.rs
use bevy::prelude::*;

/// AssignedTask の表示に必要な情報のみをミラーするコンポーネント。
/// hw_jobs の sync_soul_task_visual_system (Changed<AssignedTask> | Added<AssignedTask>) が同期する。
#[derive(Component, Default, Debug, Clone)]
pub struct SoulTaskVisualState {
    pub phase: SoulTaskPhaseVisual,
    /// プログレスバー (0.0–1.0)。None = 非表示
    pub progress: Option<f32>,
    /// タスクリンク gizmo 描画先
    pub link_target: Option<Entity>,
    /// バケツ搬送リンク（Some の場合 link_target より優先）
    pub bucket_link: Option<Entity>,
}

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
pub enum SoulTaskPhaseVisual {
    #[default]
    None,
    /// 採取中（斧アイコン）
    GatherChop,
    /// 採取中（ツルハシアイコン）
    GatherMine,
    Haul,
    HaulToBlueprint,
    Build,
    ReinforceFloor,
    PourFloor,
    FrameWall,
    CoatWall,
    Refine,
    CollectSand,
    CollectBone,
    MovePlant,
    BucketTransport,
    HaulToMixer,
    HaulWithWheelbarrow,
}
```

**同期システム（hw_jobs/src/visual_sync.rs に追加）**:

```rust
use hw_core::visual_mirror::task::{SoulTaskVisualState, SoulTaskPhaseVisual};
use crate::assigned_task::{
    AssignedTask, GatherPhase, HaulPhase, ReinforceFloorPhase,
    PourFloorPhase, FrameWallPhase, CoatWallPhase,
};
use hw_core::jobs::WorkType;

pub fn sync_soul_task_visual_system(
    mut q: Query<
        (&AssignedTask, &mut SoulTaskVisualState),
        Or<(Changed<AssignedTask>, Added<AssignedTask>)>,
    >,
) {
    for (task, mut state) in q.iter_mut() {
        let (phase, progress, link_target, bucket_link) = match task {
            AssignedTask::None => (SoulTaskPhaseVisual::None, None, None, None),
            AssignedTask::Gather(d) => {
                let phase = match d.work_type {
                    WorkType::Chop => SoulTaskPhaseVisual::GatherChop,
                    WorkType::Mine => SoulTaskPhaseVisual::GatherMine,
                    _ => SoulTaskPhaseVisual::GatherChop,
                };
                let progress = if let GatherPhase::Collecting { progress } = d.phase {
                    Some(progress)
                } else {
                    None
                };
                (phase, progress, Some(d.target), None)
            }
            AssignedTask::Haul(d) => {
                let target = match d.phase {
                    HaulPhase::GoingToItem => Some(d.item),
                    HaulPhase::GoingToStockpile => Some(d.stockpile),
                    _ => None,
                };
                (SoulTaskPhaseVisual::Haul, None, target, None)
            }
            AssignedTask::HaulToBlueprint(d) => {
                (SoulTaskPhaseVisual::HaulToBlueprint, None, Some(d.blueprint), None)
            }
            AssignedTask::Build(d) => {
                (SoulTaskPhaseVisual::Build, None, Some(d.blueprint), None)
            }
            AssignedTask::ReinforceFloorTile(d) => {
                let progress = if let ReinforceFloorPhase::Reinforcing { progress_bp } = d.phase {
                    Some((progress_bp as f32 / 10_000.0).clamp(0.0, 1.0))
                } else {
                    None
                };
                (SoulTaskPhaseVisual::ReinforceFloor, progress, Some(d.tile), None)
            }
            AssignedTask::PourFloorTile(d) => {
                let progress = if let PourFloorPhase::Pouring { progress_bp } = d.phase {
                    Some((progress_bp as f32 / 10_000.0).clamp(0.0, 1.0))
                } else {
                    None
                };
                (SoulTaskPhaseVisual::PourFloor, progress, Some(d.tile), None)
            }
            AssignedTask::FrameWallTile(d) => {
                let progress = if let FrameWallPhase::Framing { progress_bp } = d.phase {
                    Some((progress_bp as f32 / 10_000.0).clamp(0.0, 1.0))
                } else {
                    None
                };
                (SoulTaskPhaseVisual::FrameWall, progress, Some(d.tile), None)
            }
            AssignedTask::CoatWall(d) => {
                let progress = if let CoatWallPhase::Coating { progress_bp } = d.phase {
                    Some((progress_bp as f32 / 10_000.0).clamp(0.0, 1.0))
                } else {
                    None
                };
                (SoulTaskPhaseVisual::CoatWall, progress, Some(d.wall), None)
            }
            AssignedTask::Refine(d) => {
                (SoulTaskPhaseVisual::Refine, None, Some(d.mixer), None)
            }
            AssignedTask::CollectSand(d) => {
                (SoulTaskPhaseVisual::CollectSand, None, Some(d.target), None)
            }
            AssignedTask::CollectBone(d) => {
                (SoulTaskPhaseVisual::CollectBone, None, Some(d.target), None)
            }
            AssignedTask::MovePlant(_) => (SoulTaskPhaseVisual::MovePlant, None, None, None),
            AssignedTask::HaulToMixer(d) => {
                (SoulTaskPhaseVisual::HaulToMixer, None, Some(d.mixer), None)
            }
            AssignedTask::HaulWithWheelbarrow(_) => {
                (SoulTaskPhaseVisual::HaulWithWheelbarrow, None, None, None)
            }
            AssignedTask::BucketTransport(d) => {
                (SoulTaskPhaseVisual::BucketTransport, None, None, Some(d.bucket))
            }
        };

        // bucket_transport_data() が返す bucket は bucket_link 優先
        state.phase = phase;
        state.progress = progress;
        state.link_target = link_target;
        state.bucket_link = bucket_link;
    }
}
```

> **注意**: 上記バリアント名は `crates/hw_jobs/src/assigned_task.rs` の実際の定義と照合すること。`BucketTransport(d).bucket` フィールド名は実際のコードで確認する。

**Soul スポーン時に追加**:
`bevy_app/src/entities/damned_soul/spawn.rs` の spawn バンドルに `SoulTaskVisualState::default()` を追加。

**hw_visual 側の書き換え**:

`soul/idle.rs`:
```rust
// Before: if !matches!(task, AssignedTask::None)
// After:  if state.phase != SoulTaskPhaseVisual::None
use hw_core::visual_mirror::task::{SoulTaskVisualState, SoulTaskPhaseVisual};
```

`soul/mod.rs`:
```rust
// Before: match task { AssignedTask::Gather(data) => matches!(data.phase, GatherPhase::Collecting{..}), ... }
// After:
let needs_bar = state.progress.is_some()
    && matches!(
        state.phase,
        SoulTaskPhaseVisual::GatherChop
            | SoulTaskPhaseVisual::GatherMine
            | SoulTaskPhaseVisual::ReinforceFloor
            | SoulTaskPhaseVisual::PourFloor
            | SoulTaskPhaseVisual::FrameWall
            | SoulTaskPhaseVisual::CoatWall
    );
// task_link_system: link_target / bucket_link を使用
// soul_status_visual_system: state.phase == SoulTaskPhaseVisual::None を使用
```

`gather/worker_indicator.rs`:
```rust
// Before: if let AssignedTask::Gather(data) = assigned_task { ... data.work_type ... }
// After:
match state.phase {
    SoulTaskPhaseVisual::GatherChop => { /* 斧アイコン spawn */ }
    SoulTaskPhaseVisual::GatherMine => { /* ツルハシアイコン spawn */ }
    _ => continue,
}
```

`blueprint/worker_indicator.rs`:
```rust
// Before: if let AssignedTask::Build(data) = assigned_task { matches!(data.phase, BuildPhase::Building{..}) }
// After: matches!(state.phase, SoulTaskPhaseVisual::Build)
```

**完了条件**:
- `soul/mod.rs`/`soul/idle.rs`/`gather/worker_indicator.rs`/`blueprint/worker_indicator.rs` が hw_jobs をインポートしていない
- `cargo check` 成功

---

### M3: Blueprint / Construction / Inventory ミラー化

**変更内容**: `BlueprintVisualState`/`FloorTileVisualMirror`/`WallTileVisualMirror`/`FloorSiteVisualState`/`WallSiteVisualState` を追加し同期。hw_visual の残り依存を置き換え

**変更ファイル**:
```
crates/hw_core/src/visual_mirror/construction.rs (新規)
crates/hw_core/src/visual_mirror/mod.rs          (モジュール追加)
crates/bevy_app/src/plugins/logic.rs             (同期システム登録)
crates/hw_jobs/src/visual_sync.rs                (同期システム追加)
crates/hw_visual/src/blueprint/{mod,effects,material_display,progress_bar,worker_indicator}.rs
crates/hw_visual/src/wall_connection.rs          (Blueprint 部分のみ)
crates/hw_visual/src/floor_construction.rs
crates/hw_visual/src/wall_construction.rs
```

**実装詳細**:

```rust
// crates/hw_core/src/visual_mirror/construction.rs
use bevy::prelude::*;
use crate::logistics::ResourceType;

// ── Blueprint ──────────────────────────────────────────────────

/// hw_jobs::Blueprint の表示に必要な情報のみを保持するミラー。
/// hw_jobs の sync_blueprint_visual_system (Changed<Blueprint>) が同期する。
///
/// wall_connection.rs が blueprint.kind / occupied_grids を参照するため、
/// これらも含める。BuildingKind は hw_core の BuildingKindCode として不透明 u8 ではなく
/// hw_jobs::BuildingType を再エクスポートせずに済む最小限の情報として `is_wall_or_door: bool` を使う。
#[derive(Component, Default)]
pub struct BlueprintVisualState {
    pub progress: f32,
    /// 資材表示用: (resource_type, delivered, required)
    pub material_counts: Vec<(ResourceType, u32, u32)>,
    /// フレキシブル素材（Bridge 等）: (accepted_types, delivered_total, required_total)
    pub flexible_material: Option<(Vec<ResourceType>, u32, u32)>,
    /// 壁 or ドア Blueprint かどうか（wall_connection.rs のスプライト切替に使用）
    pub is_wall_or_door: bool,
    /// Blueprint が占有するグリッド（wall_connection.rs の近傍更新に使用）
    pub occupied_grids: Vec<(i32, i32)>,
}

// ── FloorTile ────────────────────────────────────────────────

/// hw_jobs::FloorTileState のミラー enum。
/// floor_construction.rs がタイル色を決定するために progress: u8 を含む。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum FloorTileStateMirror {
    #[default]
    WaitingBones,
    ReinforcingReady,
    Reinforcing { progress: u8 },
    ReinforcedComplete,
    WaitingMud,
    PouringReady,
    Pouring { progress: u8 },
    Complete,
}

/// hw_jobs::FloorTileBlueprint のビジュアルミラー。
/// hw_jobs の sync_floor_tile_visual_system (Changed<FloorTileBlueprint>) が同期する。
#[derive(Component, Default)]
pub struct FloorTileVisualMirror {
    pub state: FloorTileStateMirror,
    pub bones_delivered: u32,
}

// ── FloorSite ────────────────────────────────────────────────

/// hw_jobs::FloorConstructionPhase のミラー。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum FloorConstructionPhaseMirror {
    #[default]
    Reinforcing,
    Pouring,
    Curing,
}

/// FloorConstructionSite のビジュアルミラー（進捗バー用）。
#[derive(Component, Default)]
pub struct FloorSiteVisualState {
    pub phase: FloorConstructionPhaseMirror,
    pub curing_remaining_secs: f32,
    pub tiles_total: u32,
}

// ── WallTile ────────────────────────────────────────────────

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum WallTileStateMirror {
    #[default]
    WaitingWood,
    FramingReady,
    Framing { progress: u8 },
    FramedProvisional,
    WaitingMud,
    CoatingReady,
    Coating { progress: u8 },
    Complete,
}

/// hw_jobs::WallTileBlueprint のビジュアルミラー。
#[derive(Component, Default)]
pub struct WallTileVisualMirror {
    pub state: WallTileStateMirror,
}

/// WallConstructionSite のビジュアルミラー（進捗バー用）。
#[derive(Component, Default)]
pub struct WallSiteVisualState {
    pub phase_is_framing: bool,
    pub tiles_total: u32,
    pub tiles_framed: u32,
    pub tiles_coated: u32,
}
```

**同期システム（hw_jobs/src/visual_sync.rs に追加）**:

```rust
use hw_core::visual_mirror::construction::*;
use crate::model::Blueprint;
use crate::construction::{
    FloorConstructionSite, FloorConstructionPhase, FloorTileBlueprint, FloorTileState,
    WallConstructionSite, WallConstructionPhase, WallTileBlueprint, WallTileState,
};

pub fn sync_blueprint_visual_system(
    mut q: Query<(&Blueprint, &mut BlueprintVisualState), Or<(Changed<Blueprint>, Added<Blueprint>)>>,
) {
    for (bp, mut state) in q.iter_mut() {
        state.progress = bp.progress;
        state.material_counts = bp
            .required_materials
            .iter()
            .map(|(rt, req)| (*rt, bp.delivered_materials.get(rt).copied().unwrap_or(0), *req))
            .collect();
        state.flexible_material = bp.flexible_material_requirement.as_ref().map(|f| {
            (f.accepted_types.clone(), f.delivered_total, f.required_total)
        });
        use crate::model::BuildingType;
        state.is_wall_or_door = matches!(bp.kind, BuildingType::Wall | BuildingType::Door);
        state.occupied_grids = bp.occupied_grids.clone();
    }
}

pub fn sync_floor_tile_visual_system(
    mut q: Query<(&FloorTileBlueprint, &mut FloorTileVisualMirror), Or<(Changed<FloorTileBlueprint>, Added<FloorTileBlueprint>)>>,
) {
    for (tile, mut mirror) in q.iter_mut() {
        mirror.bones_delivered = tile.bones_delivered;
        mirror.state = match tile.state {
            FloorTileState::WaitingBones => FloorTileStateMirror::WaitingBones,
            FloorTileState::ReinforcingReady => FloorTileStateMirror::ReinforcingReady,
            FloorTileState::Reinforcing { progress } => FloorTileStateMirror::Reinforcing { progress },
            FloorTileState::ReinforcedComplete => FloorTileStateMirror::ReinforcedComplete,
            FloorTileState::WaitingMud => FloorTileStateMirror::WaitingMud,
            FloorTileState::PouringReady => FloorTileStateMirror::PouringReady,
            FloorTileState::Pouring { progress } => FloorTileStateMirror::Pouring { progress },
            FloorTileState::Complete => FloorTileStateMirror::Complete,
        };
    }
}

pub fn sync_wall_tile_visual_system(
    mut q: Query<(&WallTileBlueprint, &mut WallTileVisualMirror), Or<(Changed<WallTileBlueprint>, Added<WallTileBlueprint>)>>,
) {
    for (tile, mut mirror) in q.iter_mut() {
        mirror.state = match tile.state {
            WallTileState::WaitingWood => WallTileStateMirror::WaitingWood,
            WallTileState::FramingReady => WallTileStateMirror::FramingReady,
            WallTileState::Framing { progress } => WallTileStateMirror::Framing { progress },
            WallTileState::FramedProvisional => WallTileStateMirror::FramedProvisional,
            WallTileState::WaitingMud => WallTileStateMirror::WaitingMud,
            WallTileState::CoatingReady => WallTileStateMirror::CoatingReady,
            WallTileState::Coating { progress } => WallTileStateMirror::Coating { progress },
            WallTileState::Complete => WallTileStateMirror::Complete,
        };
    }
}

pub fn sync_floor_site_visual_system(
    mut q: Query<(&FloorConstructionSite, &mut FloorSiteVisualState), Or<(Changed<FloorConstructionSite>, Added<FloorConstructionSite>)>>,
) {
    for (site, mut state) in q.iter_mut() {
        state.phase = match site.phase {
            FloorConstructionPhase::Reinforcing => FloorConstructionPhaseMirror::Reinforcing,
            FloorConstructionPhase::Pouring => FloorConstructionPhaseMirror::Pouring,
            FloorConstructionPhase::Curing => FloorConstructionPhaseMirror::Curing,
        };
        state.curing_remaining_secs = site.curing_remaining_secs;
        state.tiles_total = site.tiles_total;
    }
}

pub fn sync_wall_site_visual_system(
    mut q: Query<(&WallConstructionSite, &mut WallSiteVisualState), Or<(Changed<WallConstructionSite>, Added<WallConstructionSite>)>>,
) {
    for (site, mut state) in q.iter_mut() {
        state.phase_is_framing = site.phase == WallConstructionPhase::Framing;
        state.tiles_total = site.tiles_total;
        state.tiles_framed = site.tiles_framed;
        state.tiles_coated = site.tiles_coated;
    }
}
```

**ミラー追加タイミング**:
- `BlueprintVisualState` → Blueprint spawn システムに `BlueprintVisualState::default()` を追加
- `FloorTileVisualMirror` → `FloorTileBlueprint` spawn 時に追加（bevy_app/src/systems/jobs/floor_construction/）
- `WallTileVisualMirror` → `WallTileBlueprint` spawn 時に追加（bevy_app/src/systems/jobs/building_completion/等）
- `FloorSiteVisualState` → `FloorConstructionSite` spawn 時に追加
- `WallSiteVisualState` → `WallConstructionSite` spawn 時に追加

**hw_visual 側の書き換え**:

`blueprint/mod.rs` の `calculate_blueprint_state`/`calculate_blueprint_visual_props`:
```rust
// Before: fn calculate_blueprint_state(bp: &Blueprint) -> BlueprintState
// After:  fn calculate_blueprint_state(vs: &BlueprintVisualState) -> BlueprintState
```
`attach_blueprint_visual_system` の `With<Blueprint>` → `With<BlueprintVisualState>`

`floor_construction.rs`:
```rust
// FloorTileBlueprint → FloorTileVisualMirror
// FloorTileState → FloorTileStateMirror
// FloorConstructionSite → FloorSiteVisualState
// Changed<FloorTileBlueprint> → Changed<FloorTileVisualMirror>
```

`wall_construction.rs`:
```rust
// WallTileBlueprint → WallTileVisualMirror
// WallTileState → WallTileStateMirror
// WallConstructionSite → WallSiteVisualState
// Changed<WallTileBlueprint> → Changed<WallTileVisualMirror>
```

`wall_connection.rs`（Blueprint 部分）:
```rust
// q_new_blueprints: Query<..., Added<Blueprint>> → Added<BlueprintVisualState>
// blueprint.kind → blueprint_vs.is_wall_or_door (チェックのみ)
// blueprint.occupied_grids → blueprint_vs.occupied_grids
```

**完了条件**:
- `blueprint/`/`floor_construction.rs`/`wall_construction.rs` + `haul/carrying_item.rs` が hw_jobs/hw_logistics をインポートしていない
- `wall_connection.rs` の `Blueprint` インポートが消えている（`Building`/`BuildingType` は Out of Scope として残存）
- `cargo check` 成功

---

### M4: 依存削除と確認（完了）

**変更内容**: 残存依存を文書化し、スコープ内ファイルの hw_jobs/hw_logistics インポートが 0 であることを確認する

> **注意**: M4 完了時点でも `mud_mixer.rs`/`tank.rs`/`wall_connection.rs`（Building 部分）は引き続き hw_jobs/hw_logistics に依存するため、`hw_visual/Cargo.toml` から hw_jobs/hw_logistics を**完全には削除できない**。これは Out of Scope として明示する。

**変更ファイル**:
```
crates/hw_visual/CLAUDE.md                    (依存制約テーブル更新)
docs/crate-boundaries.md                      (§4.1 更新)
docs/proposals/hw-visual-domain-decoupling.md (ステータス更新)
```

**完了条件**:
- スコープ内 13 ファイルで `use hw_jobs::` / `use hw_logistics::` の行がすべて削除されている
- `cargo check` 成功（ワークスペース全体）

---

## 6. リスクと対策

| リスク | 影響 | 対策 |
|---|---|---|
| `AssignedTask` バリアント名の不一致 | コンパイルエラー | M2 着手前に `assigned_task.rs` のバリアント・フィールド名を必ず確認する |
| `SoulTaskVisualState` を Soul spawn バンドルに追加し忘れ | ミラーが空のまま、gizmo/バーが表示されない | `damned_soul/spawn.rs` の spawn バンドルに `SoulTaskVisualState::default()` を含めること |
| `Changed<T>` の初回フレーム未発火 | 初期状態でミラーが空 | `Added<T>` も同一システムの Filter に含めること（`Or<(Changed<T>, Added<T>)>` パターン） |
| ミラーコンポーネントを spawn 時に追加し忘れ | sync システムがスキップされる | `FloorTileBlueprint`/`WallTileBlueprint`/`FloorConstructionSite`/`WallConstructionSite` の spawn 箇所を全て調査して追加する |
| hw_jobs にプラグイン構造がない | sync システムの登録場所が不明 | `bevy_app/src/plugins/logic.rs` の適切なシステムセットに追加する |
| `bucket_transport_data()` メソッドが存在しない | コンパイルエラー | M2 着手前に `AssignedTask` の実際の API（`impl` ブロック）を確認すること |
| `FloorTileStateMirror` と `FloorTileState` の enum variants 同期ズレ | コンパイルエラーか表示不具合 | `FloorTileState` に新バリアントが追加された場合、`FloorTileStateMirror` も更新する必要がある（`match` の網羅性チェックで発見できる） |

---

## 7. 検証計画

**必須（各マイルストーン後）**:
```bash
CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check -p hw_visual
CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check
```

**手動確認シナリオ（M4 完了後）**:
1. 採掘/伐採: 木・岩のハイライト表示と作業者アイコン（斧/ツルハシ）が切り替わる
2. 建設: Blueprint の進捗バーとマテリアル表示が更新される
3. 建設タイル: Floor/Wall 建設サイトのタイル色が施工フェーズで変わる（骨マーカー含む）
4. ロジスティクス: Soul が物を運ぶ際のアイテムスプライト表示が正しい
5. 手押し車: wheelbarrow_follow が正常動作する（WheelbarrowMarker フィルタ）
6. プログレスバー: タスク実行中に Soul のバーが表示・更新される
7. タスクリンク gizmo: Soul → ターゲットへの線と bucket_link が正しく描画される
8. RestArea 夢パーティクル: 休息エリアからパーティクルが出る（capacity に応じた量）

---

## 8. ロールバック方針

各マイルストーンを独立コミットにするため、失敗したマイルストーンのコミットのみ `git revert` できる。

---

## 9. AI 引継ぎメモ（最重要）

### 現在地

- 進捗: `0%`
- 完了済みマイルストーン: なし
- 未着手: M1（グループ A ミラー化）から着手

### 次の AI が最初にやること

1. `crates/hw_core/src/lib.rs` を読んで `visual_mirror` モジュールの配置を確認する
2. `crates/hw_jobs/src/assigned_task.rs` の **全バリアントとフィールド名** を確認する（M2 同期システムのコードと突き合わせ）
3. `crates/bevy_app/src/plugins/logic.rs` を読んでシステム登録の場所・`IntoScheduleLabel` の形式を確認する
4. `crates/hw_core/src/visual_mirror/` ディレクトリを作成して M1 に着手する

### ブロッカー / 注意点

- **型は移動しない**: ドメイン型は hw_jobs/hw_logistics に残す。hw_core には**ミラー専用の別型**を追加する
- **`WheelbarrowMarker` の OnAdd Observer は hw_logistics が担当**: hw_logistics の `Wheelbarrow` に対して Observer を登録する。hw_jobs ではない
- **`SoulTaskVisualState` の初回同期**: `Added<AssignedTask>` を必ず `Changed<AssignedTask>` と OR で処理すること
- **hw_jobs / hw_logistics プラグインなし**: hw_jobs は Plugin を持たない。hw_logistics も TransportRequestPlugin というサブプラグインのみでトップレベル Plugin はない。よって両クレートの sync システム・Observer はすべて `bevy_app/src/plugins/logic.rs` で登録する
- **工事タイルミラーの spawn 忘れ注意**: `FloorTileBlueprint`/`WallTileBlueprint` の spawn 箇所は `bevy_app/src/systems/jobs/` 配下にある。`FloorTileVisualMirror`/`WallTileVisualMirror` をそこに追加する
- **スコープ外を触らない**: `mud_mixer.rs`/`tank.rs` は本計画では扱わない

### 参照必須ファイル

- `docs/proposals/hw-visual-domain-decoupling.md`（設計判断の根拠）
- `crates/hw_jobs/src/assigned_task.rs`（M2 同期システムのバリアント名確認）
- `crates/hw_visual/src/soul/mod.rs`（AssignedTask 参照が最も複雑）
- `crates/bevy_app/src/plugins/logic.rs`（システム登録場所）
- `crates/bevy_app/src/entities/damned_soul/spawn.rs`（Soul スポーンバンドル）
- `crates/bevy_app/src/systems/jobs/floor_construction/`（タイル spawn 箇所）

### Definition of Done

- [ ] M1〜M3 の対象ファイル 13 個で `use hw_jobs::` / `use hw_logistics::` が 0
- [ ] Out of Scope 3 ファイル（`mud_mixer.rs`/`tank.rs`/`wall_connection.rs`）の残存依存が文書化されている
- [ ] `cargo check` がワークスペース全体で成功
- [ ] 手動確認シナリオ 1〜8 が全て正常動作

---

## 10. 更新履歴

| 日付 | 変更者 | 内容 |
|---|---|---|
| `2026-03-14` | Claude | 初版作成（旧提案に基づく型移動アプローチ） |
| `2026-03-15` | Claude | 提案書レビュー反映：型移動案を廃止しミラーコンポーネントに全面変更。テンプレート形式に合わせて構造を整理 |
| `2026-03-15` | Claude | コードベース精査によるブラッシュアップ：RestArea.capacity 問題修正、WorkType enum 名修正、CarryingItemVisual 名前衝突解決、構造タイルミラーの粒度問題修正、Soul spawn 登録場所特定、hw_jobs プラグインなし問題の明記 |
