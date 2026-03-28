# Phase 1b: Soul Spa + GeneratePower Task

| Item | Value |
|:---|:---|
| Status | Not started |
| Depends on | Phase 1a (data model + grid infrastructure) |
| Blocks | Phase 1c |

## Goal

Implement Soul Spa as a 2x2 building where Souls perform GeneratePower task. Dream is actively consumed during generation. At the end of this phase, Souls can be assigned to Soul Spa, generate power (updating PowerGenerator.current_output), and auto-disengage when Dream runs low.

Grid recalculation and powered/unpowered cycle are NOT in this phase (Phase 1c).

## Implementation

### Step 1: BuildingType + Entity Definitions

Add `BuildingType::SoulSpa` (category: `Plant`).

Entity structure:
```
SoulSpaSite (parent)
├─ TaskArea (area_bounds)
├─ active_slots: u32            ← numeric cap, no per-tile state
├─ PowerGenerator              ← via #[require], current_output: 0.0
├─ GeneratesFor(power_grid)    ← Observer sets on construction complete
└─ children (ChildOf):
    SoulSpaTile x4 (2x2)
    ├─ grid_pos: (i32, i32)
    ├─ Designation(GeneratePower)
    ├─ TaskSlots(1)
    └─ TaskWorkers ← auto via WorkingOn Relationship
```

`SoulSpaSite` component fields:
```rust
pub struct SoulSpaSite {
    pub active_slots: u32,  // 0..=4, default 4
    pub tiles_total: u32,   // Always 4
}
```

`SoulSpaTile` component:
```rust
pub struct SoulSpaTile {
    pub parent_site: Entity,
}
```

### Step 2: Placement UI

Add `TaskMode::SoulSpaPlace` variant (follow `FloorPlace`/`WallPlace` pattern).

Behavior:
- Click to place 2x2 at cursor position (NOT drag — fixed size)
- Validation: all 4 tiles must be Yard + walkable
- On confirm: spawn SoulSpaSite + 4 SoulSpaTile children as construction site

### Step 3: Construction Flow

Follow FloorConstructionSite reinforcing phase pattern:
- Each tile needs Bone x3 delivery via TransportRequest
- After all tiles have materials delivered, construction completes immediately (no curing)
- On completion:
  - Replace construction entities with operational SoulSpaSite/SoulSpaTile
  - Observer `On<Add, SoulSpaSite>` → attach `GeneratesFor(yard_power_grid)` (look up Yard → YardPowerGrid)
  - Add `Designation(GeneratePower)` + `TaskSlots(1)` to each tile

### Step 4: WorkType + AssignedTask

Add `WorkType::GeneratePower`.

```rust
pub struct GeneratePowerData {
    pub target_tile: Entity,  // SoulSpaTile
    pub phase: GeneratePowerPhase,
}

pub enum GeneratePowerPhase {
    GoingToTile,
    Generating,
}
```

Add `AssignedTask::GeneratePower(GeneratePowerData)`.

### Step 5: Task Execution Logic

In `task_execution/`:

**GoingToTile**: Navigate to tile position. Standard pathfinding, same as other tasks.

**Generating**:
```rust
// Every frame:
soul.dream -= DREAM_CONSUME_RATE_GENERATING * dt;
if soul.dream <= DREAM_GENERATE_FLOOR {
    clear_task_and_path(ctx.task, ctx.path);  // Same as Refine material depletion
    return;
}
```

Fatigue: accumulate at `FATIGUE_RATE_GENERATING` (lower than normal work).

### Step 6: Dream Integration

In `dream_update_system`:
- Check if Soul has `AssignedTask::GeneratePower` in `Generating` phase
- If yes: **skip Dream accumulation** (rate = 0). Dream consumption is handled by task execution, not here
- This prevents double-counting (task drains Dream, dream_update should not also add to it)

### Step 7: Site-Level Output Aggregation

System running on timer (~0.5s):
```
For each SoulSpaSite with children:
    occupied_count = children.iter()
        .filter(|tile| tile.has::<TaskWorkers>() && task_workers.len() > 0)
        .count();
    power_generator.current_output = occupied_count as f32 * power_generator.output_per_soul;
```

