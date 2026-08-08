use super::*;
use bevy::ecs::system::RunSystemOnce;
use hw_core::events::{OnTaskAbandoned, ResourceReservationRequest, TaskCompletedVisualMessage};
use hw_core::familiar::{
    ActiveCommand, Familiar, FamiliarAiState, FamiliarOperation, FamiliarPolicy,
};
use hw_core::logistics::WheelbarrowDestination;
use hw_core::relationships::{
    CommandedBy, DeliveringTo, LoadedIn, LoadedItems, ManagedBy, ManagedTasks, ParkedAt, PushedBy,
    RestAreaReservedFor, RestingIn, StoredIn, TaskWorkers, WorkingOn,
};
use hw_core::soul::{DamnedSoul, Destination, IdleBehavior, IdleState, Path, RestAreaCooldown};
use hw_core::visual::SoulTaskHandles;
use hw_core::world::DoorState;
use hw_energy::{
    ConsumesFrom, GeneratesFor, GridConsumers, GridGenerators, PowerConsumer, PowerConsumerPolicy,
    PowerGenerator, PowerGrid, PowerGridAllocationSummary, PowerPriority, PowerShedReason,
    PowerSupplyState, SoulSpaPhase, SoulSpaSite, SoulSpaTile, Unpowered, YardPowerGrid,
};
use hw_jobs::events::TaskAssignmentRequest;
use hw_jobs::mud_mixer::{MudMixerStorage, StoredByMixer};
use hw_jobs::{
    ActiveTaskIdentity, AssignedTask, BonePile, BridgeMarker, Building, BuildingType,
    DeconstructData, DeconstructPhase, DeconstructionBlockReason, DeconstructionBlocker,
    DeconstructionCancelOutcome, DeconstructionCancelRequest, DeconstructionCancelResult,
    DeconstructionCommitClaim, DeconstructionCommitOutcome, DeconstructionCommitRequest,
    DeconstructionCommitResult, DeconstructionOrder, DeconstructionPending, Designation, Door,
    GeneratePowerData, GeneratePowerPhase, HaulData, HaulPhase, HaulToMixerData, HaulToMixerPhase,
    HaulWithWheelbarrowData, HaulWithWheelbarrowPhase, RestArea, SandPile,
    TargetDeconstructionRoot, TargetSoulSpaSite, TaskDiagnosticDomainMask,
    TaskDiagnosticInputRevisions, TaskSlots, WorkType,
};
use hw_logistics::transport_request::{
    ManualHaulPinnedSource, TransportPriority, TransportRequest, TransportRequestFixedSource,
    TransportRequestKind,
};
use hw_logistics::types::WheelbarrowParking;
use hw_logistics::zone::Stockpile;
use hw_logistics::{
    BelongsTo, BucketStorage, Inventory, ResourceItem, ResourceItemVisualHandles, ResourceType,
    SharedResourceCache, Wheelbarrow,
};
use hw_spatial::{DesignationSpatialGrid, ResourceSpatialGrid, TransportRequestSpatialGrid};
use hw_visual::Building3dVisual;
use hw_world::{
    Room, RoomBoundaryLookup, RoomDetectionState, RoomTileLookup, RuntimePathSearchBudget,
    TerrainType, WalkabilityConnectivityCache, WorldMap, detect_rooms_system,
};
use std::time::Duration;

#[derive(Resource, Default)]
struct FinalizerReceipts {
    commits: Vec<DeconstructionCommitOutcome>,
    cancels: Vec<DeconstructionCancelOutcome>,
    completed: Vec<TaskCompletedVisualMessage>,
}

fn collect_outcomes(
    mut commits: MessageReader<DeconstructionCommitOutcome>,
    mut cancels: MessageReader<DeconstructionCancelOutcome>,
    mut completed: MessageReader<TaskCompletedVisualMessage>,
    mut receipts: ResMut<FinalizerReceipts>,
) {
    receipts.commits.extend(commits.read().copied());
    receipts.cancels.extend(cancels.read().copied());
    receipts.completed.extend(completed.read().copied());
}

fn empty_soul_task_handles() -> SoulTaskHandles {
    SoulTaskHandles {
        wood: default(),
        tree_animes: Vec::new(),
        rock: default(),
        icon_bone_small: default(),
        icon_sand_small: default(),
        icon_stasis_mud_small: default(),
        bucket_water: default(),
        bucket_empty: default(),
    }
}

fn test_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .init_resource::<hw_core::WorldEpoch>()
        .init_resource::<WorldMap>()
        .init_resource::<SharedResourceCache>()
        .init_resource::<TaskDiagnosticInputRevisions>()
        .init_resource::<RuntimePathSearchBudget>()
        .init_resource::<RoomDetectionState>()
        .init_resource::<hw_visual::wall_connection::WallConnectionDirty>()
        .init_resource::<crate::systems::energy::grid_recalc::EnergyUpdateDirty>()
        .init_resource::<FinalizerReceipts>()
        .insert_resource(empty_soul_task_handles())
        .insert_resource(ResourceItemVisualHandles {
            icon_bone_small: default(),
            icon_wood_small: default(),
            icon_rock_small: default(),
            icon_sand_small: default(),
            icon_stasis_mud_small: default(),
        })
        .add_message::<ResourceReservationRequest>()
        .add_message::<TaskCompletedVisualMessage>()
        .add_message::<OnTaskAbandoned>()
        .add_message::<DeconstructionCommitRequest>()
        .add_message::<DeconstructionCommitOutcome>()
        .add_message::<DeconstructionCancelRequest>()
        .add_message::<DeconstructionCancelOutcome>()
        .add_systems(
            Update,
            (
                deconstruction_finalizer_system.in_set(DeconstructionFinalizerSet::Finalize),
                collect_outcomes,
            )
                .chain(),
        );
    app
}

fn energy_test_app() -> App {
    let mut app = test_app();
    app.init_resource::<hw_core::GameSettings>()
        .init_resource::<hw_energy::PowerAllocationMode>()
        .init_resource::<ResourceSpatialGrid>()
        .init_resource::<hw_spatial::SpatialGrid>()
        .init_resource::<hw_soul_ai::soul_ai::update::slow_simulation::SlowSimulationClock>()
        .configure_sets(
            Update,
            (
                crate::systems::GameSystemSet::Logic,
                crate::systems::GameSystemSet::Visual,
            )
                .chain(),
        )
        .add_observer(hw_jobs::visual_sync::on_power_consumer_visual_added)
        .add_observer(hw_jobs::visual_sync::on_unpowered_added)
        .add_observer(hw_jobs::visual_sync::on_unpowered_removed)
        .add_systems(
            Update,
            hw_visual::power::sync_powered_visual_system
                .in_set(crate::systems::GameSystemSet::Visual),
        );
    crate::plugins::logic::register_soul_energy_pipeline(&mut app);
    app
}

fn add_diagnostic_revision_systems(app: &mut App) {
    app.init_resource::<crate::systems::familiar_ai::diagnostics::TaskDiagnosticExternalRevisionState>()
        .init_resource::<ResourceSpatialGrid>()
        .add_systems(
            Update,
            (
                crate::systems::familiar_ai::diagnostics::sync_task_diagnostic_revisions_system,
                crate::systems::familiar_ai::diagnostics::refresh_deconstruction_blockers_after_revision_sync_system,
            )
                .chain()
                .before(deconstruction_finalizer_system),
        );
}

#[derive(Debug, Clone, Copy)]
struct Fixture {
    order: Entity,
    target: Entity,
    worker: Entity,
    identity: ActiveTaskIdentity,
    grid: (i32, i32),
    visual: Entity,
}

fn spawn_fixture(app: &mut App, kind: BuildingType) -> Fixture {
    let grid = (12, 13);
    let position = WorldMap::grid_to_world(grid.0, grid.1);
    let mut target = app.world_mut().spawn((
        Building {
            kind,
            is_provisional: false,
        },
        Transform::from_translation(position.extend(0.0)),
    ));
    match kind {
        BuildingType::BonePile => {
            target.insert(BonePile);
        }
        BuildingType::SandPile => {
            target.insert(SandPile);
        }
        _ => unreachable!("M2 fixture only supports resource piles"),
    }
    let target = target.id();
    let order = app
        .world_mut()
        .spawn((
            DeconstructionOrder,
            Designation {
                work_type: WorkType::Deconstruct,
            },
            TaskSlots::new(1),
            TargetDeconstructionRoot(target),
            Transform::from_translation(position.extend(0.0)),
        ))
        .id();
    app.world_mut().flush();
    app.world_mut()
        .entity_mut(target)
        .insert(DeconstructionPending { order });
    app.world_mut()
        .resource_mut::<WorldMap>()
        .set_building_occupancy(grid, target);

    let identity = ActiveTaskIdentity::new(order, order, WorkType::Deconstruct);
    let worker = app
        .world_mut()
        .spawn((
            Transform::from_translation(WorldMap::grid_to_world(11, 13).extend(0.0)),
            DamnedSoul::default(),
            AssignedTask::Deconstruct(DeconstructData {
                order,
                target,
                phase: DeconstructPhase::AwaitingCommit,
            }),
            Destination(position),
            Path::default(),
            Inventory::default(),
            identity,
            WorkingOn(order),
        ))
        .id();
    let visual = app
        .world_mut()
        .spawn(Building3dVisual { owner: target })
        .id();
    app.world_mut().flush();

    Fixture {
        order,
        target,
        worker,
        identity,
        grid,
        visual,
    }
}

fn spawn_facility_fixture(app: &mut App, kind: BuildingType) -> Fixture {
    let lower_left = (12, 13);
    let footprint = [
        lower_left,
        (lower_left.0 + 1, lower_left.1),
        (lower_left.0, lower_left.1 + 1),
        (lower_left.0 + 1, lower_left.1 + 1),
    ];
    let position = WorldMap::grid_to_world(lower_left.0, lower_left.1)
        + Vec2::splat(hw_core::constants::TILE_SIZE * 0.5);
    let mut target = app.world_mut().spawn((
        Building {
            kind,
            is_provisional: false,
        },
        Transform::from_translation(position.extend(0.0)),
    ));
    match kind {
        BuildingType::Tank => {
            target.insert(Stockpile {
                capacity: 50,
                resource_type: Some(ResourceType::Water),
            });
        }
        BuildingType::MudMixer => {
            target.insert((
                MudMixerStorage::default(),
                Stockpile {
                    capacity: hw_core::constants::MUD_MIXER_CAPACITY as usize,
                    resource_type: Some(ResourceType::Water),
                },
            ));
        }
        BuildingType::RestArea => {
            target.insert(RestArea { capacity: 4 });
        }
        BuildingType::WheelbarrowParking => {
            target.insert(WheelbarrowParking { capacity: 2 });
        }
        _ => unreachable!("facility fixture only supports M3 facilities"),
    }
    let target = target.id();
    let order = app
        .world_mut()
        .spawn((
            DeconstructionOrder,
            Designation {
                work_type: WorkType::Deconstruct,
            },
            TaskSlots::new(1),
            TargetDeconstructionRoot(target),
            Transform::from_translation(position.extend(0.0)),
        ))
        .id();
    app.world_mut().flush();
    app.world_mut()
        .entity_mut(target)
        .insert(DeconstructionPending { order });
    for grid in footprint {
        app.world_mut()
            .resource_mut::<WorldMap>()
            .set_building_occupancy(grid, target);
    }

    let identity = ActiveTaskIdentity::new(order, order, WorkType::Deconstruct);
    let worker = app
        .world_mut()
        .spawn((
            Transform::from_translation(WorldMap::grid_to_world(11, 13).extend(0.0)),
            DamnedSoul::default(),
            AssignedTask::Deconstruct(DeconstructData {
                order,
                target,
                phase: DeconstructPhase::AwaitingCommit,
            }),
            Destination(position),
            Path::default(),
            Inventory::default(),
            identity,
            WorkingOn(order),
        ))
        .id();
    let visual = app
        .world_mut()
        .spawn(Building3dVisual { owner: target })
        .id();
    app.world_mut().flush();

    Fixture {
        order,
        target,
        worker,
        identity,
        grid: WorldMap::world_to_grid(position),
        visual,
    }
}

