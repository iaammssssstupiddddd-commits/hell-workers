use super::*;
use bevy::ecs::schedule::ApplyDeferred;
use hw_core::constants::MAP_HEIGHT;
use hw_core::events::{ResourceReservationOp, ResourceReservationRequest};
use hw_core::relationships::WorkingOn;
use hw_jobs::events::TaskAssignmentRequest;
use hw_jobs::{ActiveTaskIdentity, GeneratePowerData, GeneratePowerPhase, WorkType};
use hw_logistics::SharedResourceCache;

#[derive(Resource, Default)]
struct ReservationReceipts(Vec<ResourceReservationOp>);

#[derive(Resource, Default)]
struct BudgetClaimResults(Vec<bool>);

fn collect_reservations(
    mut reservations: MessageReader<ResourceReservationRequest>,
    mut receipts: ResMut<ReservationReceipts>,
) {
    receipts
        .0
        .extend(reservations.read().map(|request| request.op.clone()));
}

fn claim_runtime_budget(
    mut budget: ResMut<RuntimePathSearchBudget>,
    mut results: ResMut<BudgetClaimResults>,
) {
    results.0.push(budget.try_claim());
}

#[test]
fn preupdate_reset_restores_the_actor_budget_each_frame() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .insert_resource(RuntimePathSearchBudget::new(1))
        .init_resource::<BudgetClaimResults>()
        .add_systems(PreUpdate, reset_runtime_path_search_budget_system)
        .add_systems(Update, claim_runtime_budget);

    app.update();
    app.update();

    assert_eq!(
        app.world().resource::<BudgetClaimResults>().0,
        vec![true, true]
    );
    assert_eq!(app.world().resource::<RuntimePathSearchBudget>().used(), 1);
}

#[test]
fn actor_work_queue_keeps_fifo_and_drops_entity_state_on_world_epoch_change() {
    let first = Entity::from_bits(1);
    let second = Entity::from_bits(2);
    let mut epoch = WorldEpoch::default();
    let mut local = EpochLocal::<RuntimePathWorkQueue>::default();

    let queue = local.get_mut(epoch);
    queue.enqueue(first, PathRequestClass::ActiveTask);
    queue.enqueue(second, PathRequestClass::ActiveTask);
    assert_eq!(queue.pop(PathRequestClass::ActiveTask), Some(first));
    queue.requeue_back(first, PathRequestClass::ActiveTask);
    assert_eq!(queue.pop(PathRequestClass::ActiveTask), Some(second));
    assert_eq!(queue.pop(PathRequestClass::ActiveTask), Some(first));

    queue.enqueue(first, PathRequestClass::IdleOrRest);
    queue.begin_cooldown(second);
    epoch.advance();

    let reset = local.get_mut(epoch);
    assert!(reset.active_task.is_empty());
    assert!(reset.idle_or_rest.is_empty());
    assert!(reset.cooling_down.is_empty());
    assert!(reset.continuations.is_empty());
}

#[test]
fn initial_queue_admission_uses_entity_order_per_class() {
    let entity = |index| Entity::from_raw_u32(index).expect("test entity index is valid");
    let first = entity(1);
    let second = entity(2);
    let third = entity(3);
    let fourth = entity(4);
    let fifth = entity(5);
    let mut queue = RuntimePathWorkQueue::default();

    enqueue_requests_in_entity_order(
        &mut queue,
        vec![
            (fourth, PathRequestClass::IdleOrRest),
            (third, PathRequestClass::ActiveTask),
            (first, PathRequestClass::IdleOrRest),
            (second, PathRequestClass::ActiveTask),
        ],
        vec![fifth, third],
    );

    assert_eq!(queue.pop(PathRequestClass::ActiveTask), Some(second));
    assert_eq!(queue.pop(PathRequestClass::ActiveTask), Some(third));
    assert_eq!(queue.pop(PathRequestClass::IdleOrRest), Some(first));
    assert_eq!(queue.pop(PathRequestClass::IdleOrRest), Some(fourth));
    assert_eq!(queue.pop_cooldown(), Some(third));
    assert_eq!(queue.pop_cooldown(), Some(fifth));
}

#[cfg(feature = "profiling")]
#[test]
fn defer_metrics_measure_actor_wait_and_reset_at_capture_boundary() {
    let entity = Entity::from_raw_u32(1).expect("test entity index is valid");
    let mut queue = RuntimePathWorkQueue::default();
    let mut metrics = RuntimePathDeferMetrics::default();

    queue.begin_defer_metrics_frame(&metrics);
    queue.record_deferred(entity, PathRequestClass::ActiveTask, &mut metrics);
    queue.begin_defer_metrics_frame(&metrics);
    queue.record_deferred(entity, PathRequestClass::ActiveTask, &mut metrics);

    assert_eq!(metrics.active_task_max_defer_frames, 2);
    assert_eq!(metrics.deferred_actor_retries, 2);

    metrics.clear();
    queue.begin_defer_metrics_frame(&metrics);
    queue.record_deferred(entity, PathRequestClass::IdleOrRest, &mut metrics);

    assert_eq!(metrics.active_task_max_defer_frames, 0);
    assert_eq!(metrics.idle_or_rest_max_defer_frames, 1);
    assert_eq!(metrics.deferred_actor_retries, 1);
}

