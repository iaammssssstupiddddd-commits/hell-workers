use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use std::collections::{HashMap, HashSet};

use hw_core::constants::REST_AREA_RECRUIT_COOLDOWN_SECS;
use hw_core::events::{OnTaskAssigned, publish_soul_recruited};
use hw_core::logistics::WheelbarrowDestination;
use hw_core::relationships::{
    CommandedBy, DeliveringTo, ParkedAt, ParticipatingIn, RestAreaReservedFor, RestingIn,
    TaskWorkers, WorkingOn,
};
use hw_core::soul::{DamnedSoul, DriftingState, IdleBehavior, IdleState, RestAreaCooldown};
use hw_energy::{PowerConsumer, PowerGenerator, SoulSpaPhase, SoulSpaSite};
use hw_jobs::events::TaskAssignmentRequest;
use hw_jobs::mud_mixer::MudMixerStorage;
use hw_jobs::{
    ActiveTaskIdentity, AssignedTask, BonePile, BridgeMarker, Building, DeconstructPhase,
    DeconstructionBlocker, DeconstructionCommitClaim, DeconstructionOrder, DeconstructionPending,
    DeconstructionTargetMarkers, Designation, Door, IssuedBy, MovePlanned, MovePlantTask,
    PendingBuildingMove, ProvisionalWall, RestArea, SandPile, TargetDeconstructionRoot, TaskSlots,
    WorkType, deconstruction_marker_matches, supports_deconstruction_cleanup,
};
use hw_logistics::types::WheelbarrowParking;
use hw_logistics::zone::Stockpile;
use hw_logistics::{BelongsTo, ResourceType, SharedResourceCache, apply_reservation_op};

use crate::soul_ai::helpers::query_types::TaskAssignmentSoulQuery;

fn prepare_worker_for_task_apply(
    commands: &mut Commands,
    worker_entity: Entity,
    familiar_entity: Entity,
    task_entity: Entity,
    work_type: WorkType,
    already_commanded: bool,
) {
    if !already_commanded {
        publish_soul_recruited(commands, worker_entity, familiar_entity);
    }
    commands
        .entity(worker_entity)
        .try_insert(CommandedBy(familiar_entity))
        .insert((
            WorkingOn(task_entity),
            ActiveTaskIdentity::new(task_entity, task_entity, work_type),
        ));
    commands
        .entity(task_entity)
        .try_insert(IssuedBy(familiar_entity));
}

fn worker_can_receive_assignment(assigned_task: &AssignedTask, idle: &IdleState) -> bool {
    matches!(*assigned_task, AssignedTask::None)
        && idle.behavior != IdleBehavior::ExhaustedGathering
}

fn normalize_worker_idle_state(
    commands: &mut Commands,
    worker_entity: Entity,
    idle: &mut IdleState,
    participating_opt: Option<&ParticipatingIn>,
    resting_opt: Option<&RestingIn>,
    q_visibility: &mut Query<&mut Visibility, With<DamnedSoul>>,
) {
    if participating_opt.is_some() {
        commands
            .entity(worker_entity)
            .try_remove::<ParticipatingIn>();
    }
    commands
        .entity(worker_entity)
        .try_remove::<RestAreaReservedFor>();
    if resting_opt.is_some() {
        commands
            .entity(worker_entity)
            .try_remove::<RestingIn>()
            .insert(RestAreaCooldown {
                remaining_secs: REST_AREA_RECRUIT_COOLDOWN_SECS,
            });
        if let Ok(mut visibility) = q_visibility.get_mut(worker_entity) {
            *visibility = Visibility::Visible;
        }
        if matches!(
            idle.behavior,
            IdleBehavior::Resting | IdleBehavior::GoingToRest
        ) {
            idle.behavior = IdleBehavior::Wandering;
            idle.idle_timer = 0.0;
            idle.total_idle_time = 0.0;
        }
    }

    if idle.behavior == IdleBehavior::Drifting {
        idle.behavior = IdleBehavior::Wandering;
        idle.idle_timer = 0.0;
        idle.behavior_duration = 3.0;
    }
    if idle.behavior != IdleBehavior::Wandering {
        // タスク開始フレームで idle 状態を正規化し、睡眠判定の取りこぼしを防ぐ。
        idle.behavior = IdleBehavior::Wandering;
        idle.idle_timer = 0.0;
        idle.behavior_duration = 3.0;
        idle.needs_separation = false;
    }
    idle.total_idle_time = 0.0;
    commands.entity(worker_entity).try_remove::<DriftingState>();
}

fn apply_assignment_state(
    assigned_task: &mut AssignedTask,
    dest: &mut hw_core::soul::Destination,
    path: &mut hw_core::soul::Path,
    request: &TaskAssignmentRequest,
) {
    *assigned_task = request.assigned_task.clone();
    dest.0 = request.task_pos;
    path.waypoints.clear();
    path.current_index = 0;
}