fn spawn_structure_fixture(app: &mut App, kind: BuildingType) -> (Fixture, Vec<(i32, i32)>) {
    let lower_left = (12, 13);
    let footprint = match kind {
        BuildingType::Bridge => (0..5)
            .flat_map(|dy| (0..2).map(move |dx| (lower_left.0 + dx, lower_left.1 + dy)))
            .collect::<Vec<_>>(),
        BuildingType::Wall | BuildingType::Door | BuildingType::Floor => vec![lower_left],
        _ => unreachable!("structure fixture only supports Wall, Door, Floor, and Bridge"),
    };
    let position = match kind {
        BuildingType::Bridge => {
            WorldMap::grid_to_world(lower_left.0, lower_left.1)
                + Vec2::new(
                    hw_core::constants::TILE_SIZE * 0.5,
                    hw_core::constants::TILE_SIZE * 2.0,
                )
        }
        _ => WorldMap::grid_to_world(lower_left.0, lower_left.1),
    };
    let mut target = app.world_mut().spawn((
        Building {
            kind,
            is_provisional: false,
        },
        Transform::from_translation(position.extend(0.0)),
    ));
    match kind {
        BuildingType::Door => {
            target.insert(Door {
                state: DoorState::Locked,
            });
        }
        BuildingType::Bridge => {
            target.insert(BridgeMarker);
        }
        BuildingType::Wall | BuildingType::Floor => {}
        _ => unreachable!(),
    }
    let target = target.id();
    let order = app
        .world_mut()
        .spawn((
            DeconstructionOrder,
            Designation {
                work_type: WorkType::Deconstruct,
            },
            TaskSlots::new(1),
            TargetDeconstructionRoot(target),
            Transform::from_translation(position.extend(0.0)),
        ))
        .id();
    app.world_mut().flush();
    app.world_mut()
        .entity_mut(target)
        .insert(DeconstructionPending { order });
    {
        let mut map = app.world_mut().resource_mut::<WorldMap>();
        match kind {
            BuildingType::Wall => map.set_building_occupancy(lower_left, target),
            BuildingType::Door => map.register_door(lower_left, target, DoorState::Locked),
            BuildingType::Floor => map.set_floor(lower_left, target),
            BuildingType::Bridge => {
                for &grid in &footprint {
                    let index = map.pos_to_idx(grid.0, grid.1).unwrap();
                    map.set_terrain_at_idx(index, TerrainType::River);
                    map.register_bridge_tile(grid, target);
                }
            }
            _ => unreachable!(),
        }
    }

    let identity = ActiveTaskIdentity::new(order, order, WorkType::Deconstruct);
    let worker = app
        .world_mut()
        .spawn((
            Transform::from_translation(WorldMap::grid_to_world(11, 13).extend(0.0)),
            DamnedSoul::default(),
            AssignedTask::Deconstruct(DeconstructData {
                order,
                target,
                phase: DeconstructPhase::AwaitingCommit,
            }),
            Destination(position),
            Path::default(),
            Inventory::default(),
            identity,
            WorkingOn(order),
        ))
        .id();
    let visual = app
        .world_mut()
        .spawn(Building3dVisual { owner: target })
        .id();
    app.world_mut().flush();

    (
        Fixture {
            order,
            target,
            worker,
            identity,
            grid: WorldMap::world_to_grid(position),
            visual,
        },
        footprint,
    )
}

fn spawn_closed_room_using_target_wall(app: &mut App, target_grid: (i32, i32)) {
    let origin = (target_grid.0 - 4, target_grid.1 - 2);
    let shifted = |grid: (i32, i32)| (origin.0 + grid.0, origin.1 + grid.1);
    for x in 1..=3 {
        for y in 1..=3 {
            let grid = shifted((x, y));
            app.world_mut().spawn((
                Building {
                    kind: BuildingType::Floor,
                    is_provisional: false,
                },
                Transform::from_translation(WorldMap::grid_to_world(grid.0, grid.1).extend(0.0)),
            ));
        }
    }

    let door_grid = shifted((1, 4));
    let mut boundary = Vec::new();
    for x in 0..=4 {
        boundary.push(shifted((x, 0)));
        boundary.push(shifted((x, 4)));
    }
    for y in 0..=4 {
        boundary.push(shifted((0, y)));
        boundary.push(shifted((4, y)));
    }
    boundary.sort_unstable();
    boundary.dedup();
    for grid in boundary {
        if grid == target_grid || grid == door_grid {
            continue;
        }
        app.world_mut().spawn((
            Building {
                kind: BuildingType::Wall,
                is_provisional: false,
            },
            Transform::from_translation(WorldMap::grid_to_world(grid.0, grid.1).extend(0.0)),
        ));
    }
    app.world_mut().spawn((
        Building {
            kind: BuildingType::Door,
            is_provisional: false,
        },
        Door {
            state: DoorState::Closed,
        },
        Transform::from_translation(WorldMap::grid_to_world(door_grid.0, door_grid.1).extend(0.0)),
    ));
}

fn spawn_energy_fixture(
    app: &mut App,
    kind: BuildingType,
) -> (Fixture, Vec<Entity>, Entity, Vec<(i32, i32)>) {
    let lower_left = (12, 13);
    let footprint = match kind {
        BuildingType::SoulSpa => vec![
            lower_left,
            (lower_left.0 + 1, lower_left.1),
            (lower_left.0, lower_left.1 + 1),
            (lower_left.0 + 1, lower_left.1 + 1),
        ],
        BuildingType::OutdoorLamp => vec![lower_left],
        _ => unreachable!("energy fixture only supports Soul Spa and Outdoor Lamp"),
    };
    let position = if kind == BuildingType::SoulSpa {
        WorldMap::grid_to_world(lower_left.0, lower_left.1)
            + Vec2::splat(hw_core::constants::TILE_SIZE * 0.5)
    } else {
        WorldMap::grid_to_world(lower_left.0, lower_left.1)
    };
    let grid = app.world_mut().spawn(PowerGrid::default()).id();
    let mut target = app.world_mut().spawn((
        Building {
            kind,
            is_provisional: false,
        },
        Transform::from_translation(position.extend(0.0)),
    ));
    match kind {
        BuildingType::SoulSpa => {
            target.insert((
                SoulSpaSite {
                    phase: SoulSpaPhase::Operational,
                    bones_required: 12,
                    bones_delivered: 12,
                    active_slots: 4,
                },
                GeneratesFor(grid),
            ));
        }
        BuildingType::OutdoorLamp => {
            target.insert((PowerConsumer { demand: 1.0 }, ConsumesFrom(grid)));
        }
        _ => unreachable!(),
    }
    let target = target.id();
    let tiles = if kind == BuildingType::SoulSpa {
        footprint
            .iter()
            .copied()
            .map(|grid| {
                app.world_mut()
                    .spawn((
                        SoulSpaTile {
                            parent_site: target,
                            grid_pos: grid,
                        },
                        Designation {
                            work_type: WorkType::GeneratePower,
                        },
                        TaskSlots::new(1),
                        Transform::from_translation(
                            WorldMap::grid_to_world(grid.0, grid.1).extend(0.0),
                        ),
                    ))
                    .id()
            })
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };
    let order = app
        .world_mut()
        .spawn((
            DeconstructionOrder,
            Designation {
                work_type: WorkType::Deconstruct,
            },
            TaskSlots::new(1),
            TargetDeconstructionRoot(target),
            Transform::from_translation(position.extend(0.0)),
        ))
        .id();
    app.world_mut().flush();
    app.world_mut()
        .entity_mut(target)
        .insert(DeconstructionPending { order });
    for &cell in &footprint {
        app.world_mut()
            .resource_mut::<WorldMap>()
            .set_building(cell, target);
    }
    let identity = ActiveTaskIdentity::new(order, order, WorkType::Deconstruct);
    let worker = app
        .world_mut()
        .spawn((
            Transform::from_translation(WorldMap::grid_to_world(11, 13).extend(0.0)),
            DamnedSoul::default(),
            AssignedTask::Deconstruct(DeconstructData {
                order,
                target,
                phase: DeconstructPhase::AwaitingCommit,
            }),
            Destination(position),
            Path::default(),
            Inventory::default(),
            identity,
            WorkingOn(order),
        ))
        .id();
    let visual = app
        .world_mut()
        .spawn(Building3dVisual { owner: target })
        .id();
    app.world_mut().flush();
    (
        Fixture {
            order,
            target,
            worker,
            identity,
            grid: WorldMap::world_to_grid(position),
            visual,
        },
        tiles,
        grid,
        footprint,
    )
}

#[test]
fn executor_to_finalizer_headless_slice_commits_once_in_one_update() {
    let mut app = test_app();
    app.add_systems(
        Update,
        (
            hw_soul_ai::soul_ai::execute::task_execution_system::task_execution_system,
            ApplyDeferred,
        )
            .chain()
            .before(deconstruction_finalizer_system),
    );
    let fixture = spawn_fixture(&mut app, BuildingType::SandPile);
    *app.world_mut()
        .get_mut::<AssignedTask>(fixture.worker)
        .expect("fixture worker has an assigned task") =
        AssignedTask::Deconstruct(DeconstructData {
            order: fixture.order,
            target: fixture.target,
            phase: DeconstructPhase::Dismantling { progress: 1.0 },
        });

    app.update();

    assert!(app.world().get_entity(fixture.order).is_err());
    assert!(app.world().get_entity(fixture.target).is_err());
    assert!(matches!(
        app.world().get::<AssignedTask>(fixture.worker),
        Some(AssignedTask::None)
    ));
    let receipts = app.world().resource::<FinalizerReceipts>();
    assert_eq!(receipts.commits.len(), 1);
    assert_eq!(
        receipts.commits[0].result,
        DeconstructionCommitResult::Committed
    );
    assert_eq!(receipts.completed.len(), 1);
    assert_eq!(receipts.completed[0].entity, fixture.worker);
    assert_eq!(receipts.completed[0].assignment_entity, fixture.order);
    assert_eq!(receipts.completed[0].current_target_entity, fixture.order);
    assert_eq!(
        receipts.completed[0].current_work_type,
        WorkType::Deconstruct
    );

    app.update();
    let receipts = app.world().resource::<FinalizerReceipts>();
    assert_eq!(receipts.commits.len(), 1);
    assert_eq!(receipts.completed.len(), 1);
}

#[test]
fn order_to_familiar_assignment_to_finalizer_runs_in_fixed_headless_ticks() {
    use std::time::Duration;

    use bevy::time::TimeUpdateStrategy;
    use hw_core::events::OnTaskAssigned;
    use hw_familiar_ai::familiar_ai::decide::resources::FamiliarTaskDelegationTimer;
    use hw_familiar_ai::familiar_ai::decide::task_delegation::familiar_task_delegation_system;
    use hw_logistics::tile_index::TileSiteIndex;
    use hw_soul_ai::soul_ai::execute::task_assignment_apply::apply_task_assignment_requests_system;

    let mut app = test_app();
    app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(
        250,
    )))
    .init_resource::<FamiliarTaskDelegationTimer>()
    .init_resource::<DesignationSpatialGrid>()
    .init_resource::<TransportRequestSpatialGrid>()
    .init_resource::<ResourceSpatialGrid>()
    .init_resource::<TileSiteIndex>()
    .init_resource::<WalkabilityConnectivityCache>()
    .init_resource::<hw_jobs::TaskDiagnosticInputRevisions>()
    .init_resource::<hw_familiar_ai::FamiliarTaskCandidateDiagnostics>()
    .init_resource::<hw_logistics::transport_request::WheelbarrowArbitrationDiagnostics>()
    .add_message::<TaskAssignmentRequest>()
    .add_message::<OnTaskAssigned>()
    .add_systems(
        Update,
        (
            familiar_task_delegation_system,
            apply_task_assignment_requests_system,
            ApplyDeferred,
            hw_soul_ai::soul_ai::execute::task_execution_system::task_execution_system,
        )
            .chain()
            .before(deconstruction_finalizer_system),
    );

    let position = WorldMap::grid_to_world(12, 13);
    let familiar = app
        .world_mut()
        .spawn((
            Familiar::default(),
            FamiliarOperation::default(),
            FamiliarPolicy::default(),
            ActiveCommand::default(),
            FamiliarAiState::SearchingTask,
            Transform::from_translation(WorldMap::grid_to_world(11, 13).extend(0.0)),
            Destination(position),
            Path::default(),
            ManagedTasks::default(),
        ))
        .id();
    let worker = app
        .world_mut()
        .spawn((
            Transform::from_translation(WorldMap::grid_to_world(11, 13).extend(0.0)),
            Visibility::Visible,
            DamnedSoul::default(),
            AssignedTask::None,
            Destination(position),
            Path::default(),
            IdleState::default(),
            Inventory::default(),
            CommandedBy(familiar),
        ))
        .id();
    let target = app
        .world_mut()
        .spawn((
            Building {
                kind: BuildingType::SandPile,
                is_provisional: false,
            },
            SandPile,
            Transform::from_translation(position.extend(0.0)),
        ))
        .id();
    let order = app
        .world_mut()
        .spawn((
            DeconstructionOrder,
            Designation {
                work_type: WorkType::Deconstruct,
            },
            TaskSlots::new(1),
            TargetDeconstructionRoot(target),
            ManagedBy(familiar),
            Transform::from_translation(position.extend(0.0)),
        ))
        .id();
    app.world_mut().flush();
    app.world_mut()
        .entity_mut(target)
        .insert(DeconstructionPending { order });
    app.world_mut()
        .resource_mut::<WorldMap>()
        .set_building_occupancy((12, 13), target);
    app.world_mut()
        .resource_mut::<DesignationSpatialGrid>()
        .data_mut()
        .insert(order, position);

    let assigned_move_task = app.world_mut().spawn_empty().id();
    let active_move_worker = app
        .world_mut()
        .spawn((
            Transform::default(),
            DamnedSoul::default(),
            AssignedTask::MovePlant(hw_jobs::MovePlantData {
                task_entity: assigned_move_task,
                building: target,
                destination_grid: (20, 20),
                destination_pos: WorldMap::grid_to_world(20, 20),
                companion_anchor: None,
                phase: hw_jobs::MovePlantPhase::GoToBuilding,
            }),
            Destination(Vec2::ZERO),
            Path::default(),
            IdleState::default(),
        ))
        .id();

    app.update();
    assert!(matches!(
        app.world().get::<AssignedTask>(worker),
        Some(AssignedTask::None)
    ));
    *app.world_mut()
        .get_mut::<AssignedTask>(active_move_worker)
        .expect("assigned-only move fixture remains live") = AssignedTask::None;

    for _ in 0..13 {
        app.update();
    }

    assert!(matches!(
        app.world().get::<AssignedTask>(worker),
        Some(AssignedTask::Deconstruct(DeconstructData {
            order: assigned_order,
            target: assigned_target,
            phase: DeconstructPhase::Dismantling { .. },
        })) if *assigned_order == order && *assigned_target == target
    ));

    for _ in 0..2 {
        app.update();
    }

    assert!(
        app.world().get_entity(order).is_err(),
        "order remained: task={:?}, commits={:?}, blocker={:?}",
        app.world().get::<AssignedTask>(worker),
        app.world().resource::<FinalizerReceipts>().commits,
        app.world().get::<DeconstructionBlocker>(order),
    );
    assert!(app.world().get_entity(target).is_err());
    assert!(matches!(
        app.world().get::<AssignedTask>(worker),
        Some(AssignedTask::None)
    ));
    let receipts = app.world().resource::<FinalizerReceipts>();
    assert_eq!(receipts.commits.len(), 1);
    assert_eq!(
        receipts.commits[0].result,
        DeconstructionCommitResult::Committed
    );
    assert_eq!(receipts.completed.len(), 1);
}

