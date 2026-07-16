//! Floor construction completion system

use super::components::*;
use crate::entities::damned_soul::{DamnedSoul, Path};
use crate::plugins::startup::Building3dHandles;
#[cfg(feature = "profiling")]
use crate::systems::jobs::ConstructionPerfMetrics;
use crate::systems::jobs::{Building, BuildingType, ObstaclePosition, ObstacleSourceKind};
use crate::world::map::{WorldMap, WorldMapWrite};
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use hw_core::constants::{FLOOR_CURING_DURATION_SECS, TILE_SIZE, Z_MAP};
use hw_logistics::tile_index::TileSiteIndex;
use hw_spatial::{SpatialGrid, SpatialGridOps};
use hw_visual::animations::{BounceAnimation, BounceAnimationConfig};
use hw_visual::blueprint::{BOUNCE_DURATION, BuildingBounceEffect};
use hw_visual::visual3d::Building3dVisual;
use std::collections::HashSet;
#[cfg(feature = "profiling")]
use std::time::Instant;

type FloorTileData = (Entity, (i32, i32), FloorTileState);
type FloorCompletionSiteQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static mut FloorConstructionSite,
        Option<&'static mut CuringFootprint>,
    ),
    Or<(
        Added<FloorConstructionSite>,
        Changed<FloorConstructionSite>,
        With<CuringFootprint>,
    )>,
>;
type FloorCompletionTileQuery<'w, 's> = Query<'w, 's, &'static FloorTileBlueprint>;
type FloorCompletionSoulQuery<'w, 's> =
    Query<'w, 's, (Entity, &'static mut Transform, &'static mut Path), With<DamnedSoul>>;

#[derive(SystemParam)]
pub(crate) struct FloorCompletionQueries<'w, 's> {
    q_sites: FloorCompletionSiteQuery<'w, 's>,
    q_tiles: FloorCompletionTileQuery<'w, 's>,
    q_souls: FloorCompletionSoulQuery<'w, 's>,
    nearby_souls: Local<'s, Vec<Entity>>,
    handles_3d: Res<'w, Building3dHandles>,
    #[cfg(feature = "profiling")]
    metrics: ResMut<'w, ConstructionPerfMetrics>,
}

/// Runtime-only footprint for a site in the curing phase.
///
/// The durable source of truth remains the site and its tiles. This component
/// prevents repeated tile enumeration while curing and is deliberately rebuilt
/// after load without reserving the already-restored WorldMap footprint again.
#[derive(Component)]
pub(crate) struct CuringFootprint {
    tiles: Vec<(Entity, (i32, i32))>,
    blocked_tiles: HashSet<(i32, i32)>,
    search_center: Vec2,
    search_radius: f32,
    safety_audit: Timer,
}

impl CuringFootprint {
    pub(crate) fn from_tile_positions(
        tiles: impl IntoIterator<Item = (Entity, (i32, i32))>,
    ) -> Self {
        let tile_data = tiles.into_iter().collect::<Vec<_>>();
        let blocked_tiles = tile_data.iter().map(|(_, grid)| *grid).collect();
        let (search_center, search_radius) = evacuation_search_bounds(&tile_data);
        Self {
            tiles: tile_data,
            blocked_tiles,
            search_center,
            search_radius,
            safety_audit: Timer::from_seconds(0.5, TimerMode::Repeating),
        }
    }

    fn from_tiles(tiles: &[FloorTileData]) -> Self {
        let tile_data = tiles
            .iter()
            .map(|(entity, grid, _)| (*entity, *grid))
            .collect::<Vec<_>>();
        Self::from_tile_positions(tile_data)
    }
}

/// Returns a circle covering all footprint tiles plus one tile of movement
/// margin. The spatial query is a candidate filter only; exact grid membership
/// remains the evacuation condition below.
fn evacuation_search_bounds(tiles: &[(Entity, (i32, i32))]) -> (Vec2, f32) {
    let Some((_, first_grid)) = tiles.first() else {
        return (Vec2::ZERO, TILE_SIZE);
    };
    let mut min = WorldMap::grid_to_world(first_grid.0, first_grid.1);
    let mut max = min;
    for (_, grid) in tiles.iter().skip(1) {
        let world_pos = WorldMap::grid_to_world(grid.0, grid.1);
        min = min.min(world_pos);
        max = max.max(world_pos);
    }
    let center = (min + max) * 0.5;
    let radius = center.distance(max) + TILE_SIZE;
    (center, radius)
}

