# Phase 2 Code Signatures Reference

## Building Spawn System

### spawn_completed_building()
**Location:** `/crates/bevy_app/src/systems/jobs/building_completion/spawn.rs:9-92`

```rust
pub(super) fn spawn_completed_building(
    commands: &mut Commands,
    bp: &Blueprint,
    transform: &Transform,
    game_assets: &GameAssets,
) -> Entity
```

**Returns:** Building entity ID
**Creates:** Parent entity with child VisualLayer entity

**Key Logic:**
```rust
// Determine Z-layer and VisualLayerKind
let (z, layer_kind) = match bp.kind {
    BuildingType::Floor | BuildingType::SandPile | BuildingType::BonePile => {
        (Z_BUILDING_FLOOR, VisualLayerKind::Floor)
    }
    _ => (Z_BUILDING_STRUCT, VisualLayerKind::Struct),
};

// Create parent Building entity
let building_entity = commands.spawn((
    Building { kind: bp.kind, is_provisional },
    Transform::from_xyz(x, y, z),
    BuildingBounceEffect { ... },
)).with_children(|parent| {
    // Create child VisualLayer entity
    parent.spawn((
        layer_kind,
        Sprite { image, custom_size: Some(size), ... },
        Transform::default(),
    ));
}).id();
```

---

## Wall Connection System

### wall_connections_system()
**Location:** `/crates/hw_visual/src/wall_connection.rs:10-104`

```rust
pub fn wall_connections_system(
    wall_handles: Res<WallVisualHandles>,
    world_map: WorldMapRead,
    q_new_buildings: Query<
        (Entity, &Transform, &Building),
        Or<(Added<Building>, Changed<Building>)>,
    >,
    q_new_blueprints: Query<
        (Entity, &Transform, &BlueprintVisualState),
        Added<BlueprintVisualState>,
    >,
    q_walls_check: Query<
        (Option<&Building>, Option<&BlueprintVisualState>),
        Or<(With<Building>, With<BlueprintVisualState>)>,
    >,
    q_children: Query<&Children>,
    mut q_visual_layers: Query<(&VisualLayerKind, &mut Sprite)>,
    mut q_blueprint_sprites: Query<&mut Sprite, Without<VisualLayerKind>>,
)
```

**Updates:** Sprite image based on 4-way neighbor topology (16 variants)

**Key Connection Patterns:**
- `(false, false, false, false)` → isolated
- `(true, true, true, true)` → cross junction
- Corners: `(true, false, true, false)` → top-left
- T-junctions: `(true, true, true, false)` → T pointing up

---

## VisualLayerKind Definition

**Location:** `/crates/hw_visual/src/layer/mod.rs:8-17`

```rust
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub enum VisualLayerKind {
    /// Floor・Ground (Z_BUILDING_FLOOR = 0.05)
    Floor,
    /// Wall・Structure (Z_BUILDING_STRUCT = 0.12)
    Struct,
    /// Decoration layer (Z_BUILDING_DECO = 0.15)
    Deco,
    /// Lighting・Effects (Z_BUILDING_LIGHT = 0.18)
    Light,
}
```

**Design Notes:**
- Each variant is a **component** on child entities
- Maps directly to Z-coordinate constants
- Ready for `RenderLayers::layer(1)` addition in Phase 2

---

## BuildingType Enum