fn commit_request(fixture: Fixture, world_epoch: u64) -> DeconstructionCommitRequest {
    DeconstructionCommitRequest {
        world_epoch,
        worker: fixture.worker,
        identity: fixture.identity,
        order: fixture.order,
        target: fixture.target,
    }
}

#[test]
fn structure_commits_clear_exact_layers_and_place_salvage_outside_the_footprint() {
    for (kind, salvage_type, salvage_count) in [
        (BuildingType::Wall, ResourceType::Wood, 1),
        (BuildingType::Door, ResourceType::Wood, 1),
        (BuildingType::Floor, ResourceType::Bone, 1),
        (BuildingType::Bridge, ResourceType::Rock, 3),
    ] {
        let mut app = test_app();
        let (fixture, footprint) = spawn_structure_fixture(&mut app, kind);
        let stacked_building = (kind == BuildingType::Floor).then(|| {
            let owner = app.world_mut().spawn_empty().id();
            app.world_mut()
                .resource_mut::<WorldMap>()
                .set_building(footprint[0], owner);
            owner
        });
        app.world_mut().write_message(commit_request(fixture, 0));

        app.update();

        assert!(app.world().get_entity(fixture.order).is_err(), "{kind:?}");
        assert!(app.world().get_entity(fixture.target).is_err(), "{kind:?}");
        assert!(app.world().get_entity(fixture.visual).is_err(), "{kind:?}");
        assert!(matches!(
            app.world().get::<AssignedTask>(fixture.worker),
            Some(AssignedTask::None)
        ));
        let map = app.world().resource::<WorldMap>();
        match kind {
            BuildingType::Wall => {
                assert_eq!(map.building_entity(footprint[0]), None);
                assert!(!map.has_raw_obstacle(footprint[0].0, footprint[0].1));
                assert!(map.is_walkable(footprint[0].0, footprint[0].1));
            }
            BuildingType::Door => {
                assert_eq!(map.building_entity(footprint[0]), None);
                assert_eq!(map.door_entity(footprint[0].0, footprint[0].1), None);
                assert_eq!(map.door_state(footprint[0].0, footprint[0].1), None);
                assert!(!map.has_raw_obstacle(footprint[0].0, footprint[0].1));
                assert!(map.is_walkable(footprint[0].0, footprint[0].1));
            }
            BuildingType::Floor => {
                assert_eq!(map.floor_entity(footprint[0]), None);
                assert_eq!(map.building_entity(footprint[0]), stacked_building);
            }
            BuildingType::Bridge => {
                for grid in &footprint {
                    assert_eq!(map.building_entity(*grid), None);
                    assert!(!map.bridged_tiles.contains(grid));
                    assert!(!map.is_walkable(grid.0, grid.1));
                }
            }
            _ => unreachable!(),
        }
        let salvage = app
            .world_mut()
            .query::<(&ResourceItem, &Transform)>()
            .iter(app.world())
            .filter(|(item, transform)| {
                item.0 == salvage_type
                    && !footprint
                        .contains(&WorldMap::world_to_grid(transform.translation.truncate()))
            })
            .count();
        assert_eq!(salvage, salvage_count, "{kind:?}");
        assert!(
            footprint.iter().all(|grid| app
                .world()
                .resource::<RoomDetectionState>()
                .dirty_tiles
                .contains(grid)),
            "{kind:?} footprint must wake Room detection"
        );
        assert_eq!(
            app.world().resource::<FinalizerReceipts>().commits[0].result,
            DeconstructionCommitResult::Committed
        );
    }
}

#[test]
fn wall_commit_reconciles_the_production_room_set_after_opening_the_boundary() {
    let mut app = test_app();
    app.init_resource::<RoomTileLookup>()
        .init_resource::<RoomBoundaryLookup>();
    let (fixture, footprint) = spawn_structure_fixture(&mut app, BuildingType::Wall);
    let target_grid = footprint[0];
    spawn_closed_room_using_target_wall(&mut app, target_grid);
    app.world_mut().flush();
    app.world_mut()
        .resource_mut::<RoomDetectionState>()
        .mark_dirty(target_grid);
    app.world_mut()
        .resource_mut::<Time>()
        .advance_by(Duration::from_secs(1));
    app.world_mut()
        .run_system_once(detect_rooms_system)
        .unwrap();
    app.world_mut().flush();

    assert_eq!(
        app.world_mut()
            .query_filtered::<Entity, With<Room>>()
            .iter(app.world())
            .count(),
        1
    );
    assert_eq!(
        app.world()
            .resource::<RoomBoundaryLookup>()
            .rooms_at(target_grid)
            .len(),
        1
    );

    app.world_mut().write_message(commit_request(fixture, 0));
    app.update();
    assert!(
        app.world()
            .resource::<RoomDetectionState>()
            .dirty_tiles
            .contains(&target_grid)
    );
    app.world_mut()
        .resource_mut::<Time>()
        .advance_by(Duration::from_secs(1));
    app.world_mut()
        .run_system_once(detect_rooms_system)
        .unwrap();
    app.world_mut().flush();

    assert_eq!(
        app.world_mut()
            .query_filtered::<Entity, With<Room>>()
            .iter(app.world())
            .count(),
        0
    );
    assert!(
        app.world()
            .resource::<RoomTileLookup>()
            .tile_to_room
            .is_empty()
    );
    assert!(
        app.world()
            .resource::<RoomBoundaryLookup>()
            .rooms_at(target_grid)
            .is_empty()
    );
    assert!(
        app.world()
            .resource::<RoomDetectionState>()
            .dirty_tiles
            .is_empty()
    );
}

#[test]
fn bridge_without_a_post_teardown_safe_cell_fails_without_mutation() {
    let mut app = test_app();
    let (fixture, footprint) = spawn_structure_fixture(&mut app, BuildingType::Bridge);
    {
        let mut map = app.world_mut().resource_mut::<WorldMap>();
        for index in 0..map.tiles.len() {
            map.set_terrain_at_idx(index, TerrainType::River);
        }
    }
    app.world_mut().write_message(commit_request(fixture, 0));

    app.update();

    assert!(app.world().get_entity(fixture.target).is_ok());
    assert!(app.world().get_entity(fixture.order).is_ok());
    let map = app.world().resource::<WorldMap>();
    for grid in &footprint {
        assert_eq!(map.building_entity(*grid), Some(fixture.target));
        assert!(map.bridged_tiles.contains(grid));
        assert!(map.is_walkable(grid.0, grid.1));
    }
    assert_eq!(
        app.world()
            .get::<DeconstructionBlocker>(fixture.order)
            .map(|blocker| blocker.reason),
        Some(DeconstructionBlockReason::NoSafeRecovery)
    );
    assert!(matches!(
        app.world().get::<AssignedTask>(fixture.worker),
        Some(AssignedTask::None)
    ));
    assert_eq!(
        app.world_mut()
            .query::<&ResourceItem>()
            .iter(app.world())
            .filter(|item| item.0 == ResourceType::Rock)
            .count(),
        0
    );
    assert_eq!(
        app.world().resource::<FinalizerReceipts>().commits[0].result,
        DeconstructionCommitResult::NoSafeRecovery
    );
}

#[test]
fn operational_soul_spa_commit_removes_tiles_workers_requests_and_grid_relation() {
    let mut app = test_app();
    let (fixture, tiles, grid, footprint) = spawn_energy_fixture(&mut app, BuildingType::SoulSpa);
    let mut power_workers = Vec::new();
    for &tile in tiles.iter().take(2) {
        let tile_pos = app
            .world()
            .get::<Transform>(tile)
            .unwrap()
            .translation
            .truncate();
        let identity = ActiveTaskIdentity::new(tile, tile, WorkType::GeneratePower);
        power_workers.push(
            app.world_mut()
                .spawn((
                    Transform::from_translation(tile_pos.extend(0.0)),
                    DamnedSoul::default(),
                    AssignedTask::GeneratePower(GeneratePowerData {
                        tile,
                        tile_pos,
                        phase: GeneratePowerPhase::Generating,
                    }),
                    Path::default(),
                    Inventory::default(),
                    identity,
                    WorkingOn(tile),
                ))
                .id(),
        );
    }
    let delivery_request = app
        .world_mut()
        .spawn((
            TransportRequest {
                kind: TransportRequestKind::DeliverToSoulSpa,
                anchor: fixture.target,
                resource_type: ResourceType::Bone,
                issued_by: fixture.target,
                priority: TransportPriority::Normal,
                stockpile_group: vec![],
            },
            TargetSoulSpaSite(fixture.target),
            Designation {
                work_type: WorkType::Haul,
            },
            TaskSlots::new(1),
        ))
        .id();
    app.world_mut().flush();
    app.world_mut().write_message(commit_request(fixture, 0));

    app.update();

    assert!(app.world().get_entity(fixture.target).is_err());
    assert!(app.world().get_entity(fixture.order).is_err());
    assert!(app.world().get_entity(fixture.visual).is_err());
    assert!(app.world().get_entity(delivery_request).is_err());
    for tile in tiles {
        assert!(app.world().get_entity(tile).is_err());
    }
    for worker in power_workers {
        assert!(matches!(
            app.world().get::<AssignedTask>(worker),
            Some(AssignedTask::None)
        ));
        assert!(app.world().get::<WorkingOn>(worker).is_none());
        assert!(app.world().get::<ActiveTaskIdentity>(worker).is_none());
    }
    for grid_pos in &footprint {
        assert_eq!(
            app.world()
                .resource::<WorldMap>()
                .building_entity(*grid_pos),
            None
        );
    }
    assert!(
        app.world()
            .get::<GridGenerators>(grid)
            .is_none_or(GridGenerators::is_empty)
    );
    let recovered_bones = app
        .world_mut()
        .query::<(&ResourceItem, &Transform)>()
        .iter(app.world())
        .filter(|(item, transform)| {
            item.0 == ResourceType::Bone
                && !footprint.contains(&WorldMap::world_to_grid(transform.translation.truncate()))
        })
        .count();
    assert_eq!(recovered_bones, 6);
    let dirty = app
        .world()
        .resource::<crate::systems::energy::grid_recalc::EnergyUpdateDirty>();
    assert!(dirty.topology_reconcile_due);
    assert!(dirty.power_output_due);
    assert!(dirty.grid_recalc_due);
    assert_eq!(
        app.world().resource::<FinalizerReceipts>().commits[0].result,
        DeconstructionCommitResult::Committed
    );
}

fn assert_energy_transaction_settled(app: &App) {
    let dirty = app
        .world()
        .resource::<crate::systems::energy::grid_recalc::EnergyUpdateDirty>();
    assert!(!dirty.topology_reconcile_due);
    assert!(!dirty.power_output_due);
    assert!(!dirty.grid_recalc_due);
}

