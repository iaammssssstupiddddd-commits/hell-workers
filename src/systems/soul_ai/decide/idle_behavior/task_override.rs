//! タスク割り当て時の集会・休憩解除

use bevy::prelude::*;

use crate::entities::damned_soul::{IdleBehavior, IdleState};
use crate::events::{IdleBehaviorOperation, IdleBehaviorRequest};
use crate::relationships::{ParticipatingIn, RestAreaReservedFor, RestingIn};
use crate::systems::soul_ai::execute::task_execution::AssignedTask;


/// タスク割り当て中の場合、集会・休憩を解除して継続する
pub fn process_task_override(
    entity: Entity,
    task: &AssignedTask,
    participating_in: Option<&ParticipatingIn>,
    resting_in: Option<&RestingIn>,
    rest_reserved_for: Option<&RestAreaReservedFor>,
    idle: &mut IdleState,
    request_writer: &mut MessageWriter<IdleBehaviorRequest>,
) -> bool {
    if matches!(task, AssignedTask::None) {
        return false;
    }
    if let Some(p) = participating_in {
        request_writer.write(IdleBehaviorRequest {
            entity,
            operation: IdleBehaviorOperation::LeaveGathering { spot_entity: p.0 },
        });
    }
    if resting_in.is_some() {
        request_writer.write(IdleBehaviorRequest {
            entity,
            operation: IdleBehaviorOperation::LeaveRestArea,
        });
    }
    if rest_reserved_for.is_some() {
        request_writer.write(IdleBehaviorRequest {
            entity,
            operation: IdleBehaviorOperation::ReleaseRestArea,
        });
    }
    if idle.behavior != IdleBehavior::Wandering {
        idle.behavior = IdleBehavior::Wandering;
        idle.idle_timer = 0.0;
        idle.behavior_duration = 3.0;
        idle.needs_separation = false;
    }
    idle.total_idle_time = 0.0;
    true
}
