# 猫車 (Wheelbarrow) System - Quick Reference Guide

## TL;DR - What is the Wheelbarrow System?

The wheelbarrow (猫車/นักรสได้ transport) is a **7-phase task system** in Hell Workers for efficiently moving large quantities of materials:
- **Sand** (loose/uncollected)
- **StasisMud** (refined from Sand+Water+Rock)
- **Bones** (loose/uncollected)
- **Wood** (refined from trees)

It requires a Soul to:
1. Navigate to a parked wheelbarrow
2. Pick it up
3. Navigate to a source location (sand pit, bone pile, item stack, mixer)
4. Load items into the wheelbarrow
5. Navigate to destination (stockpile, blueprint, construction site, mixer)
6. Unload items
7. Return wheelbarrow to parking location

## The 7 Phases at a Glance

| # | Phase | Purpose | Key Action | Failure Handling |
|---|-------|---------|-----------|-----------------|
| 0 | GoingToParking | Find wheelbarrow | Navigate to wb position | Cancel if unreachable |
| 1 | PickingUpWheelbarrow | Acquire wb | Add PushedBy(soul) | Skip to Return if empty |
| 2 | GoingToSource | Get to items | Navigate + capacity check | Cancel if full/unreachable |
| 3 | Loading | Load items | Apply LoadedIn(wb) + hide | Cancel if items missing |
| 4 | GoingToDestination | Get to dest | Navigate to stockpile/blueprint/mixer | Cancel if destroyed |
| 5 | Unloading | Deliver items | 4 destination types (see below) | Drop + partial if full |
| 6 | ReturningWheelbarrow | Return wb | Navigate back + park | Complete at current pos |

## The Unloading Phase (Most Complex)

**Unloading handles 4 different destination types:**

### A) Stockpile (WheelbarrowDestination::Stockpile)
```
Input:  wheelbarrow with LoadedItems
Output: items in stockpile with StoredIn(stockpile)
Check:  current_stored + incoming < capacity
Action: Drop item at stockpile position
```

### B) Floor/Wall Construction Site
```
Input:  wheelbarrow with LoadedItems
Output: items on ground at site.material_center
Check:  reserved_for_site < (needed - nearby_items)
Action: Drop at site center (nearby items within 2 tiles count as delivered)
```

### C) Blueprint
```
Input:  wheelbarrow with LoadedItems
Output: items DESPAWNED (consumed by blueprint)
Check:  remaining_material_amount(resource_type) > 0
Action: Call blueprint.deliver_material(), despawn item
```

### D) Mixer (MudMixer)
```
Input:  wheelbarrow with LoadedItems
Output: A) items DESPAWNED if mixer accepts them
         B) items on ground if mixer is FULL
Check:  mixer.add_material(resource_type).is_ok()
Action: Despawn if success, drop with 5-sec timer if overflow
```

## Critical Data Types

### HaulWithWheelbarrowData
```rust
pub struct HaulWithWheelbarrowData {
    pub wheelbarrow: Entity,              // The wheelbarrow entity
    pub source_pos: Vec2,                 // Where to pick items
    pub destination: WheelbarrowDestination,  // Where items go
    pub collect_source: Option<Entity>,   // For Sand/Bone direct collection
    pub collect_amount: u32,              // How much to collect
    pub collect_resource_type: Option<ResourceType>,
    pub items: Vec<Entity>,               // Pre-selected items to load
    pub phase: HaulWithWheelbarrowPhase,  // Current phase (0-6)
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

## Two Types of Loading

### Type A: Direct Collection
Used for Sand/Bone from infinite sources (sand pits, rivers, etc.)
```
collect_source = Some(source_entity)
collect_amount = N
collect_resource_type = Some(Sand|Bone)
items = empty (filled during Loading phase)
```

### Type B: Pre-selected Items
Used for items from stockpiles or construction sites
```
collect_source = None
items = vec![item1, item2, ...] (prefilled)
```

## Visibility & Relationship State Machine

```
STARTING STATE:
- wheelbarrow: Visible, ParkedAt(parking_location)
- items: Visible (various locations)

↓ PickingUpWheelbarrow
- wheelbarrow: Visible, PushedBy(soul)  [ParkedAt removed]
- items: Visible

↓ Loading
- wheelbarrow: Visible, PushedBy(soul), LoadedItems(...)
- items: Hidden, LoadedIn(wheelbarrow), DeliveringTo(destination)

↓ Unloading [Stockpile]
- items: Visible, StoredIn(stockpile)  [LoadedIn removed]

↓ Unloading [Blueprint]
- items: DESPAWNED  [consumed]

↓ Unloading [Mixer]
- items: DESPAWNED  [if accepted] OR Visible + ItemDespawnTimer(5.0)

↓ ReturningWheelbarrow
- wheelbarrow: Visible, ParkedAt(parking_location)  [PushedBy removed]
- items: (none left)
```

## Common Error Cases

### 1. Capacity Check Failure (GoingToSource)
```
Problem: Stockpile is full
Code:    current_stored + incoming >= capacity
Result:  Cancel task
```

### 2. Item Disappeared (Loading)
```
Problem: Item was removed before loading (despawned, picked up elsewhere)
Code:    Filter targets.get(item_entity) → None
Result:  Partial load with reservation cleanup
```

### 3. Destination Destroyed (Unloading)
```
Problem: Stockpile/Blueprint/Mixer deleted before unload
Code:    queries.storage.stockpiles.get(dest) → Err
Result:  Drop items at soul position, cancel task
```

### 4. Mixer Overflow (Unloading)
```
Problem: Mixer at capacity, can't accept more
Code:    storage.add_material(res_type).is_err()
Result:  Drop item at soul position with 5-sec despawn timer
```

### 5. Wheelbarrow Lost (Any Phase)
```
Problem: Wheelbarrow entity disappeared mid-task
Code:    q_wheelbarrows.get(data.wheelbarrow) → Err
Result:  In GoingToParking: cancel
         In Returning: complete task at current position
