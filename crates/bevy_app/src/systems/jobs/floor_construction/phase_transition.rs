//! Index-backed floor construction phase transition.
//!
//! `hw_jobs` owns the durable floor model, while the root app owns
//! `TileSiteIndex`. Keeping this adapter here lets a phase transition examine
//! only the tiles belonging to a changed site instead of scanning every floor
//! tile for every site.

use super::components::*;
#[cfg(feature = "profiling")]
use crate::systems::jobs::ConstructionPerfMetrics;
use bevy::prelude::*;
use hw_jobs::remove_tile_task_components;
use hw_logistics::tile_index::TileSiteIndex;
#[cfg(feature = "profiling")]
use std::time::Instant;

type FloorTransitionSiteQuery<'w, 's> = Query<
    'w,
    's,
    (Entity, &'static mut FloorConstructionSite),
    Or<(Added<FloorConstructionSite>, Changed<FloorConstructionSite>)>,
>;

/// Advances a fully reinforced floor site to the pouring phase.
///
/// The site counter is a fast rejection only. The index count and every tile
/// state are checked before the transition so stale or corrupt counters never
/// advance a site in release builds.
pub(crate) fn floor_construction_phase_transition_system(
    tile_site_index: Res<TileSiteIndex>,
    mut q_sites: FloorTransitionSiteQuery,
    mut q_tiles: Query<&mut FloorTileBlueprint>,
    mut commands: Commands,
    #[cfg(feature = "profiling")] mut metrics: ResMut<ConstructionPerfMetrics>,
) {
    #[cfg(feature = "profiling")]
    let started_at = Instant::now();
    for (site_entity, mut site) in q_sites.iter_mut() {
        #[cfg(feature = "profiling")]
        {
            metrics.floor_sites_considered = metrics.floor_sites_considered.saturating_add(1);
        }
        if site.phase != FloorConstructionPhase::Reinforcing
            || site.tiles_total == 0
            || site.tiles_reinforced < site.tiles_total
        {
            continue;
        }

        let Some(tile_entities) = tile_site_index.floor_tiles_by_site.get(&site_entity) else {
            continue;
        };
        #[cfg(feature = "profiling")]
        {
            metrics.floor_tiles_inspected = metrics
                .floor_tiles_inspected
                .saturating_add(tile_entities.len() as u64);
        }
        if tile_entities.len() != site.tiles_total as usize
            || tile_entities.iter().any(|tile_entity| {
                !matches!(
                    q_tiles.get(*tile_entity).map(|tile| tile.state),
                    Ok(FloorTileState::ReinforcedComplete)
                )
            })
        {
            continue;
        }

        let mut transitioned_tiles = Vec::with_capacity(tile_entities.len());
        for tile_entity in tile_entities {
            let Ok(mut tile) = q_tiles.get_mut(*tile_entity) else {
                // The immutable validation above guarantees this cannot happen
                // without an interleaved structural change. Do not partially
                // advance the site in that defensive case.
                transitioned_tiles.clear();
                break;
            };
            tile.state = FloorTileState::WaitingMud;
            transitioned_tiles.push(*tile_entity);
        }
        if transitioned_tiles.len() != tile_entities.len() {
            continue;
        }

        site.phase = FloorConstructionPhase::Pouring;
        remove_tile_task_components(&mut commands, &transitioned_tiles);
        info!(
            "Floor site {:?} -> Pouring phase (all {} tiles reinforced)",
            site_entity, site.tiles_total
        );
    }
    #[cfg(feature = "profiling")]
    {
        metrics.floor_phase_elapsed_micros = metrics
            .floor_phase_elapsed_micros
            .saturating_add(started_at.elapsed().as_micros() as u64);
    }
}
