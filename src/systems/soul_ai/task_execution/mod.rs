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
pub mod haul_to_mixer;
pub mod haul_water_to_mixer;
pub mod refine;
pub mod types;

// 型の再エクスポート（外部からのアクセスを簡潔に）
pub use types::AssignedTask;

use crate::entities::damned_soul::{DamnedSoul, Destination, Path, StressBreakdown};
use crate::events::OnTaskCompleted;
use crate::systems::familiar_ai::haul_cache::HaulReservationCache;
use crate::systems::logistics::Inventory;
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
use refine::handle_refine_task;

pub fn task_execution_system(
    mut commands: Commands,
    mut q_souls: Query<(
        Entity,
        &Transform,
        &mut DamnedSoul,
        &mut AssignedTask,
        &mut Destination,
        &mut Path,
        &mut Inventory,
        Option<&StressBreakdown>,
    )>,
    mut queries: context::TaskQueries,
    game_assets: Res<crate::assets::GameAssets>,
    time: Res<Time>,
    mut haul_cache: ResMut<HaulReservationCache>,
    world_map: Res<WorldMap>,
    mut pf_context: Local<crate::world::pathfinding::PathfindingContext>,
) {
    let mut dropped_this_frame = std::collections::HashMap::<Entity, usize>::new();

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
                    &mut dropped_this_frame,
                    &mut *haul_cache,
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
                    &mut haul_cache,
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
                    &mut *haul_cache,
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
                    &mut *haul_cache,
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
                    &mut *haul_cache,
                    &time,
                    &world_map,
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
                commands.entity(soul_entity).remove::<crate::relationships::WorkingOn>();

                info!(
                    "EVENT: OnTaskCompleted triggered for Soul {:?}",
                    soul_entity
                );
            }
        }
    }
}