fn indexed_floor_tiles(
    site_entity: Entity,
    tile_site_index: &TileSiteIndex,
    q_tiles: &Query<&FloorTileBlueprint>,
) -> Vec<FloorTileData> {
    tile_site_index
        .floor_tiles_by_site
        .get(&site_entity)
        .into_iter()
        .flatten()
        .filter_map(|&tile_entity| {
            q_tiles
                .get(tile_entity)
                .ok()
                .map(|tile| (tile_entity, tile.grid_pos, tile.state))
        })
        .collect()
}

fn collect_curing_soul_candidates(
    soul_grid: &SpatialGrid,
    footprint: &CuringFootprint,
    nearby_candidates: &mut Vec<Entity>,
) {
    soul_grid.get_nearby_in_radius_into(
        footprint.search_center,
        footprint.search_radius,
        nearby_candidates,
    );
    // Spatial-grid buckets are hash-backed. Preserve a stable mutation order
    // for fixed-step audits and guard against future multi-bucket indexing.
    nearby_candidates.sort_unstable_by_key(|entity| entity.to_bits());
    nearby_candidates.dedup();
}

fn evacuate_souls_from_blocked_tiles(
    soul_grid: &SpatialGrid,
    nearby_candidates: &mut Vec<Entity>,
    q_souls: &mut Query<(Entity, &mut Transform, &mut Path), With<DamnedSoul>>,
    footprint: &CuringFootprint,
    world_map: &WorldMap,
    #[cfg(feature = "profiling")] metrics: &mut ConstructionPerfMetrics,
) -> usize {
    collect_curing_soul_candidates(soul_grid, footprint, nearby_candidates);
    #[cfg(feature = "profiling")]
    {
        metrics.evacuation_candidates_scanned = metrics
            .evacuation_candidates_scanned
            .saturating_add(nearby_candidates.len() as u64);
    }

    let mut evacuated = 0usize;
    for soul_entity in nearby_candidates.iter().copied() {
        let Ok((_, mut soul_transform, mut path)) = q_souls.get_mut(soul_entity) else {
            continue;
        };
        let soul_pos = soul_transform.translation.truncate();
        let soul_grid = WorldMap::world_to_grid(soul_pos);
        if !footprint.blocked_tiles.contains(&soul_grid) {
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
pub(crate) fn floor_construction_completion_system(
    time: Res<Time>,
    tile_site_index: Res<TileSiteIndex>,
    soul_grid: Res<SpatialGrid>,
    queries: FloorCompletionQueries,
    mut world_map: WorldMapWrite,
    mut commands: Commands,
) {
    let FloorCompletionQueries {
        mut q_sites,
        q_tiles,
        mut q_souls,
        mut nearby_souls,
        handles_3d,
        #[cfg(feature = "profiling")]
        mut metrics,
    } = queries;

    #[cfg(feature = "profiling")]
    let started_at = Instant::now();
    for (site_entity, mut site, footprint_opt) in q_sites.iter_mut() {
        #[cfg(feature = "profiling")]
        {
            metrics.floor_sites_considered = metrics.floor_sites_considered.saturating_add(1);
        }
        // A site whose progress counter is incomplete cannot transition. Do
        // not even touch its tile index in the common incomplete case.
        if site.phase != FloorConstructionPhase::Curing && site.tiles_poured < site.tiles_total {
            continue;
        }

        if site.phase != FloorConstructionPhase::Curing {
            let site_tiles = indexed_floor_tiles(site_entity, &tile_site_index, &q_tiles);
            #[cfg(feature = "profiling")]
            {
                metrics.floor_tiles_inspected = metrics
                    .floor_tiles_inspected
                    .saturating_add(site_tiles.len() as u64);
            }
            if site_tiles.len() != site.tiles_total as usize
                || !site_tiles
                    .iter()
                    .all(|(_, _, state)| *state == FloorTileState::Complete)
            {
                // Counters are only a fast rejection. A mismatch must never
                // advance the phase in release builds.
                continue;
            }

            site.phase = FloorConstructionPhase::Curing;
            site.curing_remaining_secs = FLOOR_CURING_DURATION_SECS.max(0.0);
            let footprint = CuringFootprint::from_tiles(&site_tiles);

            for (tile_entity, (gx, gy), _) in &site_tiles {
                commands.entity(*tile_entity).insert((
                    ObstaclePosition(*gx, *gy),
                    ObstacleSourceKind::ConstructionProtection,
                ));
            }
            world_map.reserve_building_footprint_tiles(footprint.blocked_tiles.iter().copied());

            let evacuated = evacuate_souls_from_blocked_tiles(
                &soul_grid,
                &mut nearby_souls,
                &mut q_souls,
                &footprint,
                &world_map,
                #[cfg(feature = "profiling")]
                &mut metrics,
            );
            commands.entity(site_entity).insert(footprint);

            info!(
                "Floor site {:?} entered curing ({:.1}s, evacuated {} souls)",
                site_entity, site.curing_remaining_secs, evacuated
            );
            continue;
        }

        let Some(mut footprint) = footprint_opt else {
            // CuringFootprint is runtime-only. Load rehydration has already
            // restored map occupancy from durable tiles, so only rebuild the
            // index-backed cache here; do not reserve the footprint twice.
            let site_tiles = indexed_floor_tiles(site_entity, &tile_site_index, &q_tiles);
            #[cfg(feature = "profiling")]
            {
                metrics.floor_tiles_inspected = metrics
                    .floor_tiles_inspected
                    .saturating_add(site_tiles.len() as u64);
            }
            if site_tiles.len() == site.tiles_total as usize {
                commands
                    .entity(site_entity)
                    .insert(CuringFootprint::from_tiles(&site_tiles));
            }
            continue;
        };

        if footprint.safety_audit.tick(time.delta()).just_finished() {
            let re_evacuated = evacuate_souls_from_blocked_tiles(
                &soul_grid,
                &mut nearby_souls,
                &mut q_souls,
                &footprint,
                &world_map,
                #[cfg(feature = "profiling")]
                &mut metrics,
            );
            if re_evacuated > 0 {
                debug!(
                    "FLOOR_CURING: Site {:?} re-evacuated {} souls still inside curing area",
                    site_entity, re_evacuated
                );
            }
        }

        site.curing_remaining_secs = (site.curing_remaining_secs - time.delta_secs()).max(0.0);
        if site.curing_remaining_secs > 0.0 {
            continue;
        }

        let completed_grids: Vec<(i32, i32)> =
            footprint.tiles.iter().map(|(_, grid)| *grid).collect();

        // For each tile: spawn Building entity with Floor type
        let mut tile_count = 0;
        for (tile_entity, (gx, gy)) in &footprint.tiles {
            let world_pos = WorldMap::grid_to_world(*gx, *gy);

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
            commands.entity(*tile_entity).despawn();
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
    #[cfg(feature = "profiling")]
    {
        metrics.floor_completion_elapsed_micros = metrics
            .floor_completion_elapsed_micros
            .saturating_add(started_at.elapsed().as_micros() as u64);
    }
}

#[cfg(test)]
mod tests {
    use super::{CuringFootprint, collect_curing_soul_candidates};
    use crate::world::map::WorldMap;
    use bevy::prelude::*;
    use hw_spatial::{SpatialGrid, SpatialGridOps};

    #[test]
    fn curing_candidates_are_local_and_stably_deduplicated() {
        let mut grid = SpatialGrid::default();
        let first = Entity::from_bits(2);
        let second = Entity::from_bits(1);
        let distant = Entity::from_bits(3);
        let blocked = WorldMap::grid_to_world(8, 9);
        grid.insert(first, blocked);
        grid.insert(second, blocked + Vec2::splat(2.0));
        grid.insert(distant, blocked + Vec2::splat(10_000.0));

        let footprint = CuringFootprint::from_tile_positions([(first, (8, 9))]);
        let mut candidates = vec![first];
        collect_curing_soul_candidates(&grid, &footprint, &mut candidates);

        assert_eq!(candidates, vec![second, first]);
    }
}
