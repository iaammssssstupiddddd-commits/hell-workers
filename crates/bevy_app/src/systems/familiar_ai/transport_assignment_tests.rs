use super::perceive::resource_sync::{
    ReservationSignatureCache, ReservationSyncTimer, sync_reservations_system,
};
use bevy::prelude::*;
use hw_core::constants::{TILE_SIZE, WHEELBARROW_CAPACITY};
use hw_core::events::{
    OnTaskAbandoned, OnTaskAssigned, ResourceReservationRequest, SoulTaskUnassignRequest,
};
use hw_core::familiar::{Familiar, FamiliarOperation, FamiliarPolicy};
use hw_core::relationships::{CommandedBy, ManagedBy, ParkedAt, PushedBy};
use hw_core::soul::{DamnedSoul, Destination, IdleState, Path};
use hw_familiar_ai::FamiliarTaskCandidateDiagnostics;
use hw_familiar_ai::familiar_ai::decide::resources::FamiliarTaskDelegationTimer;
use hw_familiar_ai::familiar_ai::decide::task_delegation::familiar_task_delegation_cycle_system;
use hw_jobs::events::TaskAssignmentRequest;
use hw_jobs::mud_mixer::MudMixerStorage;
use hw_jobs::{
    AssignedTask, Blueprint, BuildingType, Designation, Priority, SandPile,
    TaskDiagnosticInputRevisions, TaskSlots, WorkType,
};
use hw_logistics::tile_index::TileSiteIndex;
use hw_logistics::transport_request::arbitration::WheelbarrowArbitrationRuntime;
use hw_logistics::transport_request::{
    TransportDemand, TransportPriority, TransportRequest, TransportRequestKind,
    TransportRequestState, WheelbarrowArbitrationDiagnostics, WheelbarrowArbitrationMetrics,
    wheelbarrow_arbitration_system,
};
use hw_logistics::{
    BelongsTo, Inventory, ResourceItem, ResourceType, SharedResourceCache, Wheelbarrow,
    apply_reservation_requests_system,
};
use hw_soul_ai::soul_ai::execute::task_assignment_apply::apply_task_assignment_requests_system;
use hw_soul_ai::soul_ai::execute::task_unassign_apply::handle_soul_task_unassign_system;
use hw_spatial::{
    DesignationSpatialGrid, ResourceSpatialGrid, SpatialGridOps, TransportRequestSpatialGrid,
};
use hw_world::{WalkabilityConnectivityCache, WorldMap};