Use `Changed<PowerGenerator>` detection in Phase 1c for grid recalculation trigger.

### Step 8: Familiar Integration

Familiar task discovery flow must find `Designation(GeneratePower)` on SoulSpaTiles.

Gate condition: Familiar checks `site.active_slots > site.occupied_count()` before assigning. This requires looking up parent SoulSpaSite from tile.

Priority: lower than construction tasks (Build, Haul, ReinforceFloor, etc.).

### Step 9: Slot Control UI

When player clicks completed SoulSpaSite:
- Show current `active_slots` (0..4) with +/- controls
- Reducing below current occupied count: existing workers finish naturally (no immediate kick), but Familiar will not assign new workers
- VisualMirror: `SoulSpaTileVisualState { has_worker: bool }` synced from TaskWorkers

### Step 10: VisualMirror

Add to `hw_core::visual_mirror`:
- `SoulTaskPhaseVisual::GeneratePower` variant
- `SoulSpaTileVisualState { has_worker: bool }` (synced from TaskWorkers presence)

Visual rendering (sprites/animations) is Phase 1c. This step only defines the mirror components.

## Changed Files

- `crates/hw_jobs/src/model.rs` — BuildingType::SoulSpa
- `crates/hw_jobs/src/tasks/` — WorkType::GeneratePower, AssignedTask::GeneratePower, GeneratePowerData
- `crates/hw_core/src/game_state.rs` — TaskMode::SoulSpaPlace
- `crates/hw_core/src/visual_mirror/` — SoulTaskPhaseVisual::GeneratePower, SoulSpaTileVisualState
- `crates/hw_core/src/` — SoulSpaSite, SoulSpaTile components
- `crates/bevy_app/src/systems/jobs/` — Soul Spa construction system (new module)
- `crates/bevy_app/src/systems/soul_ai/execute/task_execution/` — GeneratePower execution logic
- `crates/bevy_app/src/interface/` — Placement UI, slot control UI
- `crates/bevy_app/src/plugins/` — Plugin registration
- `crates/hw_soul_ai/src/` — dream_update_system modification, fatigue rate adjustment
- `crates/hw_familiar_ai/src/` — GeneratePower task discovery + assignment gate
- `assets/textures/` — Soul Spa placeholder texture

## Completion Criteria

- [ ] Soul Spa can be placed (2x2, click, Yard+walkable)
- [ ] Construction completes after bone delivery
- [ ] Familiar assigns Souls to Soul Spa tiles
- [ ] Soul navigates to tile and enters Generating phase
- [ ] Dream is consumed during generation, auto-disengages at threshold
- [ ] Dream accumulation is stopped during generation
- [ ] PowerGenerator.current_output reflects occupied count
- [ ] active_slots UI works (reduce → no new assignments)
- [ ] GeneratesFor Relationship connects to Yard's PowerGrid
- [ ] `cargo check` passes
- [ ] `cargo clippy --workspace` 0 warnings

## AI Handoff

### First Steps
1. Read this plan + `milestone-roadmap.md`
2. Verify Phase 1a is complete (components/relationships/constants exist, PowerGrid spawns with Yard)
3. Study existing patterns:
   - `FloorConstructionSite` / `WallConstructionSite` — area construction
   - `MudMixer` — building that operates after construction
   - `Refine` task — material depletion → auto-complete pattern
   - `TaskMode::FloorPlace` — placement UI
4. Start with Step 1 (BuildingType + entity definitions)

### Key References
- `crates/bevy_app/src/systems/jobs/floor_construction/` — area construction reference
- `crates/bevy_app/src/systems/soul_ai/execute/task_execution/` — task execution framework
- `crates/hw_soul_ai/src/soul_ai/update/dream_update.rs` — Dream accumulation system
- `crates/hw_familiar_ai/src/` — task discovery + assignment
- `crates/hw_core/src/visual_mirror/` — VisualMirror pattern
- `docs/tasks.md` — task system spec
- `docs/building.md` — building system spec