fn apply_assignment_reservations(
    cache: &mut SharedResourceCache,
    reservation_ops: &[hw_core::events::ResourceReservationOp],
) {
    for op in reservation_ops {
        apply_reservation_op(cache, op);
    }
}

fn attach_delivering_to_relationship(commands: &mut Commands, assigned_task: &AssignedTask) {
    match assigned_task {
        AssignedTask::Haul(data) => {
            commands
                .entity(data.item)
                .try_insert(DeliveringTo(data.stockpile));
        }
        AssignedTask::HaulToBlueprint(data) => {
            commands
                .entity(data.item)
                .try_insert(DeliveringTo(data.blueprint));
        }
        AssignedTask::HaulToMixer(data) => {
            commands
                .entity(data.item)
                .try_insert(DeliveringTo(data.mixer));
        }
        AssignedTask::HaulWithWheelbarrow(data) => {
            let dest_entity = match data.destination {
                WheelbarrowDestination::Stockpile(e) => e,
                WheelbarrowDestination::Blueprint(e) => e,
                WheelbarrowDestination::Mixer { entity, .. } => entity,
            };
            for &item in &data.items {
                commands.entity(item).try_insert(DeliveringTo(dest_entity));
            }
        }
        _ => {}
    }
}

fn trigger_task_assigned_event(
    commands: &mut Commands,
    worker_entity: Entity,
    request: &TaskAssignmentRequest,
) {
    commands.write_message(OnTaskAssigned {
        entity: worker_entity,
        assignment_entity: request.task_entity,
        current_target_entity: request.task_entity,
        current_work_type: request.work_type,
    });
}

type DeconstructionOrdersQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static TargetDeconstructionRoot,
        &'static Designation,
        Option<&'static DeconstructionBlocker>,
    ),
    With<DeconstructionOrder>,
>;

type DeconstructionTargetsQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static DeconstructionPending,
        &'static Building,
        Option<&'static ProvisionalWall>,
        Option<&'static DeconstructionCommitClaim>,
        Option<&'static MovePlanned>,
        Option<&'static PendingBuildingMove>,
    ),
>;

type DeconstructionTargetMarkersQuery<'w, 's> = Query<
    'w,
    's,
    (
        Option<&'static SandPile>,
        Option<&'static BonePile>,
        Option<&'static MudMixerStorage>,
        Option<&'static RestArea>,
        Option<&'static WheelbarrowParking>,
        Option<&'static Stockpile>,
        Option<&'static Door>,
        Option<&'static BridgeMarker>,
        Option<&'static SoulSpaSite>,
    ),
>;

type DeconstructionTargetPowerQuery<'w, 's> = Query<
    'w,
    's,
    (
        Option<&'static PowerConsumer>,
        Option<&'static PowerGenerator>,
    ),
>;

#[derive(SystemParam)]
pub struct DeconstructionAssignmentValidationQueries<'w, 's> {
    orders: DeconstructionOrdersQuery<'w, 's>,
    targets: DeconstructionTargetsQuery<'w, 's>,
    target_markers: DeconstructionTargetMarkersQuery<'w, 's>,
    target_power: DeconstructionTargetPowerQuery<'w, 's>,
    pending: Query<'w, 's, &'static DeconstructionPending>,
    belongs_to: Query<'w, 's, &'static BelongsTo>,
    parked_at: Query<'w, 's, &'static ParkedAt>,
    soul_spa_tiles: Query<'w, 's, &'static hw_energy::SoulSpaTile>,
    move_plant_tasks: Query<'w, 's, &'static MovePlantTask>,
}

fn entity_or_owner_is_pending(
    entity: Entity,
    deconstruction: &DeconstructionAssignmentValidationQueries<'_, '_>,
) -> bool {
    deconstruction.pending.get(entity).is_ok()
        || deconstruction
            .soul_spa_tiles
            .get(entity)
            .is_ok_and(|tile| deconstruction.pending.get(tile.parent_site).is_ok())
        || deconstruction
            .belongs_to
            .get(entity)
            .is_ok_and(|owner| deconstruction.pending.get(owner.0).is_ok())
        || deconstruction
            .parked_at
            .get(entity)
            .is_ok_and(|owner| deconstruction.pending.get(owner.0).is_ok())
}

