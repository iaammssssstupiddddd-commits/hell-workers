//! 使い魔のタスク管理モジュール
//!
//! タスクの検索・割り当てロジックを提供します。

mod task_finder;
mod task_assigner;

pub use task_finder::find_unassigned_task_in_area;
pub use task_assigner::assign_task_to_worker;

use crate::entities::damned_soul::{DamnedSoul, Destination, IdleBehavior, IdleState, Path};
use crate::entities::familiar::UnderCommand;
use crate::relationships::ManagedTasks;
use crate::systems::command::TaskArea;
use crate::systems::soul_ai::gathering::ParticipatingIn;
use crate::systems::soul_ai::task_execution::types::AssignedTask;
use crate::systems::spatial::DesignationSpatialGrid;
use crate::world::map::WorldMap;
use crate::world::pathfinding::PathfindingContext;
use bevy::prelude::*;

/// タスク管理ユーティリティ
pub struct TaskManager;

impl TaskManager {
    /// タスクを委譲する（タスク検索 + 割り当て）
    #[allow(clippy::too_many_arguments)]
    pub fn delegate_task(
        commands: &mut Commands,
        fam_entity: Entity,
        fam_pos: Vec2,
        squad: &[Entity],
        task_area_opt: Option<&TaskArea>,
        fatigue_threshold: f32,
        queries: &crate::systems::soul_ai::task_execution::context::TaskQueries,
        q_souls: &mut Query<
            (
                Entity,
                &Transform,
                &DamnedSoul,
                &mut AssignedTask,
                &mut Destination,
                &mut Path,
                &IdleState,

                Option<&mut crate::systems::logistics::Inventory>,
                Option<&UnderCommand>,
                Option<&ParticipatingIn>,
            ),
            Without<crate::entities::familiar::Familiar>,
        >,
        designation_grid: &DesignationSpatialGrid,
        managed_tasks: &ManagedTasks,
        haul_cache: &mut crate::systems::familiar_ai::resource_cache::SharedResourceCache,
        world_map: &WorldMap,
        pf_context: &mut PathfindingContext,
    ) -> Option<Entity> {
        // 1. 公平性/効率のため、アイドルメンバーを全員リストアップ
        let mut idle_members = Vec::new();
        for &member_entity in squad {
            if let Ok(soul_data) = q_souls.get(member_entity) {
                let (_, transform, soul, task, _, _, idle, _, _, _) = soul_data;
                if matches!(*task, AssignedTask::None)
                    && idle.behavior != IdleBehavior::ExhaustedGathering
                    && soul.fatigue < fatigue_threshold
                {
                    idle_members.push((member_entity, transform.translation.truncate()));
                }
            }
        }

        // 2. 各メンバーに対してタスク候補を順に試みる
        for (worker_entity, pos) in idle_members {
            let candidates = find_unassigned_task_in_area(
                fam_entity,
                fam_pos,
                pos, // 個別ソウルの位置を使用
                task_area_opt,
                queries,
                designation_grid,
                managed_tasks,
                &queries.target_blueprints,
                world_map,
                pf_context,
                haul_cache,
            );

            for task_entity in candidates {
                // アサイン成功！1サイクル1人へのアサインとする（安定性のため）
                if assign_task_to_worker(
                    commands,
                    fam_entity,
                    task_entity,
                    worker_entity,
                    fatigue_threshold,
                    queries,
                    q_souls,
                    task_area_opt,
                    haul_cache,
                ) {
                    return Some(task_entity);
                }
                // このタスクに失敗した場合は、次のタスク候補を試す
            }
        }

        None
    }
}
