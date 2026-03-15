# Phase 2 Quick Reference

## Building Entity Hierarchy
```
Building Entity (parent)
├─ Building { kind: BuildingType, is_provisional: bool }
├─ Transform { z: Z_BUILDING_FLOOR (0.05) or Z_BUILDING_STRUCT (0.12) }
├─ BuildingBounceEffect (spawn animation)
├─ Name
└─ Child: VisualLayer Entity
    ├─ VisualLayerKind { Floor | Struct | Deco | Light }
    ├─ Sprite { image, custom_size, ... }
    └─ Transform::default()
```

## VisualLayerKind Enum
```rust
pub enum VisualLayerKind {
    Floor,   // Z=0.05 (SandPile, BonePile, Floor)
    Struct,  // Z=0.12 (Wall, Door, Tank, MudMixer, etc.)
    Deco,    // Z=0.15 (decorations)
    Light,   // Z=0.18 (lighting effects)
}
```

## Critical Files for Phase 2

### Building System
- **Spawn:** `/crates/bevy_app/src/systems/jobs/building_completion/spawn.rs`
  - `spawn_completed_building()` - Creates parent + child hierarchy
- **Wall Connections:** `/crates/hw_visual/src/wall_connection.rs`
  - `wall_connections_system()` - Updates sprite based on neighbors
- **Types:** `/crates/hw_jobs/src/model.rs`
  - `BuildingType` enum (10 types)
  - `Building` component

### Building Visuals
- **Tank:** `/crates/hw_visual/src/tank.rs` → updates `VisualLayerKind::Struct`
- **MudMixer:** `/crates/hw_visual/src/mud_mixer.rs` → animates sprite at 6 FPS
- **Layer Definition:** `/crates/hw_visual/src/layer/mod.rs` → `VisualLayerKind` enum

### Character System
- **Soul Spawn:** `/crates/bevy_app/src/entities/damned_soul/spawn.rs`
  - Z_CHARACTER = 1.0, size = 0.8 * TILE_SIZE
- **Familiar Spawn:** `/crates/bevy_app/src/entities/familiar/spawn.rs`
  - Z_CHARACTER + 0.5 = 1.005, size = 0.9 * TILE_SIZE
- **Soul Visuals:** `/crates/hw_visual/src/soul/mod.rs`

### RtT Infrastructure
- **Camera Setup:** `/crates/bevy_app/src/plugins/startup/mod.rs` (lines 110-143)
  - Camera3d at Y=100, looking at XZ plane (up=NEG_Z)
  - Renders to `RttTextures { texture_3d: Handle<Image> }`
- **Camera Sync:** `/crates/bevy_app/src/systems/visual/camera_sync.rs`
  - Syncs Camera2d → Camera3d each frame
- **Constants:** `/crates/hw_core/src/constants/render.rs`
  - LAYER_2D = 0, LAYER_3D = 1
  - Z-constants for all entity types

### TO DELETE
- **RtT Test Scene:** `/crates/bevy_app/src/plugins/startup/rtt_test_scene.rs` (55 lines)
  - Called from mod.rs lines 8, 88, 89

## Key Render Layer Constants
```rust
LAYER_2D: usize = 0;           // Current 2D rendering
LAYER_3D: usize = 1;           // Phase 2 3D rendering

Z_BUILDING_FLOOR: f32 = 0.05;
Z_BUILDING_STRUCT: f32 = 0.12; // Walls, doors, most buildings
Z_CHARACTER: f32 = 1.0;        // Soul, Familiar
```

## Phase 2 Workflow
1. **Delete rtt_test_scene.rs** (3 references to remove)
2. **Add RenderLayers::layer(LAYER_3D)** to VisualLayer child entities
3. **Generate 3D meshes** for buildings (replaces 2D sprites)
4. **Update wall system** for 3D topology
5. **Adapt characters** to 3D rendering
6. **Test RtT compositing** (3D output → 2D screen via texture)

## BuildingType Variants (Phase 2 Priority)
| Type | Layer | Complexity | Priority |
|------|-------|-----------|----------|
| Wall | Struct | High (topology) | 🔴 High |
| Door | Struct | Medium | �� Medium |
| Floor | Floor | Low | 🟢 Low |
| Tank | Struct | Medium (state) | 🟡 Medium |
| MudMixer | Struct | High (animated) | 🔴 High |
| RestArea | Struct | Medium | 🟡 Medium |
| Bridge | Struct | Low | 🟢 Low |
| SandPile | Floor | Low | 🟢 Low |
| BonePile | Floor | Low | 🟢 Low |
| WheelbarrowParking | Struct | Low | 🟢 Low |

## Testing Points
- [ ] Camera3d renders to RttTextures
- [ ] Camera2d displays RtT texture via composite sprite
- [ ] Camera sync works (pan/zoom)
- [ ] Building entity hierarchy is correct
- [ ] VisualLayerKind components are present on child entities
- [ ] Wall connections update in real-time
- [ ] Character sprites render at correct Z-positions