#[test]
fn operational_soul_spa_removal_sheds_lamp_visual_in_the_same_update() {
    let mut app = energy_test_app();
    let (fixture, tiles, grid, _) = spawn_energy_fixture(&mut app, BuildingType::SoulSpa);
    let yard = app
        .world_mut()
        .spawn(hw_world::Yard {
            min: Vec2::splat(-10_000.0),
            max: Vec2::splat(10_000.0),
        })
        .id();
    app.world_mut().entity_mut(grid).insert(YardPowerGrid(yard));
    let tile = tiles[0];
    let tile_pos = app
        .world()
        .get::<Transform>(tile)
        .unwrap()
        .translation
        .truncate();
    let power_worker = app
        .world_mut()
        .spawn((
            Transform::from_translation(tile_pos.extend(0.0)),
            DamnedSoul::default(),
            AssignedTask::GeneratePower(GeneratePowerData {
                tile,
                tile_pos,
                phase: GeneratePowerPhase::Generating,
            }),
            Path::default(),
            Inventory::default(),
            ActiveTaskIdentity::new(tile, tile, WorkType::GeneratePower),
            WorkingOn(tile),
        ))
        .id();
    let lamp = app
        .world_mut()
        .spawn((
            Building {
                kind: BuildingType::OutdoorLamp,
                is_provisional: false,
            },
            Transform::from_translation(WorldMap::grid_to_world(20, 13).extend(0.0)),
            PowerConsumer { demand: 0.2 },
            Sprite::default(),
        ))
        .id();
    app.world_mut().flush();

    app.update();

    assert_eq!(
        app.world()
            .get::<PowerGenerator>(fixture.target)
            .unwrap()
            .current_output,
        1.0
    );
    assert_eq!(
        app.world().get::<PowerSupplyState>(lamp),
        Some(&PowerSupplyState::Supplied)
    );
    assert!(app.world().get::<Unpowered>(lamp).is_none());
    assert!(
        app.world()
            .get::<hw_core::visual_mirror::PoweredVisualState>(lamp)
            .is_some_and(|state| state.is_powered)
    );
    assert_eq!(app.world().get::<Sprite>(lamp).unwrap().color, Color::WHITE);
    assert_energy_transaction_settled(&app);

    app.world_mut().write_message(commit_request(fixture, 0));
    app.update();

    assert!(app.world().get_entity(fixture.target).is_err());
    assert!(matches!(
        app.world().get::<AssignedTask>(power_worker),
        Some(AssignedTask::None)
    ));
    let summary = app.world().get::<PowerGridAllocationSummary>(grid).unwrap();
    assert_eq!(summary.generation, 0.0);
    assert_eq!(summary.consumer_count, 1);
    assert_eq!(summary.supplied_count, 0);
    assert_eq!(summary.shed_count, 1);
    assert_eq!(
        app.world().get::<PowerSupplyState>(lamp),
        Some(&PowerSupplyState::Shed {
            reason: PowerShedReason::InsufficientGeneration,
        })
    );
    assert!(app.world().get::<Unpowered>(lamp).is_some());
    assert!(
        app.world()
            .get::<hw_core::visual_mirror::PoweredVisualState>(lamp)
            .is_some_and(|state| !state.is_powered)
    );
    assert_eq!(
        app.world().get::<Sprite>(lamp).unwrap().color,
        Color::srgba(0.4, 0.4, 0.4, 1.0)
    );
    assert_energy_transaction_settled(&app);
}

#[test]
fn outdoor_lamp_removal_restores_survivor_visual_in_the_same_update() {
    let mut app = energy_test_app();
    let (fixture, _, grid, _) = spawn_energy_fixture(&mut app, BuildingType::OutdoorLamp);
    let yard = app
        .world_mut()
        .spawn(hw_world::Yard {
            min: Vec2::splat(-10_000.0),
            max: Vec2::splat(10_000.0),
        })
        .id();
    app.world_mut().entity_mut(grid).insert(YardPowerGrid(yard));
    app.world_mut()
        .get_mut::<PowerConsumer>(fixture.target)
        .unwrap()
        .demand = 0.2;
    app.world_mut()
        .get_mut::<PowerConsumerPolicy>(fixture.target)
        .unwrap()
        .priority = PowerPriority::High;
    app.world_mut()
        .entity_mut(fixture.target)
        .insert(Sprite::default());
    app.world_mut().spawn((
        PowerGenerator {
            current_output: 0.3,
            output_per_soul: 0.3,
        },
        Transform::from_translation(WorldMap::grid_to_world(18, 13).extend(0.0)),
    ));
    let survivor = app
        .world_mut()
        .spawn((
            PowerConsumer { demand: 0.2 },
            PowerConsumerPolicy {
                priority: PowerPriority::Low,
            },
            Transform::from_translation(WorldMap::grid_to_world(19, 13).extend(0.0)),
            Sprite::default(),
        ))
        .id();
    app.world_mut().flush();

    app.update();

    assert_eq!(
        app.world().get::<PowerSupplyState>(fixture.target),
        Some(&PowerSupplyState::Supplied)
    );
    assert!(matches!(
        app.world().get::<PowerSupplyState>(survivor),
        Some(PowerSupplyState::Shed { .. })
    ));
    assert_eq!(
        app.world().get::<Sprite>(survivor).unwrap().color,
        Color::srgba(0.4, 0.4, 0.4, 1.0)
    );
    assert_energy_transaction_settled(&app);

    app.world_mut().write_message(commit_request(fixture, 0));
    app.update();

    assert!(app.world().get_entity(fixture.target).is_err());
    let summary = app.world().get::<PowerGridAllocationSummary>(grid).unwrap();
    assert_eq!(summary.generation, 0.3);
    assert_eq!(summary.consumer_count, 1);
    assert_eq!(summary.supplied_count, 1);
    assert_eq!(summary.shed_count, 0);
    assert_eq!(
        app.world().get::<PowerSupplyState>(survivor),
        Some(&PowerSupplyState::Supplied)
    );
    assert!(app.world().get::<Unpowered>(survivor).is_none());
    assert!(
        app.world()
            .get::<hw_core::visual_mirror::PoweredVisualState>(survivor)
            .is_some_and(|state| state.is_powered)
    );
    assert_eq!(
        app.world().get::<Sprite>(survivor).unwrap().color,
        Color::WHITE
    );
    assert_energy_transaction_settled(&app);
}

#[test]
fn outdoor_lamp_commit_removes_connected_or_disconnected_consumers() {
    for connected in [true, false] {
        let mut app = test_app();
        let (fixture, _, grid, footprint) =
            spawn_energy_fixture(&mut app, BuildingType::OutdoorLamp);
        if !connected {
            app.world_mut()
                .entity_mut(fixture.target)
                .remove::<ConsumesFrom>();
            app.world_mut().flush();
        }
        app.world_mut().write_message(commit_request(fixture, 0));

        app.update();

        assert!(app.world().get_entity(fixture.target).is_err());
        assert_eq!(
            app.world()
                .resource::<WorldMap>()
                .building_entity(footprint[0]),
            None
        );
        assert!(
            app.world()
                .get::<GridConsumers>(grid)
                .is_none_or(GridConsumers::is_empty)
        );
        assert_eq!(
            app.world_mut()
                .query::<&ResourceItem>()
                .iter(app.world())
                .filter(|item| item.0 == ResourceType::Bone)
                .count(),
            1
        );
        assert_eq!(
            app.world().resource::<FinalizerReceipts>().commits[0].result,
            DeconstructionCommitResult::Committed
        );
    }
}

#[test]
fn malformed_operational_soul_spa_tile_snapshot_fails_closed() {
    let mut app = test_app();
    let (fixture, tiles, grid, footprint) = spawn_energy_fixture(&mut app, BuildingType::SoulSpa);
    app.world_mut().entity_mut(tiles[0]).despawn();
    app.world_mut().write_message(commit_request(fixture, 0));

    app.update();

    assert!(app.world().get_entity(fixture.target).is_ok());
    assert!(app.world().get_entity(fixture.order).is_ok());
    assert!(app.world().get_entity(grid).is_ok());
    for grid_pos in footprint {
        assert_eq!(
            app.world().resource::<WorldMap>().building_entity(grid_pos),
            Some(fixture.target)
        );
    }
    assert_eq!(
        app.world()
            .get::<DeconstructionBlocker>(fixture.order)
            .map(|blocker| blocker.reason),
        Some(DeconstructionBlockReason::OwnerMismatch)
    );
    assert_eq!(
        app.world().resource::<FinalizerReceipts>().commits[0].result,
        DeconstructionCommitResult::OwnerMismatch
    );
}

#[test]
fn bone_pile_commit_is_exactly_once_and_preserves_underlying_floor() {
    let mut app = test_app();
    let fixture = spawn_fixture(&mut app, BuildingType::BonePile);
    let floor = app.world_mut().spawn_empty().id();
    app.world_mut()
        .resource_mut::<WorldMap>()
        .set_floor(fixture.grid, floor);
    let request = commit_request(fixture, 0);
    app.world_mut().write_message(request);
    app.world_mut().write_message(request);

    app.update();

    assert!(app.world().get_entity(fixture.order).is_err());
    assert!(app.world().get_entity(fixture.target).is_err());
    assert!(app.world().get_entity(fixture.visual).is_err());
    assert!(matches!(
        app.world().get::<AssignedTask>(fixture.worker),
        Some(AssignedTask::None)
    ));
    assert!(app.world().get::<WorkingOn>(fixture.worker).is_none());
    assert!(
        app.world()
            .get::<ActiveTaskIdentity>(fixture.worker)
            .is_none()
    );
    let map = app.world().resource::<WorldMap>();
    assert_eq!(map.building_entity(fixture.grid), None);
    assert_eq!(map.floor_entity(fixture.grid), Some(floor));
    assert!(map.is_walkable(fixture.grid.0, fixture.grid.1));
    let bone_items = app
        .world_mut()
        .query::<(&ResourceItem, &Transform)>()
        .iter(app.world())
        .filter(|(item, transform)| {
            item.0 == ResourceType::Bone
                && WorldMap::world_to_grid(transform.translation.truncate()) != fixture.grid
        })
        .count();
    assert_eq!(bone_items, 5);
    assert_eq!(
        app.world()
            .resource::<FinalizerReceipts>()
            .commits
            .iter()
            .map(|outcome| outcome.result)
            .collect::<Vec<_>>(),
        vec![
            DeconstructionCommitResult::Committed,
            DeconstructionCommitResult::Duplicate,
        ]
    );
    assert_eq!(
        app.world().resource::<FinalizerReceipts>().completed.len(),
        1,
        "only the winning request may publish task completion"
    );

    app.world_mut().write_message(request);
    app.update();

    let replay_bone_items = app
        .world_mut()
        .query::<&ResourceItem>()
        .iter(app.world())
        .filter(|item| item.0 == ResourceType::Bone)
        .count();
    assert_eq!(replay_bone_items, 5);
    assert_eq!(
        app.world()
            .resource::<FinalizerReceipts>()
            .commits
            .iter()
            .map(|outcome| outcome.result)
            .collect::<Vec<_>>(),
        vec![
            DeconstructionCommitResult::Committed,
            DeconstructionCommitResult::Duplicate,
            DeconstructionCommitResult::StaleTarget,
        ]
    );
    assert_eq!(
        app.world().resource::<FinalizerReceipts>().completed.len(),
        1
    );
}

#[test]
fn canonical_pending_order_wins_two_order_conflict_exactly_once() {
    let mut app = test_app();
    let fixture = spawn_fixture(&mut app, BuildingType::BonePile);
    let position = WorldMap::grid_to_world(fixture.grid.0, fixture.grid.1);
    let competing_order = app
        .world_mut()
        .spawn((
            DeconstructionOrder,
            Designation {
                work_type: WorkType::Deconstruct,
            },
            TaskSlots::new(1),
            TargetDeconstructionRoot(fixture.target),
            Transform::from_translation(position.extend(0.0)),
        ))
        .id();
    let competing_identity =
        ActiveTaskIdentity::new(competing_order, competing_order, WorkType::Deconstruct);
    let competing_worker = app
        .world_mut()
        .spawn((
            Transform::from_translation(WorldMap::grid_to_world(11, 13).extend(0.0)),
            DamnedSoul::default(),
            AssignedTask::Deconstruct(DeconstructData {
                order: competing_order,
                target: fixture.target,
                phase: DeconstructPhase::AwaitingCommit,
            }),
            Destination(position),
            Path::default(),
            Inventory::default(),
            competing_identity,
            WorkingOn(competing_order),
        ))
        .id();
    app.world_mut().flush();
    app.world_mut().write_message(commit_request(fixture, 0));
    app.world_mut().write_message(DeconstructionCommitRequest {
        world_epoch: 0,
        worker: competing_worker,
        identity: competing_identity,
        order: competing_order,
        target: fixture.target,
    });

    app.update();

    assert!(app.world().get_entity(fixture.target).is_err());
    assert!(app.world().get_entity(fixture.order).is_err());
    assert!(app.world().get_entity(competing_order).is_err());
    assert_eq!(
        app.world()
            .resource::<WorldMap>()
            .building_entity(fixture.grid),
        None
    );
    assert!(matches!(
        app.world().get::<AssignedTask>(fixture.worker),
        Some(AssignedTask::None)
    ));
    assert!(matches!(
        app.world().get::<AssignedTask>(competing_worker),
        Some(AssignedTask::None)
    ));
    assert_eq!(
        app.world_mut()
            .query::<&ResourceItem>()
            .iter(app.world())
            .filter(|item| item.0 == ResourceType::Bone)
            .count(),
        5
    );
    let receipts = app.world().resource::<FinalizerReceipts>();
    assert_eq!(receipts.commits.len(), 2);
    assert_eq!(
        receipts
            .commits
            .iter()
            .find(|outcome| outcome.order == fixture.order)
            .map(|outcome| outcome.result),
        Some(DeconstructionCommitResult::Committed)
    );
    assert!(
        receipts
            .commits
            .iter()
            .find(|outcome| outcome.order == competing_order)
            .is_some_and(|outcome| matches!(
                outcome.result,
                DeconstructionCommitResult::StaleTarget | DeconstructionCommitResult::Duplicate
            ))
    );
    assert_eq!(receipts.completed.len(), 1);
}

