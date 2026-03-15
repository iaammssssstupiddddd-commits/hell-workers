# Hell Workers - 猫車 (Wheelbarrow) System Analysis

## Overview
The wheelbarrow (猫車) system is a specialized transport mechanism in Hell Workers used to move large quantities of materials (Sand, StasisMud, Bones, Wood) efficiently. It's one of the core logistics features alongside regular hauling, bucket transport, and stockpile consolidation.

## Files Related to Wheelbarrow (猫車)

### Core Implementation
- **Task Execution**: `/crates/hw_soul_ai/src/soul_ai/execute/task_execution/haul_with_wheelbarrow/`
  - `mod.rs` - Phase dispatcher
  - `cancel.rs` - Task cancellation and reservation cleanup
  - `phases/mod.rs` - Phase handler dispatcher
  - `phases/going_to_parking.rs` - Navigate to wheelbarrow parking location
  - `phases/picking_up_wheelbarrow.rs` - Acquire wheelbarrow entity
  - `phases/going_to_source.rs` - Navigate to pickup location with capacity check
  - `phases/loading.rs` - Load items into wheelbarrow
  - `phases/going_to_destination.rs` - Navigate to delivery location
  - `phases/unloading.rs` - Unload items at destination (handles Stockpiles, Blueprints, Floor/Wall Sites, Mixers)
  - `phases/returning_wheelbarrow.rs` - Return wheelbarrow to parking location

### Transport Common
- `/crates/hw_soul_ai/src/soul_ai/execute/task_execution/transport_common/wheelbarrow.rs` - Parking/reset helpers

### Logistics Core
- `/crates/hw_logistics/src/transport_request/producer/wheelbarrow.rs` - Auto-haul producer
- `/crates/hw_core/src/constants/logistics.rs` - Wheelbarrow constants

### Familiar AI (Task Assignment)
- `/crates/hw_familiar_ai/src/familiar_ai/decide/task_management/validator/wheelbarrow.rs` - Wheelbarrow selection logic
- `/crates/hw_familiar_ai/src/familiar_ai/decide/task_management/policy/haul/wheelbarrow.rs` - Wheelbarrow policy

### Visual System
- `/crates/hw_visual/src/haul/wheelbarrow_follow.rs` - Visual tracking and rotation
- `/crates/bevy_app/src/systems/soul_ai/execute/task_execution/transport_common/wheelbarrow.rs` - Bevy app integration

### Documentation
- `docs/logistics.md` - Physical logistics system design
- `docs/tasks.md` - Task lifecycle and execution (Section 4.3)
- `docs/gather_haul_visual.md` - Visual feedback including wheelbarrow graphics

## Data Types

### HaulWithWheelbarrowData (from hw_jobs::assigned_task)
```rust
pub struct HaulWithWheelbarrowData {
    pub wheelbarrow: Entity,           // Reference to wheelbarrow entity
    pub source_pos: Vec2,              // Position to pick up items
    pub destination: WheelbarrowDestination,  // Where items go
    pub collect_source: Option<Entity>, // Direct source (for Sand/Bone collection)
    pub collect_amount: u32,           // Amount to collect directly
    pub collect_resource_type: Option<ResourceType>,  // Type of resource to collect
    pub items: Vec<Entity>,            // Item entities being transported
    pub phase: HaulWithWheelbarrowPhase,  // Current execution phase
}
```

### HaulWithWheelbarrowPhase (7 phases)
```rust
pub enum HaulWithWheelbarrowPhase {
    #[default]
    GoingToParking,           // 0: Navigate to wheelbarrow
    PickingUpWheelbarrow,     // 1: Acquire wheelbarrow
    GoingToSource,            // 2: Navigate to pickup location
    Loading,                  // 3: Load items into wheelbarrow
    GoingToDestination,       // 4: Navigate to delivery location
    Unloading,                // 5: Deliver items
    ReturningWheelbarrow,     // 6: Return wheelbarrow to parking
}
```