**Location:** `/crates/hw_jobs/src/model.rs:12-24`

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Reflect, Default)]
pub enum BuildingType {
    #[default]
    Wall,
    Door,
    Floor,
    Tank,
    MudMixer,
    RestArea,
    Bridge,
    SandPile,
    BonePile,
    WheelbarrowParking,
}
```

**Category Method:**
```rust
impl BuildingType {
    pub fn category(&self) -> BuildingCategory {
        match self {
            Wall | Floor | Bridge => BuildingCategory::Structure,
            Door => BuildingCategory::Architecture,
            Tank | MudMixer => BuildingCategory::Plant,
            SandPile | BonePile | WheelbarrowParking | RestArea => BuildingCategory::Temporary,
        }
    }
}
```

---

## Tank Visual System

### update_tank_visual_system()
**Location:** `/crates/hw_visual/src/tank.rs:11-43`

```rust
pub fn update_tank_visual_system(
    handles: Res<BuildingAnimHandles>,
    q_tanks: Query<(Entity, &Building, &Stockpile, Option<&StoredItems>)>,
    q_children: Query<&Children>,
    mut q_visual_layers: Query<(&VisualLayerKind, &mut Sprite)>,
)
```

**Updates:** `VisualLayerKind::Struct` child sprite
- `0 items` → `tank_empty`
- `1..capacity-1 items` → `tank_partial`
- `capacity items` → `tank_full`

---

## MudMixer Visual System

### update_mud_mixer_visual_system()
**Location:** `/crates/hw_visual/src/mud_mixer.rs:14-56`

```rust
pub fn update_mud_mixer_visual_system(
    handles: Res<BuildingAnimHandles>,
    time: Res<Time>,
    q_souls: Query<&AssignedTask>,
    q_mixers: Query<Entity, With<MudMixerStorage>>,
    q_children: Query<&Children>,
    mut q_visual_layers: Query<(&VisualLayerKind, &mut Sprite)>,
)
```

**Updates:** `VisualLayerKind::Struct` child sprite
- **Idle:** `mud_mixer_idle`
- **Refining:** Cycles through `mud_mixer_anim_1` → `anim_4` at 6 FPS

**Animation Speed:** `MUD_MIXER_ANIMATION_FPS = 6.0`

---

## Character Systems

### Soul Spawn

**Location:** `/crates/bevy_app/src/entities/damned_soul/spawn.rs:166-214`

```rust
pub fn spawn_damned_soul_at(
    commands: &mut Commands,
    game_assets: &Res<GameAssets>,
    world_map: &WorldMap,
    pos: Vec2,
)
```

**Components Added:**
```rust
(
    DamnedSoul::default(),
    SoulUiLinks::default(),
    DreamState::default(),
    identity: SoulIdentity,
    IdleState::default(),
    AssignedTask::default(),
    InventoryItemVisual::default(),
    SoulTaskVisualState::default(),
    Sprite { image, custom_size: Vec2::splat(TILE_SIZE * 0.8), color },
    Transform::from_xyz(x, y, Z_CHARACTER),
    Destination, Path, AnimationState, SoulEmotionState,
    ConversationInitiator, Inventory,
)
```

**Sprite Color by Gender:**
- Male: `Color::srgb(0.9, 0.9, 1.0)` (blue tint)
- Female: `Color::srgb(1.0, 0.9, 0.9)` (red tint)

### Familiar Spawn

**Location:** `/crates/bevy_app/src/entities/familiar/spawn.rs:68-150`

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

**Components Added:**
```rust
(
    familiar: Familiar { familiar_type, command_radius, efficiency, name, color_index },
    Name,
    FamiliarOperation::default(),
    ActiveCommand::default(),
    FamiliarAiState::default(),
    FamiliarAiStateHistory::default(),
    Commanding::default(),
    ManagedTasks::default(),
    Destination, Path,
    FamiliarAnimation::default(),
    FamiliarVoice::random(),
    Sprite { image, custom_size: Vec2::splat(TILE_SIZE * 0.9) },
    Transform::from_xyz(x, y, Z_CHARACTER + 0.5),
)
```

**Plus 3 Aura Entities:**
1. `FamiliarRangeIndicator + AuraLayer::Border` (orange circle, 0.3 alpha)
2. `FamiliarRangeIndicator + AuraLayer::Outline` (yellow ring, 0.0 alpha)
3. `FamiliarAura + AuraLayer::Pulse` (orange circle, 0.15 alpha, pulsing)

---

## RtT Infrastructure

### Camera Setup (Startup)

**Location:** `/crates/bevy_app/src/plugins/startup/mod.rs:110-148`

```rust
// Camera3d (RtT offscreen)
commands.spawn((
    Camera3d::default(),
    Camera {
        order: -1,
        clear_color: ClearColorConfig::Custom(Color::srgba(0.0, 0.0, 0.0, 0.0)),
        ..default()
    },
    Projection::Orthographic(OrthographicProjection::default_3d()),
    Transform::from_translation(Vec3::new(0.0, 100.0, 0.0))
        .looking_at(Vec3::ZERO, Vec3::NEG_Z),
    RenderTarget::Image(rtt_handle.into()),
    RenderLayers::layer(LAYER_3D),
    Camera3dRtt,
));

