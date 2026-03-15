//! Wall construction phase transition system

use super::components::*;
use crate::plugins::startup::Building3dHandles;
use crate::systems::jobs::construction_shared::remove_tile_task_components;
use crate::systems::jobs::{Building, BuildingType, ProvisionalWall};
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

        // 3D ビジュアルエンティティを独立 spawn（仮設壁は警告色マテリアル）
        commands.spawn((
            Mesh3d(handles_3d.wall_mesh.clone()),
            MeshMaterial3d(handles_3d.wall_provisional_material.clone()),
            Transform::from_xyz(world_pos.x, TILE_SIZE / 2.0, -world_pos.y),
            handles_3d.render_layers.clone(),
            Building3dVisual { owner: wall_entity },
            Name::new("Building3dVisual (Wall, Provisional)"),
        ));

        tile.spawned_wall = Some(wall_entity);
        world_map.reserve_building_footprint(
            BuildingType::Wall,
            wall_entity,
            std::iter::once(tile.grid_pos),
        );
    }
}

/// Handles transition from Framing to Coating phase
pub fn wall_construction_phase_transition_system(
    mut q_sites: Query<(Entity, &mut WallConstructionSite)>,
    mut q_tiles: Query<(Entity, &mut WallTileBlueprint)>,
    mut commands: Commands,
) {
    for (site_entity, mut site) in q_sites.iter_mut() {
        if site.phase != WallConstructionPhase::Framing {
            continue;
        }

        let mut total_tiles = 0;
        let mut framed_tiles = 0;

        for (_, tile) in q_tiles.iter().filter(|(_, t)| t.parent_site == site_entity) {
            total_tiles += 1;
            if matches!(tile.state, WallTileState::FramedProvisional) && tile.spawned_wall.is_some()
            {
                framed_tiles += 1;
            }
        }

        // もし残存タイルが0になってしまった場合は何もしない
        if total_tiles == 0 {
            continue;
        }

        if framed_tiles >= total_tiles {
            site.phase = WallConstructionPhase::Coating;

            let tile_entities: Vec<Entity> = q_tiles
                .iter_mut()
                .filter(|(_, tile)| tile.parent_site == site_entity)
                .map(|(tile_entity, mut tile)| {
                    tile.state = WallTileState::WaitingMud;
                    tile_entity
                })
                .collect();
            remove_tile_task_components(&mut commands, &tile_entities);

            info!(
                "Wall site {:?} -> Coating phase (all {} tiles framed)",
                site_entity, site.tiles_total
            );
        }
    }
}