### WheelbarrowDestination
```rust
pub enum WheelbarrowDestination {
    Stockpile(Entity),
    Blueprint(Entity),
    Mixer { entity: Entity, resource_type: ResourceType },
}
```

## Task Execution Flow

```
┌─────────────────────────────────────────────────────────────────┐
│ 1. GoingToParking                                               │
│    - Navigate to parked wheelbarrow location                     │
│    - Query wheelbarrow position                                  │
│    - If unreachable → cancel                                     │
│    - If at wheelbarrow → PickingUpWheelbarrow                    │
└─────────────────────────────────────────────────────────────────┘
                              ↓
┌─────────────────────────────────────────────────────────────────┐
│ 2. PickingUpWheelbarrow                                         │
│    - Remove ParkedAt relationship                                │
│    - Add PushedBy(soul_entity) relationship                      │
│    - Store in Inventory                                          │
│    - Check if empty with no collect_source:                      │
│      → ReturningWheelbarrow immediately                          │
│    - Otherwise → GoingToSource                                   │
└─────────────────────────────────────────────────────────────────┘
                              ↓
┌─────────────────────────────────────────────────────────────────┐
│ 3. GoingToSource (with Capacity Check)                          │
│    - Navigate to data.source_pos                                 │
│    - For Stockpiles: Verify space available                      │
│      (capacity - current_stored - incoming) > 0                  │
│    - If unreachable → cancel                                     │
│    - If at source → Loading                                      │
└─────────────────────────────────────────────────────────────────┘
                              ↓
┌─────────────────────────────────────────────────────────────────┐
│ 4. Loading                                                       │
│    - TWO PATHS:                                                  │
│    A) Direct Collection (collect_source set):                    │
│       - Sand: spawn_loaded_sand_items()                          │
│       - Bone: spawn_loaded_bone_items()                          │
│       - Clear Designation from source                            │
│       - Release source reservation                               │
│    B) Pre-selected Items (items list set):                       │
│       - Filter out missing/disappeared items                     │
│       - Apply LoadedIn(wheelbarrow) relationship                 │
│       - Hide items visually                                      │
│       - Release mixer_mud storage if needed                      │
│       - Remove StoredIn, Designation, TaskSlots, Priority        │
│    - If items < expected: partial load, release unloaded         │
│    - Set items to LoadedItems on wheelbarrow                     │
│    - Update DeliverTo relationships                              │
│    - GoingToDestination                                          │
└─────────────────────────────────────────────────────────────────┘
                              ↓
┌─────────────────────────────────────────────────────────────────┐
│ 5. GoingToDestination                                            │
│    - Navigate to destination (stockpile/blueprint/mixer)         │
│    - Update pathfinding to adjacent reachable position           │
│    - If unreachable → cancel                                     │
│    - If at destination → Unloading                               │
└─────────────────────────────────────────────────────────────────┘
                              ↓
┌─────────────────────────────────────────────────────────────────┐
│ 6. Unloading (Complex)                                           │
│    - FOUR DESTINATION TYPES:                                     │
│                                                                  │
│    A) STOCKPILE:                                                 │
│       - Get current_count + incoming_count                       │
│       - Drop items at stockpile position                         │
│       - Apply StoredIn(stockpile) relationship                   │
│       - Respect capacity limits                                  │
│                                                                  │
│    B) FLOOR/WALL CONSTRUCTION SITE:                              │
│       - Check floor_site_remaining() or wall_site_remaining()    │
│       - Nearby items (within 2 tiles) are counted against need   │
│       - Drop at site.material_center with offset                 │
│       - Do NOT apply StoredIn                                    │
│                                                                  │
│    C) BLUEPRINT:                                                 │
│       - Call blueprint.deliver_material(res_type, 1)             │
│       - Despawn items (not drop)                                 │
│       - If materials_complete() → remove ManagedBy + set Priority│
│                                                                  │
│    D) MIXER:                                                     │
│       - Call mixer.add_material(res_type)                        │
│       - If success: despawn item                                 │
│       - If capacity full: drop item at soul position             │
│       - Apply 5-sec despawn timer to Sand/StasisMud              │
│                                                                  │
│    - Partial unload: drop undelivered items, release reservations│
│    - Full unload: check has_pending_wheelbarrow_task()           │
│      → If yes: keep for next assignment                          │
│      → If no: return to parking                                  │
└─────────────────────────────────────────────────────────────────┘
                              ↓
┌─────────────────────────────────────────────────────────────────┐
│ 7. ReturningWheelbarrow                                          │
│    - Get parking_pos from ParkedAt relationship                  │
│    - Navigate to parking location                                │
│    - If unreachable:                                             │
│      → Release wheelbarrow reservation                           │
│      → Park at current position                                  │
│      → End task                                                  │
│    - If at parking:                                              │
│      → Release wheelbarrow reservation                           │
│      → Park wheelbarrow at parking_pos                           │
│      → End task                                                  │
└─────────────────────────────────────────────────────────────────┘
                              ↓
                        TASK COMPLETE
```