// Camera2d (main screen)
commands.spawn((
    Camera2d,
    MainCamera,
    PanCamera::default(),
    RenderLayers::layer(LAYER_2D),
));
```

**Key Properties:**
- Camera3d at Y=100.0 (overhead position)
- Looking at (0, 0, 0) with up=NEG_Z
- Renders to offscreen texture
- Order: -1 (before Camera2d which defaults to 0)

### Camera Sync System

**Location:** `/crates/bevy_app/src/systems/visual/camera_sync.rs:17-28`

```rust
pub fn sync_camera3d_system(
    q_cam2d: Query<&Transform, (With<MainCamera>, Without<Camera3dRtt>)>,
    mut q_cam3d: Query<&mut Transform, With<Camera3dRtt>>,
) {
    let Ok(cam2d) = q_cam2d.single() else { return };
    let Ok(mut cam3d) = q_cam3d.single_mut() else { return };

    cam3d.translation.x = cam2d.translation.x;
    cam3d.translation.z = -cam2d.translation.y; // Negate: up=NEG_Z
    // cam3d.translation.y stays at 100.0
    cam3d.scale = cam2d.scale;
}
```

**Coordinate Mapping:**
- 2D X → 3D X (direct)
- 2D Y → 3D Z (negated for up=NEG_Z)
- 2D scale → 3D scale (zoom)
- 3D Y fixed at 100.0 (overhead height)

---

## Render Layer Constants

**Location:** `/crates/hw_core/src/constants/render.rs`

```rust
// Layer indices
pub const LAYER_2D: usize = 0;    // Camera2d renders here
pub const LAYER_3D: usize = 1;    // Camera3d (RtT) renders here

// Building Z-layers
pub const Z_BUILDING_FLOOR: f32 = 0.05;   // Floors, SandPile, BonePile
pub const Z_BUILDING_STRUCT: f32 = 0.12;  // Walls, Doors, most buildings
pub const Z_BUILDING_DECO: f32 = 0.15;    // Decorations
pub const Z_BUILDING_LIGHT: f32 = 0.18;   // Lighting/Effects

// Character Z-layer
pub const Z_CHARACTER: f32 = 1.0;         // Soul, Familiar
pub const Z_AURA: f32 = 0.2;              // Range indicators

// UI/Effects Z-layers
pub const Z_SELECTION: f32 = 2.0;
pub const Z_VISUAL_EFFECT: f32 = 3.0;
pub const Z_FLOATING_TEXT: f32 = 10.0;
pub const Z_SPEECH_BUBBLE: f32 = 11.0;
```

---

## RtT Test Scene (TO DELETE)

**Location:** `/crates/bevy_app/src/plugins/startup/rtt_test_scene.rs`

### spawn_rtt_composite_sprite()
```rust
pub fn spawn_rtt_composite_sprite(
    mut commands: Commands,
    rtt: Res<RttTextures>,
    q_cam2d: Query<Entity, With<MainCamera>>,
)
```
- Creates sprite displaying RtT texture
- Child of MainCamera (Camera2d)
- Z=20.0 (in front of all content)

### spawn_test_cube_3d()
```rust
pub fn spawn_test_cube_3d(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
)
```
- Creates red Cuboid(50, 50, 50) test mesh
- RenderLayers::layer(LAYER_3D)
- Unlit material
- Visible in Camera3d output

**Status:** Both functions currently spawned in PostStartup phase
**Action:** Delete file and remove 3 references from mod.rs

---

## Building Bounce Animation

**Location:** `/crates/hw_visual/src/blueprint/components.rs:46-49`

```rust
#[derive(Component)]
pub struct BuildingBounceEffect {
    pub bounce_animation: BounceAnimation,
}
```

**Animation Sequence:**
```rust
pub struct BounceAnimation {
    pub timer: f32,
    pub config: BounceAnimationConfig,
}

pub struct BounceAnimationConfig {
    pub duration: f32,         // Default: 0.4 seconds
    pub min_scale: f32,        // Default: 1.0
    pub max_scale: f32,        // Default: 1.2
}
```

**Applied to:** Building parent entity on spawn (affects entire building + children)
**Removed after:** Animation completes

---

## Key Structs for Phase 2

### Building (Component)
```rust
#[derive(Component, Reflect, Default)]
pub struct Building {
    pub kind: BuildingType,
    pub is_provisional: bool,
}
```

### Blueprint (Component)
```rust
pub struct Blueprint {
    pub kind: BuildingType,
    pub progress: f32,
    pub required_materials: HashMap<ResourceType, u32>,
    pub delivered_materials: HashMap<ResourceType, u32>,
    pub occupied_grids: Vec<(i32, i32)>,
    // ...
}
```

### RttTextures (Resource)
```rust
#[derive(Resource)]
pub struct RttTextures {
    pub texture_3d: Handle<Image>,
}
```

### Camera3dRtt (Component Marker)
```rust
#[derive(Component)]
pub struct Camera3dRtt;
```

