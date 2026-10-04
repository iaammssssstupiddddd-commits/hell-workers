//! Wall construction completion system

use super::components::*;
#[cfg(feature = "profiling")]
use crate::systems::jobs::ConstructionPerfMetrics;
use crate::systems::jobs::{Building, BuildingType, ProvisionalWall};
use crate::world::map::WorldMap;
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use hw_logistics::tile_index::TileSiteIndex;
use hw_visual::blueprint::BuildingBounceEffect;
use std::collections::HashSet;
#[cfg(feature = "profiling")]
use std::time::Instant;

type SiteTileData = (Entity, (i32, i32), WallTileState, Option<Entity>);
type WallCompletionSiteQuery<'w, 's> =
    Query<'w, 's, (Entity, &'static WallConstructionSite), Changed<WallConstructionSite>>;
type WallCompletionTileQuery<'w, 's> = Query<'w, 's, &'static WallTileBlueprint>;

#[derive(SystemParam)]
pub struct WallCompletionParams<'w, 's> {
    tile_site_index: Res<'w, TileSiteIndex>,
    q_sites: WallCompletionSiteQuery<'w, 's>,
    q_tiles: WallCompletionTileQuery<'w, 's>,
    #[cfg(feature = "profiling")]
    metrics: Option<ResMut<'w, ConstructionPerfMetrics>>,
}

/// Handles wall construction completion (no curing phase)
pub fn wall_construction_completion_system(mut commands: Commands, params: WallCompletionParams) {
    let WallCompletionParams {
        tile_site_index,
        q_sites,
        q_tiles,
        #[cfg(feature = "profiling")]
        mut metrics,
    } = params;

    #[cfg(feature = "profiling")]
    let started_at = Instant::now();
    for (site_entity, site) in q_sites.iter() {
        #[cfg(feature = "profiling")]
        if let Some(metrics) = metrics.as_mut() {
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
                    .filter(|tile| tile.parent_site == site_entity)
                    .map(|tile| (tile_entity, tile.grid_pos, tile.state, tile.spawned_wall))
            })
            .collect();

        #[cfg(feature = "profiling")]
        if let Some(metrics) = metrics.as_mut() {
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

        commands.queue(move |world: &mut World| {
            commit_completed_wall(world, site_entity);
        });
    }
    #[cfg(feature = "profiling")]
    if let Some(metrics) = metrics.as_mut() {
        metrics.wall_completion_elapsed_micros = metrics
            .wall_completion_elapsed_micros
            .saturating_add(started_at.elapsed().as_micros() as u64);
    }
}

fn commit_completed_wall(world: &mut World, site_entity: Entity) -> bool {
    let Some(site) = world.get::<WallConstructionSite>(site_entity) else {
        return false;
    };
    if site.phase != WallConstructionPhase::Coating
        || site.tiles_total == 0
        || site.tiles_coated < site.tiles_total
        || world
            .get::<WallConstructionCancelRequested>(site_entity)
            .is_some()
    {
        return false;
    }
    let expected_count = site.tiles_total as usize;
    let tiles: Vec<_> = world
        .query::<(Entity, &WallTileBlueprint)>()
        .iter(world)
        .filter(|(_, tile)| tile.parent_site == site_entity)
        .map(|(entity, tile)| (entity, tile.grid_pos, tile.state, tile.spawned_wall))
        .collect();
    if tiles.len() != expected_count
        || tiles
            .iter()
            .map(|(_, grid, _, _)| *grid)
            .collect::<HashSet<_>>()
            .len()
            != tiles.len()
    {
        return false;
    }
    let map = world.resource::<WorldMap>();
    for &(_, grid, state, wall) in &tiles {
        if state != WallTileState::Complete
            || map.building_entity(grid) != Some(wall.unwrap_or(site_entity))
            || wall.is_some_and(|entity| {
                !world
                    .get::<Building>(entity)
                    .is_some_and(|building| building.kind == BuildingType::Wall)
            })
        {
            return false;
        }
    }
    let released_grids: Vec<_> = tiles
        .iter()
        .filter(|(_, _, _, wall)| wall.is_none())
        .map(|(_, grid, _, _)| *grid)
        .collect();
    let removed_tiles: HashSet<_> = tiles.iter().map(|(entity, _, _, _)| *entity).collect();
    let remaining_blockers: HashSet<_> = world
        .query::<(Entity, &hw_jobs::ObstaclePosition)>()
        .iter(world)
        .filter(|(entity, _)| !removed_tiles.contains(entity))
        .map(|(_, position)| (position.0, position.1))
        .collect();
    let requests: Vec<_> = world
        .query::<(Entity, &TargetWallConstructionSite)>()
        .iter(world)
        .filter(|(_, target)| target.0 == site_entity)
        .map(|(entity, _)| entity)
        .collect();
    {
        let mut map = world.resource_mut::<WorldMap>();
        for grid in released_grids {
            map.clear_building_if_owned(grid, site_entity);
            if !remaining_blockers.contains(&grid) {
                map.remove_grid_obstacle(grid);
            }
        }
    }
    for (tile, _, _, wall) in tiles {
        if let Some(wall) = wall {
            world.get_mut::<Building>(wall).unwrap().is_provisional = false;
            world
                .entity_mut(wall)
                .remove::<ProvisionalWall>()
                .insert(BuildingBounceEffect::completion());
        }
        world.entity_mut(tile).despawn();
    }
    for request in requests {
        world.entity_mut(request).despawn();
    }
    world.entity_mut(site_entity).despawn();
    info!(
        "Wall site {:?} completed ({} tiles)",
        site_entity, expected_count
    );
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use hw_core::area::TaskArea;

    #[test]
    fn wall_completion_validates_every_tile_before_promoting_or_removing_any_test() {
        for missing_wall in [false, true] {
            let mut world = World::new();
            world.init_resource::<WorldMap>();
            let site = world
                .spawn(WallConstructionSite {
                    phase: WallConstructionPhase::Coating,
                    area_bounds: TaskArea::from_points(Vec2::ZERO, Vec2::ONE),
                    material_center: Vec2::ZERO,
                    tiles_total: 2,
                    tiles_framed: 2,
                    tiles_coated: 2,
                })
                .id();
            let mut walls = Vec::new();
            let mut tiles = Vec::new();
            for grid in [(10, 10), (11, 10)] {
                let wall = world
                    .spawn((
                        Building {
                            kind: BuildingType::Wall,
                            is_provisional: true,
                        },
                        ProvisionalWall::default(),
                    ))
                    .id();
                world
                    .resource_mut::<WorldMap>()
                    .set_building_occupancy(grid, wall);
                let tile = world
                    .spawn(WallTileBlueprint {
                        parent_site: site,
                        grid_pos: grid,
                        state: WallTileState::Complete,
                        wood_delivered: 1,
                        mud_delivered: 1,
                        spawned_wall: Some(wall),
                    })
                    .id();
                walls.push(wall);
                tiles.push(tile);
            }
            let request = world.spawn(TargetWallConstructionSite(site)).id();
            if missing_wall {
                world.entity_mut(walls[1]).despawn();
            } else {
                let other = world.spawn_empty().id();
                world
                    .resource_mut::<WorldMap>()
                    .set_building((11, 10), other);
            }
            let map = world.resource::<WorldMap>();
            let before = (
                map.buildings.clone(),
                map.obstacles.clone(),
                map.obstacle_version,
            );
            assert!(!commit_completed_wall(&mut world, site));
            let map = world.resource::<WorldMap>();
            assert_eq!(
                before,
                (
                    map.buildings.clone(),
                    map.obstacles.clone(),
                    map.obstacle_version
                )
            );
            assert!(world.get::<Building>(walls[0]).unwrap().is_provisional);
            assert!(world.get::<ProvisionalWall>(walls[0]).is_some());
            assert!(world.get::<BuildingBounceEffect>(walls[0]).is_none());
            for entity in [site, request, tiles[0], tiles[1]] {
                assert!(world.get_entity(entity).is_ok());
            }
        }
    }

    #[test]
    fn coating_completion_promotes_provisional_wall_and_starts_a_fresh_bounce() {
        let mut app = App::new();
        app.init_resource::<WorldMap>()
            .init_resource::<TileSiteIndex>()
            .add_systems(Update, wall_construction_completion_system);
        #[cfg(feature = "profiling")]
        app.init_resource::<ConstructionPerfMetrics>();

        let site = app
            .world_mut()
            .spawn(WallConstructionSite {
                phase: WallConstructionPhase::Coating,
                area_bounds: TaskArea::from_points(Vec2::ZERO, Vec2::ONE),
                material_center: Vec2::ZERO,
                tiles_total: 1,
                tiles_framed: 1,
                tiles_coated: 1,
            })
            .id();
        let wall = app
            .world_mut()
            .spawn((
                Building {
                    kind: BuildingType::Wall,
                    is_provisional: true,
                },
                ProvisionalWall::default(),
            ))
            .id();
        let tile = app
            .world_mut()
            .spawn(WallTileBlueprint {
                parent_site: site,
                grid_pos: (12, 13),
                state: WallTileState::Complete,
                wood_delivered: 1,
                mud_delivered: 1,
                spawned_wall: Some(wall),
            })
            .id();
        app.world_mut()
            .resource_mut::<TileSiteIndex>()
            .wall_tiles_by_site
            .insert(site, vec![tile]);

        app.world_mut()
            .resource_mut::<WorldMap>()
            .set_building_occupancy((12, 13), wall);

        app.update();

        let building = app
            .world()
            .get::<Building>(wall)
            .expect("completed Wall must remain");
        assert!(!building.is_provisional);
        assert!(app.world().get::<ProvisionalWall>(wall).is_none());
        let bounce = app
            .world()
            .get::<BuildingBounceEffect>(wall)
            .expect("completed Wall must begin its completion bounce");
        assert_eq!(bounce.bounce_animation.timer, 0.0);
        assert!(app.world().get_entity(site).is_err());
        assert!(app.world().get_entity(tile).is_err());
    }
}