## Key Constants (from hw_core/constants/logistics.rs)

```rust
// Wheelbarrow (猫車) Constants
WHEELBARROW_CAPACITY: usize = 10;
WHEELBARROW_OFFSET: f32 = TILE_SIZE * 0.5;
WHEELBARROW_MIN_BATCH_SIZE: usize = 3;
WHEELBARROW_PREFERRED_MIN_BATCH_SIZE: usize = 3;
SINGLE_BATCH_WAIT_SECS: f64 = 5.0;
WHEELBARROW_LEASE_MIN_DURATION_SECS: f64 = 8.0;
WHEELBARROW_LEASE_MAX_DURATION_SECS: f64 = 45.0;
WHEELBARROW_LEASE_BUFFER_RATIO: f64 = 0.5;
WHEELBARROW_SCORE_BATCH_SIZE: f32 = 10.0;
WHEELBARROW_SCORE_PRIORITY: f32 = 5.0;
WHEELBARROW_SCORE_DISTANCE: f32 = 0.1;
WHEELBARROW_SCORE_PENDING_TIME: f32 = 2.0;
WHEELBARROW_SCORE_PENDING_TIME_MAX_SECS: f64 = 30.0;
WHEELBARROW_SCORE_SMALL_BATCH_PENALTY: f32 = 20.0;
WHEELBARROW_ARBITRATION_TOP_K: usize = 24;
WHEELBARROW_ARBITRATION_FALLBACK_INTERVAL_SECS: f64 = 0.5;
WHEELBARROW_ACTIVE_SCALE: f32 = 1.8;
```

## Cancellation Logic (cancel.rs)

When a wheelbarrow task is cancelled (any phase):
1. **Drop loaded items**: Iterate LoadedItems, make visible, position at soul location
2. **Park wheelbarrow**: Remove PushedBy, add ParkedAt, hide/reset position
3. **Release all reservations**:
   - Wheelbarrow source reservation
   - collect_source reservation (if set)
   - All item source reservations
   - Mixer destination reservations (by resource type)
4. **Clear task context**: AssignedTask → None, WorkingOn relationship removed

## Unloading Edge Cases Handled

1. **Partial loads**: If some items disappeared or capacity full, drop undelivered items
2. **Destination destroyed**: 
   - Blueprint/Mixer destroyed during unload → drop items at soul position
   - Stockpile destroyed → cancel entirely
3. **Destination no longer needs items**: Skip delivery if capacity exceeded
4. **Resource type mismatch**: Skip items that don't match stockpile type
5. **Mixer overflow**: Items rejected by mixer due to capacity are dropped with 5-sec timer
6. **Continuing work**: If more pending tasks, keep wheelbarrow instead of returning

## Visibility & Relationships

