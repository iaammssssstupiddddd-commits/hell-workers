//! Index-backed construction phase transitions.
//!
//! `hw_jobs` owns the durable construction model and pure transition rules.
//! This module owns the `TileSiteIndex` adapter so production transitions only
//! inspect tiles belonging to a changed site.

use bevy::prelude::*;
use hw_jobs::construction::{
    FloorConstructionSite, FloorTileBlueprint, WallConstructionSite, WallTileBlueprint,
};
use hw_jobs::remove_tile_task_components;
use std::collections::HashSet;
#[cfg(feature = "profiling")]
use std::time::Instant;

use crate::tile_index::TileSiteIndex;

/// Profiling counters shared by indexed transitions and root-owned completion
/// adapters. They describe both work performed and elapsed transition time.
#[cfg(feature = "profiling")]
#[derive(Resource, Debug, Default)]
pub struct ConstructionPerfMetrics {
    pub floor_sites_considered: u64,
    pub wall_sites_considered: u64,
    pub floor_tiles_inspected: u64,
    pub wall_tiles_inspected: u64,
    pub evacuation_candidates_scanned: u64,
    pub floor_phase_elapsed_micros: u64,
    pub floor_completion_elapsed_micros: u64,
    pub wall_phase_elapsed_micros: u64,
    pub wall_completion_elapsed_micros: u64,
}

