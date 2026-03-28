# Phase 1a: Data Model + Grid Infrastructure

| Item | Value |
|:---|:---|
| Status | Not started |
| Depends on | Nothing |
| Blocks | Phase 1b, Phase 1c |

## Goal

Define all Soul Energy data types, ECS Relationships, and constants. Create PowerGrid entity infrastructure. No gameplay or UI changes — pure foundation.

## Implementation

### Step 1: Constants

Define in `hw_core` (new module `constants/energy.rs` or similar, following existing `constants/` pattern):

```rust
pub const OUTPUT_PER_SOUL: f32 = 1.0;           // Base generation rate per Soul per second
pub const DREAM_CONSUME_RATE_GENERATING: f32 = 0.5; // Dream consumed per second while generating
pub const DREAM_GENERATE_FLOOR: f32 = 10.0;     // Auto-disengage threshold
pub const OUTDOOR_LAMP_DEMAND: f32 = 0.2;       // = OUTPUT_PER_SOUL * 0.2
pub const OUTDOOR_LAMP_EFFECT_RADIUS: f32 = 5.0; // Tiles
pub const SOUL_SPA_BONE_COST_PER_TILE: u32 = 3; // 2x2 = 12 total
pub const FATIGUE_RATE_GENERATING: f32 = ...;    // Lower than normal work rate (check existing fatigue constants for reference)
```

### Step 2: Components

```rust
/// Attached to PowerGrid entity. Recalculated periodically.
pub struct PowerGrid {
    pub generation: f32,   // Total from all GridGenerators
    pub consumption: f32,  // Total from all GridConsumers
    pub powered: bool,     // generation >= consumption
}

/// Attached to SoulSpaSite. Site-level aggregate.
pub struct PowerGenerator {
    pub current_output: f32,   // occupied_count * output_per_soul
    pub output_per_soul: f32,  // From constants
}

/// Attached to OutdoorLamp etc.
pub struct PowerConsumer {
    pub demand: f32,
}

/// Marker on consumers when grid is unpowered. Use with #[require(Unpowered)] on PowerConsumer.
pub struct Unpowered;

/// On PowerGrid entity, points to owning Yard.
pub struct YardPowerGrid(pub Entity);
```

Design decisions:
- `PowerConsumer` uses `#[require(Unpowered)]` — safe default (unpowered until grid connects)
- `SoulSpaSite` uses `#[require(PowerGenerator)]` — safe default (`current_output: 0.0`)
- Relationships (`GeneratesFor`/`ConsumesFrom`) are **NOT** required components — use Observer for explicit connection to avoid `Entity::PLACEHOLDER` bugs

### Step 3: Relationships

Add to `hw_core/src/relationships.rs`, following existing patterns (`Default` with `Entity::PLACEHOLDER`, `Vec<Entity>` target):

```rust
#[relationship(relationship_target = GridGenerators)]
pub struct GeneratesFor(pub Entity);  // SoulSpaSite → PowerGrid

#[relationship_target(relationship = GeneratesFor)]
pub struct GridGenerators(Vec<Entity>);

#[relationship(relationship_target = GridConsumers)]
pub struct ConsumesFrom(pub Entity);  // OutdoorLamp etc. → PowerGrid

#[relationship_target(relationship = ConsumesFrom)]
pub struct GridConsumers(Vec<Entity>);
```

### Step 4: PowerGrid auto-creation with Yard

Add Observer: `On<Add, Yard>` → spawn PowerGrid entity with `YardPowerGrid(yard_entity)`.

Existing Observer precedent: `on_building_added`, `on_designation_removed` etc.

Also handle `On<Remove, Yard>` → despawn associated PowerGrid.

## Changed Files

- `crates/hw_core/src/constants/` — new energy constants module
- `crates/hw_core/src/` — PowerGrid, PowerGenerator, PowerConsumer, Unpowered, YardPowerGrid components
- `crates/hw_core/src/relationships.rs` — GeneratesFor/GridGenerators, ConsumesFrom/GridConsumers
- `crates/bevy_app/src/` — Yard↔PowerGrid Observer (investigate where Yard creation is handled)

## Completion Criteria

- [ ] All types defined, `cargo check` passes
- [ ] `cargo clippy --workspace` 0 warnings
- [ ] PowerGrid entity spawns when Yard is created (verify with log or debugger)
- [ ] No impact on existing gameplay

## AI Handoff

### First Steps
1. Read this plan + `milestone-roadmap.md`
2. Check existing `hw_core/src/constants/` structure for module organization pattern
3. Check existing `hw_core/src/relationships.rs` for `Default` impl and derive patterns
4. Find where Yard entities are spawned to add the PowerGrid Observer nearby

### Key References
- `crates/hw_core/src/relationships.rs` — existing Relationship pattern
- `crates/hw_core/src/constants/` — existing constants organization
- `docs/tasks.md` §2 — ECS connection map conventions