fn non_deconstruction_task_targets_pending_owner(
    task: &AssignedTask,
    deconstruction: &DeconstructionAssignmentValidationQueries<'_, '_>,
) -> bool {
    let pending = |entity| entity_or_owner_is_pending(entity, deconstruction);
    match task {
        AssignedTask::Gather(data) => pending(data.target),
        AssignedTask::Haul(data) => pending(data.item) || pending(data.stockpile),
        AssignedTask::HaulToBlueprint(data) => pending(data.item) || pending(data.blueprint),
        AssignedTask::Build(data) => pending(data.blueprint),
        AssignedTask::MovePlant(data) => pending(data.building),
        AssignedTask::BucketTransport(data) => {
            pending(data.bucket)
                || match data.source {
                    hw_jobs::BucketTransportSource::River => false,
                    hw_jobs::BucketTransportSource::Tank { tank, .. } => pending(tank),
                }
                || match data.destination {
                    hw_jobs::BucketTransportDestination::Tank(tank) => pending(tank),
                    hw_jobs::BucketTransportDestination::Mixer(mixer) => pending(mixer),
                }
        }
        AssignedTask::CollectBone(data) => pending(data.target),
        AssignedTask::Refine(data) => pending(data.mixer),
        AssignedTask::HaulToMixer(data) => pending(data.item) || pending(data.mixer),
        AssignedTask::HaulWithWheelbarrow(data) => {
            pending(data.wheelbarrow)
                || data.collect_source.is_some_and(pending)
                || data.items.iter().copied().any(pending)
                || match data.destination {
                    WheelbarrowDestination::Stockpile(target)
                    | WheelbarrowDestination::Blueprint(target) => pending(target),
                    WheelbarrowDestination::Mixer { entity, .. } => pending(entity),
                }
        }
        AssignedTask::ReinforceFloorTile(data) => pending(data.tile) || pending(data.site),
        AssignedTask::PourFloorTile(data) => pending(data.tile) || pending(data.site),
        AssignedTask::FrameWallTile(data) => pending(data.tile) || pending(data.site),
        AssignedTask::CoatWall(data) => {
            pending(data.tile) || pending(data.site) || pending(data.wall)
        }
        AssignedTask::GeneratePower(data) => pending(data.tile),
        AssignedTask::Deconstruct(_) | AssignedTask::None => false,
    }
}

fn assignment_is_still_valid(
    request: &TaskAssignmentRequest,
    deconstruction: &DeconstructionAssignmentValidationQueries<'_, '_>,
    assigned_move_targets: &HashSet<Entity>,
) -> bool {
    if request.assigned_task.work_type() != Some(request.work_type) {
        return false;
    }
    match &request.assigned_task {
        AssignedTask::Deconstruct(data) => {
            if request.work_type != WorkType::Deconstruct
                || request.task_entity != data.order
                || data.phase != DeconstructPhase::GoingToTarget
                || !request.reservation_ops.is_empty()
            {
                return false;
            }
            let Ok((order_target, designation, blocker)) = deconstruction.orders.get(data.order)
            else {
                return false;
            };
            if order_target.0 != data.target
                || designation.work_type != WorkType::Deconstruct
                || blocker.is_some_and(|blocker| blocker.active)
            {
                return false;
            }
            let Ok((pending, building, provisional_wall, claim, moving, pending_move)) =
                deconstruction.targets.get(data.target)
            else {
                return false;
            };
            let Ok((
                sand_pile,
                bone_pile,
                mixer_storage,
                rest_area,
                wheelbarrow_parking,
                stockpile,
                door,
                bridge,
                soul_spa,
            )) = deconstruction.target_markers.get(data.target)
            else {
                return false;
            };
            let Ok((power_consumer, power_generator)) =
                deconstruction.target_power.get(data.target)
            else {
                return false;
            };
            pending.order == data.order
                && claim.is_none()
                && moving.is_none()
                && pending_move.is_none()
                && !building.is_provisional
                && provisional_wall.is_none()
                && supports_deconstruction_cleanup(building.kind)
                && deconstruction_marker_matches(
                    building.kind,
                    DeconstructionTargetMarkers {
                        water_storage: stockpile.is_some_and(|stockpile| {
                            stockpile.resource_type == Some(ResourceType::Water)
                        }),
                        mud_mixer_storage: mixer_storage.is_some(),
                        rest_area: rest_area.is_some(),
                        wheelbarrow_parking: wheelbarrow_parking.is_some(),
                        sand_pile: sand_pile.is_some(),
                        bone_pile: bone_pile.is_some(),
                        door: door.is_some(),
                        bridge: bridge.is_some(),
                        operational_soul_spa: soul_spa
                            .is_some_and(|site| site.phase == SoulSpaPhase::Operational),
                        power_consumer: power_consumer.is_some(),
                        power_generator: power_generator.is_some(),
                    },
                )
                && !deconstruction
                    .move_plant_tasks
                    .iter()
                    .any(|move_task| move_task.building == data.target)
                && !assigned_move_targets.contains(&data.target)
        }
        task => !non_deconstruction_task_targets_pending_owner(task, deconstruction),
    }
}

