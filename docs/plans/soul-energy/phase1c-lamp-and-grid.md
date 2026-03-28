# Phase 1c: Outdoor Lamp + Grid Integration + Visual

| Item | Value |
|:---|:---|
| Status | Not started |
| Depends on | Phase 1a (data model), Phase 1b (Soul Spa + GeneratePower) |
| Blocks | Phase 2 |

## Goal

Complete the power loop: Outdoor Lamp as consumer, grid recalculation, powered/unpowered cycle, and visual feedback. At the end of this phase, the full "generate → consume → blackout → recover" cycle works in-game.

## Implementation

### Step 1: Outdoor Lamp BuildingType

Add `BuildingType::OutdoorLamp` (category: `Temporary`).

- Size: 1x1, standard building placement flow
- Cost: Bone x2
- On construction complete:
  - Attach `PowerConsumer { demand: OUTDOOR_LAMP_DEMAND }`
  - `#[require(Unpowered)]` ensures initial state is unpowered
  - Observer `On<Add, PowerConsumer>` → attach `ConsumesFrom(yard_power_grid)` (look up Yard → YardPowerGrid, same pattern as Soul Spa's GeneratesFor)

### Step 2: Grid Recalculation System

Dual trigger: **Change Detection** for responsiveness + **timer** (0.5s) as safety net.

```
// Runs when Changed<PowerGenerator> detected OR timer fires:
for (grid, generators: &GridGenerators, consumers: &GridConsumers) in grids.iter() {
    grid.generation = generators.iter()
        .map(|e| power_generator_query.get(e).current_output)
        .sum();
    grid.consumption = consumers.iter()
        .map(|e| power_consumer_query.get(e).demand)
        .sum();

    let was_powered = grid.powered;
    grid.powered = grid.generation >= grid.consumption;

    if was_powered != grid.powered {
        // Powered state changed → update Unpowered markers
        for consumer_entity in consumers.iter() {
            if grid.powered {
                commands.entity(consumer_entity).remove::<Unpowered>();
            } else {
                commands.entity(consumer_entity).insert(Unpowered);
            }
        }
    }
}
```

### Step 3: Outdoor Lamp Buff System

System that applies buffs to nearby Souls when lamp is powered:

```
for lamp WITHOUT Unpowered:
    for soul within OUTDOOR_LAMP_EFFECT_RADIUS of lamp position:
        apply stress reduction (stress accumulation rate * 0.8)
        apply fatigue recovery boost (rest recovery rate * 1.2)
```

Implementation notes:
- Use spatial query (check existing patterns for radius-based Soul queries)
- Buff is instantaneous per-tick, not a stored status — no "buff component" needed
- When lamp gains `Unpowered`, buff simply stops being applied next tick

### Step 4: Power Status UI

When player selects Yard (or clicks PowerGrid-related building):
- Display: generation / consumption / powered status
- Blackout warning icon when unpowered

Minimal implementation: text overlay in existing info panel.

### Step 5: Visual Feedback

**VisualMirror components** (define in `hw_core::visual_mirror`):
- `PoweredVisualState { is_powered: bool }` — synced from Unpowered marker presence
- Sync via Observer: `On<Add, Unpowered>` → set `is_powered = false`, `On<Remove, Unpowered>` → set `is_powered = true`

**Soul Spa visual**:
- Tile texture: placeholder (ritual pattern on ground)
- Generating Soul: use `SoulTaskPhaseVisual::GeneratePower` (defined in Phase 1b) for pose/animation selection
- Energy effect: particle-like sprite rising from generating Soul (optional, can be simple)

**Outdoor Lamp visual**:
- Powered: bright sprite + light overlay
- Unpowered: dim sprite, no overlay
- Switch driven by `PoweredVisualState.is_powered`

**Blackout indicator**:
- Unpowered buildings show "no power" icon (small lightning bolt with X)

## Changed Files

- `crates/hw_jobs/src/model.rs` — BuildingType::OutdoorLamp
- `crates/hw_core/src/visual_mirror/` — PoweredVisualState
- `crates/bevy_app/src/systems/jobs/` — Outdoor Lamp construction + completion
- `crates/bevy_app/src/systems/` — Grid recalculation system (new module)
- `crates/bevy_app/src/systems/` — Outdoor Lamp buff system
- `crates/bevy_app/src/systems/visual/` — Power-related visual systems
- `crates/bevy_app/src/interface/` — Power status UI
- `crates/bevy_app/src/plugins/` — Plugin registration
- `crates/hw_soul_ai/src/` — Buff application (stress/fatigue modifiers)
- `assets/textures/` — Outdoor Lamp texture, Soul Spa tile texture, blackout icon

## Completion Criteria

- [ ] Outdoor Lamp can be built (1x1, Bone x2)
- [ ] Lamp auto-connects to Yard's PowerGrid via ConsumesFrom
- [ ] Grid recalculation: generation/consumption correctly summed
- [ ] Blackout: consumption > generation → all consumers get Unpowered
- [ ] Recovery: generation restored → Unpowered removed
- [ ] Lamp buff applies only when powered (stress/fatigue effect)
- [ ] Lamp buff disappears on blackout
- [ ] Building despawn → auto-removed from grid (Relationship cleanup)
- [ ] Power status visible in UI
- [ ] Visual distinction between powered/unpowered states
- [ ] Soul Spa tile + generating Soul have visual representation
- [ ] `cargo check` passes
- [ ] `cargo clippy --workspace` 0 warnings

## Verification Scenarios

1. Soul Spa (2 Souls generating) + 3 lamps → powered (2.0 > 0.6)
2. Pull 1 Soul → still powered (1.0 > 0.6)
3. Pull remaining Soul → blackout (0.0 < 0.6), lamp buffs gone
4. Soul returns → recovery, buffs resume
5. Set active_slots to 0 → all Souls eventually disengage → blackout
6. Demolish lamp → consumption drops, grid recalculates

## AI Handoff

### First Steps
1. Read this plan + `milestone-roadmap.md`
2. Verify Phase 1a + 1b are complete:
   - PowerGrid spawns with Yard
   - Soul Spa operational, GeneratePower task works
   - PowerGenerator.current_output updates
3. Study existing building placement for 1x1 buildings (simpler than Soul Spa)
4. Start with Step 1 (Outdoor Lamp BuildingType)

### Key References
- `crates/bevy_app/src/systems/jobs/` — existing building construction patterns
- `crates/hw_core/src/visual_mirror/` — VisualMirror pattern
- `crates/bevy_app/src/systems/visual/` — existing visual systems
- `crates/bevy_app/src/interface/` — existing UI panels
