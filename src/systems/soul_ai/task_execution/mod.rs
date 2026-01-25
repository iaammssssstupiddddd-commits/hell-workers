//! タスク実行モジュール
//!
//! 魂に割り当てられたタスクの実行ロジックを提供します。

pub mod build;
pub mod common;
pub mod context;
pub mod gather;
pub mod gather_water;
pub mod haul;
pub mod haul_to_blueprint;
pub mod types;

// 型の再エクスポート（外部からのアクセスを簡潔に）
pub use types::AssignedTask;

use crate::entities::damned_soul::{DamnedSoul, Destination, Path, StressBreakdown};
use crate::events::OnTaskCompleted;
use crate::relationships::Holding;
use crate::systems::familiar_ai::haul_cache::HaulReservationCache;
use crate::systems::jobs::{Blueprint, Designation, DesignationCreatedEvent};
use crate::systems::logistics::Stockpile;
use crate::world::map::WorldMap;
use bevy::prelude::*;

use build::handle_build_task;
use context::TaskExecutionContext;
use gather::handle_gather_task;
use gather_water::handle_gather_water_task;
use haul::handle_haul_task;
use haul_to_blueprint::handle_haul_to_blueprint_task;

/// タスク実行システム
///
/// 各魂の割り当てられたタスクを実行し、フェーズに応じて処理を進めます。
pub fn task_execution_system(
    mut commands: Commands,
    mut q_souls: Query<(
        Entity,
        &Transform,
        &mut DamnedSoul,
        &mut AssignedTask,
        &mut Destination,
        &mut Path,
        Option<&Holding>,
        Option<&StressBreakdown>,
    )>,
    q_targets: Query<(
        &Transform,
        Option<&crate::systems::jobs::Tree>,
        Option<&crate::systems::jobs::Rock>,
        Option<&crate::systems::logistics::ResourceItem>,
        Option<&Designation>,
        Option<&crate::relationships::StoredIn>,
    )>,
    q_designations: Query<(
        Entity,
        &Transform,
        &Designation,
        Option<&crate::systems::jobs::IssuedBy>,
        Option<&crate::systems::jobs::TaskSlots>,
        Option<&crate::relationships::TaskWorkers>,
    )>,
    mut q_stockpiles: Query<(
        &Transform,
        &mut Stockpile,
        Option<&crate::relationships::StoredItems>,
    )>,
    game_assets: Res<crate::assets::GameAssets>,
    mut ev_created: MessageWriter<DesignationCreatedEvent>,
    time: Res<Time>,
    mut haul_cache: ResMut<HaulReservationCache>,
    mut q_blueprints: Query<(&Transform, &mut Blueprint, Option<&Designation>)>,
    world_map: Res<WorldMap>,
) {
    let mut dropped_this_frame = std::collections::HashMap::<Entity, usize>::new();

    for (
        soul_entity,
        soul_transform,
        mut soul,
        mut task,
        mut dest,
        mut path,
        holding_opt,
        breakdown_opt,
    ) in q_souls.iter_mut()
    {
        let was_busy = !matches!(*task, AssignedTask::None);
        let old_work_type = task.work_type();
        let old_task_entity = task.get_target_entity();

        // タスクタイプに応じてルーティング
        match *task {
            AssignedTask::Gather {
                target,
                work_type,
                phase,
            } => {
                let mut ctx = TaskExecutionContext {
                    soul_entity,
                    soul_transform,
                    soul: &mut soul,
                    task: &mut task,
                    dest: &mut dest,
                    path: &mut path,
                };
                handle_gather_task(
                    &mut ctx,
                    target,
                    &work_type,
                    phase,
                    &q_targets,
                    &mut commands,
                    &game_assets,
                    &time,
                    &world_map,
                );
            }
            AssignedTask::Haul {
                item,
                stockpile,
                phase,
            } => {
                let mut ctx = TaskExecutionContext {
                    soul_entity,
                    soul_transform,
                    soul: &mut soul,
                    task: &mut task,
                    dest: &mut dest,
                    path: &mut path,
                };
                handle_haul_task(
                    &mut ctx,
                    holding_opt,
                    item,
                    stockpile,
                    phase,
                    &q_targets,
                    &mut q_stockpiles,
                    &mut commands,
                    &mut dropped_this_frame,
                    &mut *haul_cache,
                    &world_map,
                );
            }
            AssignedTask::Build { blueprint, phase } => {
                let mut ctx = TaskExecutionContext {
                    soul_entity,
                    soul_transform,
                    soul: &mut soul,
                    task: &mut task,
                    dest: &mut dest,
                    path: &mut path,
                };
                handle_build_task(
                    &mut ctx,
                    blueprint,
                    phase,
                    &mut q_blueprints,
                    &mut commands,
                    &time,
                    &world_map,
                );
            }
            AssignedTask::HaulToBlueprint {
                item,
                blueprint,
                phase,
            } => {
                let mut ctx = TaskExecutionContext {
                    soul_entity,
                    soul_transform,
                    soul: &mut soul,
                    task: &mut task,
                    dest: &mut dest,
                    path: &mut path,
                };
                handle_haul_to_blueprint_task(
                    &mut ctx,
                    holding_opt,
                    breakdown_opt,
                    item,
                    blueprint,
                    phase,
                    &q_targets,
                    &q_designations,
                    &mut q_blueprints,
                    &mut q_stockpiles,
                    &mut haul_cache,
                    &mut commands,
                    &mut ev_created,
                    &world_map,
                );
            }
            AssignedTask::GatherWater { bucket, tank, phase } => {
                let mut ctx = TaskExecutionContext {
                    soul_entity,
                    soul_transform,
                    soul: &mut soul,
                    task: &mut task,
                    dest: &mut dest,
                    path: &mut path,
                };
                handle_gather_water_task(
                    &mut ctx,
                    bucket,
                    tank,
                    phase,
                    &q_targets,
                    &mut commands,
                    &game_assets,
                    &time,
                    &world_map,
                    holding_opt,
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

                info!(
                    "EVENT: OnTaskCompleted triggered for Soul {:?}",
                    soul_entity
                );
            }
        }
    }
}
