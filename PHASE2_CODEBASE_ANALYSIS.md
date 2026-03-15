# Hell Workers - Phase 2 of 3D-RtT Migration: Current State Analysis

## Executive Summary
The Hell Workers project has completed MS-Pre-B (Milestone 3 Pre-Build) with the implementation of the VisualLayer system for buildings. Phase 1 RtT infrastructure is in place with a functional Camera3d and render layer system. The codebase is ready for Phase 2 implementation.

---

## 1. WALL VISUAL SYSTEM

### 1.1 Wall Connection/Variant System
**File:** `/home/satotakumi/projects/hell-workers/crates/hw_visual/src/wall_connection.rs`

**Key Components:**
- **Function:** `wall_connections_system` - Updates wall sprite variants based on neighbor topology
- **Input:** Wall building entities and blueprints
- **Processing Logic:**
  - Detects when walls are added/changed
  - Queries neighbors (up, down, left, right) to determine connectivity
  - Updates sprite image and color based on:
    - Provisional vs complete walls (mud texture vs stone texture)
    - 16 different connection patterns (isolated, straight, corners, T-junctions, cross)
  
**Wall Textures Available:**
- **Mud walls (provisional):** `mud_isolated`, `mud_horizontal`, `mud_vertical`, `mud_corner_*`, `mud_t_*`, `mud_cross`
- **Stone walls (final):** `stone_isolated`, `stone_horizontal`, `stone_vertical`, `stone_corner_*`, `stone_t_*`, `stone_cross`

**VisualLayerKind Used:** `VisualLayerKind::Struct`

**Related Files:**
- `/crates/hw_logistics/src/wall_construction.rs` - Logistics management
- `/crates/hw_logistics/src/provisional_wall.rs` - Provisional wall state
- `/crates/hw_visual/src/wall_construction.rs` - Construction-phase visuals

### 1.2 Building Spawn System
**File:** `/home/satotakumi/projects/hell-workers/crates/bevy_app/src/systems/jobs/building_completion/spawn.rs`

**Function Signature:**
```rust
pub(super) fn spawn_completed_building(
    commands: &mut Commands,
    bp: &Blueprint,
    transform: &Transform,
    game_assets: &GameAssets,
) -> Entity
```

**Building Entity Hierarchy (Post MS-Pre-B):**
```
Building Entity (parent)
├─ Components: Building, Transform (Z=0.05 or 0.12), BuildingBounceEffect
└─ Child: VisualLayer (with VisualLayerKind component)
    └─ Components: VisualLayerKind, Sprite
```

**Key Properties:**
- Z-positioning by type:
  - **Floor/SandPile/BonePile:** Z=`Z_BUILDING_FLOOR` (0.05) with `VisualLayerKind::Floor`
  - **All others:** Z=`Z_BUILDING_STRUCT` (0.12) with `VisualLayerKind::Struct`
- Sprite size: `TILE_SIZE` (1x1 for simple buildings) or `TILE_SIZE * 2.0` (for Tank, MudMixer, etc.)
- BuildingBounceEffect added for completion animation

---

## 2. BUILDING VISUAL LAYER STRUCTURE

### 2.1 VisualLayerKind Definition
**File:** `/home/satotakumi/projects/hell-workers/crates/hw_visual/src/layer/mod.rs`

```rust
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub enum VisualLayerKind {
    /// 床・地面面（Z_BUILDING_FLOOR = 0.05）
    Floor,
    /// 壁・構造体（Z_BUILDING_STRUCT = 0.12）
    Struct,
    /// 装飾レイヤー（Z_BUILDING_DECO = 0.15）
    Deco,
    /// 照明・エフェクト（Z_BUILDING_LIGHT = 0.18）
    Light,
}
```

**Design Notes:**
- Currently spans 4 layers (Floor, Struct, Deco, Light)
- Each variant maps to specific Z-coordinates for 2D rendering
- Child entities can have `RenderLayers::layer(1)` added to move to 3D layer (Phase 2)

### 2.2 Entity Hierarchy After Building Completion

**Parent Building Entity Components:**
- `Building { kind: BuildingType, is_provisional: bool }`
- `Transform { z: Z_BUILDING_FLOOR or Z_BUILDING_STRUCT }`
- `BuildingBounceEffect { bounce_animation: BounceAnimation }` (for spawn animation)
- `Name`, etc.

**Child VisualLayer Entity Components:**
- `VisualLayerKind` (Floor, Struct, Deco, or Light)
- `Sprite { image, custom_size, ... }`
- `Transform::default()` (relative to parent)