### Relationships Managed During Wheelbarrow Task
| Relationship | Added | Removed | Phase |
|:---|:---|:---|:---|
| `ParkedAt` | — | PickingUpWheelbarrow | PickingUpWheelbarrow |
| `PushedBy(soul)` | PickingUpWheelbarrow | ReturningWheelbarrow | PickingUpWheelbarrow |
| `LoadedIn(wheelbarrow)` | Loading | Unloading | Loading |
| `DeliveringTo(dest)` | Loading | Unloading | Loading |
| `StoredIn(stockpile)` | Unloading (stockpiles) | Unloading | Unloading |
| `Visibility` | Hidden (Loading) | Visible (Unloading/cancel) | Loading/Unloading |

### Visual System
- **WheelbarrowMovement**: Tracks position and rotation for smooth animation
- **Rotation**: Calculated from frame-to-frame movement using `atan2`
- **Sprite switching**: `wheelbarrow_empty` ↔ `wheelbarrow_loaded` based on LoadedItems
- **Scale**: Active scale 1.8x when loaded
- **No head icon**: Unlike normal hauls, wheelbarrow tasks don't show carrying item icons

## Common Issues & Validation Points

### 1. Missing Wheelbarrow Recovery
- **Issue**: If wheelbarrow entity disappears mid-task
- **Location**: `going_to_parking.rs`, `returning_wheelbarrow.rs`
- **Handling**: Cancels task or parks at current location

### 2. Destination Validity Checks
- **GoingToSource**: Verifies stockpile has space BEFORE going
- **GoingToDestination**: Routes to adjacent reachable position
- **Unloading**: Checks destination still exists and has capacity

### 3. Partial Loads
- **Loading phase**: If expected items missing → partial load with reservation cleanup
- **Unloading phase**: If capacity exceeded → drop undelivered, continue if space opens

### 4. Resource Type Mismatches
- **Stockpiles**: Items must match stockpile.resource_type (or stockpile is empty)
- **Mixers**: Items expected to match declared resource_type

### 5. Item Visibility State
- Items are Hidden during LoadedIn phase
- Made Visible during Unloading
- Need proper Transform for drop position

## Wheelbarrow Selection & Arbitration

### find_nearest_wheelbarrow() (validator/wheelbarrow.rs)
- Filters wheelbarrows by `source_not_reserved()` check
- Returns nearest by distance_squared
- Used in haul policy decision making

### Auto-Haul Producer (wheelbarrow.rs in hw_logistics)
- Issues `BatchWheelbarrow` TransportRequests for wheelbarrow fleets
- Handles return requests when wheelbarrow drift > RETURN_DISTANCE_THRESHOLD
- Manages `desired_batch_requests` and `desired_return_requests` HashMaps

## Documentation in Code

Task.md (Section 4.3):
> **猫車運搬 (HaulWithWheelbarrow)**: GoingToParking → PickingUpWheelbarrow → GoingToSource → Loading → GoingToDestination → Unloading → ReturningWheelbarrow

docs/logistics.md (§4.3):
> **Sand / StasisMud**: 原則猫車必須。例外: ソース隣接 3x3 の立ち位置からドロップ閾値内なら徒歩可
> **搬運先ガード**: Blueprint / construction / provisional wall / stockpile は Dropping / Unloading 直前に受入可能量を再確認し、到着時点で需要が消えた cargo を搬入先へ反映しない。

docs/gather_haul_visual.md (Section 4 - Wheelbarrow):
> **スプライト切替**: `LoadedItems` の有無で `wheelbarrow_empty` / `wheelbarrow_loaded` を差し替え

## Key Review Points for Issues

1. **Capacity Checks**: Verify stockpile capacity logic in `going_to_source.rs` line 37-50
2. **Partial Unload Recovery**: Ensure `finish_partial_unload()` properly releases all reservations
3. **Mixer Overflow**: Line 424-446 in `unloading.rs` - items rejected by mixer need 5-sec timer
4. **Phantom Items**: Check if items can disappear between Loading and Unloading phases
5. **Wheelbarrow Parking**: Verify ParkedAt resolution in `returning_wheelbarrow.rs` lines 34-48
6. **Visibility State Bugs**: Ensure items transition Hidden ↔ Visible correctly throughout phases

