//! 休憩所への移動・滞在の決定

use bevy::prelude::*;

use crate::constants::*;
use crate::entities::damned_soul::{Destination, IdleBehavior, IdleState, Path};
use crate::events::{IdleBehaviorOperation, IdleBehaviorRequest};
use crate::relationships::ParticipatingIn;
use crate::world::map::WorldMap;

use super::rest_area::{has_arrived_at_rest_area, nearest_walkable_adjacent_to_rest_area};

/// Resting|GoingToRest 状態の場合の休憩所フローを処理。
/// rest_area_target は呼び出し元で事前に解決すること。
/// 継続すべきなら true を返す
pub fn process_resting_or_going_to_rest(
    entity: Entity,
    rest_area_target: Option<(Entity, Vec2)>,
    _reserved_rest_area: Option<Entity>,
    participating_in: Option<&ParticipatingIn>,
    idle: &mut IdleState,
    dest: &mut Destination,
    path: &mut Path,
    world_map: &WorldMap,
    request_writer: &mut MessageWriter<IdleBehaviorRequest>,
    current_pos: Vec2,
    just_reserved: bool,
) -> bool {
    let Some((rest_area_entity, rest_area_pos)) = rest_area_target else {
        return false;
    };

    // RestingIn が無いのに Resting へ残っている不整合を補正する。
    if idle.behavior == IdleBehavior::Resting {
        idle.behavior = IdleBehavior::GoingToRest;
    }

    if has_arrived_at_rest_area(current_pos, rest_area_pos) {
        if let Some(p) = participating_in {
            request_writer.write(IdleBehaviorRequest {
                entity,
                operation: IdleBehaviorOperation::LeaveGathering { spot_entity: p.0 },
            });
        }
        idle.idle_timer = 0.0;
        idle.total_idle_time = 0.0;
        idle.behavior_duration = REST_AREA_RESTING_DURATION;
        path.waypoints.clear();
        path.current_index = 0;
        if !just_reserved {
            request_writer.write(IdleBehaviorRequest {
                entity,
                operation: IdleBehaviorOperation::EnterRestArea { rest_area_entity },
            });
        } else {
            dest.0 = current_pos;
        }
        return true;
    }

    let destination_changed = dest.0.distance_squared(rest_area_pos) > (TILE_SIZE * 2.5).powi(2);
    let needs_new_path = destination_changed
        || path.waypoints.is_empty()
        || path.current_index >= path.waypoints.len();
    if needs_new_path {
        idle.idle_timer = 0.0;
        idle.behavior_duration = REST_AREA_RESTING_DURATION;
        dest.0 = nearest_walkable_adjacent_to_rest_area(current_pos, rest_area_pos, world_map);
        path.waypoints.clear();
        path.current_index = 0;
    }
    true
}

/// wants_rest_area が true の場合の休憩所フローを処理。
/// rest_area_target は呼び出し元で事前に解決すること。
/// 継続すべきなら true を返す
pub fn process_wants_rest_area(
    entity: Entity,
    rest_area_target: Option<(Entity, Vec2)>,
    _reserved_rest_area: Option<Entity>,
    participating_in: Option<&ParticipatingIn>,
    idle: &mut IdleState,
    dest: &mut Destination,
    path: &mut Path,
    world_map: &WorldMap,
    request_writer: &mut MessageWriter<IdleBehaviorRequest>,
    current_pos: Vec2,
    just_reserved: bool,
) -> bool {
    let Some((rest_area_entity, rest_area_pos)) = rest_area_target else {
        return false;
    };

    if has_arrived_at_rest_area(current_pos, rest_area_pos) {
        if let Some(p) = participating_in {
            request_writer.write(IdleBehaviorRequest {
                entity,
                operation: IdleBehaviorOperation::LeaveGathering { spot_entity: p.0 },
            });
        }
        // EnterRestArea 成功前は GoingToRest を維持し、Resting は Execute 側で確定する。
        idle.behavior = IdleBehavior::GoingToRest;
        idle.idle_timer = 0.0;
        idle.total_idle_time = 0.0;
        idle.behavior_duration = REST_AREA_RESTING_DURATION;
        path.waypoints.clear();
        path.current_index = 0;
        if !just_reserved {
            request_writer.write(IdleBehaviorRequest {
                entity,
                operation: IdleBehaviorOperation::EnterRestArea { rest_area_entity },
            });
        } else {
            dest.0 = current_pos;
        }
        return true;
    }

    if let Some(p) = participating_in {
        request_writer.write(IdleBehaviorRequest {
            entity,
            operation: IdleBehaviorOperation::LeaveGathering { spot_entity: p.0 },
        });
    }
    let destination_changed = dest.0.distance_squared(rest_area_pos) > (TILE_SIZE * 2.5).powi(2);
    let needs_new_path = destination_changed
        || path.waypoints.is_empty()
        || path.current_index >= path.waypoints.len();

    idle.behavior = IdleBehavior::GoingToRest;
    if needs_new_path {
        idle.idle_timer = 0.0;
        idle.behavior_duration = REST_AREA_RESTING_DURATION;
        dest.0 = nearest_walkable_adjacent_to_rest_area(current_pos, rest_area_pos, world_map);
        path.waypoints.clear();
        path.current_index = 0;
    }
    true
}
