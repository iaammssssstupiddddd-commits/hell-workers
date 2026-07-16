//! パスキャッシュ検証・経路設定ヘルパー

use bevy::prelude::*;
use hw_core::soul::{Destination, Path};
use hw_world::{
    PathGoalPolicy, PathSearchCaller, PathSearchResult, PathfindingContext,
    RuntimePathSearchBudget, WorldMap, find_path_to_adjacent_with_budget,
    find_path_to_boundary_with_budget, find_path_with_budget,
};
use std::collections::HashMap;

use crate::soul_ai::execute::task_execution::context::TaskExecutionContext;

/// Task handler の direct → adjacent 継続位置。
///
/// Handler は `Deferred` 時に task/予約/Destination/Path を一切変えないため、
/// continuation も component には保存せず system-local に保持する。WorldEpoch が
/// 変わると所有する `EpochLocal` ごと初期化される。
#[derive(Default)]
pub struct TaskPathSearchProgress {
    entries: HashMap<(Entity, PathSearchCaller), TaskPathContinuation>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct TaskPathFingerprint {
    start_grid: (i32, i32),
    target_grid: (i32, i32),
    obstacle_version: u64,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum TaskPathStage {
    Direct,
    Adjacent,
}

#[derive(Clone, Copy)]
struct TaskPathContinuation {
    fingerprint: TaskPathFingerprint,
    stage: TaskPathStage,
}

impl TaskPathSearchProgress {
    fn stage_for(
        &mut self,
        entity: Entity,
        caller: PathSearchCaller,
        fingerprint: TaskPathFingerprint,
    ) -> TaskPathStage {
        let continuation = self
            .entries
            .entry((entity, caller))
            .or_insert(TaskPathContinuation {
                fingerprint,
                stage: TaskPathStage::Direct,
            });
        if continuation.fingerprint != fingerprint {
            *continuation = TaskPathContinuation {
                fingerprint,
                stage: TaskPathStage::Direct,
            };
        }
        continuation.stage
    }

    fn advance_to_adjacent(&mut self, entity: Entity, caller: PathSearchCaller) {
        if let Some(continuation) = self.entries.get_mut(&(entity, caller)) {
            continuation.stage = TaskPathStage::Adjacent;
        }
    }

    fn finish(&mut self, entity: Entity, caller: PathSearchCaller) {
        self.entries.remove(&(entity, caller));
    }

    pub(crate) fn clear_entity(&mut self, entity: Entity) {
        self.entries.retain(|(entry, _), _| *entry != entity);
    }
}

fn apply_grid_path(path: &mut Path, dest: &mut Destination, grid_path: &[(i32, i32)]) {
    if let Some(&last_grid) = grid_path.last() {
        dest.0 = WorldMap::grid_to_world(last_grid.0, last_grid.1);
    }
    path.waypoints = grid_path
        .iter()
        .map(|&(x, y)| WorldMap::grid_to_world(x, y))
        .collect();
    path.current_index = 0;
}

struct AdjacentPathSearchInputs<'a> {
    entity: Entity,
    dest: &'a mut Destination,
    target_pos: Vec2,
    path: &'a mut Path,
    soul_pos: Vec2,
    world_map: &'a WorldMap,
    pf_context: &'a mut PathfindingContext,
    budget: &'a mut RuntimePathSearchBudget,
    caller: PathSearchCaller,
}

/// Task handler用の再開可能な隣接経路更新。
pub fn update_task_destination_to_adjacent(
    ctx: &mut TaskExecutionContext,
    target_pos: Vec2,
) -> PathSearchResult<()> {
    update_destination_to_adjacent_for_caller(ctx, target_pos, PathSearchCaller::TaskExecution)
}

/// Bucket transport用の再開可能な隣接経路更新。
pub fn update_bucket_destination_to_adjacent(
    ctx: &mut TaskExecutionContext,
    target_pos: Vec2,
) -> PathSearchResult<()> {
    update_destination_to_adjacent_for_caller(ctx, target_pos, PathSearchCaller::BucketTransport)
}

/// Context を持つ caller 指定版。
///
/// Task execution と bucket transport は経路設定契約を共有するが、
/// profiling では実際に枠を消費した subsystem を区別する。
fn update_destination_to_adjacent_for_caller(
    ctx: &mut TaskExecutionContext,
    target_pos: Vec2,
    caller: PathSearchCaller,
) -> PathSearchResult<()> {
    let soul_pos = ctx.soul_transform.translation.truncate();
    update_destination_to_adjacent_resumable(
        ctx.path_search_progress,
        AdjacentPathSearchInputs {
            entity: ctx.soul_entity,
            dest: &mut ctx.dest,
            target_pos,
            path: &mut ctx.path,
            soul_pos,
            world_map: ctx.env.world_map,
            pf_context: ctx.pf_context,
            budget: ctx.path_budget,
            caller,
        },
    )
}

fn update_destination_to_adjacent_resumable(
    progress: &mut TaskPathSearchProgress,
    inputs: AdjacentPathSearchInputs<'_>,
) -> PathSearchResult<()> {
    let AdjacentPathSearchInputs {
        entity,
        dest,
        target_pos,
        path,
        soul_pos,
        world_map,
        pf_context,
        budget,
        caller,
    } = inputs;
    let target_grid = WorldMap::world_to_grid(target_pos);
    let start_grid = WorldMap::world_to_grid(soul_pos);

    // すでに有効なパスがあり、目的地も変わっていないならスキップ
    if !path.waypoints.is_empty()
        && path.current_index < path.waypoints.len()
        && let Some(last_wp) = path.waypoints.last()
    {
        let last_grid = WorldMap::world_to_grid(*last_wp);
        // 終点がターゲットに隣接していれば、そのパスは有効
        let dx = (last_grid.0 - target_grid.0).abs();
        let dy = (last_grid.1 - target_grid.1).abs();
        if dx <= 1 && dy <= 1 {
            // 目的地をパスの終点に更新（is_near_target_or_destで正しく判定するため）
            dest.0 = *last_wp;
            progress.finish(entity, caller);
            return PathSearchResult::Found(());
        }
    }

    let fingerprint = TaskPathFingerprint {
        start_grid,
        target_grid,
        obstacle_version: world_map.obstacle_version,
    };
    let stage = progress.stage_for(entity, caller, fingerprint);

    if stage == TaskPathStage::Direct {
        match find_path_with_budget(
            world_map,
            pf_context,
            budget,
            caller,
            start_grid,
            target_grid,
            PathGoalPolicy::RespectGoalWalkability,
        ) {
            PathSearchResult::Found(grid_path) => {
                apply_grid_path(path, dest, &grid_path);
                progress.finish(entity, caller);
                return PathSearchResult::Found(());
            }
            PathSearchResult::Deferred => return PathSearchResult::Deferred,
            PathSearchResult::Unreachable => progress.advance_to_adjacent(entity, caller),
        }
    }

    let grid_path = find_path_to_adjacent_with_budget(
        world_map,
        pf_context,
        budget,
        caller,
        start_grid,
        target_grid,
        true,
    );

    match grid_path {
        PathSearchResult::Found(grid_path) => {
            apply_grid_path(path, dest, &grid_path);
            progress.finish(entity, caller);
            PathSearchResult::Found(())
        }
        PathSearchResult::Unreachable => {
            progress.finish(entity, caller);
            PathSearchResult::Unreachable
        }
        PathSearchResult::Deferred => PathSearchResult::Deferred,
    }
}

/// 設計図への到達パスを設定（予定地の中心を一意なターゲットとする）
///
/// 到達可能な経路（または既に到着済み）を設定する。
pub fn update_destination_to_blueprint(
    dest: &mut Destination,
    occupied_grids: &[(i32, i32)],
    path: &mut Path,
    soul_pos: Vec2,
    world_map: &WorldMap,
    pf_context: &mut PathfindingContext,
    budget: &mut RuntimePathSearchBudget,
) -> PathSearchResult<()> {
    use crate::soul_ai::helpers::navigation::{is_near_blueprint, update_destination_if_needed};

    let start_grid = WorldMap::world_to_grid(soul_pos);

    // 現在地がすでにゴール条件を満たしているかチェック
    if is_near_blueprint(soul_pos, occupied_grids) {
        // 到着済みなら、不要なパス（予定地内へ続くものなど）を消去して停止させる
        if !path.waypoints.is_empty() {
            path.waypoints.clear();
            path.current_index = 0;
            dest.0 = soul_pos;
        }
        return PathSearchResult::Found(());
    }

    // 現在のパスが既に有効（ターゲットの隣接点に向かっている）なら再計算しない
    if !path.waypoints.is_empty()
        && let Some(last_wp) = path.waypoints.last()
    {
        let last_grid = WorldMap::world_to_grid(*last_wp);

        // 終点が予定地外かつターゲットに隣接していれば、そのパスは有効
        if !occupied_grids.contains(&last_grid) {
            for &(gx, gy) in occupied_grids {
                let dx = (last_grid.0 - gx).abs();
                let dy = (last_grid.1 - gy).abs();
                if dx <= 1 && dy <= 1 {
                    return PathSearchResult::Found(());
                }
            }
        }
    }

    // ターゲットの中心地点を軸に「境界」までのパスを計算
    match find_path_to_boundary_with_budget(
        world_map,
        pf_context,
        budget,
        PathSearchCaller::TaskExecution,
        start_grid,
        occupied_grids,
    ) {
        PathSearchResult::Found(grid_path) => {
            let Some(last_grid) = grid_path.last() else {
                return PathSearchResult::Unreachable;
            };
            let last_pos = WorldMap::grid_to_world(last_grid.0, last_grid.1);
            update_destination_if_needed(dest, last_pos, path);

            path.waypoints = grid_path
                .iter()
                .map(|&(x, y)| WorldMap::grid_to_world(x, y))
                .collect();
            path.current_index = 0;
            PathSearchResult::Found(())
        }
        PathSearchResult::Unreachable => PathSearchResult::Unreachable,
        PathSearchResult::Deferred => PathSearchResult::Deferred,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hw_core::constants::MAP_HEIGHT;

    #[test]
    fn deferred_adjacent_retry_does_not_repeat_failed_direct_search() {
        let mut world_map = WorldMap::default();
        for y in 0..MAP_HEIGHT {
            world_map.add_grid_obstacle((50, y));
        }

        let soul_pos = WorldMap::grid_to_world(25, 50);
        let target_pos = WorldMap::grid_to_world(75, 50);
        let mut destination = Destination(soul_pos);
        let mut path = Path::default();
        let mut context = PathfindingContext::default();
        let mut progress = TaskPathSearchProgress::default();
        let entity = Entity::from_bits(1);
        let mut budget = RuntimePathSearchBudget::new(1);

        let first = update_destination_to_adjacent_resumable(
            &mut progress,
            AdjacentPathSearchInputs {
                entity,
                dest: &mut destination,
                target_pos,
                path: &mut path,
                soul_pos,
                world_map: &world_map,
                pf_context: &mut context,
                budget: &mut budget,
                caller: PathSearchCaller::TaskExecution,
            },
        );
        assert_eq!(first, PathSearchResult::Deferred);
        assert_eq!(budget.used(), 1);

        budget.reset();
        let second = update_destination_to_adjacent_resumable(
            &mut progress,
            AdjacentPathSearchInputs {
                entity,
                dest: &mut destination,
                target_pos,
                path: &mut path,
                soul_pos,
                world_map: &world_map,
                pf_context: &mut context,
                budget: &mut budget,
                caller: PathSearchCaller::TaskExecution,
            },
        );

        // The second call consumes one adjacent search and reaches the final
        // unreachable result. A repeated direct search would defer again.
        assert_eq!(second, PathSearchResult::Unreachable);
        assert_eq!(budget.used(), 1);
        assert!(progress.entries.is_empty());
    }
}
