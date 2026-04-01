# Phase 1b: Soul Spa + GeneratePower Task

| Item | Value |
|:---|:---|
| Status | Not started |
| Depends on | Phase 1a (`crates/hw_energy`: PowerGrid/Generator/Consumer/Relationships/Constants 実装済) |
| Blocks | Phase 1c |

## Goal

Soul Spa（2x2 建物）を実装し、Soul が GeneratePower タスクを実行できるようにする。Dream を消費しながら発電し、Dream 不足で自動離脱。フェーズ終了時点で `PowerGenerator.current_output` が稼働 Soul 数を反映する。

グリッド再計算・powered/unpowered サイクルは Phase 1c。ビジュアル（スプライト・アニメーション）も Phase 1c。

---

## 設計まとめ（決定済み）

| 決定事項 | 内容 |
|:---|:---|
| 建設フロー | **フェーズ遷移方式（A-2）**: 配置時から `SoulSpaSite(Constructing)` + `SoulSpaTile x4` を spawn。骨 12 本搬入完了で `Operational` に遷移 |
| GeneratesFor 接続 | 配置確定時（Yard が確定しているタイミング）に `GeneratesFor(power_grid)` を insert。完了 Observer 不要 |
| ウォーカビリティ | 建設中・稼働中とも **常に walkable**。WorldMap への obstacle 登録なし |
| タスク離脱 | `soul.dream <= DREAM_GENERATE_FLOOR` で自動 clear（Refine の材料切れパターンと同じ） |
| active_slots | 数値キャップのみ。per-tile ON/OFF なし |
| ロジスティクス | bevy_app 側の新規モジュール `soul_spa_construction/` に集約（hw_logistics への hw_energy 依存を避けるため） |

---

## 実装ステップ

### Step 1: `crates/hw_energy/src/soul_spa.rs` — エンティティ定義（新規作成）

`hw_energy/src/lib.rs` に `pub mod soul_spa;` を追記。

```rust
// crates/hw_energy/src/soul_spa.rs
use bevy::prelude::*;
use crate::components::PowerGenerator;
use crate::constants::SOUL_SPA_BONE_COST_PER_TILE;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Reflect, Default)]
pub enum SoulSpaPhase {
    #[default]
    Constructing,
    Operational,
}

/// Soul Spa サイト全体を管理するエンティティ。配置時に spawn される。
/// `#[require(PowerGenerator)]` で発電コンポーネントを自動付与。
/// PowerGenerator::default() で output_per_soul = OUTPUT_PER_SOUL が設定される。
#[derive(Component, Reflect, Debug, Clone)]
#[reflect(Component)]
#[require(PowerGenerator)]
pub struct SoulSpaSite {
    pub phase: SoulSpaPhase,
    pub active_slots: u32,       // 0..=4, default 4
    pub tiles_total: u32,        // 常に 4
    pub bones_delivered: u32,
    pub bones_required: u32,     // = tiles_total * SOUL_SPA_BONE_COST_PER_TILE = 12
}

impl Default for SoulSpaSite {
    fn default() -> Self {
        let tiles_total = 4u32;
        Self {
            phase: SoulSpaPhase::Constructing,
            active_slots: tiles_total,
            tiles_total,
            bones_delivered: 0,
            bones_required: tiles_total * SOUL_SPA_BONE_COST_PER_TILE,
        }
    }
}

impl SoulSpaSite {
    /// active_slots と実際の占有数（位相計算後に使用）を比較してスロット空きを確認
    pub fn has_available_slot(&self, occupied: u32) -> bool {
        self.phase == SoulSpaPhase::Operational && occupied < self.active_slots
    }
}

/// SoulSpaSite の子エンティティ。ChildOf(site) で親に接続。
#[derive(Component, Reflect, Debug, Clone)]
#[reflect(Component)]
pub struct SoulSpaTile {
    pub parent_site: Entity,
    pub grid_pos: (i32, i32),
}

impl Default for SoulSpaTile {
    fn default() -> Self {
        Self { parent_site: Entity::PLACEHOLDER, grid_pos: (0, 0) }
    }
}
```

**エンティティ構造（稼働時）**:
```
SoulSpaSite                      ← PowerGenerator は #[require] で自動付与
├─ GeneratesFor(power_grid)      ← 配置時に insert
└─ ChildOf/children:
   SoulSpaTile x4 (2x2)
   ├─ grid_pos: (i32, i32)
   ├─ Designation(GeneratePower) ← Operational 遷移時に insert
   ├─ TaskSlots { max: 1 }       ← 同上
   └─ TaskWorkers                ← WorkingOn Relationship で自動管理
