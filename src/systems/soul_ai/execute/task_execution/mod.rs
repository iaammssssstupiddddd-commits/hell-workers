//! タスク実行モジュール
//!
//! 魂に割り当てられたタスクの実行ロジックを提供します。

pub mod build;
pub mod coat_wall;
pub mod collect_bone;
pub mod collect_sand;
pub mod common;
pub mod context;
pub mod frame_wall;
pub mod gather;
pub mod gather_water;
pub mod handler;
pub mod haul;
pub mod haul_to_blueprint;
pub mod haul_to_mixer;
pub mod haul_water_to_mixer;
pub mod haul_with_wheelbarrow;
pub mod pour_floor;
pub mod refine;
pub mod reinforce_floor;
pub mod transport_common;
pub mod types;

// 型の再エクスポート（外部からのアクセスを簡潔に）
pub use types::AssignedTask;

use crate::entities::damned_soul::{IdleBehavior, RestAreaCooldown};
use crate::events::{
    OnGatheringLeft, OnSoulRecruited, OnTaskAssigned, OnTaskCompleted, TaskAssignmentRequest,
};
use crate::systems::familiar_ai::perceive::resource_sync::{
    SharedResourceCache, apply_reservation_op,
};
use crate::systems::soul_ai::helpers::query_types::{
    TaskAssignmentSoulQuery, TaskExecutionSoulQuery,
};
use crate::systems::soul_ai::helpers::work::unassign_task;
use crate::world::map::WorldMap;
use bevy::prelude::*;

use context::TaskExecutionContext;
use handler::run_task_handler;

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
        crate::relationships::CommandedBy(familiar_entity),
        crate::relationships::WorkingOn(task_entity),
    ));
    commands
        .entity(task_entity)
        .try_insert(crate::systems::jobs::IssuedBy(familiar_entity));
}

fn worker_can_receive_assignment(
    assigned_task: &AssignedTask,
    idle: &crate::entities::damned_soul::IdleState,
) -> bool {
    matches!(*assigned_task, AssignedTask::None)
        && idle.behavior != IdleBehavior::ExhaustedGathering
}

