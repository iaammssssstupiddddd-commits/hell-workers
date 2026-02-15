use bevy::prelude::*;

use crate::constants::*;
use crate::entities::damned_soul::{DamnedSoul, IdleBehavior, IdleState};
use crate::entities::familiar::Familiar;
use crate::events::EscapeOperation;
use crate::relationships::CommandedBy;
use crate::systems::soul_ai::decide::SoulDecideOutput;
use crate::systems::soul_ai::decide::idle_behavior::GATHERING_ARRIVAL_RADIUS;
use crate::relationships::ParticipatingIn;
use crate::systems::soul_ai::helpers::gathering::{GatheringSpot};
use crate::systems::soul_ai::perceive::escaping::{
    EscapeBehaviorTimer, EscapeDetectionTimer, calculate_escape_destination,
    detect_nearest_familiar, detect_reachable_familiar_within_safe_distance,
    find_safe_gathering_spot,
};
use crate::systems::spatial::FamiliarSpatialGrid;
use crate::world::map::WorldMap;
use crate::world::pathfinding::PathfindingContext;

/// 逃走の判定と要求生成を行う（Decide Phase）
pub fn escaping_decision_system(
    time: Res<Time>,
    mut detection_timer: ResMut<EscapeDetectionTimer>,
    mut behavior_timer: ResMut<EscapeBehaviorTimer>,
    world_map: Res<WorldMap>,
    mut pf_context: Local<PathfindingContext>,
    familiar_grid: Res<FamiliarSpatialGrid>,
    q_familiars: Query<(&Transform, &Familiar)>,
    q_gathering_spots: Query<(Entity, &GatheringSpot)>,
    q_detect: Query<(
        Entity,
        &Transform,
        &DamnedSoul,
        Option<&CommandedBy>,
        Option<&ParticipatingIn>,
        &IdleState,
    )>,
    q_behavior: Query<(
        Entity,
        &Transform,
        &IdleState,
        Option<&CommandedBy>,
        Option<&ParticipatingIn>,
    )>,
    mut decide_output: SoulDecideOutput,
) {
    let detect_tick = detection_timer.timer.tick(time.delta()).just_finished();

    let behavior_tick = {
        let finished = behavior_timer.timer.tick(time.delta()).just_finished();
        if behavior_timer.first_run_done && !finished {
            false
        } else {
            behavior_timer.first_run_done = true;
            true
        }
    };

    if !detect_tick && !behavior_tick {
        return;
    }

    if detect_tick {
        for (entity, transform, soul, under_command, participating_in, idle_state) in
            q_detect.iter()
        {
            if idle_state.behavior == IdleBehavior::Escaping {
                continue;
            }
            if under_command.is_some() {
                continue;
            }
            if idle_state.behavior == IdleBehavior::ExhaustedGathering {
                continue;
            }
            if soul.stress <= ESCAPE_STRESS_THRESHOLD {
                continue;
            }

            let soul_pos = transform.translation.truncate();
            if let Some(threat) = detect_nearest_familiar(soul_pos, &familiar_grid, &q_familiars) {
                debug!(
                    "ESCAPE_DECIDE: {:?} detected threat {:?} dist {:.1}",
                    entity, threat.entity, threat.distance
                );
                decide_output
                    .escape_requests
                    .write(crate::events::EscapeRequest {
                        entity,
                        operation: EscapeOperation::StartEscaping {
                            leave_gathering: participating_in.map(|p| p.0),
                        },
                    });
            }
        }
    }

    if behavior_tick {
        for (entity, transform, idle_state, under_command, _participating) in q_behavior.iter() {
            if idle_state.behavior != IdleBehavior::Escaping {
                continue;
            }

            if under_command.is_some() {
                decide_output
                    .escape_requests
                    .write(crate::events::EscapeRequest {
                        entity,
                        operation: EscapeOperation::ReachSafety,
                    });
                continue;
            }

            let soul_pos = transform.translation.truncate();
            if let Some(threat) = detect_reachable_familiar_within_safe_distance(
                soul_pos,
                &familiar_grid,
                &q_familiars,
                &world_map,
                &mut pf_context,
            ) {
                let safe_spot = find_safe_gathering_spot(
                    soul_pos,
                    &q_gathering_spots,
                    &familiar_grid,
                    &q_familiars,
                );

                if let Some(spot_pos) = safe_spot {
                    if soul_pos.distance(spot_pos) <= GATHERING_ARRIVAL_RADIUS {
                        decide_output
                            .escape_requests
                            .write(crate::events::EscapeRequest {
                                entity,
                                operation: EscapeOperation::JoinSafeGathering,
                            });
                        continue;
                    }
                }

                let destination =
                    calculate_escape_destination(soul_pos, &threat, safe_spot, &world_map);

                decide_output
                    .escape_requests
                    .write(crate::events::EscapeRequest {
                        entity,
                        operation: EscapeOperation::UpdateDestination { destination },
                    });
            } else {
                decide_output
                    .escape_requests
                    .write(crate::events::EscapeRequest {
                        entity,
                        operation: EscapeOperation::ReachSafety,
                    });
            }
        }
    }
}