type FloorTransitionSiteQuery<'w, 's> = Query<
    'w,
    's,
    (Entity, &'static mut FloorConstructionSite),
    Or<(Added<FloorConstructionSite>, Changed<FloorConstructionSite>)>,
>;
type WallTransitionSiteQuery<'w, 's> = Query<
    'w,
    's,
    (Entity, &'static mut WallConstructionSite),
    Or<(Added<WallConstructionSite>, Changed<WallConstructionSite>)>,
>;

/// Advances a fully reinforced floor site to the pouring phase.
///
/// Site counters are a fast rejection only. Index count, entity uniqueness,
/// tile existence, ownership, and state are all checked before any mutation.
pub fn floor_construction_phase_transition_system(
    tile_site_index: Res<TileSiteIndex>,
    mut seen_tiles: Local<HashSet<Entity>>,
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
        if !site.can_transition_to_pouring() {
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

        seen_tiles.clear();
        if tile_entities.len() != site.tiles_total as usize
            || tile_entities.iter().any(|tile_entity| {
                !seen_tiles.insert(*tile_entity)
                    || !q_tiles
                        .get(*tile_entity)
                        .is_ok_and(|tile| tile.is_reinforced_for(site_entity))
            })
        {
            continue;
        }

        for tile_entity in tile_entities {
            let mut tile = q_tiles
                .get_mut(*tile_entity)
                .expect("validated indexed floor tile must remain queryable during one system run");
            let transitioned = tile.transition_to_waiting_mud(site_entity);
            debug_assert!(transitioned, "validated floor tile transition must succeed");
        }
        let transitioned = site.transition_to_pouring();
        debug_assert!(transitioned, "validated floor site transition must succeed");
        remove_tile_task_components(&mut commands, tile_entities);
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

/// Advances a fully framed wall site to the coating phase.
///
/// Site counters are a fast rejection only. Index count, entity uniqueness,
/// tile existence, ownership, state, and provisional wall presence are all
/// checked before any mutation.
pub fn wall_construction_phase_transition_system(
    tile_site_index: Res<TileSiteIndex>,
    mut seen_tiles: Local<HashSet<Entity>>,
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
        if !site.can_transition_to_coating() {
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

        seen_tiles.clear();
        if tile_entities.len() != site.tiles_total as usize
            || tile_entities.iter().any(|tile_entity| {
                !seen_tiles.insert(*tile_entity)
                    || !q_tiles
                        .get(*tile_entity)
                        .is_ok_and(|tile| tile.is_framed_for(site_entity))
            })
        {
            continue;
        }

        for tile_entity in tile_entities {
            let mut tile = q_tiles
                .get_mut(*tile_entity)
                .expect("validated indexed wall tile must remain queryable during one system run");
            let transitioned = tile.transition_to_waiting_mud(site_entity);
            debug_assert!(transitioned, "validated wall tile transition must succeed");
        }
        let transitioned = site.transition_to_coating();
        debug_assert!(transitioned, "validated wall site transition must succeed");
        remove_tile_task_components(&mut commands, tile_entities);
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

#[cfg(test)]
mod tests {
    use super::*;
    use hw_core::area::TaskArea;
    use hw_jobs::construction::{
        FloorConstructionPhase, FloorTileState, WallConstructionPhase, WallTileState,
    };

    fn test_app() -> App {
        let mut app = App::new();
        app.init_resource::<TileSiteIndex>().add_systems(
            Update,
            (
                floor_construction_phase_transition_system,
                wall_construction_phase_transition_system,
            ),
        );
        #[cfg(feature = "profiling")]
        app.init_resource::<ConstructionPerfMetrics>();
        app
    }

    fn floor_site(tiles_total: u32) -> FloorConstructionSite {
        let mut site = FloorConstructionSite::new(
            TaskArea::from_points(Vec2::ZERO, Vec2::ONE),
            Vec2::ZERO,
            tiles_total,
        );
        site.tiles_reinforced = tiles_total;
        site
    }

    fn wall_site(tiles_total: u32) -> WallConstructionSite {
        let mut site = WallConstructionSite::new(
            TaskArea::from_points(Vec2::ZERO, Vec2::ONE),
            Vec2::ZERO,
            tiles_total,
        );
        site.tiles_framed = tiles_total;
        site
    }

    #[test]
    fn indexed_floor_transition_inspects_only_site_tiles() {
        let mut app = test_app();
        let site = app.world_mut().spawn(floor_site(1)).id();
        let mut tile = FloorTileBlueprint::new(site, (0, 0));
        tile.state = FloorTileState::ReinforcedComplete;
        let tile = app.world_mut().spawn(tile).id();
        for x in 1..64 {
            app.world_mut()
                .spawn(FloorTileBlueprint::new(Entity::PLACEHOLDER, (x, 0)));
        }
        app.world_mut()
            .resource_mut::<TileSiteIndex>()
            .floor_tiles_by_site
            .insert(site, vec![tile]);

        app.update();

        assert_eq!(
            app.world()
                .get::<FloorConstructionSite>(site)
                .unwrap()
                .phase,
            FloorConstructionPhase::Pouring
        );
        assert_eq!(
            app.world().get::<FloorTileBlueprint>(tile).unwrap().state,
            FloorTileState::WaitingMud
        );
        #[cfg(feature = "profiling")]
        assert_eq!(
            app.world()
                .resource::<ConstructionPerfMetrics>()
                .floor_tiles_inspected,
            1
        );
    }

    #[test]
    fn indexed_wall_transition_inspects_only_site_tiles() {
        let mut app = test_app();
        let site = app.world_mut().spawn(wall_site(1)).id();
        let mut tile = WallTileBlueprint::new(site, (0, 0));
        tile.state = WallTileState::FramedProvisional;
        tile.spawned_wall = Some(Entity::PLACEHOLDER);
        let tile = app.world_mut().spawn(tile).id();
        for x in 1..64 {
            app.world_mut()
                .spawn(WallTileBlueprint::new(Entity::PLACEHOLDER, (x, 0)));
        }
        app.world_mut()
            .resource_mut::<TileSiteIndex>()
            .wall_tiles_by_site
            .insert(site, vec![tile]);

        app.update();

        assert_eq!(
            app.world().get::<WallConstructionSite>(site).unwrap().phase,
            WallConstructionPhase::Coating
        );
        assert_eq!(
            app.world().get::<WallTileBlueprint>(tile).unwrap().state,
            WallTileState::WaitingMud
        );
        #[cfg(feature = "profiling")]
        assert_eq!(
            app.world()
                .resource::<ConstructionPerfMetrics>()
                .wall_tiles_inspected,
            1
        );
    }

    #[test]
    fn zero_tile_site_does_not_transition() {
        let mut app = test_app();
        let floor = app.world_mut().spawn(floor_site(0)).id();
        let wall = app.world_mut().spawn(wall_site(0)).id();

        app.update();

        assert_eq!(
            app.world()
                .get::<FloorConstructionSite>(floor)
                .unwrap()
                .phase,
            FloorConstructionPhase::Reinforcing
        );
        assert_eq!(
            app.world().get::<WallConstructionSite>(wall).unwrap().phase,
            WallConstructionPhase::Framing
        );
    }

    #[test]
    fn index_mismatch_does_not_partially_transition() {
        let mut app = test_app();

        let duplicate_site = app.world_mut().spawn(floor_site(2)).id();
        let mut duplicate_tile = FloorTileBlueprint::new(duplicate_site, (0, 0));
        duplicate_tile.state = FloorTileState::ReinforcedComplete;
        let duplicate_tile = app.world_mut().spawn(duplicate_tile).id();

        let foreign_site = app.world_mut().spawn(floor_site(1)).id();
        let mut foreign_tile = FloorTileBlueprint::new(Entity::PLACEHOLDER, (1, 0));
        foreign_tile.state = FloorTileState::ReinforcedComplete;
        let foreign_tile = app.world_mut().spawn(foreign_tile).id();

        let missing_site = app.world_mut().spawn(floor_site(1)).id();
        let missing_tile = app.world_mut().spawn_empty().id();
        assert!(app.world_mut().despawn(missing_tile));

        let wall_site = app.world_mut().spawn(wall_site(1)).id();
        let mut wall_tile = WallTileBlueprint::new(wall_site, (2, 0));
        wall_tile.state = WallTileState::FramedProvisional;
        let wall_tile = app.world_mut().spawn(wall_tile).id();

        {
            let mut index = app.world_mut().resource_mut::<TileSiteIndex>();
            index
                .floor_tiles_by_site
                .insert(duplicate_site, vec![duplicate_tile, duplicate_tile]);
            index
                .floor_tiles_by_site
                .insert(foreign_site, vec![foreign_tile]);
            index
                .floor_tiles_by_site
                .insert(missing_site, vec![missing_tile]);
            index.wall_tiles_by_site.insert(wall_site, vec![wall_tile]);
        }

        app.update();

        for site in [duplicate_site, foreign_site, missing_site] {
            assert_eq!(
                app.world()
                    .get::<FloorConstructionSite>(site)
                    .unwrap()
                    .phase,
                FloorConstructionPhase::Reinforcing
            );
        }
        assert_eq!(
            app.world()
                .get::<FloorTileBlueprint>(duplicate_tile)
                .unwrap()
                .state,
            FloorTileState::ReinforcedComplete
        );
        assert_eq!(
            app.world()
                .get::<WallConstructionSite>(wall_site)
                .unwrap()
                .phase,
            WallConstructionPhase::Framing
        );
        assert_eq!(
            app.world()
                .get::<WallTileBlueprint>(wall_tile)
                .unwrap()
                .state,
            WallTileState::FramedProvisional
        );
    }
}