```

---

### Step 2: `crates/hw_jobs/src/model.rs` — BuildingType::SoulSpa

`BuildingType` enum に追加:
```rust
SoulSpa,
```

`category()` に追加:
```rust
BuildingType::SoulSpa => BuildingCategory::Plant,
```

`hw_jobs/src/model.rs` に `TargetSoulSpaSite` コンポーネントを追加（TransportRequest のアンカー）:
```rust
/// TransportRequest エンティティに付与。搬入先 SoulSpaSite への参照。
#[derive(Component, Reflect, Debug, Clone, Copy)]
#[reflect(Component)]
pub struct TargetSoulSpaSite(pub Entity);
```

---

### Step 3: `crates/hw_core/src/jobs.rs` — WorkType::GeneratePower

```rust
pub enum WorkType {
    // ... 既存 ...
    GeneratePower,  // ← 追加
}
```

---

### Step 4: `crates/hw_jobs/src/tasks/generate_power.rs` — 新規ファイル

`hw_jobs/src/tasks/mod.rs` に `pub mod generate_power;` + `pub use generate_power::*;` を追記。

```rust
// crates/hw_jobs/src/tasks/generate_power.rs
use bevy::prelude::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Reflect, Default)]
pub enum GeneratePowerPhase {
    #[default]
    GoingToTile,
    Generating,
}

#[derive(Clone, Debug, Reflect)]
pub struct GeneratePowerData {
    pub target_tile: Entity,
    pub phase: GeneratePowerPhase,
}

impl Default for GeneratePowerData {
    fn default() -> Self {
        Self { target_tile: Entity::PLACEHOLDER, phase: GeneratePowerPhase::default() }
    }
}
```

`hw_jobs/src/tasks/mod.rs` の `AssignedTask` に追加:
```rust
GeneratePower(GeneratePowerData),
```

`work_type()` に追加:
```rust
AssignedTask::GeneratePower(_) => Some(WorkType::GeneratePower),
```

`get_target_entity()` に追加:
```rust
AssignedTask::GeneratePower(data) => Some(data.target_tile),
```

---

### Step 5: `crates/hw_core/src/visual_mirror/task.rs` — VisualMirror

`SoulTaskPhaseVisual` enum に追加:
```rust
GeneratePower,
```

`hw_jobs/src/visual_sync/sync.rs` の `sync_soul_task_visual_system` に追加:
```rust
AssignedTask::GeneratePower(d) => (
    SoulTaskPhaseVisual::GeneratePower,
    None,
    Some(d.target_tile),
    None,
),
```

---

### Step 6: `crates/hw_soul_ai/src/soul_ai/execute/task_execution/generate_power.rs` — タスク実行（新規）

`mod.rs` に `pub mod generate_power;` を追記。

```rust
// crates/hw_soul_ai/src/soul_ai/execute/task_execution/generate_power.rs
use super::common::*;
use super::context::TaskExecutionContext;
use super::types::{AssignedTask, GeneratePowerData, GeneratePowerPhase};
use bevy::prelude::*;
use hw_energy::{DREAM_CONSUME_RATE_GENERATING, DREAM_GENERATE_FLOOR, FATIGUE_RATE_GENERATING};
use hw_core::relationships::WorkingOn;
use hw_world::WorldMap;