/// Thinkで生成されたタスク割り当て要求を適用する
pub fn apply_task_assignment_requests_system(
    mut commands: Commands,
    mut requests: MessageReader<TaskAssignmentRequest>,
    mut cache: ResMut<SharedResourceCache>,
    mut q_souls: TaskAssignmentSoulQuery,
    mut q_visibility: Query<&mut Visibility, With<DamnedSoul>>,
    q_tasks: Query<(Option<&TaskSlots>, Option<&TaskWorkers>)>,
    deconstruction: DeconstructionAssignmentValidationQueries,
) {
    let assigned_move_targets = q_souls
        .iter_mut()
        .filter_map(|(_, _, task, _, _, _, _, _, _, _)| match &*task {
            AssignedTask::MovePlant(data) => Some(data.building),
            _ => None,
        })
        .collect::<HashSet<_>>();
    let mut accepted_workers_by_task = HashMap::<Entity, usize>::new();
    for request in requests.read() {
        if !assignment_is_still_valid(request, &deconstruction, &assigned_move_targets) {
            debug!(
                "ASSIGN_REQUEST: Task {:?} failed apply-time deconstruction validation",
                request.task_entity
            );
            continue;
        }
        let Ok((slots_opt, workers_opt)) = q_tasks.get(request.task_entity) else {
            debug!(
                "ASSIGN_REQUEST: Task entity {:?} already gone, skipping",
                request.task_entity
            );
            continue;
        };
        let max_workers = slots_opt.map_or(1, |slots| slots.max as usize);
        let current_workers = workers_opt.map_or(0, TaskWorkers::len);
        let accepted_workers = accepted_workers_by_task
            .get(&request.task_entity)
            .copied()
            .unwrap_or(0);
        if current_workers.saturating_add(accepted_workers) >= max_workers {
            debug!(
                "ASSIGN_REQUEST: Task {:?} has no free slots, skipping worker {:?}",
                request.task_entity, request.worker_entity
            );
            continue;
        }

        let Ok((
            worker_entity,
            worker_transform,
            mut assigned_task,
            mut dest,
            mut path,
            mut idle,
            _inventory_opt,
            under_command_opt,
            participating_opt,
            resting_opt,
        )) = q_souls.get_mut(request.worker_entity)
        else {
            warn!(
                "ASSIGN_REQUEST: Worker {:?} not found",
                request.worker_entity
            );
            continue;
        };

        if !worker_can_receive_assignment(&assigned_task, &idle) {
            continue;
        }

        normalize_worker_idle_state(
            &mut commands,
            worker_entity,
            &mut idle,
            participating_opt,
            resting_opt,
            &mut q_visibility,
        );

        prepare_worker_for_task_apply(
            &mut commands,
            worker_entity,
            request.familiar_entity,
            request.task_entity,
            request.work_type,
            request.already_commanded || under_command_opt.is_some(),
        );

        apply_assignment_state(&mut assigned_task, &mut dest, &mut path, request);
        apply_assignment_reservations(&mut cache, &request.reservation_ops);
        attach_delivering_to_relationship(&mut commands, &request.assigned_task);
        trigger_task_assigned_event(&mut commands, worker_entity, request);
        accepted_workers_by_task
            .entry(request.task_entity)
            .and_modify(|count| *count = count.saturating_add(1))
            .or_insert(1);

        debug!(
            "ASSIGN_REQUEST: Assigned {:?} to {:?} at {:?}",
            request.work_type,
            worker_entity,
            worker_transform.translation.truncate()
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hw_core::events::ResourceReservationOp;
    use hw_core::soul::{Destination, Path};
    use hw_jobs::{
        BonePile, BuildingType, CollectBoneData, CollectBonePhase, DeconstructData,
        DeconstructPhase, DeconstructionBlockReason, DeconstructionOrder, DeconstructionPending,
        GeneratePowerData, GeneratePowerPhase, MovePlantTask, SandPile, TargetDeconstructionRoot,
        TaskDiagnosticDomainMask,
    };

    #[derive(Resource, Clone, Copy)]
    struct AssignmentFixture {
        worker: Entity,
        familiar: Entity,
        task: Entity,
    }

    fn emit_assignment_request(
        fixture: Res<AssignmentFixture>,
        mut writer: MessageWriter<TaskAssignmentRequest>,
    ) {
        writer.write(TaskAssignmentRequest {
            familiar_entity: fixture.familiar,
            worker_entity: fixture.worker,
            task_entity: fixture.task,
            work_type: WorkType::GeneratePower,
            task_pos: Vec2::ZERO,
            assigned_task: AssignedTask::GeneratePower(GeneratePowerData {
                tile: fixture.task,
                tile_pos: Vec2::ZERO,
                phase: GeneratePowerPhase::GoingToTile,
            }),
            reservation_ops: Vec::new(),
            already_commanded: true,
        });
    }

    fn assert_assignment_identity_after_defer(
        fixture: Res<AssignmentFixture>,
        q_workers: Query<(&WorkingOn, &ActiveTaskIdentity)>,
    ) {
        let (working_on, identity) = q_workers
            .get(fixture.worker)
            .expect("assignment must materialize WorkingOn and ActiveTaskIdentity together");
        assert_eq!(working_on.0, fixture.task);
        assert_eq!(identity.assignment_entity, fixture.task);
        assert_eq!(identity.current_target_entity, fixture.task);
        assert_eq!(identity.current_work_type, WorkType::GeneratePower);
    }

    fn generate_power_request(
        familiar: Entity,
        worker: Entity,
        task: Entity,
    ) -> TaskAssignmentRequest {
        TaskAssignmentRequest {
            familiar_entity: familiar,
            worker_entity: worker,
            task_entity: task,
            work_type: WorkType::GeneratePower,
            task_pos: Vec2::ZERO,
            assigned_task: AssignedTask::GeneratePower(GeneratePowerData {
                tile: task,
                tile_pos: Vec2::ZERO,
                phase: GeneratePowerPhase::GoingToTile,
            }),
            reservation_ops: Vec::new(),
            already_commanded: true,
        }
    }

    fn assignment_test_app() -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<SharedResourceCache>()
            .add_message::<TaskAssignmentRequest>()
            .add_message::<OnTaskAssigned>()
            .add_systems(
                Update,
                (apply_task_assignment_requests_system, ApplyDeferred).chain(),
            );
        app
    }

    fn spawn_idle_worker(app: &mut App) -> Entity {
        app.world_mut()
            .spawn((
                Transform::default(),
                Visibility::Visible,
                DamnedSoul::default(),
                AssignedTask::None,
                Destination(Vec2::ZERO),
                Path::default(),
                IdleState::default(),
            ))
            .id()
    }

    fn spawn_deconstruction_order(app: &mut App) -> (Entity, Entity) {
        let target = app
            .world_mut()
            .spawn((
                Building {
                    kind: BuildingType::SandPile,
                    is_provisional: false,
                },
                SandPile,
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
            ))
            .id();
        app.world_mut().flush();
        app.world_mut()
            .entity_mut(target)
            .insert(DeconstructionPending { order });
        (order, target)
    }

    fn deconstruction_request(
        familiar: Entity,
        worker: Entity,
        order: Entity,
        target: Entity,
    ) -> TaskAssignmentRequest {
        TaskAssignmentRequest {
            familiar_entity: familiar,
            worker_entity: worker,
            task_entity: order,
            work_type: WorkType::Deconstruct,
            task_pos: Vec2::ZERO,
            assigned_task: AssignedTask::Deconstruct(DeconstructData {
                order,
                target,
                phase: DeconstructPhase::GoingToTarget,
            }),
            reservation_ops: Vec::new(),
            already_commanded: true,
        }
    }

    #[test]
    fn exact_deconstruction_request_binds_identity_to_order_not_target() {
        let mut app = assignment_test_app();
        let familiar = app.world_mut().spawn_empty().id();
        let worker = spawn_idle_worker(&mut app);
        let (order, target) = spawn_deconstruction_order(&mut app);
        app.world_mut()
            .write_message(deconstruction_request(familiar, worker, order, target));

        app.update();

        assert!(matches!(
            app.world().get::<AssignedTask>(worker),
            Some(AssignedTask::Deconstruct(data))
                if data.order == order && data.target == target
        ));
        assert_eq!(
            app.world()
                .get::<WorkingOn>(worker)
                .map(|working| working.0),
            Some(order)
        );
        let identity = app.world().get::<ActiveTaskIdentity>(worker).unwrap();
        assert_eq!(identity.assignment_entity, order);
        assert_eq!(identity.current_target_entity, order);
        assert_eq!(
            app.world().get::<TaskWorkers>(order).map(TaskWorkers::len),
            Some(1)
        );
        assert!(
            app.world()
                .get::<TaskWorkers>(target)
                .is_none_or(TaskWorkers::is_empty)
        );
    }

    #[test]
    fn deconstruction_request_rechecks_designation_before_apply() {
        let mut app = assignment_test_app();
        let familiar = app.world_mut().spawn_empty().id();
        let worker = spawn_idle_worker(&mut app);
        let (order, target) = spawn_deconstruction_order(&mut app);
        app.world_mut().entity_mut(order).remove::<Designation>();
        app.world_mut()
            .write_message(deconstruction_request(familiar, worker, order, target));

        app.update();

        assert!(matches!(
            app.world().get::<AssignedTask>(worker),
            Some(AssignedTask::None)
        ));
        assert!(app.world().get::<WorkingOn>(worker).is_none());
    }

    #[test]
    fn non_initial_deconstruction_phases_are_rejected_without_side_effects() {
        for phase in [
            DeconstructPhase::Dismantling { progress: 0.5 },
            DeconstructPhase::AwaitingCommit,
        ] {
            let mut app = assignment_test_app();
            let familiar = app.world_mut().spawn_empty().id();
            let worker = spawn_idle_worker(&mut app);
            let (order, target) = spawn_deconstruction_order(&mut app);
            let mut request = deconstruction_request(familiar, worker, order, target);
            let AssignedTask::Deconstruct(data) = &mut request.assigned_task else {
                unreachable!("fixture must produce deconstruction work");
            };
            data.phase = phase;
            app.world_mut().write_message(request);

            app.update();

            assert!(matches!(
                app.world().get::<AssignedTask>(worker),
                Some(AssignedTask::None)
            ));
            assert!(app.world().get::<WorkingOn>(worker).is_none());
            assert!(app.world().get::<ActiveTaskIdentity>(worker).is_none());
            assert!(
                app.world()
                    .get::<TaskWorkers>(order)
                    .is_none_or(TaskWorkers::is_empty)
            );
        }
    }

    #[test]
    fn deconstruction_assignment_rejects_kind_marker_mismatch() {
        let mut app = assignment_test_app();
        let familiar = app.world_mut().spawn_empty().id();
        let worker = spawn_idle_worker(&mut app);
        let (order, target) = spawn_deconstruction_order(&mut app);
        app.world_mut().entity_mut(target).remove::<SandPile>();
        app.world_mut().entity_mut(target).insert(BonePile);
        app.world_mut()
            .write_message(deconstruction_request(familiar, worker, order, target));

        app.update();

        assert!(matches!(
            app.world().get::<AssignedTask>(worker),
            Some(AssignedTask::None)
        ));
        assert!(app.world().get::<WorkingOn>(worker).is_none());
    }

    #[test]
    fn active_deconstruction_blocker_rejects_until_it_is_deactivated() {
        let mut app = assignment_test_app();
        let familiar = app.world_mut().spawn_empty().id();
        let worker = spawn_idle_worker(&mut app);
        let (order, target) = spawn_deconstruction_order(&mut app);
        app.world_mut()
            .entity_mut(order)
            .insert(DeconstructionBlocker::pending(
                DeconstructionBlockReason::OwnerMismatch,
                TaskDiagnosticDomainMask::TASK,
            ));
        app.world_mut()
            .write_message(deconstruction_request(familiar, worker, order, target));

        app.update();

        assert!(matches!(
            app.world().get::<AssignedTask>(worker),
            Some(AssignedTask::None)
        ));
        app.world_mut()
            .get_mut::<DeconstructionBlocker>(order)
            .expect("fixture order keeps its blocker")
            .active = false;
        app.world_mut()
            .write_message(deconstruction_request(familiar, worker, order, target));

        app.update();

        assert!(matches!(
            app.world().get::<AssignedTask>(worker),
            Some(AssignedTask::Deconstruct(data))
                if data.order == order && data.target == target
        ));
    }

    #[test]
    fn deconstruction_assignment_rejects_pending_durable_and_assigned_move_evidence() {
        let mut app = assignment_test_app();
        let familiar = app.world_mut().spawn_empty().id();
        let workers = [
            spawn_idle_worker(&mut app),
            spawn_idle_worker(&mut app),
            spawn_idle_worker(&mut app),
        ];
        let (pending_order, pending_target) = spawn_deconstruction_order(&mut app);
        app.world_mut()
            .entity_mut(pending_target)
            .insert(PendingBuildingMove {
                old_occupied: vec![(1, 1)],
                new_occupied: vec![(2, 2)],
                companion_anchor: None,
            });
        let (durable_order, durable_target) = spawn_deconstruction_order(&mut app);
        app.world_mut().spawn(MovePlantTask {
            building: durable_target,
            destination_grid: (4, 4),
            destination_pos: Vec2::splat(64.0),
            companion_anchor: None,
        });
        let (assigned_order, assigned_target) = spawn_deconstruction_order(&mut app);
        let assigned_move_task = app.world_mut().spawn_empty().id();
        app.world_mut().spawn((
            Transform::default(),
            Visibility::Visible,
            DamnedSoul::default(),
            AssignedTask::MovePlant(hw_jobs::MovePlantData {
                task_entity: assigned_move_task,
                building: assigned_target,
                destination_grid: (5, 5),
                destination_pos: Vec2::splat(80.0),
                companion_anchor: None,
                phase: hw_jobs::MovePlantPhase::GoToBuilding,
            }),
            Destination(Vec2::ZERO),
            Path::default(),
            IdleState::default(),
        ));
        app.world_mut().write_message(deconstruction_request(
            familiar,
            workers[0],
            pending_order,
            pending_target,
        ));
        app.world_mut().write_message(deconstruction_request(
            familiar,
            workers[1],
            durable_order,
            durable_target,
        ));
        app.world_mut().write_message(deconstruction_request(
            familiar,
            workers[2],
            assigned_order,
            assigned_target,
        ));

        app.update();

        for worker in workers {
            assert!(matches!(
                app.world().get::<AssignedTask>(worker),
                Some(AssignedTask::None)
            ));
            assert!(app.world().get::<WorkingOn>(worker).is_none());
        }
    }

    #[test]
    fn pending_collect_source_is_rejected_before_reservation_apply() {
        let mut app = assignment_test_app();
        let familiar = app.world_mut().spawn_empty().id();
        let worker = spawn_idle_worker(&mut app);
        let order = app.world_mut().spawn_empty().id();
        let source = app
            .world_mut()
            .spawn((BonePile, TaskSlots::new(1), DeconstructionPending { order }))
            .id();
        app.world_mut().write_message(TaskAssignmentRequest {
            familiar_entity: familiar,
            worker_entity: worker,
            task_entity: source,
            work_type: WorkType::CollectBone,
            task_pos: Vec2::ZERO,
            assigned_task: AssignedTask::CollectBone(CollectBoneData {
                target: source,
                phase: CollectBonePhase::GoingToBone,
            }),
            reservation_ops: vec![ResourceReservationOp::ReserveSource { source, amount: 1 }],
            already_commanded: true,
        });

        app.update();

        assert!(matches!(
            app.world().get::<AssignedTask>(worker),
            Some(AssignedTask::None)
        ));
        assert_eq!(
            app.world()
                .resource::<SharedResourceCache>()
                .get_source_reservation(source),
            0
        );
    }

    #[test]
    fn pending_facility_rejects_direct_and_owned_work_before_reservations_apply() {
        let mut app = assignment_test_app();
        let familiar = app.world_mut().spawn_empty().id();
        let order = app.world_mut().spawn_empty().id();
        let facility = app.world_mut().spawn(DeconstructionPending { order }).id();
        let companion = app.world_mut().spawn(BelongsTo(facility)).id();
        let wheelbarrow = app.world_mut().spawn(BelongsTo(facility)).id();
        let item = app.world_mut().spawn_empty().id();
        let external_destination = app.world_mut().spawn_empty().id();

        let task_cases = [
            (
                WorkType::Refine,
                AssignedTask::Refine(hw_jobs::RefineData {
                    mixer: facility,
                    phase: hw_jobs::RefinePhase::GoingToMixer,
                }),
            ),
            (
                WorkType::HaulToMixer,
                AssignedTask::HaulToMixer(hw_jobs::HaulToMixerData {
                    item,
                    mixer: facility,
                    resource_type: ResourceType::Sand,
                    phase: hw_jobs::HaulToMixerPhase::GoingToItem,
                }),
            ),
            (
                WorkType::Haul,
                AssignedTask::Haul(hw_jobs::HaulData {
                    item,
                    stockpile: companion,
                    phase: hw_jobs::HaulPhase::GoingToItem,
                }),
            ),
            (
                WorkType::WheelbarrowHaul,
                AssignedTask::HaulWithWheelbarrow(hw_jobs::HaulWithWheelbarrowData {
                    wheelbarrow,
                    source_pos: Vec2::ZERO,
                    destination: WheelbarrowDestination::Stockpile(external_destination),
                    collect_source: None,
                    collect_amount: 0,
                    collect_resource_type: None,
                    items: vec![item],
                    phase: hw_jobs::HaulWithWheelbarrowPhase::GoingToParking,
                }),
            ),
            (
                WorkType::Move,
                AssignedTask::MovePlant(hw_jobs::MovePlantData {
                    task_entity: external_destination,
                    building: facility,
                    destination_grid: (1, 1),
                    destination_pos: Vec2::ZERO,
                    companion_anchor: None,
                    phase: hw_jobs::MovePlantPhase::GoToBuilding,
                }),
            ),
        ];

        let mut workers = Vec::new();
        for (work_type, assigned_task) in task_cases {
            let worker = spawn_idle_worker(&mut app);
            let task = app.world_mut().spawn(TaskSlots::new(1)).id();
            workers.push(worker);
            app.world_mut().write_message(TaskAssignmentRequest {
                familiar_entity: familiar,
                worker_entity: worker,
                task_entity: task,
                work_type,
                task_pos: Vec2::ZERO,
                assigned_task,
                reservation_ops: vec![ResourceReservationOp::ReserveSource {
                    source: facility,
                    amount: 1,
                }],
                already_commanded: true,
            });
        }

        app.update();

        for worker in workers {
            assert!(matches!(
                app.world().get::<AssignedTask>(worker),
                Some(AssignedTask::None)
            ));
            assert!(app.world().get::<WorkingOn>(worker).is_none());
        }
        assert_eq!(
            app.world()
                .resource::<SharedResourceCache>()
                .get_source_reservation(facility),
            0
        );
    }

    #[test]
    fn assignment_defer_makes_identity_available_to_same_frame_execution() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<SharedResourceCache>()
            .add_message::<TaskAssignmentRequest>()
            .add_message::<OnTaskAssigned>();

        let familiar = app.world_mut().spawn_empty().id();
        let task = app.world_mut().spawn_empty().id();
        let worker = app
            .world_mut()
            .spawn((
                Transform::default(),
                Visibility::Visible,
                DamnedSoul::default(),
                AssignedTask::None,
                Destination(Vec2::ZERO),
                Path::default(),
                IdleState::default(),
            ))
            .id();
        app.insert_resource(AssignmentFixture {
            worker,
            familiar,
            task,
        });
        app.add_systems(
            Update,
            (
                emit_assignment_request,
                apply_task_assignment_requests_system,
                ApplyDeferred,
                assert_assignment_identity_after_defer,
            )
                .chain(),
        );

        app.update();
    }

    #[test]
    fn assignment_apply_keeps_one_slot_task_at_one_worker_across_competing_requests() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<SharedResourceCache>()
            .add_message::<TaskAssignmentRequest>()
            .add_message::<OnTaskAssigned>()
            .add_systems(
                Update,
                (apply_task_assignment_requests_system, ApplyDeferred).chain(),
            );

        let familiar = app.world_mut().spawn_empty().id();
        let task = app.world_mut().spawn(TaskSlots::new(1)).id();
        let workers = [
            app.world_mut()
                .spawn((
                    Transform::default(),
                    Visibility::Visible,
                    DamnedSoul::default(),
                    AssignedTask::None,
                    Destination(Vec2::ZERO),
                    Path::default(),
                    IdleState::default(),
                ))
                .id(),
            app.world_mut()
                .spawn((
                    Transform::default(),
                    Visibility::Visible,
                    DamnedSoul::default(),
                    AssignedTask::None,
                    Destination(Vec2::ZERO),
                    Path::default(),
                    IdleState::default(),
                ))
                .id(),
        ];
        for worker in workers {
            app.world_mut()
                .write_message(generate_power_request(familiar, worker, task));
        }

        app.update();

        let assigned_count = workers
            .iter()
            .filter(|worker| {
                !matches!(
                    app.world().get::<AssignedTask>(**worker),
                    Some(AssignedTask::None)
                )
            })
            .count();
        assert_eq!(assigned_count, 1);
        assert_eq!(
            app.world().get::<TaskWorkers>(task).map(TaskWorkers::len),
            Some(1)
        );
    }

    #[test]
    fn rejected_slot_competitor_can_take_the_next_open_task() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<SharedResourceCache>()
            .add_message::<TaskAssignmentRequest>()
            .add_message::<OnTaskAssigned>()
            .add_systems(
                Update,
                (apply_task_assignment_requests_system, ApplyDeferred).chain(),
            );

        let familiar = app.world_mut().spawn_empty().id();
        let tasks = [
            app.world_mut().spawn(TaskSlots::new(1)).id(),
            app.world_mut().spawn(TaskSlots::new(1)).id(),
        ];
        let workers = [
            app.world_mut()
                .spawn((
                    Transform::default(),
                    Visibility::Visible,
                    DamnedSoul::default(),
                    AssignedTask::None,
                    Destination(Vec2::ZERO),
                    Path::default(),
                    IdleState::default(),
                ))
                .id(),
            app.world_mut()
                .spawn((
                    Transform::default(),
                    Visibility::Visible,
                    DamnedSoul::default(),
                    AssignedTask::None,
                    Destination(Vec2::ZERO),
                    Path::default(),
                    IdleState::default(),
                ))
                .id(),
        ];

        // Simulate overlapping producers: both first target task 0, then the
        // second producer offers worker 1 the still-open task 1.
        for (worker, task) in [
            (workers[0], tasks[0]),
            (workers[1], tasks[0]),
            (workers[1], tasks[1]),
        ] {
            app.world_mut()
                .write_message(generate_power_request(familiar, worker, task));
        }

        app.update();

        assert_eq!(
            app.world().get::<WorkingOn>(workers[0]).map(|task| task.0),
            Some(tasks[0])
        );
        assert_eq!(
            app.world().get::<WorkingOn>(workers[1]).map(|task| task.0),
            Some(tasks[1])
        );
        assert_eq!(
            tasks.map(|task| app.world().get::<TaskWorkers>(task).map(TaskWorkers::len)),
            [Some(1), Some(1)]
        );
    }
}