**Special Cases:**
- **ProvisionalWall:** Added to Building if `bp.is_fully_complete() == false`
- **Door:** Added to Building if `kind == BuildingType::Door`

### 2.3 Building Types and Layer Assignments
**File:** `/home/satotakumi/projects/hell-workers/crates/hw_jobs/src/model.rs`

```rust
pub enum BuildingType {
    Wall,           // Struct layer
    Door,           // Struct layer
    Floor,          // Floor layer
    Tank,           // Struct layer (state-dependent visuals)
    MudMixer,       // Struct layer (animated)
    RestArea,       // Struct layer
    Bridge,         // Struct layer
    SandPile,       // Floor layer
    BonePile,       // Floor layer
    WheelbarrowParking,  // Struct layer
}
```

### 2.4 Building-Specific Visual Updates

**Tank Visual System:**
- **File:** `/crates/hw_visual/src/tank.rs`
- **Function:** `update_tank_visual_system`
- **Updates:** `VisualLayerKind::Struct` sprite based on capacity
  - Empty → `tank_empty`
  - Partial → `tank_partial`
  - Full → `tank_full`

**MudMixer Visual System:**
- **File:** `/crates/hw_visual/src/mud_mixer.rs`
- **Function:** `update_mud_mixer_visual_system`
- **Updates:** `VisualLayerKind::Struct` sprite with animation frames
  - Idle → `mud_mixer_idle`
  - Refining → cycles through `mud_mixer_anim_1` to `mud_mixer_anim_4` at 6 FPS

---

## 3. CHARACTER VISUAL SYSTEM

### 3.1 Soul (Damned Soul) Rendering

**File:** `/home/satotakumi/projects/hell-workers/crates/bevy_app/src/entities/damned_soul/spawn.rs`

**Soul Spawn Function:**
```rust
pub fn spawn_damned_soul_at(
    commands: &mut Commands,
    game_assets: &Res<GameAssets>,
    world_map: &WorldMap,
    pos: Vec2,
)
```

**Soul Entity Components:**
- `DamnedSoul { laziness, motivation, fatigue, stress, dream }`
- `Sprite { image: soul_sprite, custom_size: Vec2::splat(TILE_SIZE * 0.8), color }`
- `Transform { x, y, z: Z_CHARACTER (1.0) }`
- `AnimationState::default()`
- `SoulEmotionState::default()`
- `ConversationInitiator { timer }`
- `Inventory::default()`
- `Destination`, `Path` (navigation)
- `SoulIdentity { name, gender }` (Male: blue tint, Female: red tint)
- `AssignedTask::default()`
- `InventoryItemVisual::default()`
- `SoulTaskVisualState::default()`
- `IdleState::default()`
- `DreamState::default()`
- `SoulUiLinks::default()`

**Visual Details:**
- Sprite color varies by gender (male: srgb(0.9, 0.9, 1.0), female: srgb(1.0, 0.9, 0.9))
- Z-position: `Z_CHARACTER` (1.0)
- Size: 0.8 * TILE_SIZE

**Related Files:**
- `/crates/hw_visual/src/soul/mod.rs` - Soul visual system (idle, gathering, vitals)
- `/crates/hw_visual/src/soul/idle.rs` - Idle behavior visuals
- `/crates/hw_visual/src/soul/gathering.rs` - Gathering aura effects

### 3.2 Familiar (Minion) Rendering

**File:** `/home/satotakumi/projects/hell-workers/crates/bevy_app/src/entities/familiar/spawn.rs`

**Familiar Spawn Function:**
```rust
pub fn spawn_familiar_at(
    commands: &mut Commands,
    game_assets: &Res<GameAssets>,
    world_map: &WorldMap,
    pos: Vec2,
    familiar_type: FamiliarType,
    color_index: u32,
)
```

**Familiar Entity Components:**
- `Familiar { familiar_type, command_radius, efficiency, name, color_index }`
- `Sprite { image: familiar_sprite, custom_size: Vec2::splat(TILE_SIZE * 0.9), ... }`
- `Transform { x, y, z: Z_CHARACTER + 0.5 (1.005) }`
- `Destination`, `Path`
- `FamiliarAnimation::default()`
- `FamiliarVoice::random()`
- `FamiliarOperation::default()` (fatigue_threshold, max_controlled_soul)
- `ActiveCommand::default()`
- `crate::systems::familiar_ai::FamiliarAiState::default()`
- `hw_core::relationships::Commanding::default()`
- `hw_core::relationships::ManagedTasks::default()`

