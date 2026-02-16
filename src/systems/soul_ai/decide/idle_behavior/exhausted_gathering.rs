//! 疲労による強制集会（ExhaustedGathering）状態の処理

use bevy::prelude::*;

use crate::entities::damned_soul::{Destination, IdleBehavior, IdleState, Path};
use crate::events::{IdleBehaviorOperation, IdleBehaviorRequest};
use crate::relationships::ParticipatingIn;

use super::GATHERING_ARRIVAL_RADIUS;

/// ExhaustedGathering 状態を処理。継続すべきなら true を返す
pub fn process_exhausted_gathering(
    entity: Entity,
    current_pos: Vec2,
    gathering_center: Option<Vec2>,
    target_spot_entity: Option<Entity>,
    participating_in: Option<&ParticipatingIn>,
    idle: &mut IdleState,
    dest: &mut Destination,
    path: &mut Path,
    request_writer: &mut MessageWriter<IdleBehaviorRequest>,
) -> bool {
    if idle.behavior != IdleBehavior::ExhaustedGathering {
        return false;
    }
    if let Some(center) = gathering_center {
        let dist_from_center = (center - current_pos).length();
        let has_arrived = dist_from_center <= GATHERING_ARRIVAL_RADIUS;

        if has_arrived {
            idle.behavior = IdleBehavior::Gathering;
            idle.needs_separation = true;
            if participating_in.is_none() {
                if let Some(spot_entity) = target_spot_entity {
                    request_writer.write(IdleBehaviorRequest {
                        entity,
                        operation: IdleBehaviorOperation::ArriveAtGathering { spot_entity },
                    });
                }
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