fn normalize_worker_idle_state(
    commands: &mut Commands,
    worker_entity: Entity,
    idle: &mut crate::entities::damned_soul::IdleState,
    participating_opt: Option<&crate::relationships::ParticipatingIn>,
    resting_opt: Option<&crate::relationships::RestingIn>,
    q_visibility: &mut Query<&mut Visibility, With<crate::entities::damned_soul::DamnedSoul>>,
) {
    if participating_opt.is_some() {
        commands
            .entity(worker_entity)
            .try_remove::<crate::relationships::ParticipatingIn>();
        commands.trigger(OnGatheringLeft {
            entity: worker_entity,
        });
    }
    commands
        .entity(worker_entity)
        .try_remove::<crate::relationships::RestAreaReservedFor>();
    if resting_opt.is_some() {
        commands
            .entity(worker_entity)
            .try_remove::<crate::relationships::RestingIn>()
            .insert(RestAreaCooldown {
                remaining_secs: crate::constants::REST_AREA_RECRUIT_COOLDOWN_SECS,
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
        .try_remove::<crate::entities::damned_soul::DriftingState>();
}

fn apply_assignment_state(
    assigned_task: &mut AssignedTask,
    dest: &mut crate::entities::damned_soul::Destination,
    path: &mut crate::entities::damned_soul::Path,
    request: &TaskAssignmentRequest,
) {
    *assigned_task = request.assigned_task.clone();
    dest.0 = request.task_pos;
    path.waypoints.clear();
    path.current_index = 0;
}

fn apply_assignment_reservations(
    cache: &mut SharedResourceCache,
    reservation_ops: &[crate::events::ResourceReservationOp],
) {
    for op in reservation_ops {
        apply_reservation_op(cache, op);
    }
}

fn attach_delivering_to_relationship(commands: &mut Commands, assigned_task: &AssignedTask) {
    // タスクの内容に応じて DeliveringTo リレーションシップを自動適用する
    match assigned_task {
        AssignedTask::Haul(data) => {
            commands
                .entity(data.item)
                .try_insert(crate::relationships::DeliveringTo(data.stockpile));
        }
        AssignedTask::HaulToBlueprint(data) => {
            commands
                .entity(data.item)
                .try_insert(crate::relationships::DeliveringTo(data.blueprint));
        }
        AssignedTask::GatherWater(data) => {
            commands
                .entity(data.bucket)
                .try_insert(crate::relationships::DeliveringTo(data.tank));
        }
        AssignedTask::HaulToMixer(data) => {
            commands
                .entity(data.item)
                .try_insert(crate::relationships::DeliveringTo(data.mixer));
        }
        AssignedTask::HaulWaterToMixer(data) => {
            // バケツがミキサーに向かう
            commands
                .entity(data.bucket)
                .try_insert(crate::relationships::DeliveringTo(data.mixer));
        }
        AssignedTask::HaulWithWheelbarrow(data) => {
            let dest_entity = match data.destination {
                crate::systems::logistics::transport_request::WheelbarrowDestination::Stockpile(
                    e,
                ) => e,
                crate::systems::logistics::transport_request::WheelbarrowDestination::Blueprint(
                    e,
                ) => e,
                crate::systems::logistics::transport_request::WheelbarrowDestination::Mixer {
                    entity,
                    ..
                } => entity,
            };
            // 積載済みアイテムも目的地に向かう
            for &item in &data.items {
                commands
                    .entity(item)
                    .try_insert(crate::relationships::DeliveringTo(dest_entity));
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
    mut q_visibility: Query<&mut Visibility, With<crate::entities::damned_soul::DamnedSoul>>,
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

pub fn task_execution_system(
    mut commands: Commands,
    mut q_souls: TaskExecutionSoulQuery,
    mut queries: context::TaskQueries,
    game_assets: Res<crate::assets::GameAssets>,
    time: Res<Time>,
    // haul_cache is removed
    world_map: Res<WorldMap>,
    mut pf_context: Local<crate::world::pathfinding::PathfindingContext>,
    q_wheelbarrows: Query<
        (&Transform, Option<&crate::relationships::ParkedAt>),
        With<crate::systems::logistics::Wheelbarrow>,
    >,
    q_entities: Query<Entity>,
) {
    for (
        soul_entity,
        soul_transform,
        mut soul,
        mut task,
        mut dest,
        mut path,
        mut inventory,
        breakdown_opt,
    ) in q_souls.iter_mut()
    {
        if let Some(expected_item) = task.expected_item() {
            let needs_item = task.requires_item_in_inventory();
            let expected_item_alive = q_entities.get(expected_item).is_ok();
            let has_expected = inventory.0 == Some(expected_item) && expected_item_alive;
            let has_mismatch = inventory.0.is_some() && !has_expected;
            let missing_required = needs_item && !has_expected;

            if has_mismatch || missing_required {
                unassign_task(
                    &mut commands,
                    soul_entity,
                    soul_transform.translation.truncate(),
                    &mut task,
                    &mut path,
                    Some(&mut inventory),
                    None,
                    &mut queries,
                    // haul_cache removed
                    &world_map,
                    true,
                );
                continue;
            }
        }

        let was_busy = !matches!(*task, AssignedTask::None);
        let old_work_type = task.work_type();
        let old_task_entity = task.get_target_entity();

        // 共通コンテキストの構築
        let mut ctx = TaskExecutionContext {
            soul_entity,
            soul_transform,
            soul: &mut soul,
            task: &mut task,
            dest: &mut dest,
            path: &mut path,
            inventory: &mut inventory,
            pf_context: &mut *pf_context,
            queries: &mut queries,
        };

        // Phase 4: タスクタイプに応じてルーティング（共通ディスパッチ + HaulWithWheelbarrow 特別扱い）
        run_task_handler(
            &mut ctx,
            &mut commands,
            &game_assets,
            &time,
            &world_map,
            breakdown_opt.as_deref(),
            &q_wheelbarrows,
        );

        // 完了イベントの発行
        if was_busy && matches!(*task, AssignedTask::None) {
            if let Some(work_type) = old_work_type {
                // Observer をトリガー
                commands.trigger(OnTaskCompleted {
                    entity: soul_entity,
                    task_entity: old_task_entity.unwrap_or(Entity::PLACEHOLDER),
                    work_type,
                });

                // WorkingOn コンポーネントを削除（これでTaskWorkersも自動更新される）
                commands
                    .entity(soul_entity)
                    .remove::<crate::relationships::WorkingOn>();

                info!(
                    "EVENT: OnTaskCompleted triggered for Soul {:?}",
                    soul_entity
                );
            }
        }
    }
}