**Familiar Range Indicators:**
- Three separate entities spawned as aura indicators:
  1. `AuraLayer::Border` - Semi-transparent circle (orange, 0.3 alpha)
  2. `AuraLayer::Outline` - Ring outline (yellow, 0.0 alpha initially)
  3. `AuraLayer::Pulse` - Pulsing circle (orange, 0.15 alpha)
  - All use `FamiliarRangeIndicator(fam_entity)` component

**Visual Details:**
- Sprite size: 0.9 * TILE_SIZE
- Z-position: Z_CHARACTER + 0.5 (1.005)
- Command radius: TILE_SIZE * 7.0 (default)
- Aura visuals at Z_AURA (0.2)

---

## 4. RtT INFRASTRUCTURE FROM PHASE 1

### 4.1 Camera Setup
**File:** `/home/satotakumi/projects/hell-workers/crates/bevy_app/src/plugins/startup/mod.rs`

**3D Camera Configuration:**
```rust
Camera3d::default()
Camera { order: -1, clear_color: ClearColorConfig::Custom(Color::srgba(0.0, 0.0, 0.0, 0.0)) }
Projection::Orthographic(OrthographicProjection::default_3d())
Transform::from_translation(Vec3::new(0.0, 100.0, 0.0))
    .looking_at(Vec3::ZERO, Vec3::NEG_Z)
RenderTarget::Image(rtt_handle.into())
RenderLayers::layer(LAYER_3D)
Camera3dRtt
```

**2D Camera Setup:**
```rust
Camera2d
MainCamera (marker)
PanCamera::default()
RenderLayers::layer(LAYER_2D)
```

**Key Properties:**
- Camera3d at Y=100 (overhead position)
- Looking down at XZ plane (up=NEG_Z)
- Renders to offscreen texture (`RttTextures { texture_3d: Handle<Image> }`)
- Synchronized with Camera2d each frame

### 4.2 Camera Sync System
**File:** `/home/satotakumi/projects/hell-workers/crates/bevy_app/src/systems/visual/camera_sync.rs`

**Function:** `sync_camera3d_system`

**Coordinate Mapping:**
- Camera2d.translation.x → Camera3d.translation.x (same)
- Camera2d.translation.y → Camera3d.translation.z (negated: up=NEG_Z in 3D)
- Camera2d.scale → Camera3d.scale (zoom synchronization)
- Camera3d.translation.y stays at 100.0 (overhead height)

### 4.3 Render Layer Constants
**File:** `/home/satotakumi/projects/hell-workers/crates/hw_core/src/constants/render.rs`

```rust
pub const LAYER_2D: usize = 0;           // Camera2d layer
pub const LAYER_3D: usize = 1;           // Camera3d (RtT) layer

// Z-axis layering (2D)
pub const Z_MAP: f32 = 0.0;
pub const Z_MAP_SAND: f32 = 0.01;
pub const Z_MAP_DIRT: f32 = 0.02;
pub const Z_MAP_GRASS: f32 = 0.03;
pub const Z_ROOM_OVERLAY: f32 = 0.08;
pub const Z_ITEM: f32 = 0.1;
pub const Z_BUILDING_FLOOR: f32 = 0.05;
pub const Z_BUILDING_STRUCT: f32 = 0.12;
pub const Z_BUILDING_DECO: f32 = 0.15;
pub const Z_BUILDING_LIGHT: f32 = 0.18;
pub const Z_AURA: f32 = 0.2;
pub const Z_ITEM_OBSTACLE: f32 = 0.5;
pub const Z_CHARACTER: f32 = 1.0;
pub const Z_SELECTION: f32 = 2.0;
pub const Z_VISUAL_EFFECT: f32 = 3.0;
pub const Z_BAR_BG: f32 = 4.0;
pub const Z_BAR_FILL: f32 = 4.1;
pub const Z_FLOATING_TEXT: f32 = 10.0;
pub const Z_SPEECH_BUBBLE: f32 = 11.0;
pub const Z_SPEECH_BUBBLE_BG: f32 = 10.9;
```

### 4.4 RtT Test Scene
**File:** `/home/satotakumi/projects/hell-workers/crates/bevy_app/src/plugins/startup/rtt_test_scene.rs`

**Status:** ⚠️ **ACTIVE - TO BE DELETED IN PHASE 2**

**Functions:**
1. `spawn_rtt_composite_sprite()` - Creates a sprite displaying the RtT texture
   - Child of MainCamera (Camera2d)
   - Z=20.0 (in front of all content)
   - Visible in LAYER_2D