pub fn handle_generate_power_task(
    ctx: &mut TaskExecutionContext,
    data: GeneratePowerData,
    commands: &mut Commands,
    time: &Res<Time>,
    world_map: &WorldMap,
) {
    let soul_pos = ctx.soul_pos();

    match data.phase {
        GeneratePowerPhase::GoingToTile => {
            // タイルが存在するか確認
            let Ok(tile_transform) = ctx.queries.soul_spa_tiles.get(data.target_tile) else {
                clear_task_and_path(ctx.task, ctx.path);
                return;
            };
            let tile_pos = tile_transform.translation.truncate();

            let reachable = update_destination_to_adjacent(
                ctx.dest, tile_pos, ctx.path, soul_pos, world_map, ctx.pf_context,
            );
            if !reachable {
                info!("GENERATE_POWER: Soul {:?} cannot reach tile {:?}, canceling", ctx.soul_entity, data.target_tile);
                clear_task_and_path(ctx.task, ctx.path);
                return;
            }

            if is_near_target_or_dest(soul_pos, tile_pos, ctx.dest.0) {
                commands.entity(ctx.soul_entity).insert(WorkingOn(data.target_tile));
                *ctx.task = AssignedTask::GeneratePower(GeneratePowerData {
                    target_tile: data.target_tile,
                    phase: GeneratePowerPhase::Generating,
                });
                ctx.path.waypoints.clear();
            }
        }

        GeneratePowerPhase::Generating => {
            // タイル消滅チェック（サイトが despawn された場合など）
            if ctx.queries.soul_spa_tiles.get(data.target_tile).is_err() {
                commands.entity(ctx.soul_entity).remove::<WorkingOn>();
                clear_task_and_path(ctx.task, ctx.path);
                return;
            }

            let dt = time.delta_secs();

            // Dream 消費
            ctx.soul.dream -= DREAM_CONSUME_RATE_GENERATING * dt;
            ctx.soul.dream = ctx.soul.dream.max(0.0);

            // 疲労蓄積（瞑想的行為のため低レート）
            ctx.soul.fatigue = (ctx.soul.fatigue + FATIGUE_RATE_GENERATING * dt).min(1.0);

            // Dream 閾値チェック → 自動離脱（Refine の材料切れと同パターン）
            if ctx.soul.dream <= DREAM_GENERATE_FLOOR {
                info!("GENERATE_POWER: Soul {:?} dream too low, disengaging", ctx.soul_entity);
                commands.entity(ctx.soul_entity).remove::<WorkingOn>();
                clear_task_and_path(ctx.task, ctx.path);
            }
        }
    }
}
```

`context/queries.rs` の `TaskQueries` に追加:
```rust
pub soul_spa_tiles: Query<'w, 's, &'static Transform, With<hw_energy::SoulSpaTile>>,
```

`handler/dispatch.rs` の `run_task_handler` に追加:
```rust
AssignedTask::GeneratePower(data) => {
    crate::soul_ai::execute::task_execution::generate_power::handle_generate_power_task(
        ctx, data.clone(), commands, time, world_map,
    );
}
```

---

### Step 7: `crates/hw_soul_ai/src/.../dream_update.rs` — Dream 蓄積スキップ

`dream_update_system` の「非睡眠・非休憩中の蓄積ブロック」直前に挿入:

```rust
use hw_jobs::tasks::{AssignedTask, GeneratePowerPhase};

// 発電中は Dream 消費がタスク実行側で行われるため、蓄積をスキップ
let is_generating = matches!(
    *task,
    AssignedTask::GeneratePower(ref d) if d.phase == GeneratePowerPhase::Generating
);
if is_generating {
    continue;  // ← 蓄積も排出もスキップ
}
```

このガードは `if !is_sleeping { ... }` ブロックの最初（`dream.quality` リセットの直後）に挿入する。

---

### Step 8: `crates/hw_familiar_ai/src/` — Familiar 統合

#### 8-1. `task_management/context.rs` の `FamiliarTaskAssignmentQueries` に追加

```rust
pub soul_spa_sites: Query<'w, 's, &'static hw_energy::SoulSpaSite>,
pub soul_spa_tiles: Query<'w, 's, (&'static hw_energy::SoulSpaTile, Option<&'static TaskWorkers>)>,
```

#### 8-2. `task_management/builders/basic.rs` に追加

```rust
use hw_energy::SoulSpaPhase;
use hw_jobs::{AssignedTask, GeneratePowerData, GeneratePowerPhase};
use hw_jobs::WorkType;

pub fn issue_generate_power(
    task_pos: Vec2,
    already_commanded: bool,
    ctx: &AssignTaskContext<'_>,
    queries: &mut FamiliarTaskAssignmentQueries,
    shadow: &mut ReservationShadow,
) {
    let assigned_task = AssignedTask::GeneratePower(GeneratePowerData {
        target_tile: ctx.task_entity,
        phase: GeneratePowerPhase::GoingToTile,
    });
    submit_assignment_with_source_entities(
        ctx,
        queries,
        shadow,
        TaskTarget { work_type: WorkType::GeneratePower, task_pos },
        assigned_task,
        &[ctx.task_entity],
        already_commanded,
    );
}
```

#### 8-3. `task_management/policy/basic.rs` に追加

```rust
// assign_by_work_type の match 内
WorkType::GeneratePower => {
    assign_generate_power(task_pos, already_commanded, ctx, queries, shadow)
}