fn transport_fixture() -> (App, Entity, Entity, [Entity; 2]) {
    // This fixture isolates the production assignment/reservation/interrupt
    // boundary. It does not simulate recruitment, movement, or construction.
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .init_resource::<FamiliarTaskDelegationTimer>()
        .init_resource::<DesignationSpatialGrid>()
        .init_resource::<TransportRequestSpatialGrid>()
        .init_resource::<ResourceSpatialGrid>()
        .init_resource::<TileSiteIndex>()
        .init_resource::<WorldMap>()
        .init_resource::<WalkabilityConnectivityCache>()
        .init_resource::<SharedResourceCache>()
        .init_resource::<WheelbarrowArbitrationRuntime>()
        .init_resource::<WheelbarrowArbitrationMetrics>()
        .init_resource::<WheelbarrowArbitrationDiagnostics>()
        .init_resource::<TaskDiagnosticInputRevisions>()
        .init_resource::<FamiliarTaskCandidateDiagnostics>()
        .init_resource::<ReservationSignatureCache>()
        .init_resource::<ReservationSyncTimer>()
        .insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
            std::time::Duration::from_millis(250),
        ))
        .add_message::<ResourceReservationRequest>()
        .add_message::<TaskAssignmentRequest>()
        .add_message::<OnTaskAssigned>()
        .add_message::<OnTaskAbandoned>()
        .add_message::<SoulTaskUnassignRequest>()
        .add_systems(
            Update,
            (
                handle_soul_task_unassign_system,
                ApplyDeferred,
                apply_reservation_requests_system,
                sync_reservations_system,
                wheelbarrow_arbitration_system,
                ApplyDeferred,
                familiar_task_delegation_cycle_system,
                apply_task_assignment_requests_system,
                ApplyDeferred,
            )
                .chain(),
        );
    let pos = WorldMap::grid_to_world(30, 30);
    let familiar = app
        .world_mut()
        .spawn((
            Familiar::default(),
            FamiliarOperation::default(),
            FamiliarPolicy::default(),
            Transform::from_translation(pos.extend(0.0)),
        ))
        .id();
    for offset in [-1.0, 1.0] {
        app.world_mut().spawn((
            Transform::from_translation((pos + Vec2::new(offset * TILE_SIZE, 0.0)).extend(0.0)),
            DamnedSoul::default(),
            AssignedTask::None,
            Destination(pos),
            Path::default(),
            IdleState::default(),
            CommandedBy(familiar),
            Inventory::default(),
            Visibility::Visible,
        ));
    }
    let parking = app
        .world_mut()
        .spawn(Transform::from_translation(pos.extend(0.0)))
        .id();
    for offset in [-8.0, 8.0] {
        app.world_mut().spawn((
            ResourceItem(ResourceType::Wheelbarrow),
            Wheelbarrow {
                capacity: WHEELBARROW_CAPACITY,
            },
            BelongsTo(parking),
            ParkedAt(parking),
            Transform::from_translation((pos + Vec2::splat(offset)).extend(0.0)),
            Visibility::Visible,
        ));
    }
    let mixer_pos = pos + Vec2::new(TILE_SIZE * 4.0, 0.0);
    let mixer = app
        .world_mut()
        .spawn((
            Transform::from_translation(mixer_pos.extend(0.0)),
            MudMixerStorage::default(),
        ))
        .id();
    app.world_mut().spawn((
        SandPile,
        Transform::from_translation((pos + Vec2::Y * TILE_SIZE * 3.0).extend(0.0)),
    ));
    let blueprint_pos = pos - Vec2::X * TILE_SIZE * 4.0;
    let mut blueprint = Blueprint::new(
        BuildingType::Wall,
        vec![WorldMap::world_to_grid(blueprint_pos)],
    );
    blueprint.required_materials = [(ResourceType::StasisMud, 5)].into();
    let blueprint = app
        .world_mut()
        .spawn((
            blueprint,
            Transform::from_translation(blueprint_pos.extend(0.0)),
        ))
        .id();
    for offset in 0..5 {
        let item_pos = pos + Vec2::Y * (TILE_SIZE * 2.0 + offset as f32);
        let item = app
            .world_mut()
            .spawn((
                ResourceItem(ResourceType::StasisMud),
                Visibility::Visible,
                Transform::from_translation(item_pos.extend(0.0)),
            ))
            .id();
        app.world_mut()
            .resource_mut::<ResourceSpatialGrid>()
            .insert(item, item_pos);
    }
    let tasks = [
        (
            mixer,
            mixer_pos,
            ResourceType::Sand,
            WorkType::HaulToMixer,
            TransportRequestKind::DeliverToMixerSolid,
            0,
        ),
        (
            blueprint,
            blueprint_pos,
            ResourceType::StasisMud,
            WorkType::Haul,
            TransportRequestKind::DeliverToBlueprint,
            10,
        ),
    ]
    .map(
        |(anchor, target_pos, resource_type, work_type, kind, priority)| {
            app.world_mut()
                .spawn((
                    Transform::from_translation(target_pos.extend(0.0)),
                    ManagedBy(familiar),
                    Designation { work_type },
                    TaskSlots::new(1),
                    Priority(priority),
                    TransportRequest {
                        kind,
                        anchor,
                        resource_type,
                        issued_by: familiar,
                        priority: TransportPriority::Normal,
                        stockpile_group: vec![],
                    },
                    TransportDemand {
                        desired_slots: 5,
                        inflight: 0,
                    },
                    TransportRequestState::Pending,
                ))
                .id()
        },
    );
    app.world_mut().flush();
    (app, parking, mixer, tasks)
}

#[test]
fn pending_sand_request_can_fill_last_mixer_slot() {
    let (mut app, _, mixer, tasks) = transport_fixture();
    app.world_mut().entity_mut(tasks[1]).remove::<Designation>();
    app.world_mut()
        .get_mut::<MudMixerStorage>(mixer)
        .unwrap()
        .sand = hw_core::constants::MUD_MIXER_CAPACITY - 1;
    app.world_mut()
        .get_mut::<TransportDemand>(tasks[0])
        .unwrap()
        .desired_slots = 1;
    app.update();
    let assignments = app.world().resource::<Messages<TaskAssignmentRequest>>();
    let request = assignments
        .iter_current_update_messages()
        .find(|request| request.task_entity == tasks[0])
        .expect("a pending request must not reserve its own last free mixer slot");
    let AssignedTask::HaulWithWheelbarrow(data) = &request.assigned_task else {
        panic!("sand must use an available initial wheelbarrow");
    };
    assert_eq!(data.collect_amount, 1);
}

