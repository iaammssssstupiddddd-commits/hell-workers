//! Root-only provisional wall spawning.
//!
//! The indexed phase transition lives in `hw_logistics`; this system remains
//! here because it depends on root-owned 3D handles and visual wiring.

use super::components::*;
use crate::plugins::startup::Building3dHandles;
use crate::systems::jobs::{Building, BuildingType, ProvisionalWall};
use crate::world::map::{WorldMap, WorldMapWrite};
use bevy::prelude::*;
use hw_core::constants::Z_MAP;
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

    crate::systems::jobs::building_completion::spawn_building_3d_visual(
        commands,
        wall_entity,
        BuildingType::Wall,
        world_pos,
        is_provisional,
        handles_3d,
    );
    wall_entity
}

#[cfg(test)]
mod tests {
    use bevy::ecs::schedule::ApplyDeferred;
    use hw_core::area::TaskArea;
    use hw_core::visual_mirror::construction::WallTileVisualMirror;
    use hw_logistics::tile_index::TileSiteIndex;
    use hw_visual::Building3dVisual;
    use hw_visual::blueprint::BuildingBounceEffect;
    use hw_visual::wall_connection::{
        WallConnectionDirty, WallConnectionMask, WallTopologyIndex, WallTopologyState,
        wall_connections_system,
    };

    use super::*;

    #[test]
    fn two_tile_site_keeps_exact_topology_through_framing_and_completion() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<WorldMap>()
            .init_resource::<TileSiteIndex>()
            .insert_resource(crate::test_support::empty_building_3d_handles())
            .insert_resource(crate::test_support::empty_wall_visual_handles())
            .init_resource::<WallConnectionDirty>()
            .init_resource::<WallTopologyIndex>()
            .add_observer(hw_jobs::visual_sync::on_building_added_sync_visual)
            .add_systems(Update, hw_jobs::visual_sync::sync_building_visual_system)
            .add_systems(
                Update,
                (
                    wall_framed_tile_spawn_system,
                    hw_logistics::wall_construction_phase_transition_system,
                    crate::systems::jobs::wall_construction::wall_construction_completion_system,
                )
                    .chain(),
            )
            .add_systems(PostUpdate, (wall_connections_system, ApplyDeferred).chain());

        let grids = [(20, 20), (21, 20)];
        let mut site = WallConstructionSite::new(
            TaskArea::from_points(
                WorldMap::grid_to_world(grids[0].0, grids[0].1),
                WorldMap::grid_to_world(grids[1].0, grids[1].1),
            ),
            WorldMap::grid_to_world(grids[0].0, grids[0].1),
            grids.len() as u32,
        );
        site.tiles_framed = grids.len() as u32;
        let site_entity = app.world_mut().spawn(site).id();
        let tile_entities = grids
            .into_iter()
            .map(|grid| {
                let mut tile = WallTileBlueprint::new(site_entity, grid);
                tile.state = WallTileState::FramedProvisional;
                app.world_mut()
                    .spawn((
                        tile,
                        WallTileVisualMirror::default(),
                        Transform::from_translation(
                            WorldMap::grid_to_world(grid.0, grid.1).extend(0.0),
                        ),
                    ))
                    .id()
            })
            .collect::<Vec<_>>();
        app.world_mut()
            .resource_mut::<TileSiteIndex>()
            .wall_tiles_by_site
            .insert(site_entity, tile_entities.clone());
        for grid in grids {
            app.world_mut()
                .resource_mut::<WorldMap>()
                .set_building_occupancy(grid, site_entity);
        }

        app.update();

        assert_eq!(
            app.world()
                .get::<WallConstructionSite>(site_entity)
                .unwrap()
                .phase,
            WallConstructionPhase::Coating
        );
        let walls = tile_entities
            .iter()
            .map(|tile_entity| {
                let tile = app.world().get::<WallTileBlueprint>(*tile_entity).unwrap();
                assert_eq!(tile.state, WallTileState::WaitingMud);
                tile.spawned_wall.expect("framing must spawn a Wall")
            })
            .collect::<Vec<_>>();
        assert_eq!(
            app.world().get::<WallTopologyState>(walls[0]).unwrap().mask,
            WallConnectionMask::from_neighbors(false, false, false, true)
        );
        assert_eq!(
            app.world().get::<WallTopologyState>(walls[1]).unwrap().mask,
            WallConnectionMask::from_neighbors(false, false, true, false)
        );
        for wall in &walls {
            let building = app.world().get::<Building>(*wall).unwrap();
            assert!(building.is_provisional);
            assert!(app.world().get::<ProvisionalWall>(*wall).is_some());
            let visual_count = {
                let world = app.world_mut();
                let mut query = world.query::<&Building3dVisual>();
                query
                    .iter(world)
                    .filter(|visual| visual.owner == *wall)
                    .count()
            };
            assert_eq!(visual_count, 1);
        }

        {
            let mut site = app
                .world_mut()
                .get_mut::<WallConstructionSite>(site_entity)
                .unwrap();
            site.tiles_coated = site.tiles_total;
        }
        for tile_entity in &tile_entities {
            app.world_mut()
                .get_mut::<WallTileBlueprint>(*tile_entity)
                .unwrap()
                .state = WallTileState::Complete;
        }

        app.update();

        assert!(app.world().get_entity(site_entity).is_err());
        assert!(
            tile_entities
                .iter()
                .all(|entity| app.world().get_entity(*entity).is_err())
        );
        for (index, wall) in walls.into_iter().enumerate() {
            let building = app.world().get::<Building>(wall).unwrap();
            assert!(!building.is_provisional);
            assert!(app.world().get::<ProvisionalWall>(wall).is_none());
            assert!(app.world().get::<BuildingBounceEffect>(wall).is_some());
            let expected_mask = if index == 0 {
                WallConnectionMask::from_neighbors(false, false, false, true)
            } else {
                WallConnectionMask::from_neighbors(false, false, true, false)
            };
            assert_eq!(
                app.world().get::<WallTopologyState>(wall).unwrap().mask,
                expected_mask
            );
        }
    }
}