// 新関数
pub(super) fn assign_generate_power(
    task_pos: Vec2,
    already_commanded: bool,
    ctx: &AssignTaskContext<'_>,
    queries: &mut FamiliarTaskAssignmentQueries,
    shadow: &mut ReservationShadow,
) -> bool {
    // タイルの parent_site を引き、active_slots ゲートを確認
    let Ok((tile, workers_opt)) = queries.soul_spa_tiles.get(ctx.task_entity) else {
        return false;
    };
    let Ok(site) = queries.soul_spa_sites.get(tile.parent_site) else {
        return false;
    };
    let occupied = queries.soul_spa_tiles
        .iter()
        .filter(|(t, w)| t.parent_site == tile.parent_site && w.map(|w| !w.is_empty()).unwrap_or(false))
        .count() as u32;
    if !site.has_available_slot(occupied) {
        return false;
    }
    // 既存の WorkingOn スロットチェック（TaskSlots と TaskWorkers）も filter.rs が行う
    issue_generate_power(task_pos, already_commanded, ctx, queries, shadow);
    true
}
```

#### 8-4. `policy/mod.rs` の `assign_by_work_type` に arm を追加

```rust
WorkType::GeneratePower => {
    basic::assign_generate_power(task_pos, already_commanded, ctx, queries, shadow)
}
```

#### 8-5. タスク優先度（`task_finder/score.rs`）

`score_candidate` でデフォルト優先度のまま（追加調整なし）。Build/ReinforceFloor など construction タスクはすでに `priority += 10` されているため、GeneratePower は自然に低優先となる。

---

### Step 9: `crates/bevy_app/src/interface/selection/soul_spa_place/` — 配置 UI（新規モジュール）

#### 9-1. `crates/hw_core/src/game_state.rs` に `TaskMode::SoulSpaPlace` を追加

```rust
pub enum TaskMode {
    // ... 既存（FloorPlace, WallPlace 等）...
    SoulSpaPlace(Option<Vec2>),  // Soul Spa 2x2 配置中（FloorPlace と同型）
}
```

`get_drag_start()` にも arm を追加:
```rust
TaskMode::SoulSpaPlace(s) => s,
```

#### 9-2. 新規ディレクトリ: `bevy_app/src/interface/selection/soul_spa_place/`

**`mod.rs`**:
```rust
mod input;
mod spawn;

pub use input::soul_spa_place_input_system;
pub use spawn::spawn_soul_spa;
```

**`input.rs`**（単一クリックで 2x2 配置）:
```rust
pub fn soul_spa_place_input_system(
    input: Res<ButtonInput<MouseButton>>,
    ui_state: Res<UiInputState>,
    q_window: Query<&Window, With<PrimaryWindow>>,
    q_camera: Query<(&Camera, &GlobalTransform), With<MainCamera>>,
    mut task_mode: ResMut<TaskMode>,
    world_map: WorldMapRead,
    q_yards: Query<&Yard>,
    q_grids: Query<&YardPowerGrid>,    // PowerGrid 逆引き用
    q_power_grids: Query<(Entity, &YardPowerGrid)>,
    mut commands: Commands,
) {
    if ui_state.pointer_over_ui { return; }
    if !input.just_pressed(MouseButton::Left) { return; }

    let Some(world_pos) = hw_ui::camera::world_cursor_pos(&q_window, &q_camera) else { return; };
    let (gx, gy) = WorldMap::world_to_grid(world_pos);

    // 2x2 フットプリント: [(gx, gy), (gx+1, gy), (gx, gy-1), (gx+1, gy-1)]
    let tiles = [(gx, gy), (gx + 1, gy), (gx, gy - 1), (gx + 1, gy - 1)];

    // バリデーション: 全タイルが Yard 内かつ walkable かつ建物なし
    let all_valid = tiles.iter().all(|&(tx, ty)| {
        let wpos = WorldMap::grid_to_world(tx, ty);
        let in_yard = q_yards.iter().any(|y| y.contains(wpos));
        let walkable = world_map.is_walkable(tx, ty);
        let no_building = world_map.building_entity((tx, ty)).is_none();
        in_yard && walkable && no_building
    });

    if !all_valid { return; }

    // 配置 → SoulSpaSite + SoulSpaTile x4 をスポーン
    let center_pos = WorldMap::grid_to_world(gx, gy);
    spawn::spawn_soul_spa(&mut commands, &q_power_grids, &q_yards, tiles, center_pos);

    *task_mode = TaskMode::None;
}
```

**`spawn.rs`**（SoulSpaSite + SoulSpaTile + GeneratesFor 接続）:
```rust
use hw_energy::{GeneratesFor, SoulSpaSite, SoulSpaTile, YardPowerGrid};
use hw_world::zones::Yard;