#[test]
fn canonical_order_collects_an_owned_sibling_with_missing_designation() {
    let mut app = test_app();
    let fixture = spawn_fixture(&mut app, BuildingType::BonePile);
    let position = WorldMap::grid_to_world(fixture.grid.0, fixture.grid.1);
    let malformed_sibling = app
        .world_mut()
        .spawn((
            DeconstructionOrder,
            TaskSlots::new(1),
            TargetDeconstructionRoot(fixture.target),
            Transform::from_translation(position.extend(0.0)),
        ))
        .id();
    app.world_mut().flush();
    app.world_mut().write_message(commit_request(fixture, 0));

    app.update();

    assert!(app.world().get_entity(fixture.target).is_err());
    assert!(app.world().get_entity(fixture.order).is_err());
    assert!(app.world().get_entity(malformed_sibling).is_err());
    assert!(matches!(
        app.world().get::<AssignedTask>(fixture.worker),
        Some(AssignedTask::None)
    ));
    assert_eq!(
        app.world().resource::<FinalizerReceipts>().commits[0].result,
        DeconstructionCommitResult::Committed
    );
}

#[test]
fn unowned_malformed_sibling_cannot_discard_the_canonical_order() {
    let mut app = test_app();
    let fixture = spawn_fixture(&mut app, BuildingType::SandPile);
    let malformed_sibling = app
        .world_mut()
        .spawn(TargetDeconstructionRoot(fixture.target))
        .id();
    app.world_mut().flush();
    app.world_mut().write_message(commit_request(fixture, 0));

    app.update();

    assert!(app.world().get_entity(fixture.target).is_ok());
    assert!(app.world().get_entity(fixture.order).is_ok());
    assert!(app.world().get_entity(malformed_sibling).is_ok());
    assert_eq!(
        app.world()
            .get::<DeconstructionPending>(fixture.target)
            .map(|pending| pending.order),
        Some(fixture.order)
    );
    assert!(
        app.world()
            .get::<DeconstructionBlocker>(fixture.order)
            .is_some_and(|blocker| {
                blocker.active && blocker.reason == DeconstructionBlockReason::StaleTarget
            })
    );
    assert!(matches!(
        app.world().get::<AssignedTask>(fixture.worker),
        Some(AssignedTask::None)
    ));
    assert_eq!(
        app.world().resource::<FinalizerReceipts>().commits[0].result,
        DeconstructionCommitResult::StaleTarget
    );
}

#[test]
fn repairing_an_unowned_sibling_marker_wakes_the_canonical_blocker() {
    let mut app = test_app();
    add_diagnostic_revision_systems(&mut app);
    let fixture = spawn_fixture(&mut app, BuildingType::SandPile);
    let malformed_sibling = app
        .world_mut()
        .spawn(TargetDeconstructionRoot(fixture.target))
        .id();
    app.world_mut().flush();
    app.world_mut().write_message(commit_request(fixture, 0));

    app.update();
    assert!(
        app.world()
            .get::<DeconstructionBlocker>(fixture.order)
            .is_some_and(|blocker| blocker.active)
    );

    app.update();
    assert!(
        app.world()
            .get::<DeconstructionBlocker>(fixture.order)
            .is_some_and(|blocker| blocker.active),
        "the anticipated worker cleanup revision must not wake the blocker"
    );

    app.world_mut()
        .entity_mut(malformed_sibling)
        .insert(DeconstructionOrder);
    app.update();

    assert!(
        app.world()
            .get::<DeconstructionBlocker>(fixture.order)
            .is_some_and(|blocker| !blocker.active),
        "repairing the sibling role must wake the canonical order"
    );
    assert!(app.world().get_entity(fixture.order).is_ok());
    assert!(app.world().get_entity(fixture.target).is_ok());
}

#[test]
fn stale_world_commit_touches_nothing() {
    let mut app = test_app();
    let fixture = spawn_fixture(&mut app, BuildingType::SandPile);
    app.world_mut().write_message(commit_request(fixture, 1));

    app.update();

    assert!(app.world().get_entity(fixture.order).is_ok());
    assert!(app.world().get_entity(fixture.target).is_ok());
    assert!(matches!(
        app.world().get::<AssignedTask>(fixture.worker),
        Some(AssignedTask::Deconstruct(_))
    ));
    assert_eq!(
        app.world()
            .resource::<WorldMap>()
            .building_entity(fixture.grid),
        Some(fixture.target)
    );
    assert_eq!(
        app.world().resource::<FinalizerReceipts>().commits[0].result,
        DeconstructionCommitResult::StaleWorld
    );
}

#[test]
fn stale_identity_commit_touches_nothing() {
    let mut app = test_app();
    let fixture = spawn_fixture(&mut app, BuildingType::SandPile);
    let stale_order = app.world_mut().spawn_empty().id();
    let mut request = commit_request(fixture, 0);
    request.identity = ActiveTaskIdentity::new(stale_order, stale_order, WorkType::Deconstruct);
    app.world_mut().write_message(request);

    app.update();

    assert!(app.world().get_entity(fixture.order).is_ok());
    assert!(app.world().get_entity(fixture.target).is_ok());
    assert!(matches!(
        app.world().get::<AssignedTask>(fixture.worker),
        Some(AssignedTask::Deconstruct(_))
    ));
    assert!(
        app.world()
            .get::<DeconstructionBlocker>(fixture.order)
            .is_none()
    );
    assert_eq!(
        app.world().resource::<FinalizerReceipts>().commits[0].result,
        DeconstructionCommitResult::StaleIdentity
    );
}

#[test]
fn missing_transform_fails_closed_but_releases_the_exact_worker_shell() {
    let mut app = test_app();
    let fixture = spawn_fixture(&mut app, BuildingType::BonePile);
    app.world_mut()
        .entity_mut(fixture.worker)
        .remove::<Transform>();
    app.world_mut().write_message(commit_request(fixture, 0));

    app.update();

    assert!(app.world().get_entity(fixture.target).is_ok());
    assert!(app.world().get_entity(fixture.order).is_ok());
    assert!(
        app.world()
            .get::<DeconstructionCommitClaim>(fixture.target)
            .is_none()
    );
    assert_eq!(
        app.world().resource::<FinalizerReceipts>().commits[0].result,
        DeconstructionCommitResult::StaleIdentity
    );
    assert!(matches!(
        app.world().get::<AssignedTask>(fixture.worker),
        Some(AssignedTask::None)
    ));
    assert!(
        app.world()
            .get::<ActiveTaskIdentity>(fixture.worker)
            .is_none()
    );
    assert!(app.world().get::<WorkingOn>(fixture.worker).is_none());
    assert!(
        app.world()
            .get::<TaskWorkers>(fixture.order)
            .is_none_or(TaskWorkers::is_empty)
    );
    assert!(
        app.world()
            .resource::<FinalizerReceipts>()
            .completed
            .is_empty()
    );
    assert_eq!(
        app.world_mut()
            .query::<&ResourceItem>()
            .iter(app.world())
            .count(),
        0
    );
}

#[test]
fn missing_soul_marker_fails_closed_but_releases_the_exact_worker_shell() {
    let mut app = test_app();
    let fixture = spawn_fixture(&mut app, BuildingType::BonePile);
    app.world_mut()
        .entity_mut(fixture.worker)
        .remove::<DamnedSoul>();
    app.world_mut().write_message(commit_request(fixture, 0));

    app.update();

    assert!(app.world().get_entity(fixture.target).is_ok());
    assert!(app.world().get_entity(fixture.order).is_ok());
    assert!(matches!(
        app.world().get::<AssignedTask>(fixture.worker),
        Some(AssignedTask::None)
    ));
    assert!(
        app.world()
            .get::<ActiveTaskIdentity>(fixture.worker)
            .is_none()
    );
    assert!(app.world().get::<WorkingOn>(fixture.worker).is_none());
    assert!(
        app.world()
            .get::<TaskWorkers>(fixture.order)
            .is_none_or(TaskWorkers::is_empty)
    );
    assert_eq!(
        app.world().resource::<FinalizerReceipts>().commits[0].result,
        DeconstructionCommitResult::StaleIdentity
    );
    assert!(
        app.world()
            .resource::<FinalizerReceipts>()
            .completed
            .is_empty()
    );
}

#[test]
fn missing_path_is_repaired_while_owner_failure_terminalizes_the_worker() {
    let mut app = test_app();
    let fixture = spawn_fixture(&mut app, BuildingType::SandPile);
    let other_owner = app.world_mut().spawn_empty().id();
    app.world_mut()
        .resource_mut::<WorldMap>()
        .set_building_occupancy(fixture.grid, other_owner);
    app.world_mut().entity_mut(fixture.worker).remove::<Path>();
    app.world_mut().write_message(commit_request(fixture, 0));

    app.update();

    assert!(app.world().get_entity(fixture.target).is_ok());
    assert!(app.world().get_entity(fixture.order).is_ok());
    let blocker = app
        .world()
        .get::<DeconstructionBlocker>(fixture.order)
        .expect("owner mismatch must leave a retryable blocker");
    assert!(blocker.active);
    assert!(matches!(
        app.world().get::<AssignedTask>(fixture.worker),
        Some(AssignedTask::None)
    ));
    assert!(app.world().get::<Path>(fixture.worker).is_some());
    assert_eq!(
        app.world().resource::<FinalizerReceipts>().commits[0].result,
        DeconstructionCommitResult::OwnerMismatch
    );
}

#[test]
fn vanished_canonical_target_terminalizes_worker_and_discards_orphaned_order() {
    let mut app = test_app();
    let fixture = spawn_fixture(&mut app, BuildingType::SandPile);
    let request = commit_request(fixture, 0);
    app.world_mut().entity_mut(fixture.target).despawn();
    app.world_mut().write_message(request);

    app.update();

    assert!(app.world().get_entity(fixture.target).is_err());
    assert!(app.world().get_entity(fixture.order).is_err());
    assert!(matches!(
        app.world().get::<AssignedTask>(fixture.worker),
        Some(AssignedTask::None)
    ));
    assert_eq!(
        app.world().resource::<FinalizerReceipts>().commits[0].result,
        DeconstructionCommitResult::StaleTarget
    );
    assert!(
        app.world()
            .resource::<FinalizerReceipts>()
            .completed
            .is_empty()
    );
}

#[test]
fn missing_order_relationship_terminalizes_worker_and_discards_invalid_order() {
    let mut app = test_app();
    let fixture = spawn_fixture(&mut app, BuildingType::SandPile);
    app.world_mut()
        .entity_mut(fixture.order)
        .remove::<TargetDeconstructionRoot>();
    app.world_mut().write_message(commit_request(fixture, 0));

    app.update();

    assert!(app.world().get_entity(fixture.target).is_ok());
    assert!(app.world().get_entity(fixture.order).is_err());
    assert!(
        app.world()
            .get::<DeconstructionPending>(fixture.target)
            .is_none()
    );
    assert!(matches!(
        app.world().get::<AssignedTask>(fixture.worker),
        Some(AssignedTask::None)
    ));
    assert_eq!(
        app.world().resource::<FinalizerReceipts>().commits[0].result,
        DeconstructionCommitResult::StaleTarget
    );
}

#[test]
fn missing_order_marker_releases_pending_and_discards_the_orphan_shell() {
    let mut app = test_app();
    let fixture = spawn_fixture(&mut app, BuildingType::SandPile);
    app.world_mut()
        .entity_mut(fixture.order)
        .remove::<DeconstructionOrder>();
    app.world_mut().write_message(commit_request(fixture, 0));

    app.update();

    assert!(app.world().get_entity(fixture.target).is_ok());
    assert!(app.world().get_entity(fixture.order).is_err());
    assert!(
        app.world()
            .get::<DeconstructionPending>(fixture.target)
            .is_none()
    );
    assert!(matches!(
        app.world().get::<AssignedTask>(fixture.worker),
        Some(AssignedTask::None)
    ));
    assert_eq!(
        app.world().resource::<FinalizerReceipts>().commits[0].result,
        DeconstructionCommitResult::StaleTarget
    );
}

