//! 休憩所への移動・滞在の決定

use bevy::prelude::*;

use hw_core::constants::*;
use hw_core::events::{IdleBehaviorOperation, IdleBehaviorRequest};
use hw_core::relationships::ParticipatingIn;
use hw_core::soul::{Destination, IdleBehavior, IdleState, Path};

/// 休憩所フロー helper に渡すリクエストコンテキスト。
pub(super) struct RestDecisionCtx<'a> {
    pub entity: Entity,
    pub rest_area_target: (Entity, Vec2),
    pub participating_in: Option<&'a ParticipatingIn>,
    pub current_pos: Vec2,
    pub just_reserved: bool,
}

/// 休憩所フロー helper に渡す Soul 移動状態。
pub(super) struct RestMoveState<'a> {
    pub idle: &'a mut IdleState,
    pub dest: &'a mut Destination,
    pub path: &'a mut Path,
}

/// Moves toward the selected reachable entry and requests admission after arrival.
pub(super) fn process_rest_target(
    ctx: RestDecisionCtx<'_>,
    state: RestMoveState<'_>,
    request_writer: &mut MessageWriter<IdleBehaviorRequest>,
) {
    let (rest_area_entity, entry) = ctx.rest_area_target;
    state.idle.behavior = IdleBehavior::GoingToRest;
    if let Some(p) = ctx.participating_in {
        request_writer.write(IdleBehaviorRequest {
            entity: ctx.entity,
            operation: IdleBehaviorOperation::LeaveGathering { spot_entity: p.0 },
        });
    }
    if ctx.current_pos.distance_squared(entry) <= (TILE_SIZE * 0.75).powi(2) {
        state.idle.idle_timer = 0.0;
        state.idle.total_idle_time = 0.0;
        state.idle.behavior_duration = REST_AREA_RESTING_DURATION;
        state.path.waypoints.clear();
        state.path.current_index = 0;
        state.dest.0 = entry;
        if !ctx.just_reserved {
            request_writer.write(IdleBehaviorRequest {
                entity: ctx.entity,
                operation: IdleBehaviorOperation::EnterRestArea { rest_area_entity },
            });
        }
    } else if ctx.just_reserved || state.dest.0 != entry {
        state.idle.idle_timer = 0.0;
        state.idle.behavior_duration = REST_AREA_RESTING_DURATION;
        state.dest.0 = entry;
        state.path.waypoints.clear();
        state.path.current_index = 0;
    }
}
