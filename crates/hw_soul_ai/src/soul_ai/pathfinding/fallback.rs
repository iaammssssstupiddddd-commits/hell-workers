//! 休憩所 fallback・到達不能 cleanup

use bevy::prelude::*;
use hw_core::constants::PATHFINDING_RETRY_COOLDOWN_FRAMES;
use hw_core::relationships::RestAreaReservedFor;
use hw_core::soul::{Destination, IdleBehavior, IdleState, Path};
use hw_world::{PathGoalPolicy, PathfindingContext, WorldMap, find_path};

use crate::soul_ai::execute::task_execution::AssignedTask;
use crate::soul_ai::execute::task_execution::context::TaskAssignmentQueries;
use crate::soul_ai::helpers::work::{SoulDropCtx, cleanup_task_assignment};

use super::PathCooldown;

/// rest area の中心から占有グリッド 4 マスを返す。
fn rest_area_occupied_grids_from_center(center: Vec2) -> [(i32, i32); 4] {
    let top_right = WorldMap::world_to_grid(center);
    [
        (top_right.0 - 1, top_right.1 - 1),
        (top_right.0, top_right.1 - 1),
        (top_right.0 - 1, top_right.1),
        (top_right.0, top_right.1),
    ]
}

/// rest area 周辺の歩行可能候補グリッドを現在位置からの距離順で返す。
fn rest_area_adjacent_candidates(
    center: Vec2,
    current_pos: Vec2,
    world_map: &WorldMap,
) -> Vec<(i32, i32)> {
    let occupied = rest_area_occupied_grids_from_center(center);
    let directions: [(i32, i32); 8] = [
        (0, 1),
        (0, -1),
        (1, 0),
        (-1, 0),
        (1, 1),
        (1, -1),
        (-1, 1),
        (-1, -1),
    ];

    let mut candidates: Vec<(i32, i32)> = occupied
        .iter()
        .flat_map(|&(gx, gy)| directions.iter().map(move |&(dx, dy)| (gx + dx, gy + dy)))
        .filter(|grid| !occupied.contains(grid))
        .filter(|&(gx, gy)| world_map.is_walkable(gx, gy))
        .collect();

    candidates.sort_unstable();
    candidates.dedup();
    candidates.sort_by(|a, b| {
        let a_pos = WorldMap::grid_to_world(a.0, a.1);
        let b_pos = WorldMap::grid_to_world(b.0, b.1);
        a_pos
            .distance_squared(current_pos)
            .partial_cmp(&b_pos.distance_squared(current_pos))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    candidates
}

/// `try_rest_area_fallback_path` へのソウルのグリッド位置情報。
pub(super) struct SoulGridPos {
    pub entity: Entity,
    pub current_pos: Vec2,
    pub start_grid: (i32, i32),
    pub goal_grid: (i32, i32),
}

/// `try_rest_area_fallback_path` へのパス探索コンテキスト。
pub(super) struct FallbackPfState<'a> {
    pub world_map: &'a WorldMap,
    pub pf_context: &'a mut PathfindingContext,
}

/// `cleanup_unreachable_destination` の Soul 位置・タスク情報。
pub(super) struct SoulEntityCtx<'a> {
    pub entity: Entity,
    pub transform: &'a Transform,
    pub current_pos: Vec2,
    pub has_task: bool,
}

/// `cleanup_unreachable_destination` の Soul 移動状態。
pub(super) struct SoulMoveState<'a> {
    pub idle: &'a mut IdleState,
    pub destination: &'a mut Destination,
    pub task: &'a mut AssignedTask,
    pub path: &'a mut Path,
}

/// 休憩所の周辺タイルへの代替パスを探す（GoingToRest の idle worker 専用）。
/// 代替パスが見つかった場合は destination と path を更新して true を返す。
pub(super) fn try_rest_area_fallback_path(
    commands: &mut Commands,
    destination: &mut Destination,
    path: &mut Path,
    rest_reserved_for: Option<&RestAreaReservedFor>,
    q_rest_areas: &Query<&Transform, With<hw_jobs::RestArea>>,
    soul_grid: SoulGridPos,
    pf: FallbackPfState<'_>,
) -> bool {
    let Some(reserved) = rest_reserved_for else {
        return false;
    };
    let Ok(rest_transform) = q_rest_areas.get(reserved.0) else {
        return false;
    };

    let FallbackPfState {
        world_map,
        pf_context,
    } = pf;
    let rest_center = rest_transform.translation.truncate();
    for candidate_grid in
        rest_area_adjacent_candidates(rest_center, soul_grid.current_pos, world_map)
            .into_iter()
            .filter(|grid| *grid != soul_grid.goal_grid)
    {
        if let Some(candidate_path) = find_path(
            world_map,
            pf_context,
            soul_grid.start_grid,
            candidate_grid,
            PathGoalPolicy::RespectGoalWalkability,
        ) {
            destination.0 = WorldMap::grid_to_world(candidate_grid.0, candidate_grid.1);
            path.waypoints = candidate_path
                .iter()
                .map(|&(x, y)| WorldMap::grid_to_world(x, y))
                .collect();
            path.current_index = 0;
            commands.entity(soul_grid.entity).remove::<PathCooldown>();
            return true;
        }
    }
    false
}

/// 到達不能な destination を破棄し PathCooldown を付与する。
/// idle の GoingToRest なら予約を解放し、タスク実行中なら unassign する。
pub(super) fn cleanup_unreachable_destination(
    commands: &mut Commands,
    soul: SoulEntityCtx<'_>,
    state: SoulMoveState<'_>,
    inventory_opt: Option<&mut hw_logistics::Inventory>,
    queries: &mut TaskAssignmentQueries,
    world_map: &WorldMap,
) {
    state.path.waypoints.clear();
    commands.entity(soul.entity).insert(PathCooldown {
        remaining_frames: PATHFINDING_RETRY_COOLDOWN_FRAMES,
    });

    // 休憩所に到達不能な予約を握り続けると、容量が詰まって
    // 他の非使役 Soul も休憩に向かえなくなるため解放する。
    if !soul.has_task && state.idle.behavior == IdleBehavior::GoingToRest {
        commands.entity(soul.entity).remove::<RestAreaReservedFor>();
        state.idle.behavior = IdleBehavior::Wandering;
        state.idle.idle_timer = 0.0;
        state.idle.behavior_duration = 3.0;
        state.destination.0 = soul.current_pos;
    }

    // タスク実行中なら放棄
    if soul.has_task {
        info!(
            "PATH: Soul {:?} abandoning task due to unreachable destination",
            soul.entity
        );
        cleanup_task_assignment(
            commands,
            SoulDropCtx {
                soul_entity: soul.entity,
                drop_pos: soul.transform.translation.truncate(),
                inventory: inventory_opt,
                dropped_item_res: None,
            },
            state.task,
            state.path,
            queries,
            world_map,
            true,
        );
        commands
            .entity(soul.entity)
            .remove::<hw_core::relationships::WorkingOn>();
    }
}
