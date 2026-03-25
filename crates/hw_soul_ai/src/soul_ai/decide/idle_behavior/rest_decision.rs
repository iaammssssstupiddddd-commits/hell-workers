//! 休憩所への移動・滞在の決定

use bevy::prelude::*;

use hw_core::constants::*;
use hw_core::events::{IdleBehaviorOperation, IdleBehaviorRequest};
use hw_core::relationships::ParticipatingIn;
use hw_core::soul::{Destination, IdleBehavior, IdleState, Path};
use hw_world::WorldMap;

use super::rest_area::{has_arrived_at_rest_area, nearest_walkable_adjacent_to_rest_area};

fn has_reached_rest_entry(current_pos: Vec2, destination: Vec2, rest_area_center: Vec2) -> bool {
    let near_destination = current_pos.distance_squared(destination) <= (TILE_SIZE * 0.75).powi(2);
    near_destination && has_arrived_at_rest_area(current_pos, rest_area_center)
}

/// 休憩所フロー helper に渡すリクエストコンテキスト。
pub struct RestDecisionCtx<'a> {
    pub entity: Entity,
    pub rest_area_target: Option<(Entity, Vec2)>,
    pub participating_in: Option<&'a ParticipatingIn>,
    pub current_pos: Vec2,
    pub just_reserved: bool,
}

/// 休憩所フロー helper に渡す Soul 移動状態。
pub struct RestMoveState<'a> {
    pub idle: &'a mut IdleState,
    pub dest: &'a mut Destination,
    pub path: &'a mut Path,
}

/// Resting|GoingToRest 状態の場合の休憩所フローを処理。
/// rest_area_target は呼び出し元で事前に解決すること。
/// 継続すべきなら true を返す
pub fn process_resting_or_going_to_rest(
    ctx: RestDecisionCtx<'_>,
    state: RestMoveState<'_>,
    world_map: &WorldMap,
    request_writer: &mut MessageWriter<IdleBehaviorRequest>,
) -> bool {
    let Some((rest_area_entity, rest_area_pos)) = ctx.rest_area_target else {
        return false;
    };

    // RestingIn が無いのに Resting へ残っている不整合を補正する。
    if state.idle.behavior == IdleBehavior::Resting {
        state.idle.behavior = IdleBehavior::GoingToRest;
    }

    if has_reached_rest_entry(ctx.current_pos, state.dest.0, rest_area_pos) {
        if let Some(p) = ctx.participating_in {
            request_writer.write(IdleBehaviorRequest {
                entity: ctx.entity,
                operation: IdleBehaviorOperation::LeaveGathering { spot_entity: p.0 },
            });
        }
        state.idle.idle_timer = 0.0;
        state.idle.total_idle_time = 0.0;
        state.idle.behavior_duration = REST_AREA_RESTING_DURATION;
        state.path.waypoints.clear();
        state.path.current_index = 0;
        if !ctx.just_reserved {
            request_writer.write(IdleBehaviorRequest {
                entity: ctx.entity,
                operation: IdleBehaviorOperation::EnterRestArea { rest_area_entity },
            });
        } else {
            state.dest.0 = ctx.current_pos;
        }
        return true;
    }

    // dest.0 が休憩所の近傍を指していない場合は古いパス（ワンダリング等）とみなしリセット。
    // ただしパスファインディングが設定した代替経路（休憩所隣接タイル）は保護する。
    let dest_is_near_rest_area = has_arrived_at_rest_area(state.dest.0, rest_area_pos);
    let needs_new_path = !dest_is_near_rest_area
        || state.path.waypoints.is_empty()
        || state.path.current_index >= state.path.waypoints.len();
    if needs_new_path {
        state.idle.idle_timer = 0.0;
        state.idle.behavior_duration = REST_AREA_RESTING_DURATION;
        state.dest.0 =
            nearest_walkable_adjacent_to_rest_area(ctx.current_pos, rest_area_pos, world_map);
        state.path.waypoints.clear();
        state.path.current_index = 0;
    }
    true
}

/// wants_rest_area が true の場合の休憩所フローを処理。
/// rest_area_target は呼び出し元で事前に解決すること。
/// 継続すべきなら true を返す
pub fn process_wants_rest_area(
    ctx: RestDecisionCtx<'_>,
    state: RestMoveState<'_>,
    world_map: &WorldMap,
    request_writer: &mut MessageWriter<IdleBehaviorRequest>,
) -> bool {
    let Some((rest_area_entity, rest_area_pos)) = ctx.rest_area_target else {
        return false;
    };

    if has_reached_rest_entry(ctx.current_pos, state.dest.0, rest_area_pos) {
        if let Some(p) = ctx.participating_in {
            request_writer.write(IdleBehaviorRequest {
                entity: ctx.entity,
                operation: IdleBehaviorOperation::LeaveGathering { spot_entity: p.0 },
            });
        }
        // EnterRestArea 成功前は GoingToRest を維持し、Resting は Execute 側で確定する。
        state.idle.behavior = IdleBehavior::GoingToRest;
        state.idle.idle_timer = 0.0;
        state.idle.total_idle_time = 0.0;
        state.idle.behavior_duration = REST_AREA_RESTING_DURATION;
        state.path.waypoints.clear();
        state.path.current_index = 0;
        if !ctx.just_reserved {
            request_writer.write(IdleBehaviorRequest {
                entity: ctx.entity,
                operation: IdleBehaviorOperation::EnterRestArea { rest_area_entity },
            });
        } else {
            state.dest.0 = ctx.current_pos;
        }
        return true;
    }

    if let Some(p) = ctx.participating_in {
        request_writer.write(IdleBehaviorRequest {
            entity: ctx.entity,
            operation: IdleBehaviorOperation::LeaveGathering { spot_entity: p.0 },
        });
    }

    // dest.0 が休憩所の近傍を指していない場合は古いパスとみなしリセット。
    let dest_is_near_rest_area = has_arrived_at_rest_area(state.dest.0, rest_area_pos);
    let needs_new_path = !dest_is_near_rest_area
        || state.path.waypoints.is_empty()
        || state.path.current_index >= state.path.waypoints.len();

    state.idle.behavior = IdleBehavior::GoingToRest;
    if needs_new_path {
        state.idle.idle_timer = 0.0;
        state.idle.behavior_duration = REST_AREA_RESTING_DURATION;
        state.dest.0 =
            nearest_walkable_adjacent_to_rest_area(ctx.current_pos, rest_area_pos, world_map);
        state.path.waypoints.clear();
        state.path.current_index = 0;
    }
    true
}