pub fn spawn_soul_spa(
    commands: &mut Commands,
    q_yards: &Query<(Entity, &Yard)>,
    q_power_grids: &Query<(Entity, &YardPowerGrid)>,
    tiles: [(i32, i32); 4],
    center_pos: Vec2,
) {
    // Yard → PowerGrid 逆引き（find_owner_yard パターン）
    let yard_entity = q_yards.iter()
        .find(|(_, yard)| yard.contains(center_pos))
        .map(|(e, _)| e);

    let power_grid_entity = yard_entity.and_then(|ye| {
        q_power_grids.iter().find(|(_, ypg)| ypg.0 == ye).map(|(g, _)| g)
    });

    let site_entity = commands.spawn((
        SoulSpaSite::default(),
        Transform::from_translation(center_pos.extend(Z_MAP)),
        Visibility::default(),
        Name::new("SoulSpaSite"),
    )).id();

    // GeneratesFor 接続（Yard が存在する場合のみ）
    if let Some(grid) = power_grid_entity {
        commands.entity(site_entity).insert(GeneratesFor(grid));
    }

    // SoulSpaTile x4 を子としてスポーン
    for (gx, gy) in tiles {
        let tile_pos = WorldMap::grid_to_world(gx, gy);
        commands.spawn((
            SoulSpaTile { parent_site: site_entity, grid_pos: (gx, gy) },
            ChildOf(site_entity),
            Transform::from_translation(tile_pos.extend(Z_MAP + 0.01)),
            Visibility::default(),
            Name::new("SoulSpaTile"),
        ));
    }
}
```

**注**: `Z_MAP` は `hw_core::constants` から。`Yard::contains()` の実際のシグネチャは実装時に確認すること（`contains(Vec2)` or `contains_grid(i32, i32)` 等）。

---

### Step 10: `crates/bevy_app/src/systems/jobs/soul_spa_construction/` — 建設ロジスティクス（新規モジュール）

新規ファイル一覧:
- `mod.rs`
- `auto_haul.rs` — Bone の TransportRequest 自動生成
- `delivery_sync.rs` — 搬入検知 + フェーズ遷移

#### 10-1. `hw_logistics/src/transport_request/kinds.rs` に追加

```rust
pub enum TransportRequestKind {
    // ... 既存 ...
    DeliverToSoulSpa,
}
```

#### 10-2. `auto_haul.rs`

```rust
/// SoulSpaPhase::Constructing のサイトに Bone の TransportRequest を生成。
/// floor_construction_auto_haul_system と同じパターン。
pub fn soul_spa_auto_haul_system(
    mut commands: Commands,
    q_sites: Query<(Entity, &Transform, &SoulSpaSite)>,
    q_existing: Query<(&TargetSoulSpaSite, &TransportRequest, Option<&TaskWorkers>)>,
    resource_grid: Res<ResourceSpatialGrid>,
    // ... familiar/yard queries for area ownership ...
) {
    // 在庫中フライト数の集計
    let mut in_flight: HashMap<Entity, u32> = HashMap::new();
    for (target, req, workers) in q_existing.iter() {
        if req.kind == TransportRequestKind::DeliverToSoulSpa {
            let assigned = workers.map(|w| w.len()).unwrap_or(0) as u32;
            if assigned > 0 {
                *in_flight.entry(target.0).or_default() += assigned;
            }
        }
    }

    for (site_entity, transform, site) in q_sites.iter() {
        if site.phase != SoulSpaPhase::Constructing { continue; }

        let remaining = site.bones_required
            .saturating_sub(site.bones_delivered)
            .saturating_sub(*in_flight.get(&site_entity).unwrap_or(&0));

        if remaining == 0 { continue; }

        let site_pos = transform.translation.truncate();

        // TransportRequest を spawn（blueprint.rs の DeliverToBlueprint パターン参考）
        commands.spawn((
            TransportRequest {
                kind: TransportRequestKind::DeliverToSoulSpa,
                resource_type: ResourceType::Bone,
                amount: remaining.min(MAX_HAUL_PER_REQUEST),  // 定数は既存から流用
                anchor: site_pos,
                ..default()
            },
            TargetSoulSpaSite(site_entity),
            Transform::from_translation(site_pos.extend(0.0)),
            Name::new("TransportReq(SoulSpa Bone)"),
        ));
    }
}
```

#### 10-3. `delivery_sync.rs`

```rust
/// SoulSpaSite 周辺の Bone ResourceItem を走査し、骨搬入カウンタを更新。
/// bones_delivered >= bones_required で Operational に遷移。
/// 既存の `sync_construction_delivery` (floor_construction) と同パターンで
/// `ResourceSpatialGrid` を使って O(1) 周辺検索する。
pub fn soul_spa_delivery_sync_system(
    mut commands: Commands,
    mut q_sites: Query<(Entity, &Transform, &mut SoulSpaSite)>,
    q_resources: Query<(Entity, &Transform, &ResourceItem), Without<Designation>>,
    resource_grid: Res<ResourceSpatialGrid>,
) {
    const PICKUP_RADIUS: f32 = TILE_SIZE * 1.5;  // hw_core::constants

    for (site_entity, site_transform, mut site) in q_sites.iter_mut() {
        if site.phase != SoulSpaPhase::Constructing { continue; }

        let site_pos = site_transform.translation.truncate();

        // ResourceSpatialGrid で周辺の Bone を O(1) 検索・消費
        // （実装時は collect_nearby_resource_entities パターンを参照:
        //   hw_logistics/src/transport_request/producer/floor_construction.rs）
        let mut consumed = 0u32;
        for (res_entity, res_transform, res_item) in q_resources.iter() {
            if res_item.0 != ResourceType::Bone { continue; }
            let dist = res_transform.translation.truncate().distance(site_pos);
            if dist <= PICKUP_RADIUS {
                commands.entity(res_entity).despawn();
                consumed += 1;
            }
        }

        if consumed == 0 { continue; }

        site.bones_delivered = (site.bones_delivered + consumed).min(site.bones_required);

        if site.bones_delivered >= site.bones_required {
            // Operational に遷移
            site.phase = SoulSpaPhase::Operational;
            info!("SoulSpaSite {:?} construction complete, transitioning to Operational", site_entity);

            // 各 SoulSpaTile に Designation + TaskSlots を付与
            // ← tile エンティティ一覧は ChildOf 逆引きで取得
            // （systems/jobs/soul_spa_construction/delivery_sync.rs では
            //   Query<&Children, With<SoulSpaSite>> + q_tiles から取得）
        }
    }
}

