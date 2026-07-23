use super::{
    BlueprintSpriteHandles, clear_rehydrate_presentation, rehydrate_construction_runtime,
    rehydrate_construction_shells, rehydrate_obstacle_runtime, rehydrate_soul_shells,
    rehydrate_stockpile_policies, validate_rehydrate_prerequisites,
};
use crate::entities::damned_soul::{Gender, SoulIdentity};
use crate::plugins::startup::Building3dHandles;
use crate::systems::jobs::floor_construction::CuringFootprint;
use crate::world::map::WorldMap;
use bevy::prelude::*;
use bevy::time::Virtual;
use hw_core::area::TaskArea;
use hw_core::constants::TILE_SIZE;
use hw_core::jobs::WorkType;
use hw_core::logistics::ResourceType;
use hw_core::soul::DamnedSoul;
use hw_core::system_sets::GameSystemSet;
use hw_core::visual_mirror::construction::{
    BlueprintVisualState, FloorConstructionPhaseMirror, FloorSiteVisualState, FloorTileStateMirror,
    FloorTileVisualMirror, WallSiteVisualState, WallTileStateMirror, WallTileVisualMirror,
};
use hw_core::world::DoorState;
use hw_energy::SoulSpaSite;
use hw_jobs::construction::{
    FloorConstructionPhase, FloorConstructionSite, FloorTileBlueprint, FloorTileState,
    WallConstructionPhase, WallConstructionSite, WallTileBlueprint, WallTileState,
};
use hw_jobs::{
    Blueprint, Building, BuildingType, Designation, Door, ObstaclePosition, ObstacleSourceKind,
    Rock, Tree, TreeVariant,
};
use hw_logistics::tile_index::TileSiteIndex;
use hw_visual::MaterialIconHandles;
use hw_visual::blueprint::{
    BlueprintProgressBars, BlueprintState, BlueprintVisual, DeliveryPopup, MaterialCounter,
    MaterialIcon, material_delivery_vfx_system, spawn_material_display_system,
    spawn_progress_bar_system, update_blueprint_visual_system, update_progress_bar_fill_system,
};
use hw_visual::floor_construction::{
    FloorCuringProgressBar, manage_floor_curing_progress_bars_system,
    update_floor_curing_progress_bars_system,
};
use hw_visual::wall_construction::{
    WallConstructionProgressBar, manage_wall_progress_bars_system, update_wall_progress_bars_system,
};
use hw_world::TerrainType;

fn empty_building_3d_handles() -> Building3dHandles {
    Building3dHandles {
        wall_mesh: Handle::default(),
        wall_material: Handle::default(),
        wall_provisional_material: Handle::default(),
        wall_orientation_aid_mesh: Handle::default(),
        wall_orientation_aid_material: Handle::default(),
        floor_mesh: Handle::default(),
        floor_material: Handle::default(),
        door_mesh: Handle::default(),
        door_material: Handle::default(),
        equipment_1x1_mesh: Handle::default(),
        equipment_2x2_mesh: Handle::default(),
        equipment_material: Handle::default(),
        soul_scene: Handle::default(),
        familiar_mesh: Handle::default(),
        familiar_material: Handle::default(),
        render_layers: bevy::camera::visibility::RenderLayers::default(),
    }
}

fn empty_material_icon_handles() -> MaterialIconHandles {
    MaterialIconHandles {
        wood_small: Handle::default(),
        rock_small: Handle::default(),
        sand_small: Handle::default(),
        bone_small: Handle::default(),
        stasis_mud_small: Handle::default(),
        water_small: Handle::default(),
        font_ui: Handle::default(),
    }
}

#[derive(Resource, Default)]
struct LogicRunCount(u32);

fn count_logic_run(mut count: ResMut<LogicRunCount>) {
    count.0 += 1;
}

fn component_count<T: Component>(world: &mut World) -> usize {
    let mut query = world.query::<&T>();
    query.iter(world).count()
}

mod construction;
mod obstacles;
mod presentation;
mod stockpile_policy;