#[test]
fn despawned_order_root_releases_pending_and_the_remaining_worker_shell() {
    let mut app = test_app();
    let fixture = spawn_fixture(&mut app, BuildingType::SandPile);
    app.world_mut().entity_mut(fixture.order).despawn();
    app.world_mut().write_message(commit_request(fixture, 0));

    app.update();

    assert!(app.world().get_entity(fixture.target).is_ok());
    assert!(
        app.world()
            .get::<DeconstructionPending>(fixture.target)
            .is_none()
    );
    assert!(matches!(
        app.world().get::<AssignedTask>(fixture.worker),
        Some(AssignedTask::None)
    ));
    assert!(
        app.world()
            .get::<ActiveTaskIdentity>(fixture.worker)
            .is_none()
    );
    assert!(app.world().get::<WorkingOn>(fixture.worker).is_none());
    assert_eq!(
        app.world().resource::<FinalizerReceipts>().commits[0].result,
        DeconstructionCommitResult::StaleTarget
    );
}

#[test]
fn missing_pending_marker_discards_the_order_instead_of_arming_a_dead_blocker() {
    let mut app = test_app();
    let fixture = spawn_fixture(&mut app, BuildingType::SandPile);
    app.world_mut()
        .entity_mut(fixture.target)
        .remove::<DeconstructionPending>();
    app.world_mut().write_message(commit_request(fixture, 0));

    app.update();

    assert!(app.world().get_entity(fixture.target).is_ok());
    assert!(app.world().get_entity(fixture.order).is_err());
    assert!(matches!(
        app.world().get::<AssignedTask>(fixture.worker),
        Some(AssignedTask::None)
    ));
    assert_eq!(
        app.world().resource::<FinalizerReceipts>().commits[0].result,
        DeconstructionCommitResult::StaleTarget
    );
}

#[test]
fn invalid_foreign_pending_is_removed_while_the_stale_order_is_discarded() {
    let mut app = test_app();
    let fixture = spawn_fixture(&mut app, BuildingType::SandPile);
    let invalid_order = app.world_mut().spawn_empty().id();
    app.world_mut()
        .entity_mut(fixture.target)
        .insert(DeconstructionPending {
            order: invalid_order,
        });
    app.world_mut().write_message(commit_request(fixture, 0));

    app.update();

    assert!(app.world().get_entity(fixture.target).is_ok());
    assert!(app.world().get_entity(fixture.order).is_err());
    assert!(
        app.world()
            .get::<DeconstructionPending>(fixture.target)
            .is_none()
    );
    assert!(matches!(
        app.world().get::<AssignedTask>(fixture.worker),
        Some(AssignedTask::None)
    ));
}

#[test]
fn valid_foreign_pending_survives_discard_of_a_stale_competing_order() {
    let mut app = test_app();
    let fixture = spawn_fixture(&mut app, BuildingType::SandPile);
    let position = WorldMap::grid_to_world(fixture.grid.0, fixture.grid.1);
    let canonical_order = app
        .world_mut()
        .spawn((
            DeconstructionOrder,
            Designation {
                work_type: WorkType::Deconstruct,
            },
            TaskSlots::new(1),
            TargetDeconstructionRoot(fixture.target),
            Transform::from_translation(position.extend(0.0)),
        ))
        .id();
    app.world_mut().flush();
    app.world_mut()
        .entity_mut(fixture.target)
        .insert(DeconstructionPending {
            order: canonical_order,
        });
    app.world_mut().write_message(commit_request(fixture, 0));

    app.update();

    assert!(app.world().get_entity(fixture.target).is_ok());
    assert!(app.world().get_entity(fixture.order).is_err());
    assert!(app.world().get_entity(canonical_order).is_ok());
    assert_eq!(
        app.world()
            .get::<DeconstructionPending>(fixture.target)
            .map(|pending| pending.order),
        Some(canonical_order)
    );
    assert!(matches!(
        app.world().get::<AssignedTask>(fixture.worker),
        Some(AssignedTask::None)
    ));
}

#[test]
fn stale_request_after_retarget_aborts_old_worker_without_blocking_current_order() {
    let mut app = test_app();
    let fixture = spawn_fixture(&mut app, BuildingType::SandPile);
    let new_grid = (16, 17);
    let new_position = WorldMap::grid_to_world(new_grid.0, new_grid.1);
    let new_target = app
        .world_mut()
        .spawn((
            Building {
                kind: BuildingType::SandPile,
                is_provisional: false,
            },
            SandPile,
            Transform::from_translation(new_position.extend(0.0)),
        ))
        .id();
    app.world_mut()
        .entity_mut(fixture.target)
        .remove::<DeconstructionPending>();
    app.world_mut()
        .entity_mut(fixture.order)
        .insert(TargetDeconstructionRoot(new_target))
        .insert(Transform::from_translation(new_position.extend(0.0)));
    app.world_mut()
        .entity_mut(new_target)
        .insert(DeconstructionPending {
            order: fixture.order,
        });
    app.world_mut()
        .resource_mut::<WorldMap>()
        .set_building_occupancy(new_grid, new_target);
    app.world_mut().write_message(commit_request(fixture, 0));

    app.update();

    assert!(app.world().get_entity(fixture.order).is_ok());
    assert!(app.world().get_entity(new_target).is_ok());
    assert!(
        app.world()
            .get::<DeconstructionBlocker>(fixture.order)
            .is_none(),
        "a stale request must not poison the retargeted durable order"
    );
    assert!(matches!(
        app.world().get::<AssignedTask>(fixture.worker),
        Some(AssignedTask::None)
    ));
    assert_eq!(
        app.world().resource::<FinalizerReceipts>().commits[0].result,
        DeconstructionCommitResult::StaleTarget
    );
}

#[test]
fn owner_mismatch_releases_preexisting_claim_and_blocks_without_clearing_other_owner() {
    let mut app = test_app();
    add_diagnostic_revision_systems(&mut app);
    let fixture = spawn_fixture(&mut app, BuildingType::SandPile);
    let other_owner = app.world_mut().spawn_empty().id();
    app.world_mut()
        .entity_mut(fixture.target)
        .insert(DeconstructionCommitClaim {
            world_epoch: 0,
            order: fixture.order,
        });
    app.world_mut()
        .resource_mut::<WorldMap>()
        .set_building_occupancy(fixture.grid, other_owner);
    app.world_mut().write_message(commit_request(fixture, 0));

    app.update();

    assert!(app.world().get_entity(fixture.order).is_ok());
    assert!(app.world().get_entity(fixture.target).is_ok());
    assert!(
        app.world()
            .get::<DeconstructionCommitClaim>(fixture.target)
            .is_none()
    );
    assert!(matches!(
        app.world().get::<AssignedTask>(fixture.worker),
        Some(AssignedTask::None)
    ));
    assert_eq!(
        app.world()
            .resource::<WorldMap>()
            .building_entity(fixture.grid),
        Some(other_owner)
    );
    let blocker = app
        .world()
        .get::<DeconstructionBlocker>(fixture.order)
        .expect("owner mismatch must block the durable order");
    assert!(blocker.active);
    assert!(blocker.stamp.is_some());
    assert_eq!(
        app.world().resource::<FinalizerReceipts>().commits[0].result,
        DeconstructionCommitResult::OwnerMismatch
    );

    app.world_mut()
        .resource_mut::<WorldMap>()
        .set_building_occupancy(fixture.grid, fixture.target);
    app.update();

    assert!(
        app.world()
            .get::<DeconstructionBlocker>(fixture.order)
            .is_some_and(|blocker| !blocker.active),
        "owner repair before the first post-failure sync must wake the blocker"
    );
}

#[test]
fn owner_mismatch_blocker_ignores_only_its_known_worker_cleanup_revision() {
    let mut app = test_app();
    add_diagnostic_revision_systems(&mut app);
    let fixture = spawn_fixture(&mut app, BuildingType::SandPile);
    let other_owner = app.world_mut().spawn_empty().id();
    app.world_mut()
        .resource_mut::<WorldMap>()
        .set_building_occupancy(fixture.grid, other_owner);
    app.world_mut().write_message(commit_request(fixture, 0));

    app.update();
    app.update();

    assert!(
        app.world()
            .get::<DeconstructionBlocker>(fixture.order)
            .is_some_and(|blocker| blocker.active),
        "the predicted TaskWorkers cleanup bump must not cause immediate reassignment"
    );
    assert!(matches!(
        app.world().get::<AssignedTask>(fixture.worker),
        Some(AssignedTask::None)
    ));
}

#[test]
fn no_safe_recovery_waits_on_both_topology_and_availability() {
    let mut app = test_app();
    add_diagnostic_revision_systems(&mut app);
    let fixture = spawn_fixture(&mut app, BuildingType::BonePile);
    let stockpile_owner = app
        .world_mut()
        .spawn(Stockpile {
            capacity: 1,
            resource_type: None,
        })
        .id();
    {
        let mut map = app.world_mut().resource_mut::<WorldMap>();
        for y in 0..hw_core::constants::MAP_HEIGHT {
            for x in 0..hw_core::constants::MAP_WIDTH {
                if (x, y) != fixture.grid {
                    map.set_stockpile((x, y), stockpile_owner);
                }
            }
        }
    }
    app.world_mut().write_message(commit_request(fixture, 0));

    app.update();

    assert!(app.world().get_entity(fixture.target).is_ok());
    assert!(app.world().get_entity(fixture.order).is_ok());
    let blocker = app
        .world()
        .get::<DeconstructionBlocker>(fixture.order)
        .expect("missing recovery cell must block the durable order");
    assert_eq!(blocker.reason, DeconstructionBlockReason::NoSafeRecovery);
    assert!(blocker.domains.contains(TaskDiagnosticDomainMask::TOPOLOGY));
    assert!(
        blocker
            .domains
            .contains(TaskDiagnosticDomainMask::AVAILABILITY)
    );
    assert_eq!(
        app.world().resource::<FinalizerReceipts>().commits[0].result,
        DeconstructionCommitResult::NoSafeRecovery
    );

    app.world_mut()
        .resource_mut::<WorldMap>()
        .clear_stockpile((fixture.grid.0 - 1, fixture.grid.1));
    app.world_mut()
        .entity_mut(stockpile_owner)
        .remove::<Stockpile>();
    app.update();

    assert!(
        app.world()
            .get::<DeconstructionBlocker>(fixture.order)
            .is_some_and(|blocker| !blocker.active),
        "a recovery-cell availability change before first refresh must wake the blocker"
    );
}

#[test]
fn no_safe_recovery_blocker_ignores_its_own_empty_inventory_cleanup() {
    let mut app = test_app();
    add_diagnostic_revision_systems(&mut app);
    let fixture = spawn_fixture(&mut app, BuildingType::BonePile);
    let stockpile_owner = app
        .world_mut()
        .spawn(Stockpile {
            capacity: 1,
            resource_type: None,
        })
        .id();
    {
        let mut map = app.world_mut().resource_mut::<WorldMap>();
        for y in 0..hw_core::constants::MAP_HEIGHT {
            for x in 0..hw_core::constants::MAP_WIDTH {
                if (x, y) != fixture.grid {
                    map.set_stockpile((x, y), stockpile_owner);
                }
            }
        }
    }
    app.world_mut().write_message(commit_request(fixture, 0));

    app.update();
    app.update();

    assert!(
        app.world()
            .get::<DeconstructionBlocker>(fixture.order)
            .is_some_and(|blocker| blocker.active),
        "empty Inventory cleanup must not fabricate an availability change"
    );
    assert!(matches!(
        app.world().get::<AssignedTask>(fixture.worker),
        Some(AssignedTask::None)
    ));
}

#[test]
fn same_batch_cancel_wins_before_commit_without_removing_target() {
    let mut app = test_app();
    let fixture = spawn_fixture(&mut app, BuildingType::BonePile);
    app.world_mut().write_message(DeconstructionCancelRequest {
        world_epoch: 0,
        order: fixture.order,
    });
    app.world_mut().write_message(commit_request(fixture, 0));

    app.update();

    assert!(app.world().get_entity(fixture.order).is_err());
    assert!(app.world().get_entity(fixture.target).is_ok());
    assert!(
        app.world()
            .get::<DeconstructionPending>(fixture.target)
            .is_none()
    );
    assert!(matches!(
        app.world().get::<AssignedTask>(fixture.worker),
        Some(AssignedTask::None)
    ));
    assert!(
        app.world_mut()
            .query::<&ResourceItem>()
            .iter(app.world())
            .next()
            .is_none()
    );
    let receipts = app.world().resource::<FinalizerReceipts>();
    assert_eq!(
        receipts.cancels[0].result,
        DeconstructionCancelResult::Canceled
    );
    assert_eq!(
        receipts.commits[0].result,
        DeconstructionCommitResult::Canceled
    );
}

