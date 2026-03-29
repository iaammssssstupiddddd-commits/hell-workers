# Soul Energy System — Milestone Roadmap

Created: 2026-03-27
Last Updated: 2026-03-28
Status: Phase 1a complete, Phase 1b complete, Phase 1c not started

---

## Vision

**Goal**: Introduce a power grid system fueled by Soul Energy, adding lighting, production facilities, and Room bonuses as new gameplay layers.

**Core Principle**: Dream tradeoff at the center — "how many Souls to dedicate to power generation" becomes a macro-management decision.

---

## Core Concepts

### Soul Energy

Soul performs ritual meditation at a **Soul Spa** to generate energy. Appears to be a relaxation facility, but is actually a power plant that exploits Soul's Dream accumulation.

### Dream Tradeoff

- Generating Souls **actively consume Dream** (accumulation stops)
- Auto-disengage when Dream falls below threshold (same pattern as Refine material depletion)
- Low fatigue accumulation (meditative act)
- More generators → DreamPool growth slows + labor force shrinks
- Souls with more Dream can generate longer → new axis for Familiar assignment
- Player manages the "Labor vs Soul Energy vs Dream" triangle

### Power Grid Model

- **No storage**: Real-time supply/demand balance (future: battery buildings)
- **Blackout**: consumption > generation → all consumers in the grid stop
- **Grid-local**: Per connected building group, not global pool

### Distribution Rules

1. **Yard-shared**: Same Yard = same grid
2. **Room adjacency**: Power entity adjacent to Room exterior wall → Room joins grid (Phase 2)
3. **Room propagation**: Rooms sharing wall/door = same grid (Phase 2)

### ECS Relationship Design

New Relationships in `hw_energy/src/relationships.rs` (formerly planned for `hw_core/src/relationships.rs`):

| Source | Target (Bevy auto) | Purpose |
|:---|:---|:---|
| `GeneratesFor(grid)` on SoulSpaSite | `GridGenerators` on PowerGrid | Generator → grid membership |
| `ConsumesFrom(grid)` on OutdoorLamp etc. | `GridConsumers` on PowerGrid | Consumer → grid membership |

Key decisions:
- **Site-level granularity**: SoulSpaSite (not individual tiles) connects to grid
- **Soul-Tile via existing Relationships**: `WorkingOn`/`TaskWorkers` covers occupancy
- **Separate generator/consumer**: Type-safe, no filter needed, future battery support
- **Not Relationships**: Yard↔PowerGrid (plain component), Room↔PowerGrid (Phase 2, Resource-based)

### Decided Specifications (Phase 1)

| Item | Value |
|:---|:---|
| Soul Spa size | 2x2 fixed (4 tiles = max 4 slots) |
| Soul Spa placement | Yard + walkable (Floor NOT required) |
| Soul Spa cost | Bone x3 per tile (12 total) |
| active_slots | Numeric cap only, no per-tile ON/OFF |
| Task exit | Dream consumption model (auto-complete at threshold) |
| DREAM_GENERATE_FLOOR | 10.0 (prevent full depletion) |
| Outdoor Lamp demand | OUTPUT_PER_SOUL * 0.2 (1 Soul = 5 lamps) |
| Outdoor Lamp radius | 5 tiles |
| PowerGrid lifecycle | Created with Yard (always exists, even if empty) |

---

## Phase Structure

```
Phase 1: Foundation (Yard-scoped)
  1a: Data model + Grid infrastructure
  1b: Soul Spa + GeneratePower task
  1c: Outdoor Lamp + Grid integration + Visual
    └─ Phase 2: Room connectivity
         Room adjacency + Room propagation + Indoor lighting
           └─ Phase 3: Expansion
                Power lines + Battery + Electric room + New consumers
                  └─ Phase 4: Room naming
                       Furniture-based Room type detection
```

---

## Phase 1 Plans

### Phase 1a: Data Model + Grid Infrastructure ✅ Done
> **File**: `phase1a-data-model.md`
> **Scope**: Component/Relationship/constant definitions, PowerGrid entity auto-creation with Yard
> **Size**: Small. No UI, no gameplay changes.
> **Delivered**: `crates/hw_energy` crate (`components`, `constants`, `relationships`); `bevy_app/src/systems/energy/grid_lifecycle.rs` (Yard observers)

### Phase 1b: Soul Spa + GeneratePower Task
> **File**: `phase1b-soul-spa.md`
> **Scope**: BuildingType, entity structure, placement UI, construction, task execution, Dream consumption, Familiar integration, slot control, GeneratesFor connection
> **Size**: Large. Core generation-side feature.
> **Depends on**: Phase 1a

### Phase 1c: Outdoor Lamp + Grid Integration + Visual
> **File**: `phase1c-lamp-and-grid.md`
> **Scope**: Outdoor Lamp building, ConsumesFrom connection, buff system, grid recalculation, powered/unpowered cycle, visual feedback
> **Size**: Medium-large. Completes the full power loop.
> **Depends on**: Phase 1a + Phase 1b

---

## Phase 2: Room Connectivity (not yet detailed)

- Room adjacency → grid connection
- Room-to-room propagation via shared walls/doors
- Union-Find for grid topology
- Indoor lighting (Room Light)

## Phase 3: Expansion (concept)

- Power lines, Battery, Electric room, New consumer facilities

## Phase 4: Room Naming (concept)

- Furniture composition → Room type detection → Bonuses/unlocks

---

## Existing System Integration

| System | Integration |
|:---|:---|
| Dream | Active Dream consumption during GeneratePower, accumulation stops, auto-complete at threshold |
| Task system | `WorkType::GeneratePower`, `Designation`, `TaskSlots` |
| ECS Relationship | `WorkingOn`/`TaskWorkers` (Soul↔Tile), `GeneratesFor`/`ConsumesFrom` (grid membership) |
| Familiar AI | GeneratePower task discovery + assignment |
| Building system | Soul Spa area construction (FloorConstructionSite-like) |
| Logistics | Bone delivery via existing TransportRequest |

---

## Risks

| Risk | Impact | Mitigation |
|:---|:---|:---|
| Dream balance | Too costly/cheap, no real dilemma | Constants in SoulEnergyConstants, iterate via playtesting |
| Area construction overlap | Similar-but-different logic alongside FloorConstructionSite | No premature abstraction in Phase 1, refactor in Phase 2+ |
| Familiar task priority | All Souls sent to generate during construction | GeneratePower priority lower than construction tasks |
| Phase 2 migration | Yard-scoped grid → Room connectivity | Relationship-based design: just retarget GeneratesFor/ConsumesFrom |
