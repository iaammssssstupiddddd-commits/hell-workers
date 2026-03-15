# Phase 2 Documentation Index

## Overview
This directory contains comprehensive documentation for the Hell Workers Phase 2 of 3D-RtT migration codebase analysis. The project has successfully completed MS-Pre-B with the VisualLayer system implementation and is ready for Phase 2.

---

## Documentation Files

### 1. **PHASE2_QUICK_REFERENCE.md** (102 lines)
**Start here for a quick overview**
- Building entity hierarchy diagram
- Critical file locations
- VisualLayerKind enum
- Key render constants
- BuildingType variants with priority
- Phase 2 workflow steps
- Testing checklist

**Use this:** For rapid lookups during development

---

### 2. **PHASE2_CODEBASE_ANALYSIS.md** (403 lines)
**Comprehensive codebase exploration**

#### Sections:
1. **Wall Visual System** (1.1-1.2)
   - Wall connection/variant system details
   - Texture variants (mud vs stone)
   - Building spawn logic
   
2. **Building Visual Layer Structure** (2.1-2.4)
   - VisualLayerKind definition and variants
   - Entity hierarchy after MS-Pre-B
   - Building types and layer assignments
   - Building-specific visual updates (Tank, MudMixer)

3. **Character Visual System** (3.1-3.2)
   - Soul rendering (sprite, components, Z-position)
   - Familiar rendering (sprite, components, aura indicators)
   - Component lists with explanations

4. **RtT Infrastructure from Phase 1** (4.1-4.4)
   - Camera3d setup details
   - Camera sync system (coordinate mapping)
   - Render layer constants (complete list)
   - RtT test scene status (TO DELETE)

5. **Building Type Enum** (Section 5)
   - All 10 building types
   - Category classifications
   - Phase 2 priorities

6. **Phase 2 Readiness Checklist**
   - Completed items (✅)
   - Items to clean up (⚠️)
   - Implementation tasks (🔄)

7. **Key File Locations Summary**
   - Table of all critical files

**Use this:** For detailed understanding of system architecture

---

### 3. **PHASE2_CODE_SIGNATURES.md** (462 lines)
**Detailed code structure and function signatures**

#### Sections:
- **Building Spawn System**
  - `spawn_completed_building()` signature and logic
  
- **Wall Connection System**
  - `wall_connections_system()` with query structure
  - Connection pattern descriptions

- **VisualLayerKind Definition**
  - Enum definition and design notes

- **BuildingType Enum**
  - Full enum with default
  - Category classification logic

- **Tank & MudMixer Visual Systems**
  - Function signatures
  - State-dependent sprite logic
  - Animation details

- **Character Systems**
  - Soul spawn function and components
  - Familiar spawn function and components
  - Aura entity creation

- **RtT Infrastructure**
  - Camera setup code
  - Camera sync logic with coordinate mapping
  - Render layer constants (complete)
  - RtT test scene functions (marked for deletion)

- **Building Bounce Animation**
  - BounceAnimationConfig details
  - Animation lifecycle

- **Key Structs for Phase 2**
  - Building, Blueprint, RttTextures, Camera3dRtt

**Use this:** For implementation reference and copy-paste code patterns

---

## Quick Navigation

### By Task

#### "I need to understand building entity structure"
→ PHASE2_QUICK_REFERENCE.md (Building Entity Hierarchy section)
→ PHASE2_CODE_SIGNATURES.md (Building Spawn System section)

#### "I need to modify wall visuals"
→ PHASE2_CODEBASE_ANALYSIS.md (Section 1: Wall Visual System)
→ PHASE2_CODE_SIGNATURES.md (Wall Connection System section)

#### "I need to add VisualLayerKind to new building types"
→ PHASE2_QUICK_REFERENCE.md (VisualLayerKind enum section)
→ PHASE2_CODE_SIGNATURES.md (VisualLayerKind Definition section)

#### "I need to implement Phase 2 (3D migration)"
→ PHASE2_CODEBASE_ANALYSIS.md (Section 4: RtT Infrastructure)
→ PHASE2_QUICK_REFERENCE.md (Phase 2 Workflow section)
→ PHASE2_CODE_SIGNATURES.md (RtT Infrastructure section)

#### "I need to render a character in 3D"
→ PHASE2_CODEBASE_ANALYSIS.md (Section 3: Character Visual System)
→ PHASE2_CODE_SIGNATURES.md (Character Systems section)

#### "I need to clean up test code"
→ PHASE2_QUICK_REFERENCE.md (TO DELETE section)
→ PHASE2_CODE_SIGNATURES.md (RtT Test Scene section)

---

## Key Facts

### Building Entity Hierarchy (MS-Pre-B Complete)
```
Building Entity (parent)
└─ VisualLayer Child Entity
   └─ Sprite Component
```

### VisualLayerKind Variants (4 total)
- **Floor** (Z=0.05): SandPile, BonePile, Floor
- **Struct** (Z=0.12): Wall, Door, Tank, MudMixer, RestArea, Bridge, WheelbarrowParking
- **Deco** (Z=0.15): Decorations (future)
- **Light** (Z=0.18): Lighting/Effects (future)

