use bevy::prelude::*;

use crate::entities::damned_soul::{DriftingState, IdleBehavior};
use crate::events::{EscapeOperation, EscapeRequest};
use crate::relationships::ParticipatingIn;
use crate::systems::soul_ai::helpers::query_types::EscapingBehaviorSoulQuery;

/// EscapeRequest を適用する（Execute Phase）
pub fn escaping_apply_system(
    mut commands: Commands,
    mut request_reader: MessageReader<EscapeRequest>,
    mut q_souls: EscapingBehaviorSoulQuery,
) {
    for request in request_reader.read() {
        let Ok((entity, _transform, mut idle_state, mut destination, mut path, _under_command)) =
            q_souls.get_mut(request.entity)
        else {
            continue;
        };

        match &request.operation {
            EscapeOperation::StartEscaping { leave_gathering } => {
                if let Some(_spot_entity) = *leave_gathering {
                    commands.entity(entity).remove::<ParticipatingIn>();
                    commands.trigger(crate::events::OnGatheringLeft { entity });
                }

                commands.entity(entity).remove::<DriftingState>();

                idle_state.behavior = IdleBehavior::Escaping;
                idle_state.idle_timer = 0.0;
                idle_state.behavior_duration = 5.0;
            }
            EscapeOperation::UpdateDestination { destination: next } => {
                destination.0 = *next;
                path.waypoints.clear();
                path.current_index = 0;
            }
            EscapeOperation::ReachSafety => {
                idle_state.behavior = IdleBehavior::Wandering;
                idle_state.behavior_duration = 3.0;
                path.waypoints.clear();
                path.current_index = 0;
            }
            EscapeOperation::JoinSafeGathering => {
                idle_state.behavior = IdleBehavior::Gathering;
                idle_state.idle_timer = 0.0;
                idle_state.behavior_duration = 3.0;
                idle_state.needs_separation = true;
                path.waypoints.clear();
                path.current_index = 0;
            }
        }
    }
}
