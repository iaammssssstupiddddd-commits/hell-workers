use crate::plugins::startup::Building3dHandles;
use crate::systems::jobs::BuildingType;
use crate::systems::jobs::TaskSlots;
use crate::systems::jobs::wall_construction::{
    WallConstructionSite, WallTileBlueprint, spawn_wall_shell,
};
use crate::world::map::WorldMap;
use bevy::prelude::*;
use hw_core::constants::*;
use hw_core::visual_mirror::construction::{WallSiteVisualState, WallTileVisualMirror};
use hw_ui::selection::AreaPlacementPlan;
use hw_visual::blueprint::BuildingBounceEffect;

pub(super) fn apply_wall_placement(
    commands: &mut Commands,
    world_map: &mut WorldMap,
    area: &crate::systems::command::TaskArea,
    plan: &AreaPlacementPlan,
    instant_build_handles: Option<&Building3dHandles>,
) {
    if let Some(handles_3d) = instant_build_handles {
        for &grid in &plan.valid_tiles {
            let wall_entity = spawn_wall_shell(commands, handles_3d, grid, false);
            commands
                .entity(wall_entity)
                .insert(BuildingBounceEffect::completion());
            world_map.reserve_building_footprint(
                BuildingType::Wall,
                wall_entity,
                std::iter::once(grid),
            );
        }
        return;
    }

    let Some(&center_grid) = plan.valid_tiles.get(plan.valid_tiles.len() / 2) else {
        return;
    };
    let tiles_total = plan.valid_tiles.len() as u32;
    let material_center = WorldMap::grid_to_world(center_grid.0, center_grid.1);

    let site_entity = commands
        .spawn((
            WallConstructionSite::new(area.clone(), material_center, tiles_total),
            WallSiteVisualState::default(),
            Transform::from_translation(material_center.extend(Z_MAP + 0.01)),
            Visibility::default(),
            Name::new("WallConstructionSite"),
        ))
        .id();

    for &(gx, gy) in &plan.valid_tiles {
        let world_pos = WorldMap::grid_to_world(gx, gy);

        commands.spawn((
            WallTileBlueprint::new(site_entity, (gx, gy)),
            WallTileVisualMirror::default(),
            TaskSlots::new(1),
            Sprite {
                color: Color::srgba(0.8, 0.55, 0.3, 0.25),
                custom_size: Some(Vec2::splat(TILE_SIZE)),
                ..default()
            },
            Transform::from_translation(world_pos.extend(Z_MAP + 0.02)),
            Visibility::default(),
            Name::new(format!("WallTile({},{})", gx, gy)),
        ));

        world_map.set_building_occupancy((gx, gy), site_entity);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::systems::jobs::Building;
    use crate::test_support::empty_building_3d_handles;
    use bevy::ecs::{schedule::ApplyDeferred, world::CommandQueue};
    use hw_core::visual_mirror::building::{BuildingTypeVisual, BuildingVisualState};
    use hw_visual::Building3dVisual;
    use hw_visual::wall_connection::{
        WallConnectionDirty, WallConnectionMask, WallTopologyIndex, WallTopologyState,
        wall_connections_system,
    };

    fn apply_test_placement(instant: bool) -> (World, WorldMap, Vec<(i32, i32)>) {
        let grids = vec![(7, 8), (8, 8)];
        let area = crate::systems::command::TaskArea::from_points(
            WorldMap::grid_to_world(7, 8),
            WorldMap::grid_to_world(8, 8),
        );
        let plan = AreaPlacementPlan {
            valid_tiles: grids.clone(),
            total_tile_count: grids.len(),
            first_reject: None,
        };
        let handles = empty_building_3d_handles();
        let mut world = World::new();
        let mut world_map = WorldMap::default();
        let mut queue = CommandQueue::default();
        {
            let mut commands = Commands::new(&mut queue, &world);
            apply_wall_placement(
                &mut commands,
                &mut world_map,
                &area,
                &plan,
                instant.then_some(&handles),
            );
        }
        queue.apply(&mut world);
        (world, world_map, grids)
    }

    #[test]
    fn instant_placement_commits_completed_walls_without_construction_sites() {
        let (mut world, world_map, grids) = apply_test_placement(true);

        assert_eq!(
            world.query::<&WallConstructionSite>().iter(&world).count(),
            0
        );
        assert_eq!(world.query::<&WallTileBlueprint>().iter(&world).count(), 0);
        let walls: Vec<_> = world
            .query::<(Entity, &Building, &BuildingBounceEffect)>()
            .iter(&world)
            .map(|(entity, building, _)| (entity, building.kind, building.is_provisional))
            .collect();
        assert_eq!(walls.len(), grids.len());
        assert!(
            walls
                .iter()
                .all(|(_, kind, provisional)| *kind == BuildingType::Wall && !provisional)
        );
        for grid in grids {
            let owner = world_map
                .building_entity(grid)
                .expect("instant wall must own its tile");
            assert!(walls.iter().any(|(entity, _, _)| *entity == owner));
        }
    }

    #[test]
    fn normal_placement_still_creates_one_site_and_tile_blueprints() {
        let (mut world, world_map, grids) = apply_test_placement(false);

        let sites: Vec<_> = world
            .query_filtered::<Entity, With<WallConstructionSite>>()
            .iter(&world)
            .collect();
        assert_eq!(sites.len(), 1);
        assert_eq!(
            world.query::<&WallTileBlueprint>().iter(&world).count(),
            grids.len()
        );
        assert_eq!(world.query::<&Building>().iter(&world).count(), 0);
        for grid in grids {
            assert_eq!(world_map.building_entity(grid), Some(sites[0]));
        }
    }

    #[test]
    fn normal_and_instant_placement_enter_the_same_topology_route() {
        for instant in [false, true] {
            let mut app = App::new();
            app.insert_resource(crate::test_support::empty_wall_visual_handles())
                .init_resource::<WallConnectionDirty>()
                .init_resource::<WallTopologyIndex>()
                .add_observer(hw_jobs::visual_sync::on_building_added_sync_visual)
                .add_systems(Update, hw_jobs::visual_sync::sync_building_visual_system)
                .add_systems(PostUpdate, (wall_connections_system, ApplyDeferred).chain());

            let target_grid = (6, 8);
            let target = app
                .world_mut()
                .spawn((
                    Building {
                        kind: BuildingType::Wall,
                        is_provisional: false,
                    },
                    Transform::from_translation(
                        WorldMap::grid_to_world(target_grid.0, target_grid.1).extend(0.0),
                    ),
                ))
                .id();
            let grids = vec![(7, 8), (8, 8)];
            let area = crate::systems::command::TaskArea::from_points(
                WorldMap::grid_to_world(7, 8),
                WorldMap::grid_to_world(8, 8),
            );
            let plan = AreaPlacementPlan {
                valid_tiles: grids.clone(),
                total_tile_count: grids.len(),
                first_reject: None,
            };
            let handles = empty_building_3d_handles();
            let mut world_map = WorldMap::default();
            let mut queue = CommandQueue::default();
            {
                let mut commands = Commands::new(&mut queue, app.world());
                apply_wall_placement(
                    &mut commands,
                    &mut world_map,
                    &area,
                    &plan,
                    instant.then_some(&handles),
                );
            }
            queue.apply(app.world_mut());
            app.insert_resource(world_map);

            app.update();

            assert_eq!(
                app.world().get::<WallTopologyState>(target).unwrap().mask,
                WallConnectionMask::from_neighbors(false, false, false, true),
                "first placed tile did not connect to its west neighbor (instant={instant})"
            );
            let wall_mirror_count = {
                let world = app.world_mut();
                let mut query = world.query::<&BuildingVisualState>();
                query
                    .iter(world)
                    .filter(|state| state.kind == BuildingTypeVisual::Wall)
                    .count()
            };
            assert_eq!(wall_mirror_count, if instant { 3 } else { 1 });
            let wall_visual_count = {
                let world = app.world_mut();
                let mut query = world.query::<&Building3dVisual>();
                query.iter(world).count()
            };
            assert_eq!(wall_visual_count, if instant { grids.len() } else { 0 });
        }
    }
}
