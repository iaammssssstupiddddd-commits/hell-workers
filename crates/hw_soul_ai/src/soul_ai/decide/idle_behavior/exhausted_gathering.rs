//! 疲労による強制集会（ExhaustedGathering）状態の処理

use bevy::prelude::*;

use hw_core::constants::{GATHERING_ARRIVAL_RADIUS_BASE, TILE_SIZE};
use hw_core::events::{IdleBehaviorOperation, IdleBehaviorRequest};
use hw_core::relationships::ParticipatingIn;
use hw_core::soul::{Destination, IdleBehavior, IdleState, Path};

const GATHERING_ARRIVAL_RADIUS: f32 = TILE_SIZE * GATHERING_ARRIVAL_RADIUS_BASE;

/// `process_exhausted_gathering` に渡す集会コンテキスト。
pub struct GatheringCtx<'a> {
    pub center: Option<Vec2>,
    pub target_spot_entity: Option<Entity>,
    pub participating_in: Option<&'a ParticipatingIn>,
}

/// ExhaustedGathering 状態を処理。継続すべきなら true を返す
pub fn process_exhausted_gathering(
    entity: Entity,
    current_pos: Vec2,
    gathering: GatheringCtx<'_>,
    idle: &mut IdleState,
    dest: &mut Destination,
    path: &mut Path,
    request_writer: &mut MessageWriter<IdleBehaviorRequest>,
) -> bool {
    if idle.behavior != IdleBehavior::ExhaustedGathering {
        return false;
    }
    if let Some(center) = gathering.center {
        let dist_from_center = (center - current_pos).length();
        let has_arrived = dist_from_center <= GATHERING_ARRIVAL_RADIUS;

        if has_arrived {
            idle.behavior = IdleBehavior::Gathering;
            idle.needs_separation = true;
            if gathering.participating_in.is_none()
                && let Some(spot_entity) = gathering.target_spot_entity {
                    request_writer.write(IdleBehaviorRequest {
                        entity,
                        operation: IdleBehaviorOperation::ArriveAtGathering { spot_entity },
                    });
                }
            return false;
        }
        if path.waypoints.is_empty() || path.current_index >= path.waypoints.len() {
            dest.0 = center;
            path.waypoints.clear();
        }
        return true;
    }
    idle.behavior = IdleBehavior::Wandering;
    false
}
