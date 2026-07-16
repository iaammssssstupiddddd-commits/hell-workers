//! Wall construction phase transition systems.
//!
//! - `wall_framed_tile_spawn_system`: `Building3dHandles` (bevy_app-only) に依存するため bevy_app に残留。
//! - `wall_construction_phase_transition_system`: `TileSiteIndex`を使うためrootが所有する。

use super::components::*;
use crate::plugins::startup::Building3dHandles;
#[cfg(feature = "profiling")]
use crate::systems::jobs::ConstructionPerfMetrics;
use crate::systems::jobs::{Building, BuildingType, ProvisionalWall};
use crate::systems::visual::wall_orientation_aid::attach_wall_orientation_aid;
use crate::world::map::{WorldMap, WorldMapWrite};
use bevy::prelude::*;
use hw_core::constants::{TILE_SIZE, Z_MAP};
use hw_jobs::remove_tile_task_components;
use hw_logistics::tile_index::TileSiteIndex;
use hw_visual::visual3d::Building3dVisual;
#[cfg(feature = "profiling")]
use std::time::Instant;

type WallTransitionSiteQuery<'w, 's> = Query<
    'w,
    's,
    (Entity, &'static mut WallConstructionSite),
    Or<(Added<WallConstructionSite>, Changed<WallConstructionSite>)>,
>;
type ChangedWallTileQuery<'w, 's> = Query<
    'w,
    's,
    &'static mut WallTileBlueprint,
    Or<(Added<WallTileBlueprint>, Changed<WallTileBlueprint>)>,
>;

/// Advances a fully framed wall site to the coating phase using the reverse
/// tile index. Counter values only reject early; index count and tile state
/// remain the release-build correctness check.
pub(crate) fn wall_construction_phase_transition_system(
    tile_site_index: Res<TileSiteIndex>,
    mut q_sites: WallTransitionSiteQuery,
    mut q_tiles: Query<&mut WallTileBlueprint>,
    mut commands: Commands,
    #[cfg(feature = "profiling")] mut metrics: ResMut<ConstructionPerfMetrics>,
) {
    #[cfg(feature = "profiling")]
    let started_at = Instant::now();
    for (site_entity, mut site) in q_sites.iter_mut() {
        #[cfg(feature = "profiling")]
        {
            metrics.wall_sites_considered = metrics.wall_sites_considered.saturating_add(1);
        }
        if site.phase != WallConstructionPhase::Framing
            || site.tiles_total == 0
            || site.tiles_framed < site.tiles_total
        {
            continue;
        }

        let Some(tile_entities) = tile_site_index.wall_tiles_by_site.get(&site_entity) else {
            continue;
        };
        #[cfg(feature = "profiling")]
        {
            metrics.wall_tiles_inspected = metrics
                .wall_tiles_inspected
                .saturating_add(tile_entities.len() as u64);
        }
        if tile_entities.len() != site.tiles_total as usize
            || tile_entities.iter().any(|tile_entity| {
                !matches!(
                    q_tiles
                        .get(*tile_entity)
                        .map(|tile| (tile.state, tile.spawned_wall)),
                    Ok((WallTileState::FramedProvisional, Some(_)))
                )
            })
        {
            continue;
        }

        let mut transitioned_tiles = Vec::with_capacity(tile_entities.len());
        for tile_entity in tile_entities {
            let Ok(mut tile) = q_tiles.get_mut(*tile_entity) else {
                transitioned_tiles.clear();
                break;
            };
            tile.state = WallTileState::WaitingMud;
            transitioned_tiles.push(*tile_entity);
        }
        if transitioned_tiles.len() != tile_entities.len() {
            continue;
        }

        site.phase = WallConstructionPhase::Coating;
        remove_tile_task_components(&mut commands, &transitioned_tiles);
        info!(
            "Wall site {:?} -> Coating phase (all {} tiles framed)",
            site_entity, site.tiles_total
        );
    }
    #[cfg(feature = "profiling")]
    {
        metrics.wall_phase_elapsed_micros = metrics
            .wall_phase_elapsed_micros
            .saturating_add(started_at.elapsed().as_micros() as u64);
    }
}

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