#[test]
fn unreachable_task_destination_unassigns_and_releases_its_reservation() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .insert_resource(WorldMap::default())
        .init_resource::<RuntimePathSearchBudget>()
        .init_resource::<SharedResourceCache>()
        .init_resource::<ReservationReceipts>()
        .add_message::<ResourceReservationRequest>()
        .add_message::<TaskAssignmentRequest>()
        .add_systems(
            Update,
            (pathfinding_system, ApplyDeferred, collect_reservations).chain(),
        );
    #[cfg(feature = "profiling")]
    app.init_resource::<RuntimePathDeferMetrics>();

    let target = app.world_mut().spawn_empty().id();
    let start = WorldMap::grid_to_world(10, 12);
    let soul = app
        .world_mut()
        .spawn((
            Transform::from_translation(start.extend(0.0)),
            DamnedSoul::default(),
            Destination(WorldMap::grid_to_world(-1, 12)),
            Path::default(),
            AssignedTask::GeneratePower(GeneratePowerData {
                tile: target,
                tile_pos: start,
                phase: GeneratePowerPhase::Generating,
            }),
            IdleState::default(),
            ActiveTaskIdentity::new(target, target, WorkType::GeneratePower),
            WorkingOn(target),
        ))
        .id();

    app.update();

    assert!(matches!(
        app.world().get::<AssignedTask>(soul),
        Some(AssignedTask::None)
    ));
    assert!(app.world().get::<ActiveTaskIdentity>(soul).is_none());
    assert!(app.world().get::<WorkingOn>(soul).is_none());
    assert!(app.world().get::<PathCooldown>(soul).is_some());
    assert_eq!(
        app.world().resource::<ReservationReceipts>().0,
        vec![ResourceReservationOp::ReleaseSource {
            source: target,
            amount: 1,
        }]
    );
}

#[test]
fn exhausted_core_budget_defers_task_pathfinding_without_unassigning() {
    let mut blocked_map = WorldMap::default();
    for y in 0..MAP_HEIGHT {
        blocked_map.add_grid_obstacle((50, y));
    }

    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .insert_resource(blocked_map)
        .insert_resource(RuntimePathSearchBudget::new(1))
        .init_resource::<SharedResourceCache>()
        .init_resource::<ReservationReceipts>()
        .add_message::<ResourceReservationRequest>()
        .add_message::<TaskAssignmentRequest>()
        .add_systems(
            Update,
            (pathfinding_system, ApplyDeferred, collect_reservations).chain(),
        );
    #[cfg(feature = "profiling")]
    app.init_resource::<RuntimePathDeferMetrics>();

    let target = app.world_mut().spawn_empty().id();
    let start = WorldMap::grid_to_world(25, 50);
    let destination = WorldMap::grid_to_world(75, 50);
    let soul = app
        .world_mut()
        .spawn((
            Transform::from_translation(start.extend(0.0)),
            DamnedSoul::default(),
            Destination(destination),
            Path::default(),
            AssignedTask::GeneratePower(GeneratePowerData {
                tile: target,
                tile_pos: destination,
                phase: GeneratePowerPhase::Generating,
            }),
            IdleState::default(),
            ActiveTaskIdentity::new(target, target, WorkType::GeneratePower),
            WorkingOn(target),
        ))
        .id();

    app.update();

    assert!(matches!(
        app.world().get::<AssignedTask>(soul),
        Some(AssignedTask::GeneratePower(_))
    ));
    assert!(app.world().get::<ActiveTaskIdentity>(soul).is_some());
    assert!(app.world().get::<WorkingOn>(soul).is_some());
    assert_eq!(
        app.world()
            .get::<Destination>(soul)
            .map(|destination| destination.0),
        Some(destination)
    );
    assert!(app.world().get::<PathCooldown>(soul).is_none());
    assert_eq!(app.world().resource::<RuntimePathSearchBudget>().used(), 1);
    assert!(app.world().resource::<ReservationReceipts>().0.is_empty());

    // The first frame consumed direct A* and deferred its adjacent
    // fallback. After a new frame budget, the continuation must resume at
    // adjacent; retrying direct here would leave the task assigned again.
    app.world_mut()
        .resource_mut::<RuntimePathSearchBudget>()
        .reset();
    app.update();

    assert!(matches!(
        app.world().get::<AssignedTask>(soul),
        Some(AssignedTask::None)
    ));
    assert!(app.world().get::<PathCooldown>(soul).is_some());
}