```

### 6. Path Unreachable (Any Navigation)
```
Problem: Destination blocked/unreachable
Code:    update_destination_to_adjacent() → reachable = false
Result:  Cancel (GoingToParking/GoingToSource/GoingToDestination)
         OR complete at current pos (ReturningWheelbarrow)
```

## Partial Unload vs Full Unload

### Full Unload (Success)
```
All items delivered/despawned
↓
Check: has_pending_wheelbarrow_task() ?
  YES → Keep wheelbarrow, wait for next task
  NO  → Return to parking location
```

### Partial Unload (Some Capacity/Items Missing)
```
Some items dropped undelivered
↓
finish_partial_unload():
  1. Drop undelivered items at soul position
  2. Release destination reservations for undelivered count
  3. Return to parking location
```

## Cancellation Procedure (cancel.rs)

When task cancelled from ANY phase:
```
1. Drop all LoadedItems:
   - Make Visible
   - Position at soul location
   - Remove LoadedIn, DeliveringTo

2. Park wheelbarrow:
   - Remove PushedBy
   - Add ParkedAt
   - Position at soul location

3. Release all reservations:
   - wheelbarrow (1 count)
   - collect_source (1 count, if set)
   - each item in items (1 count each)
   - mixer destinations (by resource type)

4. Clear task context:
   - AssignedTask → None
   - WorkingOn → removed
```

## Key Constants Worth Knowing

| Constant | Value | Purpose |
|----------|-------|---------|
| WHEELBARROW_CAPACITY | 10 | Max items in wheelbarrow |
| WHEELBARROW_LEASE_MIN_DURATION_SECS | 8.0 | Minimum task duration |
| WHEELBARROW_LEASE_MAX_DURATION_SECS | 45.0 | Maximum before timeout |
| WHEELBARROW_ACTIVE_SCALE | 1.8 | Visual scale when loaded |
| Item lifetime (Sand/StasisMud) | 5.0 secs | Time until despawn if dropped |
| Nearby item search radius | 2.0 tiles | For site material checking |
| Stockpile capacity default | 10 items | Per stockpile entity |

## Reservation System Integration

Wheelbarrow tasks interact with the reservation system:
```
Loading phase:
  - Release source reservation when picking up
  - Record picked source when loading
  - Apply DeliveringTo relationship

Unloading phase:
  - Record stored destination (stockpiles)
  - Release mixer destination (mixers)
  - Release wheelbarrow reservation

Cancellation:
  - Release ALL reservations (source, items, destination)
```

## Visual System Integration

```
WheelbarrowMovement component:
  - Tracks previous frame position
  - Calculates rotation via atan2(delta_y, delta_x)
  - Updates sprite: empty ↔ loaded based on LoadedItems

Visibility system:
  - Hidden during LoadedIn phase
  - Visible during Unloading/Drop

Scale system:
  - 1.8x when loaded (WHEELBARROW_ACTIVE_SCALE)
  - Normal when empty
```

## Files to Audit When Finding Bugs

1. **Capacity Check Bug?**
   → `crates/hw_soul_ai/.../haul_with_wheelbarrow/phases/going_to_source.rs:37-50`

2. **Items Disappearing?**
   → `crates/hw_soul_ai/.../haul_with_wheelbarrow/phases/loading.rs:88-102`
   → `crates/hw_soul_ai/.../haul_with_wheelbarrow/phases/unloading.rs:244-256`

3. **Mixer Overflow Not Working?**
   → `crates/hw_soul_ai/.../haul_with_wheelbarrow/phases/unloading.rs:420-447`

4. **Items Not Dropping?**
   → Check Visibility state in `unloading.rs:60-70`

5. **Wheelbarrow Not Parking?**
   → `crates/hw_soul_ai/...transport_common/wheelbarrow.rs:14-32`

6. **Destination Not Checked?**
   → `crates/hw_soul_ai/.../haul_with_wheelbarrow/phases/going_to_destination.rs`

## Quick Testing Checklist

- [ ] Wheelbarrow picks up correctly (PushedBy added)
- [ ] Items become hidden during loading (Visibility::Hidden)
- [ ] Items become visible during unloading (Visibility::Visible)
- [ ] Stockpile receives StoredIn relationship
- [ ] Blueprint items despawn (not drop)
- [ ] Mixer overflow items get 5-sec timer
- [ ] Partial load releases unloaded items' reservations
- [ ] Destination destroyed → items drop at soul
- [ ] Wheelbarrow returns to parking (ParkedAt restored)
- [ ] Cannot pick up if destination full (stockpile capacity check)

## Related Systems (Dependency Tree)

```
HaulWithWheelbarrow
├── Requires: TransportRequest (logistics)
├── Requires: WheelbarrowLease (arbitration)
├── Requires: Relationships (Bevy 0.18)
│   ├── ParkedAt, PushedBy, LoadedIn
│   ├── DeliveringTo, StoredIn
│   └── WorkingOn
├── Requires: Pathfinding (WorldMap, A*)
├── Requires: Inventory system
├── Visual: WheelbarrowMovement
├── Animation: ItemDespawnTimer
└── Policy: Familiar task assignment
```

---

**Last Updated**: With Bevy 0.18, 7-phase wheelbarrow system
**Document Accuracy**: Based on source code as of latest commit