### BuildingType Variants (10 total)
Wall, Door, Floor, Tank, MudMixer, RestArea, Bridge, SandPile, BonePile, WheelbarrowParking

### RtT Infrastructure Status
- ✅ Camera3d (orthographic, at Y=100, looking at XZ plane)
- ✅ Camera2d (main viewport)
- ✅ Camera sync system (updates Camera3d from Camera2d each frame)
- ✅ RttTextures resource (offscreen render target)
- ✅ RenderLayers system (LAYER_2D=0, LAYER_3D=1)
- ⚠️ RtT test scene (active, ready for deletion)

### Phase 2 Cleanup (3 items to delete)
1. File: `/crates/bevy_app/src/plugins/startup/rtt_test_scene.rs`
2. Ref: Line 8 (`mod rtt_test_scene;`)
3. Ref: Line 88 (`rtt_test_scene::spawn_rtt_composite_sprite`)
4. Ref: Line 89 (`rtt_test_scene::spawn_test_cube_3d`)

---

## Related Files in Repository

### Planning & Design
- `/docs/plans/3d-rtt/milestone-roadmap.md` - Overall roadmap
- `/docs/plans/3d-rtt/archived/building-visual-layer-implementation-plan-2026-03-15.md` - MS-Pre-B design
- `/docs/plans/3d-rtt/archived/phase1-rtt-infrastructure-plan-2026-03-15.md` - Phase 1 design

### Source Code Structure
```
crates/
├─ bevy_app/
│  ├─ plugins/startup/
│  │  ├─ mod.rs (camera setup, startup)
│  │  ├─ rtt_setup.rs (RttTextures resource)
│  │  ├─ rtt_test_scene.rs (⚠️ TO DELETE)
│  │  └─ visual_handles.rs
│  ├─ systems/visual/
│  │  ├─ camera_sync.rs (sync Camera3d)
│  │  └─ mod.rs
│  ├─ systems/jobs/building_completion/
│  │  └─ spawn.rs (spawn_completed_building)
│  ├─ entities/damned_soul/
│  │  └─ spawn.rs (spawn_damned_soul_at)
│  └─ entities/familiar/
│     └─ spawn.rs (spawn_familiar_at)
├─ hw_visual/
│  ├─ src/layer/
│  │  └─ mod.rs (VisualLayerKind enum)
│  ├─ src/wall_connection.rs (wall_connections_system)
│  ├─ src/tank.rs (update_tank_visual_system)
│  ├─ src/mud_mixer.rs (update_mud_mixer_visual_system)
│  ├─ src/soul/
│  │  └─ mod.rs (soul visual systems)
│  ├─ src/blueprint/
│  │  ├─ mod.rs
│  │  ├─ components.rs (BuildingBounceEffect)
│  │  └─ effects.rs (bounce animation system)
│  └─ src/animations.rs (BounceAnimation)
├─ hw_core/
│  ├─ src/constants/render.rs (render layer constants)
│  ├─ src/soul.rs (DamnedSoul component)
│  └─ src/familiar.rs (Familiar component)
└─ hw_jobs/
   └─ src/model.rs (BuildingType enum)
```

---

## Common Development Workflows

### Adding a New Building Type
1. Add variant to `BuildingType` enum in `/crates/hw_jobs/src/model.rs`
2. Add sprite asset to `GameAssets` in `/crates/bevy_app/src/assets.rs`
3. Update `spawn_completed_building()` to handle new type
4. Assign correct `VisualLayerKind` and Z-position
5. If state-dependent, add visual update system (see Tank/MudMixer)

### Implementing Phase 2 3D Rendering
1. Delete rtt_test_scene.rs (3 references)
2. Update `spawn_completed_building()` to add `RenderLayers::layer(LAYER_3D)` to children
3. Generate 3D meshes instead of sprites
4. Update system queries to handle 3D components
5. Test RtT output via composite sprite

### Updating Wall Visuals for 3D
1. Keep `wall_connections_system()` topology logic (still needed for 3D)
2. Generate 3D mesh variants instead of sprite variants
3. Update `WallVisualHandles` to contain mesh handles
4. Adapt sprite update logic to mesh material/scale updates

---

## Version Information
- **Project:** Hell Workers
- **Analysis Date:** March 15, 2026
- **Bevy Version:** 0.17+
- **Current Phase:** 2 of 3D-RtT migration
- **Previous Milestone:** MS-Pre-B (VisualLayer system)
- **Next Milestone:** Phase 2 (3D model integration)

---

## Questions or Issues?

Refer to the appropriate documentation:
- **Architecture questions** → PHASE2_CODEBASE_ANALYSIS.md
- **Code implementation** → PHASE2_CODE_SIGNATURES.md
- **Quick lookup** → PHASE2_QUICK_REFERENCE.md

All files are in the project root directory.