#[test]
fn cancel_only_aborts_order_workers_and_preserves_target_work() {
    let mut app = test_app();
    let fixture = spawn_fixture(&mut app, BuildingType::BonePile);
    let collect_assignment = app.world_mut().spawn_empty().id();
    let collector_identity =
        ActiveTaskIdentity::new(collect_assignment, fixture.target, WorkType::CollectBone);
    let collector = app
        .world_mut()
        .spawn((
            Transform::default(),
            DamnedSoul::default(),
            AssignedTask::CollectBone(hw_jobs::CollectBoneData {
                target: fixture.target,
                phase: hw_jobs::CollectBonePhase::GoingToBone,
            }),
            Path::default(),
            Inventory::default(),
            collector_identity,
            WorkingOn(fixture.target),
        ))
        .id();
    app.world_mut().flush();
    app.world_mut().write_message(DeconstructionCancelRequest {
        world_epoch: 0,
        order: fixture.order,
    });

    app.update();

    assert!(app.world().get_entity(fixture.target).is_ok());
    assert!(app.world().get_entity(fixture.order).is_err());
    assert!(matches!(
        app.world().get::<AssignedTask>(collector),
        Some(AssignedTask::CollectBone(_))
    ));
    assert_eq!(
        app.world()
            .get::<WorkingOn>(collector)
            .map(|working| working.0),
        Some(fixture.target)
    );
}

#[test]
fn cancel_refuses_a_target_with_an_acquired_claim() {
    let mut app = test_app();
    let fixture = spawn_fixture(&mut app, BuildingType::SandPile);
    app.world_mut()
        .entity_mut(fixture.target)
        .insert(DeconstructionCommitClaim {
            world_epoch: 0,
            order: fixture.order,
        });
    app.world_mut().write_message(DeconstructionCancelRequest {
        world_epoch: 0,
        order: fixture.order,
    });

    app.update();

    assert!(app.world().get_entity(fixture.order).is_ok());
    assert!(app.world().get_entity(fixture.target).is_ok());
    assert!(matches!(
        app.world().get::<AssignedTask>(fixture.worker),
        Some(AssignedTask::Deconstruct(_))
    ));
    assert_eq!(
        app.world().resource::<FinalizerReceipts>().cancels[0].result,
        DeconstructionCancelResult::ClaimInProgress
    );
}

#[test]
fn exact_preexisting_claim_resumes_the_same_commit_owner() {
    let mut app = test_app();
    let fixture = spawn_fixture(&mut app, BuildingType::SandPile);
    app.world_mut()
        .entity_mut(fixture.target)
        .insert(DeconstructionCommitClaim {
            world_epoch: 0,
            order: fixture.order,
        });
    app.world_mut().write_message(commit_request(fixture, 0));

    app.update();

    assert!(app.world().get_entity(fixture.target).is_err());
    assert!(app.world().get_entity(fixture.order).is_err());
    assert_eq!(
        app.world().resource::<FinalizerReceipts>().commits[0].result,
        DeconstructionCommitResult::Committed
    );
    assert_eq!(
        app.world().resource::<FinalizerReceipts>().completed.len(),
        1
    );
}

#[test]
fn successful_commit_aborts_an_existing_collect_source_and_releases_its_reservation() {
    let mut app = test_app();
    let fixture = spawn_fixture(&mut app, BuildingType::BonePile);
    let collect_assignment = app.world_mut().spawn_empty().id();
    let collector_identity =
        ActiveTaskIdentity::new(collect_assignment, fixture.target, WorkType::CollectBone);
    let collector = app
        .world_mut()
        .spawn((
            Transform::default(),
            DamnedSoul::default(),
            AssignedTask::CollectBone(hw_jobs::CollectBoneData {
                target: fixture.target,
                phase: hw_jobs::CollectBonePhase::Done,
            }),
            Path::default(),
            Inventory::default(),
            collector_identity,
            WorkingOn(fixture.target),
        ))
        .id();
    app.world_mut().flush();
    app.world_mut()
        .resource_mut::<SharedResourceCache>()
        .reserve_source(fixture.target, 1);
    app.world_mut().write_message(commit_request(fixture, 0));

    app.update();

    assert!(matches!(
        app.world().get::<AssignedTask>(collector),
        Some(AssignedTask::None)
    ));
    let releases = app
        .world()
        .resource::<Messages<ResourceReservationRequest>>()
        .iter_current_update_messages()
        .filter(|request| {
            request.op
                == hw_core::events::ResourceReservationOp::ReleaseSource {
                    source: fixture.target,
                    amount: 1,
                }
        })
        .count();
    assert_eq!(releases, 1);
    assert_eq!(
        app.world()
            .resource::<SharedResourceCache>()
            .get_source_reservation(fixture.target),
        0,
        "destroyed owners must not retain a reservation until the next frame"
    );
}

#[test]
fn successful_commit_closes_transport_requests_referencing_the_target() {
    let mut app = test_app();
    let fixture = spawn_fixture(&mut app, BuildingType::SandPile);
    let issuer = app.world_mut().spawn_empty().id();
    app.world_mut()
        .entity_mut(fixture.target)
        .insert(ManualHaulPinnedSource);
    let request = app
        .world_mut()
        .spawn((
            TransportRequest {
                kind: TransportRequestKind::DepositToStockpile,
                anchor: issuer,
                resource_type: ResourceType::Sand,
                issued_by: issuer,
                priority: TransportPriority::Normal,
                stockpile_group: Vec::new(),
            },
            TransportRequestFixedSource(fixture.target),
        ))
        .id();
    let item = app.world_mut().spawn_empty().id();
    let request_identity = ActiveTaskIdentity::new(request, request, WorkType::Haul);
    let transport_worker = app
        .world_mut()
        .spawn((
            Transform::default(),
            DamnedSoul::default(),
            AssignedTask::Haul(HaulData {
                item,
                stockpile: issuer,
                phase: HaulPhase::GoingToItem,
            }),
            Path::default(),
            Inventory::default(),
            request_identity,
            WorkingOn(request),
        ))
        .id();
    app.world_mut().flush();
    assert_eq!(
        app.world()
            .get::<hw_core::relationships::TaskWorkers>(request)
            .map(hw_core::relationships::TaskWorkers::len),
        Some(1)
    );
    app.world_mut().write_message(commit_request(fixture, 0));

    app.update();

    assert!(app.world().get_entity(request).is_err());
    assert!(app.world().get_entity(fixture.target).is_err());
    assert!(matches!(
        app.world().get::<AssignedTask>(transport_worker),
        Some(AssignedTask::None)
    ));
    assert_eq!(
        app.world().resource::<FinalizerReceipts>().commits[0].result,
        DeconstructionCommitResult::Committed
    );
}

#[test]
fn tank_commit_recovers_water_and_bucket_tools_before_removing_companions() {
    let mut app = test_app();
    let fixture = spawn_facility_fixture(&mut app, BuildingType::Tank);
    let storage_grids = [(9, 13), (10, 13)];
    let mut storages = Vec::new();
    for grid in storage_grids {
        let storage = app
            .world_mut()
            .spawn((
                BucketStorage,
                BelongsTo(fixture.target),
                Stockpile {
                    capacity: 10,
                    resource_type: None,
                },
                Transform::from_translation(WorldMap::grid_to_world(grid.0, grid.1).extend(0.0)),
            ))
            .id();
        app.world_mut()
            .resource_mut::<WorldMap>()
            .set_stockpile(grid, storage);
        storages.push(storage);
    }
    let water = app
        .world_mut()
        .spawn((
            ResourceItem(ResourceType::Water),
            StoredIn(fixture.target),
            Visibility::Hidden,
            Transform::default(),
        ))
        .id();
    let buckets = storages
        .iter()
        .enumerate()
        .map(|(index, storage)| {
            app.world_mut()
                .spawn((
                    ResourceItem(if index == 0 {
                        ResourceType::BucketEmpty
                    } else {
                        ResourceType::BucketWater
                    }),
                    BelongsTo(fixture.target),
                    StoredIn(*storage),
                    Visibility::Hidden,
                    Transform::default(),
                ))
                .id()
        })
        .collect::<Vec<_>>();
    let issuer = app.world_mut().spawn_empty().id();
    let companion_request = app
        .world_mut()
        .spawn((
            TransportRequest {
                kind: TransportRequestKind::ReturnBucket,
                anchor: fixture.target,
                resource_type: ResourceType::BucketEmpty,
                issued_by: issuer,
                priority: TransportPriority::Normal,
                stockpile_group: storages.clone(),
            },
            TransportRequestFixedSource(storages[0]),
        ))
        .id();
    app.world_mut().flush();
    app.world_mut().write_message(commit_request(fixture, 0));

    app.update();

    assert!(app.world().get_entity(fixture.target).is_err());
    assert!(app.world().get_entity(companion_request).is_err());
    for storage in storages {
        assert!(app.world().get_entity(storage).is_err());
    }
    for item in std::iter::once(water).chain(buckets) {
        assert!(app.world().get_entity(item).is_ok());
        assert!(app.world().get::<StoredIn>(item).is_none());
        assert!(app.world().get::<BelongsTo>(item).is_none());
        assert_eq!(
            app.world().get::<Visibility>(item),
            Some(&Visibility::Visible)
        );
        let grid = WorldMap::world_to_grid(
            app.world()
                .get::<Transform>(item)
                .unwrap()
                .translation
                .truncate(),
        );
        assert!(![(12, 13), (13, 13), (12, 14), (13, 14)].contains(&grid));
    }
    let map = app.world().resource::<WorldMap>();
    assert!(
        storage_grids
            .iter()
            .all(|grid| map.stockpile_entity(*grid).is_none())
    );
    assert_eq!(
        app.world().resource::<FinalizerReceipts>().commits[0].result,
        DeconstructionCommitResult::Committed
    );
}

#[test]
fn rest_area_commit_releases_occupants_and_reservations_through_owner_lifecycle() {
    let mut app = test_app();
    let fixture = spawn_facility_fixture(&mut app, BuildingType::RestArea);
    let occupant = app
        .world_mut()
        .spawn((
            RestingIn(fixture.target),
            IdleState {
                behavior: IdleBehavior::Resting,
                ..default()
            },
            Path {
                waypoints: vec![Vec2::ONE],
                ..default()
            },
            Visibility::Hidden,
        ))
        .id();
    let reserved = app
        .world_mut()
        .spawn((
            RestAreaReservedFor(fixture.target),
            IdleState {
                behavior: IdleBehavior::GoingToRest,
                ..default()
            },
            Path::default(),
            Visibility::Visible,
        ))
        .id();
    app.world_mut().flush();
    app.world_mut().write_message(commit_request(fixture, 0));

    app.update();

    assert!(app.world().get_entity(fixture.target).is_err());
    for soul in [occupant, reserved] {
        assert!(app.world().get::<RestingIn>(soul).is_none());
        assert!(app.world().get::<RestAreaReservedFor>(soul).is_none());
        assert!(app.world().get::<RestAreaCooldown>(soul).is_some());
        assert_eq!(
            app.world().get::<IdleState>(soul).unwrap().behavior,
            IdleBehavior::Wandering
        );
        assert_eq!(
            app.world().get::<Visibility>(soul),
            Some(&Visibility::Visible)
        );
    }
}

#[test]
fn parking_commit_unparks_wheelbarrows_without_unloading_volatile_cargo() {
    let mut app = test_app();
    let fixture = spawn_facility_fixture(&mut app, BuildingType::WheelbarrowParking);
    let wheelbarrow = app
        .world_mut()
        .spawn((
            ResourceItem(ResourceType::Wheelbarrow),
            Wheelbarrow { capacity: 8 },
            BelongsTo(fixture.target),
            ParkedAt(fixture.target),
            LoadedItems::default(),
            Visibility::Hidden,
            Transform::default(),
        ))
        .id();
    let sand = app
        .world_mut()
        .spawn((
            ResourceItem(ResourceType::Sand),
            LoadedIn(wheelbarrow),
            Visibility::Hidden,
            Transform::default(),
        ))
        .id();
    let mud = app
        .world_mut()
        .spawn((
            ResourceItem(ResourceType::StasisMud),
            LoadedIn(wheelbarrow),
            Visibility::Hidden,
            Transform::default(),
        ))
        .id();
    app.world_mut().flush();
    app.world_mut().write_message(commit_request(fixture, 0));

    app.update();

    assert!(app.world().get_entity(fixture.target).is_err());
    assert!(app.world().get_entity(wheelbarrow).is_ok());
    assert!(app.world().get::<ParkedAt>(wheelbarrow).is_none());
    assert!(app.world().get::<BelongsTo>(wheelbarrow).is_none());
    assert_eq!(
        app.world().get::<LoadedIn>(sand).map(|owner| owner.0),
        Some(wheelbarrow)
    );
    assert_eq!(
        app.world().get::<LoadedIn>(mud).map(|owner| owner.0),
        Some(wheelbarrow)
    );
    assert_eq!(
        app.world().get::<Visibility>(wheelbarrow),
        Some(&Visibility::Visible)
    );
}

