//! タスク実行モジュール
//!
//! 魂に割り当てられたタスクの実行ロジックを提供します。

pub mod bucket_transport;
pub mod build;
pub mod coat_wall;
pub mod collect_bone;
pub mod collect_sand;
pub mod common;
pub mod context;
pub mod frame_wall;
pub mod gather;
pub mod handler;
pub mod haul;
pub mod haul_to_blueprint;
pub mod haul_to_mixer;
pub mod haul_with_wheelbarrow;
pub mod move_plant;
pub mod pour_floor;
pub mod refine;
pub mod reinforce_floor;
pub mod transport_common;
pub mod types;

// 型の再エクスポート（外部からのアクセスを簡潔に）
pub use types::AssignedTask;

// apply_task_assignment_requests_system は hw_ai に移設済み
pub use hw_ai::soul_ai::execute::task_assignment_apply::apply_task_assignment_requests_system;

use crate::events::OnTaskCompleted;
use crate::systems::soul_ai::helpers::query_types::TaskExecutionSoulQuery;
use crate::systems::soul_ai::helpers::work::unassign_task;
use crate::world::map::WorldMapRead;
use bevy::prelude::*;

use context::TaskExecutionContext;
use handler::run_task_handler;

pub fn task_execution_system(
    mut commands: Commands,
    mut q_souls: TaskExecutionSoulQuery,
    mut queries: context::TaskQueries,
    game_assets: Res<crate::assets::GameAssets>,
    time: Res<Time>,
    // haul_cache is removed
    world_map: WorldMapRead,
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
                    world_map.as_ref(),
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
            world_map.as_ref(),
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
