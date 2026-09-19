use super::*;
use hw_core::relationships::WorkingOn;
use hw_jobs::{ActiveTaskIdentity, MovePlantTask, ObstaclePosition, ObstacleSourceKind, WorkType};

struct Fixture {
    world: World,
    building: Entity,
    task: Entity,
    worker: Entity,
    old: Vec<(i32, i32)>,
    new: Vec<(i32, i32)>,
}

fn fixture(kind: BuildingType) -> Fixture {
    let mut world = World::new();
    world.init_resource::<WorldMap>();
    let old = occupied_grids_for_kind(kind, (10, 10));
    let new = occupied_grids_for_kind(kind, (20, 20));
    let transform = Transform::from_translation(spawn_pos_for_kind(kind, (10, 10)).extend(2.0));
    let proposed = Transform::from_translation(spawn_pos_for_kind(kind, (20, 20)).extend(2.0));
    let companion = (kind == BuildingType::Tank).then_some((22, 20));
    let building = world
        .spawn((
            Building {
                kind,
                is_provisional: false,
            },
            transform,
        ))
        .id();
    let task = world
        .spawn(MovePlantTask {
            building,
            destination_grid: (20, 20),
            destination_pos: proposed.translation.truncate(),
            companion_anchor: companion,
        })
        .id();
    let identity = ActiveTaskIdentity::new(task, task, WorkType::Move);
    let worker = world
        .spawn((
            AssignedTask::MovePlant(MovePlantData {
                task_entity: task,
                building,
                destination_grid: (20, 20),
                destination_pos: proposed.translation.truncate(),
                companion_anchor: companion,
                phase: MovePlantPhase::Moving,
            }),
            identity,
            WorkingOn(task),
        ))
        .id();
    world.entity_mut(building).insert((
        MovePlanned { task_entity: task },
        PendingBuildingMove {
            worker,
            task_entity: task,
            expected_identity: identity,
            expected_transform: transform,
            proposed_transform: proposed,
            expected_kind: kind,
            old_occupied: old.clone(),
            new_occupied: new.clone(),
            companion_anchor: companion,
            rejected: false,
        },
    ));
    world
        .resource_mut::<WorldMap>()
        .set_building_occupancies(building, old.iter().copied());
    for &(x, y) in &old {
        world.spawn((
            ObstaclePosition(x, y),
            ObstacleSourceKind::BuildingFootprint,
            ChildOf(building),
        ));
    }
    let mut reserved = new.clone();
    if companion.is_some() {
        for x in 12..14 {
            let storage = world
                .spawn((
                    hw_logistics::BucketStorage,
                    hw_logistics::BelongsTo(building),
                    Transform::from_translation(WorldMap::grid_to_world(x, 10).extend(1.0)),
                ))
                .id();
            world
                .resource_mut::<WorldMap>()
                .set_stockpile((x, 10), storage);
            reserved.push((x + 10, 20));
        }
    }
    world
        .resource_mut::<WorldMap>()
        .add_grid_obstacles(reserved.iter().copied());
    for &(x, y) in &reserved {
        world.spawn((
            ObstaclePosition(x, y),
            ObstacleSourceKind::PlacementReservation,
            ChildOf(task),
        ));
    }
    world
        .entity_mut(task)
        .insert(MovePlantReservation { occupied: reserved });
    Fixture {
        world,
        building,
        task,
        worker,
        old,
        new,
    }
}

#[test]
fn move_plant_commits_building_markers_and_tank_companions_together_test() {
    for kind in [BuildingType::MudMixer, BuildingType::Tank] {
        let mut f = fixture(kind);
        let expected = f
            .world
            .get::<PendingBuildingMove>(f.building)
            .unwrap()
            .proposed_transform;
        apply_pending_building_move_system(&mut f.world);
        assert_eq!(f.world.get::<Transform>(f.building), Some(&expected));
        assert!(f.world.get::<PendingBuildingMove>(f.building).is_none());
        let map = f.world.resource::<WorldMap>();
        for grid in f.old {
            assert_eq!(map.building_entity(grid), None);
            assert!(map.is_walkable(grid.0, grid.1));
        }
        for grid in &f.new {
            assert_eq!(map.building_entity(*grid), Some(f.building));
            assert!(map.has_raw_obstacle(grid.0, grid.1));
        }
        if kind == BuildingType::Tank {
            for x in 12..14 {
                assert_eq!(map.stockpile_entity((x, 10)), None);
                assert!(map.stockpile_entity((x + 10, 20)).is_some());
                assert!(map.is_walkable(x + 10, 20));
            }
        }
        let marker_positions: std::collections::HashSet<_> = f
            .world
            .query::<(&ObstaclePosition, &ChildOf)>()
            .iter(&f.world)
            .filter(|(_, parent)| parent.parent() == f.building)
            .map(|(position, _)| (position.0, position.1))
            .collect();
        assert_eq!(marker_positions, f.new.into_iter().collect());
        assert!(
            matches!(f.world.get::<AssignedTask>(f.worker), Some(AssignedTask::MovePlant(data)) if data.phase == MovePlantPhase::Done)
        );
        assert!(f.world.get::<MovePlantReservation>(f.task).is_none());
    }
}

