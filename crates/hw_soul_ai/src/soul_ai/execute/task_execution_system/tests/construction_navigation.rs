use super::*;
use hw_core::{area::TaskArea, constants::MAP_HEIGHT};
use hw_jobs::construction::{FloorTileBlueprint, WallTileBlueprint};
use hw_jobs::{
    FloorConstructionSite, FloorTileState, FrameWallPhase, FrameWallTileData, PourFloorPhase,
    PourFloorTileData, ReinforceFloorPhase, ReinforceFloorTileData, WallConstructionSite,
    WallTileState,
};

fn construction_fixture(
    work: WorkType,
    to_tile: bool,
    target_grid: (i32, i32),
) -> (App, Entity, Entity, Entity) {
    let mut app = task_execution_test_app();
    let target_pos = WorldMap::grid_to_world(target_grid.0, target_grid.1);
    let site = app
        .world_mut()
        .spawn(Transform::from_translation(target_pos.extend(0.0)))
        .id();
    let tile = app.world_mut().spawn_empty().id();
    let area = TaskArea::from_points(target_pos, target_pos);
    let task = match work {
        WorkType::ReinforceFloorTile | WorkType::PourFloorTile => {
            app.world_mut()
                .entity_mut(site)
                .insert(FloorConstructionSite::new(area, target_pos, 1));
            let mut blueprint = FloorTileBlueprint::new(site, target_grid);
            blueprint.state = if work == WorkType::ReinforceFloorTile {
                FloorTileState::ReinforcingReady
            } else {
                FloorTileState::PouringReady
            };
            app.world_mut().entity_mut(tile).insert(blueprint);
            if work == WorkType::ReinforceFloorTile {
                AssignedTask::ReinforceFloorTile(ReinforceFloorTileData {
                    tile,
                    site,
                    phase: if to_tile {
                        ReinforceFloorPhase::GoingToTile
                    } else {
                        ReinforceFloorPhase::GoingToMaterialCenter
                    },
                })
            } else {
                AssignedTask::PourFloorTile(PourFloorTileData {
                    tile,
                    site,
                    phase: if to_tile {
                        PourFloorPhase::GoingToTile
                    } else {
                        PourFloorPhase::GoingToMaterialCenter
                    },
                })
            }
        }
        WorkType::FrameWallTile => {
            app.world_mut()
                .entity_mut(site)
                .insert(WallConstructionSite::new(area, target_pos, 1));
            let mut blueprint = WallTileBlueprint::new(site, target_grid);
            blueprint.state = WallTileState::FramingReady;
            app.world_mut().entity_mut(tile).insert(blueprint);
            AssignedTask::FrameWallTile(FrameWallTileData {
                tile,
                site,
                phase: if to_tile {
                    FrameWallPhase::GoingToTile
                } else {
                    FrameWallPhase::GoingToMaterialCenter
                },
            })
        }
        _ => unreachable!(),
    };
    let soul_pos = WorldMap::grid_to_world(25, 50);
    let soul = spawn_task_execution_soul(app.world_mut(), task);
    app.world_mut().entity_mut(soul).insert((
        Transform::from_translation(soul_pos.extend(0.0)),
        Destination(soul_pos),
        ActiveTaskIdentity::new(site, tile, work),
        WorkingOn(tile),
    ));
    (app, soul, tile, site)
}

fn construction_cases() -> impl Iterator<Item = (WorkType, bool)> {
    [
        WorkType::ReinforceFloorTile,
        WorkType::PourFloorTile,
        WorkType::FrameWallTile,
    ]
    .into_iter()
    .flat_map(|work| [false, true].map(|to_tile| (work, to_tile)))
}

