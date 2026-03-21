//! Wall construction phase transition systems.
//!
//! - `wall_framed_tile_spawn_system`: `Building3dHandles` (bevy_app-only) に依存するため bevy_app に残留。
//! - `wall_construction_phase_transition_system`: root-only 依存なし。hw_jobs へ移設済み。

pub use hw_jobs::wall_construction_phase_transition_system;

use super::components::*;
use crate::plugins::startup::Building3dHandles;
use crate::systems::jobs::{Building, BuildingType, ProvisionalWall};
use crate::systems::visual::wall_orientation_aid::attach_wall_orientation_aid;
use crate::world::map::{WorldMap, WorldMapWrite};
use bevy::prelude::*;
use hw_core::constants::{TILE_SIZE, Z_MAP};
use hw_visual::visual3d::Building3dVisual;

/// Spawns provisional wall entities for framed tiles that do not have spawned walls yet.
pub fn wall_framed_tile_spawn_system(
    mut q_tiles: Query<&mut WallTileBlueprint>,
    handles_3d: Res<Building3dHandles>,
    mut world_map: WorldMapWrite,
    mut commands: Commands,
) {
    for mut tile in q_tiles.iter_mut() {
        if tile.state != WallTileState::FramedProvisional || tile.spawned_wall.is_some() {
            continue;
        }

        let world_pos = WorldMap::grid_to_world(tile.grid_pos.0, tile.grid_pos.1);
        let wall_entity = commands
            .spawn((
                Building {
                    kind: BuildingType::Wall,
                    is_provisional: true,
                },
                ProvisionalWall::default(),
                Transform::from_translation(world_pos.extend(Z_MAP + 0.01)),
                Visibility::default(),
                Name::new("Building (Wall, Provisional)"),
            ))
            .id();

        let visual_entity = commands
            .spawn((
            Mesh3d(handles_3d.wall_mesh.clone()),
            MeshMaterial3d(handles_3d.wall_provisional_material.clone()),
            Transform::from_xyz(world_pos.x, TILE_SIZE / 2.0, -world_pos.y),
            handles_3d.render_layers.clone(),
            Building3dVisual { owner: wall_entity },
            Name::new("Building3dVisual (Wall, Provisional)"),
        ))
            .id();
        attach_wall_orientation_aid(&mut commands, visual_entity, &handles_3d);

        tile.spawned_wall = Some(wall_entity);
        world_map.reserve_building_footprint(
            BuildingType::Wall,
            wall_entity,
            std::iter::once(tile.grid_pos),
        );
    }
}