#[test]
fn move_plant_conflict_keeps_all_map_and_transform_state_test() {
    for conflict in 0..5 {
        let mut f = fixture(BuildingType::Tank);
        let other = f.world.spawn_empty().id();
        match conflict {
            0 => f
                .world
                .resource_mut::<WorldMap>()
                .set_building(f.old[1], other),
            1 => f
                .world
                .resource_mut::<WorldMap>()
                .set_building(f.new[1], other),
            2 => f
                .world
                .resource_mut::<WorldMap>()
                .set_stockpile((22, 20), other),
            3 => {
                f.world.spawn((
                    ObstaclePosition(20, 20),
                    ObstacleSourceKind::ConstructionProtection,
                ));
            }
            _ => {
                // Raw-only reservations are not proof of ownership.
                let marker = f
                    .world
                    .query::<(Entity, &ObstaclePosition, &ChildOf)>()
                    .iter(&f.world)
                    .find(|(_, position, parent)| {
                        parent.parent() == f.task && (position.0, position.1) == (20, 20)
                    })
                    .unwrap()
                    .0;
                f.world.entity_mut(marker).despawn();
            }
        }
        let map = f.world.resource::<WorldMap>();
        let before = (
            map.buildings.clone(),
            map.stockpiles.clone(),
            map.obstacles.clone(),
            map.obstacle_version,
        );
        let transforms: Vec<_> = f
            .world
            .query::<(Entity, &Transform)>()
            .iter(&f.world)
            .map(|(entity, transform)| (entity, *transform))
            .collect();
        let markers: Vec<_> = f
            .world
            .query::<(Entity, &ObstaclePosition)>()
            .iter(&f.world)
            .map(|(entity, pos)| (entity, (pos.0, pos.1)))
            .collect();
        apply_pending_building_move_system(&mut f.world);
        let map = f.world.resource::<WorldMap>();
        assert_eq!(
            before,
            (
                map.buildings.clone(),
                map.stockpiles.clone(),
                map.obstacles.clone(),
                map.obstacle_version
            )
        );
        for (entity, transform) in transforms {
            assert_eq!(f.world.get::<Transform>(entity), Some(&transform));
        }
        for (entity, grid) in markers {
            let pos = f.world.get::<ObstaclePosition>(entity).unwrap();
            assert_eq!((pos.0, pos.1), grid);
        }
        assert!(
            f.world
                .get::<PendingBuildingMove>(f.building)
                .unwrap()
                .rejected
        );
        assert!(
            matches!(f.world.get::<AssignedTask>(f.worker), Some(AssignedTask::MovePlant(data)) if data.phase == MovePlantPhase::Moving)
        );
    }
}

#[test]
fn move_plant_stale_pending_does_not_abort_a_replacement_assignment_test() {
    let mut f = fixture(BuildingType::MudMixer);
    let replacement = f.world.spawn_empty().id();
    let identity = ActiveTaskIdentity::new(replacement, replacement, WorkType::Move);
    f.world.entity_mut(f.worker).insert(identity);
    f.world.entity_mut(f.building).insert(MovePlanned {
        task_entity: replacement,
    });
    let transform = *f.world.get::<Transform>(f.building).unwrap();
    apply_pending_building_move_system(&mut f.world);
    assert_eq!(f.world.get::<Transform>(f.building), Some(&transform));
    assert_eq!(f.world.get::<ActiveTaskIdentity>(f.worker), Some(&identity));
    assert_eq!(
        f.world.get::<MovePlanned>(f.building).unwrap().task_entity,
        replacement
    );
    assert!(f.world.get::<PendingBuildingMove>(f.building).is_none());
}

