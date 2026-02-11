//! タスク実行モジュール
//!
//! 魂に割り当てられたタスクの実行ロジックを提供します。

pub mod build;
pub mod collect_sand;
pub mod common;
pub mod context;
pub mod gather;
pub mod gather_water;
pub mod haul;
pub mod haul_to_blueprint;
pub mod haul_with_wheelbarrow;
pub mod haul_to_mixer;
pub mod haul_water_to_mixer;
pub mod refine;
pub mod types;

// 型の再エクスポート（外部からのアクセスを簡潔に）
pub use types::AssignedTask;

use crate::entities::damned_soul::IdleBehavior;
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

use build::handle_build_task;
use collect_sand::handle_collect_sand_task;
use context::TaskExecutionContext;
use gather::handle_gather_task;
use gather_water::handle_gather_water_task;
use haul::handle_haul_task;
use haul_to_blueprint::handle_haul_to_blueprint_task;
use haul_to_mixer::handle_haul_to_mixer_task;
use haul_water_to_mixer::handle_haul_water_to_mixer_task;
use haul_with_wheelbarrow::handle_haul_with_wheelbarrow_task;
use refine::handle_refine_task;

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
    commands.entity(worker_entity).insert((
        crate::relationships::CommandedBy(familiar_entity),
        crate::relationships::WorkingOn(task_entity),
    ));
    commands
        .entity(task_entity)
        .insert(crate::systems::jobs::IssuedBy(familiar_entity));
}

/// Thinkで生成されたタスク割り当て要求を適用する
pub fn apply_task_assignment_requests_system(
    mut commands: Commands,
    mut requests: MessageReader<TaskAssignmentRequest>,
    mut cache: ResMut<SharedResourceCache>,
    mut q_souls: TaskAssignmentSoulQuery,
) {
    for request in requests.read() {
        let Ok((
            worker_entity,
            worker_transform,
            mut assigned_task,
            mut dest,
            mut path,
            idle,
            _inventory_opt,
            under_command_opt,
            participating_opt,
        )) = q_souls.get_mut(request.worker_entity)
        else {
            warn!(
                "ASSIGN_REQUEST: Worker {:?} not found",
                request.worker_entity
            );
            continue;
        };

        if !matches!(*assigned_task, AssignedTask::None) {
            continue;
        }
        if idle.behavior == IdleBehavior::ExhaustedGathering {
            continue;
        }

        if let Some(p) = participating_opt {
            commands
                .entity(worker_entity)
                .remove::<crate::systems::soul_ai::helpers::gathering::ParticipatingIn>();
            commands.trigger(OnGatheringLeft {
                entity: worker_entity,
                spot_entity: p.0,
            });
        }

        prepare_worker_for_task_apply(
            &mut commands,
            worker_entity,
            request.familiar_entity,
            request.task_entity,
            request.already_commanded || under_command_opt.is_some(),
        );

        *assigned_task = request.assigned_task.clone();
        dest.0 = request.task_pos;
        path.waypoints.clear();
        path.current_index = 0;

        for op in &request.reservation_ops {
            apply_reservation_op(&mut cache, op);
        }

        commands.trigger(OnTaskAssigned {
            entity: worker_entity,
            task_entity: request.task_entity,
            work_type: request.work_type,
        });

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
        (
            &Transform,
            Option<&crate::relationships::ParkedAt>,
        ),
        With<crate::systems::logistics::Wheelbarrow>,
    >,
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
            let has_expected = inventory.0 == Some(expected_item);
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

        // タスクタイプに応じてルーティング
        match &*ctx.task {
            AssignedTask::Gather(data) => {
                let data = data.clone();
                handle_gather_task(
                    &mut ctx,
                    data.target,
                    &data.work_type,
                    data.phase,
                    &mut commands,
                    &game_assets,
                    &time,
                    &world_map,
                );
            }
            AssignedTask::Haul(data) => {
                let data = data.clone();
                handle_haul_task(
                    &mut ctx,
                    data.item,
                    data.stockpile,
                    data.phase,
                    &mut commands,
                    // haul_cache removed
                    &world_map,
                );
            }
            AssignedTask::Build(data) => {
                let data = data.clone();
                handle_build_task(
                    &mut ctx,
                    data.blueprint,
                    data.phase,
                    &mut commands,
                    &time,
                    &world_map,
                );
            }
            AssignedTask::HaulToBlueprint(data) => {
                let data = data.clone();
                handle_haul_to_blueprint_task(
                    &mut ctx,
                    breakdown_opt,
                    data.item,
                    data.blueprint,
                    data.phase,
                    &mut commands,
                    &world_map,
                );
            }
            AssignedTask::GatherWater(data) => {
                let data = data.clone();
                handle_gather_water_task(
                    &mut ctx,
                    data.bucket,
                    data.tank,
                    data.phase,
                    &mut commands,
                    &game_assets,
                    // haul_cache removed
                    &time,
                    &world_map,
                );
            }
            AssignedTask::CollectSand(data) => {
                let data = data.clone();
                handle_collect_sand_task(
                    &mut ctx,
                    data.target,
                    data.phase,
                    &mut commands,
                    &game_assets,
                    &time,
                    &world_map,
                );
            }
            AssignedTask::Refine(data) => {
                let data = data.clone();
                handle_refine_task(
                    &mut ctx,
                    data.mixer,
                    data.phase,
                    &mut commands,
                    &game_assets,
                    &time,
                    &world_map,
                );
            }
            AssignedTask::HaulToMixer(data) => {
                let data = data.clone();
                handle_haul_to_mixer_task(
                    &mut ctx,
                    data.item,
                    data.mixer,
                    data.resource_type,
                    data.phase,
                    &mut commands,
                    // haul_cache removed
                    &world_map,
                );
            }
            AssignedTask::HaulWaterToMixer(data) => {
                let data = data.clone();
                handle_haul_water_to_mixer_task(
                    &mut ctx,
                    data.bucket,
                    data.tank,
                    data.mixer,
                    data.phase,
                    &mut commands,
                    &game_assets,
                    // haul_cache removed
                    &time,
                    &world_map,
                );
            }
            AssignedTask::HaulWithWheelbarrow(data) => {
                let data = data.clone();
                handle_haul_with_wheelbarrow_task(
                    &mut ctx,
                    data,
                    &mut commands,
                    &world_map,
                    &q_wheelbarrows,
                );
            }
            AssignedTask::None => {}
        }

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