#[test]
fn sand_collection_keeps_mixer_capacity_reserved_before_loading() {
    let (mut app, _, mixer, tasks) = transport_fixture();
    app.world_mut().entity_mut(tasks[1]).remove::<Designation>();
    // Leave another eligible worker/source available on the next cycle.
    app.world_mut().get_mut::<TaskSlots>(tasks[0]).unwrap().max = 2;
    let pos = WorldMap::grid_to_world(34, 34);
    app.world_mut()
        .spawn((SandPile, Transform::from_translation(pos.extend(0.0))));
    app.update();
    let reserved = app
        .world()
        .resource::<SharedResourceCache>()
        .get_mixer_destination_reservation(mixer, ResourceType::Sand);
    assert_eq!(reserved, hw_core::constants::MUD_MIXER_CAPACITY as usize);
    app.update();
    assert_eq!(
        app.world()
            .resource::<SharedResourceCache>()
            .get_mixer_destination_reservation(mixer, ResourceType::Sand),
        reserved,
        "reservation sync must retain sand that will be collected later"
    );
    app.update();
    let active = app
        .world_mut()
        .query::<&AssignedTask>()
        .iter(app.world())
        .filter(|task| matches!(task, AssignedTask::HaulWithWheelbarrow(_)))
        .count();
    assert_eq!(active, 1, "a second cycle must not overbook the full mixer");
}

#[test]
fn initial_wheelbarrows_can_be_reassigned_after_transport_interruption() {
    let (mut app, parking, mixer, tasks) = transport_fixture();
    app.update();
    let requests = app
        .world()
        .resource::<Messages<TaskAssignmentRequest>>()
        .iter_current_update_messages()
        .collect::<Vec<_>>();
    let diagnostics = app.world().resource::<FamiliarTaskCandidateDiagnostics>();
    assert_eq!(
        requests.len(),
        2,
        "sand={:?}, mud={:?}, arbitration={:?}, lease={:?}",
        diagnostics.record(tasks[0]),
        diagnostics.record(tasks[1]),
        app.world().resource::<WheelbarrowArbitrationDiagnostics>(),
        app.world()
            .get::<hw_logistics::transport_request::WheelbarrowLease>(tasks[1])
    );
    let mut vehicles = Vec::new();
    for task in tasks {
        let assignment = requests
            .iter()
            .find(|request| request.task_entity == task)
            .expect("both transport requests must be submitted");
        let AssignedTask::HaulWithWheelbarrow(data) = &assignment.assigned_task else {
            panic!("transport must use a wheelbarrow")
        };
        vehicles.push(data.wheelbarrow);
    }
    assert_ne!(
        vehicles[0], vehicles[1],
        "the two assignments must reserve different initial wheelbarrows"
    );
    let workers = requests
        .iter()
        .map(|request| request.worker_entity)
        .collect::<Vec<_>>();
    for &worker in &workers {
        // Model pickup before issuing the same interruption message used by
        // squad release. The cleanup and subsequent assignment are production systems.
        let AssignedTask::HaulWithWheelbarrow(data) = app
            .world()
            .get::<AssignedTask>(worker)
            .expect("assignment applied")
        else {
            panic!("wheelbarrow assignment must be applied")
        };
        let vehicle = data.wheelbarrow;
        app.world_mut()
            .entity_mut(vehicle)
            .remove::<ParkedAt>()
            .insert(PushedBy(worker));
        if let AssignedTask::HaulWithWheelbarrow(data) =
            &mut *app.world_mut().get_mut::<AssignedTask>(worker).unwrap()
        {
            data.phase = hw_jobs::HaulWithWheelbarrowPhase::GoingToSource;
        }
        app.world_mut().write_message(SoulTaskUnassignRequest {
            soul_entity: worker,
            emit_abandoned: false,
        });
    }
    app.update();
    for &vehicle in &vehicles {
        assert!(app.world().get::<PushedBy>(vehicle).is_none());
        assert_eq!(
            app.world().get::<ParkedAt>(vehicle).map(|owner| owner.0),
            Some(parking)
        );
        assert_eq!(
            app.world()
                .resource::<SharedResourceCache>()
                .get_source_reservation(vehicle),
            0
        );
    }
    assert_eq!(
        app.world()
            .resource::<SharedResourceCache>()
            .get_mixer_destination_reservation(mixer, ResourceType::Sand),
        0,
        "interruption must release the pending collection's destination capacity"
    );
    app.update();
    for &worker in &workers {
        assert!(
            matches!(
                app.world().get::<AssignedTask>(worker),
                Some(AssignedTask::HaulWithWheelbarrow(_))
            ),
            "released worker must receive another transport assignment"
        );
    }
}