2. `spawn_test_cube_3d()` - Creates a red test cube
   - Mesh3d with Cuboid(50, 50, 50)
   - RenderLayers::layer(LAYER_3D)
   - Unlit material
   - Visible in Camera3d output

**Activation:** Currently spawned in PostStartup phase (lines 88-89 of `/plugins/startup/mod.rs`)

---

## 5. BUILDING TYPE ENUM

**File:** `/home/satotakumi/projects/hell-workers/crates/hw_jobs/src/model.rs`

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Reflect, Default)]
pub enum BuildingType {
    #[default]
    Wall,                    // VisualLayerKind::Struct
    Door,                    // VisualLayerKind::Struct
    Floor,                   // VisualLayerKind::Floor
    Tank,                    // VisualLayerKind::Struct (state-dependent)
    MudMixer,                // VisualLayerKind::Struct (animated)
    RestArea,                // VisualLayerKind::Struct
    Bridge,                  // VisualLayerKind::Struct
    SandPile,                // VisualLayerKind::Floor
    BonePile,                // VisualLayerKind::Floor
    WheelbarrowParking,      // VisualLayerKind::Struct
}
```

**Phase 2 Relevant Types:**
- **Wall** (primary focus): Complex connection system, provisional states
- **Door** (secondary): Static visual, door state (open/closed/locked)
- **Floor** (dependency): Base layer for rooms
- **Tank** (complex): State-dependent visuals (empty, partial, full)
- **MudMixer** (complex): Animated during refining phase

**Category Classification:**
- **Structure:** Wall, Floor, Bridge
- **Architecture:** Door
- **Plant:** Tank, MudMixer
- **Temporary:** SandPile, BonePile, WheelbarrowParking, RestArea

---

## PHASE 2 READINESS CHECKLIST

### ✅ Completed (Phase 1 / MS-Pre-B)
- [x] RtT infrastructure (Camera3d, RenderTarget, Camera sync)
- [x] VisualLayerKind component system
- [x] Building entity hierarchy (parent + child VisualLayer)
- [x] Wall connection/variant system
- [x] Building-specific animations (Tank, MudMixer)
- [x] Character sprite rendering (Soul, Familiar)
- [x] Render layer constants (LAYER_2D=0, LAYER_3D=1)

### ⚠️ To Be Cleaned Up (Phase 2)
- [ ] Delete `/crates/bevy_app/src/plugins/startup/rtt_test_scene.rs` (55 lines)
- [ ] Remove `rtt_test_scene::spawn_rtt_composite_sprite` from startup (line 88)
- [ ] Remove `rtt_test_scene::spawn_test_cube_3d` from startup (line 89)
- [ ] Remove `mod rtt_test_scene;` from `/plugins/startup/mod.rs` (line 8)

### 🔄 Phase 2 Implementation Tasks
- [ ] Add `RenderLayers::layer(LAYER_3D)` to VisualLayer child entities
- [ ] Implement 3D mesh generation for buildings
- [ ] Replace 2D sprites with 3D models (walls, doors, etc.)
- [ ] Adapt wall connection system for 3D geometry
- [ ] Update character rendering (Soul, Familiar) to 3D
- [ ] Implement 3D animations for Tank, MudMixer
- [ ] Handle layer transitions (2D → 3D sprite compositing via RtT)

---

## KEY FILE LOCATIONS SUMMARY

| Component | File Path |
|-----------|-----------|
| VisualLayerKind | `/crates/hw_visual/src/layer/mod.rs` |
| Wall Connection | `/crates/hw_visual/src/wall_connection.rs` |
| Building Spawn | `/crates/bevy_app/src/systems/jobs/building_completion/spawn.rs` |
| Tank Visuals | `/crates/hw_visual/src/tank.rs` |
| MudMixer Visuals | `/crates/hw_visual/src/mud_mixer.rs` |
| Soul Spawn | `/crates/bevy_app/src/entities/damned_soul/spawn.rs` |
| Soul Visuals | `/crates/hw_visual/src/soul/mod.rs` |
| Familiar Spawn | `/crates/bevy_app/src/entities/familiar/spawn.rs` |
| RtT Setup | `/crates/bevy_app/src/plugins/startup/rtt_setup.rs` |
| Camera Sync | `/crates/bevy_app/src/systems/visual/camera_sync.rs` |
| Render Constants | `/crates/hw_core/src/constants/render.rs` |
| BuildingType | `/crates/hw_jobs/src/model.rs` |
| RtT Test Scene | `/crates/bevy_app/src/plugins/startup/rtt_test_scene.rs` ⚠️ |

