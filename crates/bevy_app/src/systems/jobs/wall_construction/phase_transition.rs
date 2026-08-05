//! Root-only provisional wall spawning.
//!
//! The indexed phase transition lives in `hw_logistics`; this system remains
//! here because it depends on root-owned 3D handles and visual wiring.

use super::components::*;
use crate::plugins::startup::Building3dHandles;
use crate::systems::jobs::{Building, BuildingType, ProvisionalWall};
use crate::systems::visual::wall_orientation_aid::attach_wall_orientation_aid;
use crate::world::map::{WorldMap, WorldMapWrite};
use bevy::prelude::*;
use hw_core::constants::{TILE_SIZE, Z_MAP};
use hw_visual::visual3d::Building3dVisual;
type ChangedWallTileQuery<'w, 's> = Query<
    'w,
    's,
    &'static mut WallTileBlueprint,
    Or<(Added<WallTileBlueprint>, Changed<WallTileBlueprint>)>,
>;

/// Spawns provisional wall entities for framed tiles that do not have spawned walls yet.
pub fn wall_framed_tile_spawn_system(
    mut q_tiles: ChangedWallTileQuery,
    handles_3d: Res<Building3dHandles>,
    mut world_map: WorldMapWrite,
    mut commands: Commands,
) {
    for mut tile in q_tiles.iter_mut() {
        if tile.state != WallTileState::FramedProvisional || tile.spawned_wall.is_some() {
            continue;
        }

        let wall_entity = spawn_wall_shell(&mut commands, &handles_3d, tile.grid_pos, true);

        tile.spawned_wall = Some(wall_entity);
        world_map.reserve_building_footprint(
            BuildingType::Wall,
            wall_entity,
            std::iter::once(tile.grid_pos),
        );
    }
}

/// Spawns the production Wall root and owner-linked 3D shell.
///
/// Area construction uses the provisional form and later promotes the same
/// root. Profiling fixtures use the completed form so their final topology is
/// identical without creating synthetic Sprite children.
pub(crate) fn spawn_wall_shell(
    commands: &mut Commands,
    handles_3d: &Building3dHandles,
    grid: (i32, i32),
    is_provisional: bool,
) -> Entity {
    let world_pos = WorldMap::grid_to_world(grid.0, grid.1);
    let wall_entity = commands
        .spawn((
            Building {
                kind: BuildingType::Wall,
                is_provisional,
            },
            Transform::from_translation(world_pos.extend(Z_MAP + 0.01)),
            Visibility::default(),
            Name::new(if is_provisional {
                "Building (Wall, Provisional)"
            } else {
                "Building (Wall)"
            }),
        ))
        .id();
    if is_provisional {
        commands
            .entity(wall_entity)
            .insert(ProvisionalWall::default());
    }

    let material = if is_provisional {
        handles_3d.wall_provisional_material.clone()
    } else {
        handles_3d.wall_material.clone()
    };
    let visual_entity = commands
        .spawn((
            Mesh3d(handles_3d.wall_mesh.clone()),
            MeshMaterial3d(material),
            Transform::from_xyz(world_pos.x, TILE_SIZE / 2.0, -world_pos.y),
            handles_3d.render_layers.clone(),
            Building3dVisual { owner: wall_entity },
            Name::new(if is_provisional {
                "Building3dVisual (Wall, Provisional)"
            } else {
                "Building3dVisual (Wall)"
            }),
        ))
        .id();
    attach_wall_orientation_aid(commands, visual_entity, handles_3d);
    wall_entity
}