#[test]
fn construction_unreachable_keeps_work_unstarted_test() {
    for (work, to_tile) in construction_cases() {
        let (mut app, soul, tile, site) = construction_fixture(work, to_tile, (75, 50));
        for y in 0..MAP_HEIGHT {
            app.world_mut()
                .resource_mut::<WorldMap>()
                .add_grid_obstacle((50, y));
        }
        app.update();

        assert!(
            matches!(
                app.world().get::<AssignedTask>(soul),
                Some(AssignedTask::None)
            ),
            "{work:?}, to_tile={to_tile}"
        );
        assert!(app.world().get::<WorkingOn>(soul).is_none());
        assert!(app.world().get::<ActiveTaskIdentity>(soul).is_none());
        let receipts = app.world().resource::<TaskNotificationReceipts>();
        assert!(receipts.completed_domain.is_empty());
        assert!(receipts.completed_visual.is_empty());
        assert!(receipts.abandoned.is_empty());
        assert_eq!(
            receipts.reservation_ops,
            vec![ResourceReservationOp::ReleaseSource {
                source: tile.into(),
                amount: 1
            }]
        );
        if let Some(floor) = app.world().get::<FloorTileBlueprint>(tile) {
            assert_eq!(floor.bones_delivered, 0);
            assert_eq!(floor.mud_delivered, 0);
            assert!(matches!(
                floor.state,
                FloorTileState::ReinforcingReady | FloorTileState::PouringReady
            ));
            let site = app.world().get::<FloorConstructionSite>(site).unwrap();
            assert_eq!((site.tiles_reinforced, site.tiles_poured), (0, 0));
        } else {
            let wall = app.world().get::<WallTileBlueprint>(tile).unwrap();
            assert_eq!(wall.state, WallTileState::FramingReady);
            assert_eq!(wall.wood_delivered, 0);
            assert_eq!(
                app.world()
                    .get::<WallConstructionSite>(site)
                    .unwrap()
                    .tiles_framed,
                0
            );
        }
    }
}

#[test]
fn construction_deferred_preserves_assignment_test() {
    for (work, to_tile) in construction_cases() {
        let (mut app, soul, tile, site) = construction_fixture(work, to_tile, (75, 50));
        app.world_mut()
            .insert_resource(RuntimePathSearchBudget::new(0));
        let task_before = format!("{:?}", app.world().get::<AssignedTask>(soul).unwrap());
        let destination_before = app.world().get::<Destination>(soul).unwrap().0;
        app.world_mut().clear_trackers();
        app.update();

        assert_eq!(
            format!("{:?}", app.world().get::<AssignedTask>(soul).unwrap()),
            task_before
        );
        assert_eq!(
            app.world().get::<Destination>(soul).unwrap().0,
            destination_before
        );
        assert!(app.world().get::<Path>(soul).unwrap().waypoints.is_empty());
        let identity = app.world().get::<ActiveTaskIdentity>(soul).unwrap();
        assert_eq!(identity.assignment_entity, site);
        assert_eq!(identity.current_target_entity, tile);
        assert_eq!(app.world().get::<WorkingOn>(soul).unwrap().0, tile);
        let receipts = app.world().resource::<TaskNotificationReceipts>();
        assert!(receipts.reservation_ops.is_empty());
        assert!(receipts.completed_domain.is_empty());
        assert!(receipts.abandoned.is_empty());
        assert_component_unchanged::<AssignedTask>(app.world_mut(), soul);
        assert_component_unchanged::<Destination>(app.world_mut(), soul);
        assert_component_unchanged::<Path>(app.world_mut(), soul);
    }
}

#[test]
fn construction_found_preserves_arrival_policy_test() {
    for (work, to_tile) in construction_cases() {
        for (target, arrived) in [((26, 50), true), ((27, 50), true), ((35, 50), false)] {
            let (mut app, soul, _, _) = construction_fixture(work, to_tile, target);
            if target == (27, 50) {
                let adjacent = WorldMap::grid_to_world(26, 50);
                app.world_mut().get_mut::<Path>(soul).unwrap().waypoints = vec![adjacent];
                app.world_mut().get_mut::<Destination>(soul).unwrap().0 = adjacent;
            }
            let before = format!("{:?}", app.world().get::<AssignedTask>(soul).unwrap());
            app.update();
            let after = app.world().get::<AssignedTask>(soul).unwrap();
            assert!(!matches!(after, AssignedTask::None));
            assert_eq!(
                format!("{after:?}") != before,
                arrived,
                "{work:?}, to_tile={to_tile}"
            );
            assert!(
                app.world()
                    .resource::<TaskNotificationReceipts>()
                    .completed_domain
                    .is_empty()
            );
            if arrived && to_tile {
                assert!(app.world().get::<Path>(soul).unwrap().waypoints.is_empty());
            }
        }
    }
}