/// Operational 遷移後の Tile 初期化サブシステム（delivery_sync から commands.trigger で呼ぶか、
/// 直後のシステムとして分けてもよい）
pub fn soul_spa_tile_activate_system(
    mut commands: Commands,
    q_sites: Query<(&SoulSpaSite, &Children), Changed<SoulSpaSite>>,
    q_tiles: Query<&SoulSpaTile>,
) {
    for (site, children) in q_sites.iter() {
        if site.phase != SoulSpaPhase::Operational { continue; }
        for &child in children.iter() {
            if q_tiles.get(child).is_ok() {
                commands.entity(child).insert((
                    Designation { work_type: WorkType::GeneratePower },
                    TaskSlots { max: 1 },
                ));
            }
        }
    }
}
```

---

### Step 11: `crates/bevy_app/src/systems/energy/` — 発電出力集計

`bevy_app/src/systems/energy/mod.rs` に追加: `pub mod power_output;`

```rust
// crates/bevy_app/src/systems/energy/power_output.rs

/// Soul Spa の稼働タイル数から PowerGenerator.current_output を更新。
/// FixedUpdate (0.5s 程度) で実行。Phase 1c で Changed<PowerGenerator> を検知してグリッド再計算する。
pub fn soul_spa_power_output_system(
    q_sites: Query<(Entity, &SoulSpaSite, &mut PowerGenerator)>,
    q_tiles: Query<(&SoulSpaTile, Option<&TaskWorkers>)>,
) {
    for (site_entity, site, mut generator) in q_sites.iter_mut() {
        if site.phase != SoulSpaPhase::Operational { continue; }

        let occupied = q_tiles
            .iter()
            .filter(|(tile, workers)| {
                tile.parent_site == site_entity
                    && workers.map(|w| !w.is_empty()).unwrap_or(false)
            })
            .count() as f32;

        let new_output = occupied * generator.output_per_soul;
        if (generator.current_output - new_output).abs() > f32::EPSILON {
            generator.current_output = new_output;
        }
    }
}
```

---

### Step 12: スロット制御 UI

稼働中の SoulSpaSite をクリックした際のインスペクタパネル:

- `active_slots (0..=4)` を `+/-` ボタンで操作
- 減少時: 即時 kick なし。既存作業者は自然に完了・離脱後に補充されない
- 実装場所: `bevy_app/src/interface/ui/interaction/handlers/` 内に新規ハンドラ or 既存 `building_inspector` に追加
- `active_slots` の変更は `commands.entity(site).insert(SoulSpaSite { active_slots: new_val, ..site })` で更新

---

### Step 13: プラグイン登録 (`bevy_app/src/plugins/logic.rs`)

#### register_type に追加
```rust
app.register_type::<SoulSpaSite>()
   .register_type::<SoulSpaTile>()
   .register_type::<SoulSpaPhase>()
   .register_type::<GeneratePowerData>()
   .register_type::<GeneratePowerPhase>()
   .register_type::<TargetSoulSpaSite>();
