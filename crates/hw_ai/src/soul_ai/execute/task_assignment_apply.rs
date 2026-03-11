use bevy::prelude::*;

use hw_core::constants::REST_AREA_RECRUIT_COOLDOWN_SECS;
use hw_core::events::{OnGatheringLeft, OnSoulRecruited, OnTaskAssigned};
use hw_core::logistics::WheelbarrowDestination;
use hw_core::relationships::{
    CommandedBy, DeliveringTo, ParticipatingIn, RestAreaReservedFor, RestingIn, WorkingOn,
};
use hw_core::soul::{DamnedSoul, DriftingState, IdleBehavior, IdleState, RestAreaCooldown};
use hw_jobs::events::TaskAssignmentRequest;
use hw_jobs::{AssignedTask, IssuedBy};
use hw_logistics::{SharedResourceCache, apply_reservation_op};

use crate::soul_ai::helpers::query_types::TaskAssignmentSoulQuery;

fn prepare_worker_for_task_apply(
    commands: &mut Commands,
    worker_entity: Entity,
    familiar_entity: Entity,
    task_entity: Entity,
    already_commanded: bool,
) {
    if !already_commanded {
        commands.trigger(OnSoulRecruited {
            entity: worker_entity,
            familiar_entity,
        });
    }
    commands.entity(worker_entity).try_insert((
        CommandedBy(familiar_entity),
        WorkingOn(task_entity),
    ));
    commands
        .entity(task_entity)
        .try_insert(IssuedBy(familiar_entity));
}

fn worker_can_receive_assignment(
    assigned_task: &AssignedTask,
    idle: &IdleState,
) -> bool {
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
        commands.trigger(OnGatheringLeft {
            entity: worker_entity,
        });
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
    commands
        .entity(worker_entity)
        .try_remove::<DriftingState>();
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
                commands
                    .entity(item)
                    .try_insert(DeliveringTo(dest_entity));
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
    commands.trigger(OnTaskAssigned {
        entity: worker_entity,
        task_entity: request.task_entity,
        work_type: request.work_type,
    });
}

/// Thinkで生成されたタスク割り当て要求を適用する
pub fn apply_task_assignment_requests_system(
    mut commands: Commands,
    mut requests: MessageReader<TaskAssignmentRequest>,
    mut cache: ResMut<SharedResourceCache>,
    mut q_souls: TaskAssignmentSoulQuery,
    mut q_visibility: Query<&mut Visibility, With<DamnedSoul>>,
    q_entities: Query<Entity>,
) {
    for request in requests.read() {
        if q_entities.get(request.task_entity).is_err() {
            debug!(
                "ASSIGN_REQUEST: Task entity {:?} already gone, skipping",
                request.task_entity
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
            request.already_commanded || under_command_opt.is_some(),
        );

        apply_assignment_state(&mut assigned_task, &mut dest, &mut path, request);
        apply_assignment_reservations(&mut cache, &request.reservation_ops);
        attach_delivering_to_relationship(&mut commands, &request.assigned_task);
        trigger_task_assigned_event(&mut commands, worker_entity, request);

        debug!(
            "ASSIGN_REQUEST: Assigned {:?} to {:?} at {:?}",
            request.work_type,
            worker_entity,
            worker_transform.translation.truncate()
        );
    }
}
