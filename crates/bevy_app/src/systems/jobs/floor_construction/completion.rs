//! Floor construction completion system

use super::components::*;
use crate::entities::damned_soul::{DamnedSoul, Path};
use crate::plugins::startup::Building3dHandles;
use crate::systems::jobs::{Building, BuildingType, ObstaclePosition};
use crate::world::map::{WorldMap, WorldMapWrite};
use bevy::prelude::*;
use hw_core::constants::{FLOOR_CURING_DURATION_SECS, Z_MAP};
use hw_visual::animations::{BounceAnimation, BounceAnimationConfig};
use hw_visual::blueprint::{BOUNCE_DURATION, BuildingBounceEffect};
use hw_visual::visual3d::Building3dVisual;
use std::collections::HashSet;

fn evacuate_souls_from_blocked_tiles(
    q_souls: &mut Query<(Entity, &mut Transform, &mut Path), With<DamnedSoul>>,
    blocked_tiles: &HashSet<(i32, i32)>,
    world_map: &WorldMap,
) -> usize {
    let mut evacuated = 0usize;
    for (soul_entity, mut soul_transform, mut path) in q_souls.iter_mut() {
        let soul_pos = soul_transform.translation.truncate();
        let soul_grid = WorldMap::world_to_grid(soul_pos);
        if !blocked_tiles.contains(&soul_grid) {
            continue;
        }

        if let Some((target_gx, target_gy)) = world_map.get_nearest_walkable_grid(soul_pos) {
            let target_pos = WorldMap::grid_to_world(target_gx, target_gy);
            soul_transform.translation.x = target_pos.x;
            soul_transform.translation.y = target_pos.y;
            path.waypoints.clear();
            path.current_index = 0;
            evacuated += 1;
        } else {
            warn!(
                "FLOOR_CURING: Soul {:?} could not find evacuation tile from {:?}",
                soul_entity, soul_grid
            );
        }
    }

    evacuated
}

/// Handles floor construction completion
pub fn floor_construction_completion_system(
    time: Res<Time>,
    mut q_sites: Query<(Entity, &mut FloorConstructionSite)>,
    q_tiles: Query<(Entity, &FloorTileBlueprint)>,
    mut q_souls: Query<(Entity, &mut Transform, &mut Path), With<DamnedSoul>>,
    handles_3d: Res<Building3dHandles>,
    mut world_map: WorldMapWrite,
    mut commands: Commands,
) {
    for (site_entity, mut site) in q_sites.iter_mut() {
        let site_tiles: Vec<(Entity, (i32, i32), FloorTileState)> = q_tiles
            .iter()
            .filter(|(_, tile)| tile.parent_site == site_entity)
            .map(|(tile_entity, tile)| (tile_entity, tile.grid_pos, tile.state))
            .collect();

        if site_tiles.is_empty() {
            continue;
        }

        // Check if all tiles complete
        let all_complete = site_tiles
            .iter()
            .all(|(_, _, state)| *state == FloorTileState::Complete);

        if !all_complete {
            continue;
        }

        // Enter curing phase once all pouring is done.
        if site.phase != FloorConstructionPhase::Curing {
            site.phase = FloorConstructionPhase::Curing;
            site.curing_remaining_secs = FLOOR_CURING_DURATION_SECS.max(0.0);

            let blocked_tiles: HashSet<(i32, i32)> = site_tiles
                .iter()
                .map(|(_, grid_pos, _)| *grid_pos)
                .collect();

            for (tile_entity, (gx, gy), _) in &site_tiles {
                commands
                    .entity(*tile_entity)
                    .insert(ObstaclePosition(*gx, *gy));
            }
            world_map.reserve_building_footprint_tiles(blocked_tiles.iter().copied());

            let evacuated =
                evacuate_souls_from_blocked_tiles(&mut q_souls, &blocked_tiles, &world_map);

            info!(
                "Floor site {:?} entered curing ({:.1}s, evacuated {} souls)",
                site_entity, site.curing_remaining_secs, evacuated
            );
            continue;
        }

        let blocked_tiles: HashSet<(i32, i32)> = site_tiles
            .iter()
            .map(|(_, grid_pos, _)| *grid_pos)
            .collect();
        let re_evacuated =
            evacuate_souls_from_blocked_tiles(&mut q_souls, &blocked_tiles, &world_map);
        if re_evacuated > 0 {
            debug!(
                "FLOOR_CURING: Site {:?} re-evacuated {} souls still inside curing area",
                site_entity, re_evacuated
            );
        }

        site.curing_remaining_secs = (site.curing_remaining_secs - time.delta_secs()).max(0.0);
        if site.curing_remaining_secs > 0.0 {
            continue;
        }

        let completed_grids: Vec<(i32, i32)> =
            site_tiles.iter().map(|(_, grid, _)| *grid).collect();

        // For each tile: spawn Building entity with Floor type
        let mut tile_count = 0;
        for (tile_entity, (gx, gy), _) in site_tiles {
            let world_pos = WorldMap::grid_to_world(gx, gy);

            let building_entity = commands
                .spawn((
                    Building {
                        kind: BuildingType::Floor,
                        is_provisional: false,
                    },
                    BuildingBounceEffect {
                        bounce_animation: BounceAnimation {
                            timer: 0.0,
                            config: BounceAnimationConfig {
                                duration: BOUNCE_DURATION,
                                min_scale: 1.0,
                                max_scale: 1.2,
                            },
                        },
                    },
                    Transform::from_translation(world_pos.extend(Z_MAP + 0.01)),
                    Visibility::default(),
                    Name::new("Building (Floor)"),
                ))
                .id();

            // 3D ビジュアルエンティティを独立 spawn（Floor は y=0 の地面レベル）
            commands.spawn((
                Mesh3d(handles_3d.floor_mesh.clone()),
                MeshMaterial3d(handles_3d.floor_material.clone()),
                Transform::from_xyz(world_pos.x, 0.0, -world_pos.y),
                handles_3d.render_layers.clone(),
                Building3dVisual {
                    owner: building_entity,
                },
                Name::new("Building3dVisual (Floor)"),
            ));

            // Despawn tile blueprint
            commands.entity(tile_entity).despawn();
            tile_count += 1;
        }

        // Curing is complete: tile becomes walkable again.
        world_map.clear_building_footprint(completed_grids);

        // Despawn site
        commands.entity(site_entity).despawn();

        info!(
            "Floor site {:?} completed after curing ({} tiles, total {}/{})",
            site_entity, tile_count, site.tiles_poured, site.tiles_total
        );
    }
}