```

#### add_systems に追加
```rust
// Logic セットに追加（既存の soul_spa 関連システム）
.add_systems(
    Update,
    (
        soul_spa_auto_haul_system,
        soul_spa_delivery_sync_system,
        soul_spa_tile_activate_system,
    ).in_set(GameSystemSet::Logic),
)
// FixedUpdate で発電集計（0.5s 間隔程度）
.add_systems(
    FixedUpdate,
    soul_spa_power_output_system,
)
```

#### 配置 UI システムを TaskDesignation モード内で追加
```rust
// 既存の TaskDesignation 系システムグループ内に追加
// TaskMode::SoulSpaPlace のとき実行される（TaskMode は Resource なのでシステム内で if ガード）
.add_systems(
    Update,
    soul_spa_place_input_system
        .run_if(in_state(PlayMode::TaskDesignation)),
)
```

#### インポートに追加
```rust
use hw_energy::{SoulSpaSite, SoulSpaTile, SoulSpaPhase};
use hw_jobs::tasks::{GeneratePowerData, GeneratePowerPhase};
use hw_jobs::model::TargetSoulSpaSite;
use crate::systems::jobs::soul_spa_construction::{
    soul_spa_auto_haul_system, soul_spa_delivery_sync_system, soul_spa_tile_activate_system,
};
use crate::systems::energy::power_output::soul_spa_power_output_system;
use crate::interface::selection::soul_spa_place::soul_spa_place_input_system;
```

---

## Cargo.toml 変更点

| ファイル | 追加依存 |
|:---|:---|
| `crates/hw_soul_ai/Cargo.toml` | `hw_energy = { path = "../hw_energy" }` |
| `crates/hw_familiar_ai/Cargo.toml` | `hw_energy = { path = "../hw_energy" }` |

`hw_logistics` は hw_energy への直接依存を持たない（logistics 系システムを bevy_app 側に置くことで回避）。

---

## 変更ファイル一覧

| クレート | ファイル | 操作 |
|:---|:---|:---|
| `hw_energy` | `src/soul_spa.rs` | **新規** SoulSpaSite / SoulSpaTile / SoulSpaPhase |
| `hw_energy` | `src/lib.rs` | `pub mod soul_spa; pub use soul_spa::*;` |
| `hw_jobs` | `src/model.rs` | BuildingType::SoulSpa + TargetSoulSpaSite |
| `hw_jobs` | `src/tasks/generate_power.rs` | **新規** GeneratePowerData / Phase |
| `hw_jobs` | `src/tasks/mod.rs` | AssignedTask::GeneratePower + work_type/get_target_entity |
| `hw_core` | `src/jobs.rs` | WorkType::GeneratePower |
| `hw_core` | `src/visual_mirror/task.rs` | SoulTaskPhaseVisual::GeneratePower |
| `hw_core` | `src/game_state.rs` | TaskMode::SoulSpaPlace |
| `hw_jobs` | `src/visual_sync/sync.rs` | AssignedTask::GeneratePower の sync ケース追加 |
| `hw_logistics` | `src/transport_request/kinds.rs` | TransportRequestKind::DeliverToSoulSpa |
| `hw_soul_ai` | `Cargo.toml` | `hw_energy` 依存追加 |
| `hw_soul_ai` | `src/soul_ai/execute/task_execution/generate_power.rs` | **新規** タスク実行 |
| `hw_soul_ai` | `src/soul_ai/execute/task_execution/mod.rs` | `pub mod generate_power;` |
| `hw_soul_ai` | `src/soul_ai/execute/task_execution/context/queries.rs` | `soul_spa_tiles` クエリ追加 |
| `hw_soul_ai` | `src/soul_ai/execute/task_execution/handler/dispatch.rs` | GeneratePower arm 追加 |
| `hw_soul_ai` | `src/soul_ai/update/dream_update.rs` | Generating 中の蓄積スキップ |
| `hw_familiar_ai` | `Cargo.toml` | `hw_energy` 依存追加 |
| `hw_familiar_ai` | `src/familiar_ai/decide/task_management/context.rs` | SoulSpa クエリ追加 |
| `hw_familiar_ai` | `src/familiar_ai/decide/task_management/builders/basic.rs` | `issue_generate_power` 追加 |
| `hw_familiar_ai` | `src/familiar_ai/decide/task_management/policy/basic.rs` | `assign_generate_power` 追加 |
| `hw_familiar_ai` | `src/familiar_ai/decide/task_management/policy/mod.rs` | WorkType::GeneratePower arm 追加 |
| `bevy_app` | `src/interface/selection/soul_spa_place/mod.rs` | **新規** |
| `bevy_app` | `src/interface/selection/soul_spa_place/input.rs` | **新規** クリック配置 |
| `bevy_app` | `src/interface/selection/soul_spa_place/spawn.rs` | **新規** SoulSpaSite spawn |
| `bevy_app` | `src/interface/selection/mod.rs` | `pub mod soul_spa_place;` |
| `bevy_app` | `src/systems/jobs/soul_spa_construction/mod.rs` | **新規** |
| `bevy_app` | `src/systems/jobs/soul_spa_construction/auto_haul.rs` | **新規** TransportRequest 生成 |
| `bevy_app` | `src/systems/jobs/soul_spa_construction/delivery_sync.rs` | **新規** 搬入検知 + 遷移 |
| `bevy_app` | `src/systems/jobs/mod.rs` | `pub mod soul_spa_construction;` |
| `bevy_app` | `src/systems/energy/power_output.rs` | **新規** 発電集計 |
| `bevy_app` | `src/systems/energy/mod.rs` | `pub mod power_output;` |
| `bevy_app` | `src/plugins/logic.rs` | 全システム・型登録 |

---

## 完了基準

- [ ] Soul Spa を 2x2 でクリック配置できる（Yard + walkable バリデーション付き）
- [ ] 建設中: Bone 12 本の TransportRequest が自動発行される
- [ ] 骨搬入完了後に Operational へ遷移し、Designation(GeneratePower) が付与される
- [ ] Familiar が Soul を Soul Spa タイルに割り当てる（active_slots ゲート動作）
- [ ] Soul がタイルまで経路移動 → Generating フェーズに入る
- [ ] 発電中に Dream が `DREAM_CONSUME_RATE_GENERATING` で減少する
- [ ] Dream が `DREAM_GENERATE_FLOOR` 以下で自動離脱（WorkingOn 削除 + タスククリア）
- [ ] 発電中は `dream_update_system` の Dream 蓄積がスキップされる
- [ ] `PowerGenerator.current_output` が占有 Soul 数 × `OUTPUT_PER_SOUL` を反映する
- [ ] `active_slots` UI で上限を下げると新規割り当てが止まる
- [ ] `GeneratesFor` Relationship が Yard の PowerGrid に接続されている
- [ ] `CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check` がエラーなし
- [ ] `cargo clippy --workspace` 0 warnings

---

## AI Handoff

### 開始前に確認

1. このファイル + `milestone-roadmap.md` を読む
2. Phase 1a 完了確認: `crates/hw_energy/` の 4 ファイル + `bevy_app/src/systems/energy/grid_lifecycle.rs` が存在し `cargo check` が通ること
3. 以下の参照コードを読む:

| 目的 | 参照ファイル |
|:---|:---|
| フェーズ遷移方式の建設完了 | `crates/bevy_app/src/systems/jobs/floor_construction/completion.rs` |
| TransportRequest 自動生成パターン | `crates/hw_logistics/src/transport_request/producer/floor_construction.rs`（先頭 100 行） |
| タスク実行フレームワーク | `crates/hw_soul_ai/src/soul_ai/execute/task_execution/refine.rs`（Dream 離脱の参考）|
| Familiar タスク割り当て | `crates/hw_familiar_ai/src/familiar_ai/decide/task_management/policy/basic.rs` |
| Familiar ポリシー dispatch | `crates/hw_familiar_ai/src/familiar_ai/decide/task_management/policy/mod.rs`（`assign_by_work_type`）|
| Dream 蓄積システム | `crates/hw_soul_ai/src/soul_ai/update/dream_update.rs` |
| Designation + TaskSlots の付与例 | `crates/hw_logistics/src/transport_request/producer/floor_construction/designation.rs` |
| Yard → PowerGrid 逆引き | `crates/hw_logistics/src/transport_request/producer/mod.rs`（`find_owner_yard`） |
| 配置 UI パターン | `crates/bevy_app/src/interface/selection/building_place/placement.rs` |
| 型登録パターン | `crates/bevy_app/src/plugins/logic.rs`（73 行目付近の `register_type` ブロック） |

### 推奨実装順

```
Step 1-4 (型定義)     → cargo check でコンパイルエラー解消
Step 5    (VisualMirror) → sync.rs の exhaustive match 対応
Step 6    (task execution) → cargo check
Step 7    (dream update) → cargo check
Step 8    (familiar AI) → cargo check
Step 9    (placement UI) → 配置動作確認
Step 10   (construction logistics) → 骨搬入〜Operational 遷移確認
Step 11   (power output) → current_output 反映確認
Step 12   (UI) → active_slots 操作確認
Step 13   (plugin registration) → 最後に全システムを接続
```

### 注意事項

- `hw_jobs/src/tasks/mod.rs` の `AssignedTask` の `match` は exhaustive。`GeneratePower` arm の追加漏れは cargo check で検出される（`work_type()`・`get_target_entity()`・`requires_item_in_inventory()`・`expected_item()`）。
- `soul_spa_delivery_sync_system` で `Children` クエリを使う場合、`Query<(&SoulSpaSite, &Children)>` の `Children` は Bevy の `ChildOf` Relationship で自動管理される。
- `spawn.rs` の `find_owner_yard` 呼び出しには `Query<(Entity, &Yard)>` が必要。関数シグネチャを `yards_with_entity: &[(Entity, &Yard)]` に合わせること（`hw_logistics::transport_request::producer::find_owner_yard` の既存シグネチャを確認）。
- `TaskMode::SoulSpaPlace` の UI ボタン接続（ビルドメニューへの追加）は Step 9 の一部。`hw_ui/src/intents.rs` や `setup/submenus.rs` も参照すること。
