use bevy::ecs::system::SystemParam;
use bevy::prelude::*;

use hw_core::constants::*;
use hw_core::events::{EscapeOperation, EscapeRequest};
use hw_core::familiar::Familiar;
use hw_core::relationships::{CommandedBy, ParticipatingIn};
use hw_core::soul::{DamnedSoul, IdleBehavior, IdleState};
use hw_spatial::FamiliarSpatialGrid;
use hw_world::{PathfindingContext, WorldMap};

use crate::soul_ai::decide::SoulDecideOutput;
use crate::soul_ai::decide::idle_behavior::GATHERING_ARRIVAL_RADIUS;
use crate::soul_ai::helpers::gathering::GatheringSpot;
use crate::soul_ai::perceive::escaping::{
    EscapeBehaviorTimer, EscapeDetectionTimer, calculate_escape_destination,
    detect_nearest_familiar, detect_reachable_familiar_within_safe_distance,
    find_safe_gathering_spot,
};

type EscapeDetectQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static Transform,
        &'static DamnedSoul,
        Option<&'static CommandedBy>,
        Option<&'static ParticipatingIn>,
        &'static IdleState,
    ),
>;

type EscapeBehaviorQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static Transform,
        &'static IdleState,
        Option<&'static CommandedBy>,
        Option<&'static ParticipatingIn>,
    ),
>;

#[derive(SystemParam)]
pub(crate) struct EscapeTimers<'w> {
    detection_timer: ResMut<'w, EscapeDetectionTimer>,
    behavior_timer: ResMut<'w, EscapeBehaviorTimer>,
}

#[derive(SystemParam)]
pub(crate) struct EscapeLocalState<'s> {
    pf_context: Local<'s, PathfindingContext>,
    nearby_buf: Local<'s, Vec<Entity>>,
}

#[derive(SystemParam)]
pub(crate) struct EscapeSpatialInputs<'w, 's> {
    world_map: Res<'w, WorldMap>,
    familiar_grid: Res<'w, FamiliarSpatialGrid>,
    q_familiars: Query<'w, 's, (&'static Transform, &'static Familiar)>,
    q_gathering_spots: Query<'w, 's, (Entity, &'static GatheringSpot)>,
}

/// 逃走の判定と要求生成を行う（Decide Phase）
pub(crate) fn escaping_decision_system(
    time: Res<Time>,
    mut timers: EscapeTimers,
    mut local: EscapeLocalState,
    spatial: EscapeSpatialInputs,
    q_detect: EscapeDetectQuery,
    q_behavior: EscapeBehaviorQuery,
    mut decide_output: SoulDecideOutput,
) {
    let detect_tick = timers
        .detection_timer
        .timer
        .tick(time.delta())
        .just_finished();

    let behavior_tick = {
        let finished = timers
            .behavior_timer
            .timer
            .tick(time.delta())
            .just_finished();
        if timers.behavior_timer.first_run_done && !finished {
            false
        } else {
            timers.behavior_timer.first_run_done = true;
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
            if matches!(
                idle_state.behavior,
                IdleBehavior::Escaping
                    | IdleBehavior::Drifting
                    | IdleBehavior::Resting
                    | IdleBehavior::GoingToRest
            ) {
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
            if let Some(threat) = detect_nearest_familiar(
                soul_pos,
                &spatial.familiar_grid,
                &spatial.q_familiars,
                &mut local.nearby_buf,
            ) {
                debug!(
                    "ESCAPE_DECIDE: {:?} detected threat {:?} dist {:.1}",
                    entity, threat.entity, threat.distance
                );
                decide_output.escape_requests.write(EscapeRequest {
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
                decide_output.escape_requests.write(EscapeRequest {
                    entity,
                    operation: EscapeOperation::ReachSafety,
                });
                continue;
            }

            let soul_pos = transform.translation.truncate();
            if let Some(threat) = detect_reachable_familiar_within_safe_distance(
                soul_pos,
                &spatial.familiar_grid,
                &spatial.q_familiars,
                spatial.world_map.as_ref(),
                &mut local.pf_context,
                &mut local.nearby_buf,
            ) {
                let safe_spot = find_safe_gathering_spot(
                    soul_pos,
                    &spatial.q_gathering_spots,
                    &spatial.familiar_grid,
                    &spatial.q_familiars,
                    &mut local.nearby_buf,
                );

                if let Some(spot_pos) = safe_spot
                    && soul_pos.distance(spot_pos) <= GATHERING_ARRIVAL_RADIUS
                {
                    decide_output.escape_requests.write(EscapeRequest {
                        entity,
                        operation: EscapeOperation::JoinSafeGathering,
                    });
                    continue;
                }

                let destination = calculate_escape_destination(
                    soul_pos,
                    &threat,
                    safe_spot,
                    spatial.world_map.as_ref(),
                );

                decide_output.escape_requests.write(EscapeRequest {
                    entity,
                    operation: EscapeOperation::UpdateDestination { destination },
                });
            } else {
                decide_output.escape_requests.write(EscapeRequest {
                    entity,
                    operation: EscapeOperation::ReachSafety,
                });
            }
        }
    }
}