#[test]
fn move_plant_rejected_pending_is_removed_after_external_unassignment_test() {
    let mut f = fixture(BuildingType::MudMixer);
    f.world
        .get_mut::<PendingBuildingMove>(f.building)
        .unwrap()
        .rejected = true;
    f.world
        .entity_mut(f.worker)
        .remove::<(ActiveTaskIdentity, WorkingOn)>();
    f.world.entity_mut(f.worker).insert(AssignedTask::None);
    apply_pending_building_move_system(&mut f.world);
    assert!(f.world.get::<PendingBuildingMove>(f.building).is_none());
    assert!(matches!(
        f.world.get::<AssignedTask>(f.worker),
        Some(AssignedTask::None)
    ));
}

#[test]
fn move_plant_replaced_plan_rejects_current_worker_without_clearing_new_plan_test() {
    let mut f = fixture(BuildingType::MudMixer);
    let replacement = f.world.spawn_empty().id();
    f.world.entity_mut(f.building).insert(MovePlanned {
        task_entity: replacement,
    });
    apply_pending_building_move_system(&mut f.world);
    assert!(
        f.world
            .get::<PendingBuildingMove>(f.building)
            .unwrap()
            .rejected
    );
    assert_eq!(
        f.world.get::<MovePlanned>(f.building).unwrap().task_entity,
        replacement
    );
}

#[test]
fn move_plant_commit_result_reaches_real_terminal_handler_once_test() {
    use bevy::ecs::system::RunSystemOnce;
    use hw_core::events::{
        OnTaskAbandoned, ResourceReservationRequest, TaskCompletedVisualMessage,
    };
    use hw_core::soul::{DamnedSoul, Destination, Path};
    use hw_core::visual::SoulTaskHandles;
    use hw_logistics::{Inventory, SharedResourceCache};
    use hw_world::RuntimePathSearchBudget;
    for rejected in [false, true] {
        let mut f = fixture(BuildingType::MudMixer);
        f.world.insert_resource(Time::<()>::default());
        f.world.init_resource::<SharedResourceCache>();
        f.world.init_resource::<RuntimePathSearchBudget>();
        f.world.insert_resource(SoulTaskHandles {
            wood: default(),
            tree_animes: vec![],
            icon_rock_small: default(),
            icon_bone_small: default(),
            icon_sand_small: default(),
            icon_stasis_mud_small: default(),
            bucket_water: default(),
            bucket_empty: default(),
        });
        f.world
            .init_resource::<Messages<ResourceReservationRequest>>();
        f.world.init_resource::<Messages<OnTaskAbandoned>>();
        f.world
            .init_resource::<Messages<TaskCompletedVisualMessage>>();
        f.world
            .init_resource::<Messages<hw_jobs::DeconstructionCommitRequest>>();
        #[cfg(feature = "profiling")]
        f.world
            .init_resource::<crate::soul_ai::execute::task_execution::TaskExecutionPerfMetrics>();
        f.world.entity_mut(f.worker).insert((
            DamnedSoul::default(),
            Transform::default(),
            Destination(Vec2::ZERO),
            Path::default(),
            Inventory::default(),
        ));
        let replacement = f.world.spawn_empty().id();
        if rejected {
            f.world.entity_mut(f.building).insert(MovePlanned {
                task_entity: replacement,
            });
        }
        apply_pending_building_move_system(&mut f.world);
        f.world
            .run_system_once(crate::soul_ai::execute::task_execution_system::task_execution_system)
            .unwrap();
        apply_pending_building_move_system(&mut f.world);
        assert!(matches!(
            f.world.get::<AssignedTask>(f.worker),
            Some(AssignedTask::None)
        ));
        assert!(f.world.get::<WorkingOn>(f.worker).is_none());
        assert!(f.world.get::<ActiveTaskIdentity>(f.worker).is_none());
        assert!(f.world.get_entity(f.task).is_err());
        assert!(f.world.get::<PendingBuildingMove>(f.building).is_none());
        assert_eq!(
            f.world
                .resource::<Messages<TaskCompletedVisualMessage>>()
                .len(),
            usize::from(!rejected)
        );
        assert_eq!(f.world.resource::<Messages<OnTaskAbandoned>>().len(), 0);
        if rejected {
            assert_eq!(
                f.world.get::<MovePlanned>(f.building).unwrap().task_entity,
                replacement
            );
        }
        f.world
            .run_system_once(crate::soul_ai::execute::task_execution_system::task_execution_system)
            .unwrap();
        assert_eq!(
            f.world
                .resource::<Messages<TaskCompletedVisualMessage>>()
                .len(),
            usize::from(!rejected)
        );
        assert_eq!(f.world.resource::<Messages<OnTaskAbandoned>>().len(), 0);
    }
}
