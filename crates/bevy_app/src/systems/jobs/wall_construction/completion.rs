//! Wall construction completion system

use super::components::*;
#[cfg(feature = "profiling")]
use crate::systems::jobs::ConstructionPerfMetrics;
use crate::systems::jobs::{Building, BuildingType, ProvisionalWall};
use crate::world::map::WorldMapWrite;
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use hw_logistics::tile_index::TileSiteIndex;
#[cfg(feature = "profiling")]
use std::time::Instant;

type SiteTileData = (Entity, (i32, i32), WallTileState, Option<Entity>);
type WallCompletionSiteQuery<'w, 's> =
    Query<'w, 's, (Entity, &'static WallConstructionSite), Changed<WallConstructionSite>>;
type WallCompletionTileQuery<'w, 's> = Query<'w, 's, &'static WallTileBlueprint>;
type WallCompletionRequestQuery<'w, 's> =
    Query<'w, 's, (Entity, &'static TargetWallConstructionSite)>;
type WallCompletionBuildingQuery<'w, 's> = Query<'w, 's, &'static mut Building>;

#[derive(SystemParam)]
pub struct WallCompletionParams<'w, 's> {
    tile_site_index: Res<'w, TileSiteIndex>,
    q_sites: WallCompletionSiteQuery<'w, 's>,
    q_tiles: WallCompletionTileQuery<'w, 's>,
    q_requests: WallCompletionRequestQuery<'w, 's>,
    q_buildings: WallCompletionBuildingQuery<'w, 's>,
    #[cfg(feature = "profiling")]
    metrics: ResMut<'w, ConstructionPerfMetrics>,
}

/// Handles wall construction completion (no curing phase)
pub fn wall_construction_completion_system(
    mut world_map: WorldMapWrite,
    mut commands: Commands,
    params: WallCompletionParams,
) {
    let WallCompletionParams {
        tile_site_index,
        q_sites,
        q_tiles,
        q_requests,
        mut q_buildings,
        #[cfg(feature = "profiling")]
        mut metrics,
    } = params;

    #[cfg(feature = "profiling")]
    let started_at = Instant::now();
    for (site_entity, site) in q_sites.iter() {
        #[cfg(feature = "profiling")]
        {
            metrics.wall_sites_considered = metrics.wall_sites_considered.saturating_add(1);
        }
        if site.phase != WallConstructionPhase::Coating
            || site.tiles_total == 0
            || site.tiles_coated < site.tiles_total
        {
            continue;
        }

        let site_tiles: Vec<SiteTileData> = tile_site_index
            .wall_tiles_by_site
            .get(&site_entity)
            .into_iter()
            .flatten()
            .filter_map(|&tile_entity| {
                q_tiles
                    .get(tile_entity)
                    .ok()
                    .map(|tile| (tile_entity, tile.grid_pos, tile.state, tile.spawned_wall))
            })
            .collect();

        #[cfg(feature = "profiling")]
        {
            metrics.wall_tiles_inspected = metrics
                .wall_tiles_inspected
                .saturating_add(site_tiles.len() as u64);
        }

        if site_tiles.len() != site.tiles_total as usize
            || !site_tiles
                .iter()
                .all(|(_, _, state, _)| *state == WallTileState::Complete)
        {
            continue;
        }

        for (request_entity, target_site) in q_requests.iter() {
            if target_site.0 == site_entity {
                commands.entity(request_entity).try_despawn();
            }
        }

        let mut released_site_grids = Vec::new();
        for (tile_entity, (gx, gy), _, spawned_wall) in site_tiles {
            if let Some(wall_entity) = spawned_wall {
                if let Ok(mut building) = q_buildings.get_mut(wall_entity)
                    && building.kind == BuildingType::Wall
                {
                    building.is_provisional = false;
                }
                commands.entity(wall_entity).remove::<ProvisionalWall>();
            } else {
                released_site_grids.push((gx, gy));
            }

            commands.entity(tile_entity).try_despawn();
        }
        world_map.release_building_footprint_if_owned(site_entity, released_site_grids);

        commands.entity(site_entity).try_despawn();

        info!(
            "Wall site {:?} completed ({} tiles, coated {}/{})",
            site_entity, site.tiles_total, site.tiles_coated, site.tiles_total
        );
    }
    #[cfg(feature = "profiling")]
    {
        metrics.wall_completion_elapsed_micros = metrics
            .wall_completion_elapsed_micros
            .saturating_add(started_at.elapsed().as_micros() as u64);
    }
}