#[test]
fn parking_commit_preserves_loaded_cargo_while_terminalizing_an_active_haul() {
    let mut app = test_app();
    let fixture = spawn_facility_fixture(&mut app, BuildingType::WheelbarrowParking);
    let destination = app.world_mut().spawn_empty().id();
    let wheelbarrow = app
        .world_mut()
        .spawn((
            ResourceItem(ResourceType::Wheelbarrow),
            Wheelbarrow { capacity: 8 },
            BelongsTo(fixture.target),
            LoadedItems::default(),
            Visibility::Visible,
            Transform::default(),
        ))
        .id();
    let sand = app
        .world_mut()
        .spawn((
            ResourceItem(ResourceType::Sand),
            LoadedIn(wheelbarrow),
            DeliveringTo(destination),
            Visibility::Hidden,
            Transform::default(),
        ))
        .id();
    let mud = app
        .world_mut()
        .spawn((
            ResourceItem(ResourceType::StasisMud),
            LoadedIn(wheelbarrow),
            DeliveringTo(destination),
            Visibility::Hidden,
            Transform::default(),
        ))
        .id();
    let identity =
        ActiveTaskIdentity::new(fixture.target, fixture.target, WorkType::WheelbarrowHaul);
    let worker = app
        .world_mut()
        .spawn((
            Transform::from_translation(WorldMap::grid_to_world(11, 13).extend(0.0)),
            DamnedSoul::default(),
            AssignedTask::HaulWithWheelbarrow(HaulWithWheelbarrowData {
                wheelbarrow,
                source_pos: Vec2::ZERO,
                destination: WheelbarrowDestination::Stockpile(destination),
                collect_source: None,
                collect_amount: 0,
                collect_resource_type: None,
                items: vec![sand, mud],
                phase: HaulWithWheelbarrowPhase::GoingToDestination,
            }),
            Path::default(),
            Inventory(Some(wheelbarrow)),
            identity,
            WorkingOn(fixture.target),
        ))
        .id();
    app.world_mut()
        .entity_mut(wheelbarrow)
        .insert(PushedBy(worker));
    app.world_mut().flush();
    app.world_mut().write_message(commit_request(fixture, 0));

    app.update();

    assert!(app.world().get_entity(fixture.target).is_err());
    assert!(matches!(
        app.world().get::<AssignedTask>(worker),
        Some(AssignedTask::None)
    ));
    assert_eq!(app.world().get::<Inventory>(worker).unwrap().0, None);
    assert!(app.world().get::<PushedBy>(wheelbarrow).is_none());
    for item in [sand, mud] {
        assert_eq!(
            app.world().get::<LoadedIn>(item).map(|owner| owner.0),
            Some(wheelbarrow)
        );
        assert!(app.world().get::<DeliveringTo>(item).is_none());
        assert_eq!(
            app.world().get::<Visibility>(item),
            Some(&Visibility::Hidden)
        );
    }
}

#[test]
fn mixer_commit_transfers_sand_and_existing_mud_entities_and_materializes_only_rock() {
    let mut app = test_app();
    let fixture = spawn_facility_fixture(&mut app, BuildingType::MudMixer);
    *app.world_mut()
        .get_mut::<MudMixerStorage>(fixture.target)
        .unwrap() = MudMixerStorage {
        sand: 2,
        rock: 3,
        mud: 2,
    };
    let target_mud = (0..2)
        .map(|_| {
            app.world_mut()
                .spawn((
                    ResourceItem(ResourceType::StasisMud),
                    StoredByMixer(fixture.target),
                    Visibility::Hidden,
                    Transform::default(),
                ))
                .id()
        })
        .collect::<Vec<_>>();
    let water = app
        .world_mut()
        .spawn((
            ResourceItem(ResourceType::Water),
            StoredIn(fixture.target),
            Visibility::Hidden,
            Transform::default(),
        ))
        .id();
    let receiver_pos = WorldMap::grid_to_world(18, 13);
    let receiver = app
        .world_mut()
        .spawn((
            Building {
                kind: BuildingType::MudMixer,
                is_provisional: false,
            },
            MudMixerStorage {
                sand: 1,
                rock: 0,
                mud: 1,
            },
            Stockpile {
                capacity: hw_core::constants::MUD_MIXER_CAPACITY as usize,
                resource_type: Some(ResourceType::Water),
            },
            Transform::from_translation(receiver_pos.extend(0.0)),
        ))
        .id();
    app.world_mut().spawn((
        ResourceItem(ResourceType::StasisMud),
        StoredByMixer(receiver),
        Visibility::Hidden,
        Transform::default(),
    ));
    app.world_mut().flush();
    app.world_mut().write_message(commit_request(fixture, 0));

    app.update();

    assert!(app.world().get_entity(fixture.target).is_err());
    let storage = app.world().get::<MudMixerStorage>(receiver).unwrap();
    assert_eq!((storage.sand, storage.rock, storage.mud), (3, 0, 3));
    assert!(target_mud.iter().all(|entity| {
        app.world()
            .get::<StoredByMixer>(*entity)
            .is_some_and(|owner| owner.0 == receiver)
    }));
    assert!(app.world().get_entity(water).is_ok());
    assert!(app.world().get::<StoredIn>(water).is_none());
    let mut item_query = app.world_mut().query::<&ResourceItem>();
    let rock_count = item_query
        .iter(app.world())
        .filter(|item| item.0 == ResourceType::Rock)
        .count();
    let wood_count = item_query
        .iter(app.world())
        .filter(|item| item.0 == ResourceType::Wood)
        .count();
    assert_eq!(rock_count, 3);
    assert_eq!(
        wood_count, 2,
        "building salvage stays separate from storage"
    );
}

#[test]
fn mixer_commit_absorbs_volatile_items_held_by_related_souls() {
    let mut app = test_app();
    let fixture = spawn_facility_fixture(&mut app, BuildingType::MudMixer);
    let receiver = app
        .world_mut()
        .spawn((
            Building {
                kind: BuildingType::MudMixer,
                is_provisional: false,
            },
            MudMixerStorage::default(),
            Transform::from_translation(WorldMap::grid_to_world(18, 13).extend(0.0)),
        ))
        .id();
    let sand = app
        .world_mut()
        .spawn((
            ResourceItem(ResourceType::Sand),
            DeliveringTo(fixture.target),
            Visibility::Hidden,
            Transform::default(),
        ))
        .id();
    let mud = app
        .world_mut()
        .spawn((
            ResourceItem(ResourceType::StasisMud),
            DeliveringTo(fixture.target),
            Visibility::Hidden,
            Transform::default(),
        ))
        .id();
    let mut workers = Vec::new();
    for (item, resource_type) in [(sand, ResourceType::Sand), (mud, ResourceType::StasisMud)] {
        let identity =
            ActiveTaskIdentity::new(fixture.target, fixture.target, WorkType::HaulToMixer);
        workers.push(
            app.world_mut()
                .spawn((
                    Transform::from_translation(WorldMap::grid_to_world(11, 13).extend(0.0)),
                    DamnedSoul::default(),
                    AssignedTask::HaulToMixer(HaulToMixerData {
                        item,
                        mixer: fixture.target,
                        resource_type,
                        phase: HaulToMixerPhase::GoingToMixer,
                    }),
                    Path::default(),
                    Inventory(Some(item)),
                    identity,
                    WorkingOn(fixture.target),
                ))
                .id(),
        );
    }
    app.world_mut().flush();
    app.world_mut().write_message(commit_request(fixture, 0));

    app.update();

    assert!(app.world().get_entity(fixture.target).is_err());
    assert!(app.world().get_entity(sand).is_err());
    assert_eq!(
        app.world().get::<StoredByMixer>(mud).map(|owner| owner.0),
        Some(receiver)
    );
    let storage = app.world().get::<MudMixerStorage>(receiver).unwrap();
    assert_eq!((storage.sand, storage.mud), (1, 1));
    for worker in workers {
        assert!(matches!(
            app.world().get::<AssignedTask>(worker),
            Some(AssignedTask::None)
        ));
        assert_eq!(app.world().get::<Inventory>(worker).unwrap().0, None);
    }
}

#[test]
fn malformed_receiver_mixer_is_skipped_when_a_valid_receiver_exists() {
    let mut app = test_app();
    let fixture = spawn_facility_fixture(&mut app, BuildingType::MudMixer);
    app.world_mut()
        .get_mut::<MudMixerStorage>(fixture.target)
        .unwrap()
        .sand = 1;

    let malformed_receiver = app
        .world_mut()
        .spawn((
            Building {
                kind: BuildingType::MudMixer,
                is_provisional: false,
            },
            MudMixerStorage {
                sand: 0,
                rock: 0,
                mud: 1,
            },
            Transform::from_translation(WorldMap::grid_to_world(15, 13).extend(0.0)),
        ))
        .id();
    let valid_receiver = app
        .world_mut()
        .spawn((
            Building {
                kind: BuildingType::MudMixer,
                is_provisional: false,
            },
            MudMixerStorage::default(),
            Transform::from_translation(WorldMap::grid_to_world(18, 13).extend(0.0)),
        ))
        .id();
    app.world_mut().flush();
    app.world_mut().write_message(commit_request(fixture, 0));

    app.update();

    assert!(app.world().get_entity(fixture.target).is_err());
    assert_eq!(
        app.world()
            .get::<MudMixerStorage>(malformed_receiver)
            .unwrap()
            .sand,
        0
    );
    assert_eq!(
        app.world()
            .get::<MudMixerStorage>(valid_receiver)
            .unwrap()
            .sand,
        1
    );
    assert_eq!(
        app.world().resource::<FinalizerReceipts>().commits[0].result,
        DeconstructionCommitResult::Committed
    );
}

#[test]
fn inconsistent_mixer_inventory_rejects_without_mutating_storage_or_target() {
    let mut app = test_app();
    let fixture = spawn_facility_fixture(&mut app, BuildingType::MudMixer);
    *app.world_mut()
        .get_mut::<MudMixerStorage>(fixture.target)
        .unwrap() = MudMixerStorage {
        sand: 0,
        rock: 0,
        mud: 2,
    };
    let mud = app
        .world_mut()
        .spawn((
            ResourceItem(ResourceType::StasisMud),
            StoredByMixer(fixture.target),
        ))
        .id();
    app.world_mut().flush();
    app.world_mut().write_message(commit_request(fixture, 0));

    app.update();

    assert!(app.world().get_entity(fixture.target).is_ok());
    assert!(app.world().get_entity(mud).is_ok());
    assert_eq!(
        app.world().get::<StoredByMixer>(mud).map(|owner| owner.0),
        Some(fixture.target)
    );
    assert_eq!(
        app.world().resource::<FinalizerReceipts>().commits[0].result,
        DeconstructionCommitResult::InconsistentMixerInventory
    );
    assert_eq!(
        app.world()
            .get::<DeconstructionBlocker>(fixture.order)
            .unwrap()
            .reason,
        DeconstructionBlockReason::InconsistentMixerInventory
    );
}

#[test]
fn volatile_recovery_without_another_mixer_rejects_without_grounding_sand() {
    let mut app = test_app();
    let fixture = spawn_facility_fixture(&mut app, BuildingType::MudMixer);
    app.world_mut()
        .get_mut::<MudMixerStorage>(fixture.target)
        .unwrap()
        .sand = 1;
    app.world_mut().write_message(commit_request(fixture, 0));

    app.update();

    assert!(app.world().get_entity(fixture.target).is_ok());
    assert_eq!(
        app.world()
            .get::<MudMixerStorage>(fixture.target)
            .unwrap()
            .sand,
        1
    );
    assert_eq!(
        app.world().resource::<FinalizerReceipts>().commits[0].result,
        DeconstructionCommitResult::NoSafeRecovery
    );
    let mut items = app.world_mut().query::<&ResourceItem>();
    assert_eq!(
        items
            .iter(app.world())
            .filter(|item| item.0 == ResourceType::Sand)
            .count(),
        0
    );
}

#[cfg(feature = "profiling")]
#[test]
fn profiling_metrics_track_the_actual_commit_without_idle_validation_passes() {
    let mut app = test_app();
    app.init_resource::<DeconstructionPerfMetrics>();
    let fixture = spawn_fixture(&mut app, BuildingType::BonePile);
    app.world_mut().write_message(commit_request(fixture, 0));

    app.update();

    let after_commit = *app.world().resource::<DeconstructionPerfMetrics>();
    assert_eq!(after_commit.commit_validation_passes, 1);
    assert_eq!(after_commit.successful_cleanup_transactions, 1);
    assert_eq!(after_commit.recovery_items_spawned, 5);
    assert!(after_commit.successful_transaction_elapsed_ns > 0);

    app.update();

    assert_eq!(
        *app.world().resource::<DeconstructionPerfMetrics>(),
        after_commit,
        "an empty deconstruction queue must not create a validation pass"
    );
}
