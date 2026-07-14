use std::collections::HashMap;

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;

use hw_core::constants::*;
use hw_core::events::{IdleBehaviorOperation, IdleBehaviorRequest};
use hw_core::gathering::{GATHERING_LEAVE_RADIUS, GatheringSpot};
use hw_core::relationships::GatheringParticipants;
#[cfg(feature = "profiling")]
use hw_core::simulation_rng::{FixedAuditSeed, SimulationRng};
use hw_core::soul::IdleBehavior;
use hw_spatial::{GatheringSpotSpatialGrid, SpatialGrid, SpatialGridOps};
use hw_world::WorldMap;

#[cfg(feature = "profiling")]
use crate::soul_ai::helpers::query_types::IdleDecisionRandomStateQuery;
use crate::soul_ai::helpers::query_types::IdleDecisionSoulQuery;

use super::rest_area::{RestAreasQuery, find_nearest_available_rest_area};
use super::{exhausted_gathering, motion_dispatch, rest_decision, task_override, transitions};

#[cfg(feature = "profiling")]
const IDLE_BEHAVIOR_DURATION_STREAM: u64 = 0x6964_6c65_5f64_7572;
#[cfg(feature = "profiling")]
const IDLE_GATHERING_BEHAVIOR_STREAM: u64 = 0x6964_6c65_5f67_6268;
#[cfg(feature = "profiling")]
const IDLE_GATHERING_DURATION_STREAM: u64 = 0x6964_6c65_5f67_6472;
#[cfg(feature = "profiling")]
const IDLE_SELECT_BEHAVIOR_STREAM: u64 = 0x6964_6c65_5f73_656c;

#[derive(SystemParam)]
pub(crate) struct IdleLocalState<'s> {
    pending_rest_reservations: Local<'s, HashMap<Entity, usize>>,
    nearby_buf: Local<'s, Vec<Entity>>,
}

#[derive(SystemParam)]
pub(crate) struct IdleGatheringQueries<'w, 's> {
    q_spots: Query<
        'w,
        's,
        (
            Entity,
            &'static GatheringSpot,
            &'static GatheringParticipants,
        ),
    >,
    q_rest_areas: RestAreasQuery<'w, 's>,
    spot_grid: Res<'w, GatheringSpotSpatialGrid>,
    soul_grid: Res<'w, SpatialGrid>,
}

/// アイドル行動の決定システム (Decide Phase)
///
/// 怠惰行動のAIロジック。やる気が低い魂は怠惰な行動をする。
/// タスクがある魂は怠惰行動をしない。
///
/// このシステムはIdleState, Destination, Pathの更新と、
/// IdleBehaviorRequestの発行を行う。実際のエンティティ操作は
/// idle_behavior_apply_systemで行われる。
pub(crate) fn idle_behavior_decision_system(
    time: Res<Time>,
    #[cfg(feature = "profiling")] audit_seed: Option<Res<FixedAuditSeed>>,
    mut request_writer: MessageWriter<IdleBehaviorRequest>,
    world_map: Res<WorldMap>,
    mut local: IdleLocalState,
    gq: IdleGatheringQueries,
    mut query: IdleDecisionSoulQuery,
    #[cfg(feature = "profiling")] mut random_states: IdleDecisionRandomStateQuery,
) {
    let dt = time.delta_secs();
    local.pending_rest_reservations.clear();

    for (
        entity,
        transform,
        mut idle,
        mut dest,
        soul,
        mut path,
        task,
        participating_in,
        resting_in,
        rest_reserved_for,
        rest_cooldown,
    ) in query.iter_mut()
    {
        #[cfg(feature = "profiling")]
        let mut random_state = random_states.get_mut(entity).ok();

        let (gathering_center, target_spot_entity) = resolve_gathering_target(
            participating_in,
            &gq.q_spots,
            &gq.spot_grid,
            transform,
            &mut local.nearby_buf,
        );

        if exhausted_gathering::process_exhausted_gathering(
            entity,
            transform.translation.truncate(),
            exhausted_gathering::GatheringCtx {
                center: gathering_center,
                target_spot_entity,
                participating_in,
            },
            &mut idle,
            &mut dest,
            &mut path,
            &mut request_writer,
        ) {
            continue;
        }

        if task_override::process_task_override(
            entity,
            task,
            participating_in,
            resting_in,
            rest_reserved_for,
            &mut idle,
            &mut request_writer,
        ) {
            continue;
        }

        let reserved_rest_area = rest_reserved_for.map(|reserved| reserved.0);
        let current_pos = transform.translation.truncate();
        let rest_cooldown_active = rest_cooldown
            .map(|cooldown| cooldown.remaining_secs > f32::EPSILON)
            .unwrap_or(false);

        if rest_cooldown_active && resting_in.is_none() {
            if reserved_rest_area.is_some() {
                request_writer.write(IdleBehaviorRequest {
                    entity,
                    operation: IdleBehaviorOperation::ReleaseRestArea,
                });
            }
            if matches!(
                idle.behavior,
                IdleBehavior::Resting | IdleBehavior::GoingToRest
            ) {
                idle.behavior = IdleBehavior::Wandering;
                idle.idle_timer = 0.0;
                path.waypoints.clear();
                path.current_index = 0;
                dest.0 = current_pos;
            }
        }

        if matches!(
            idle.behavior,
            IdleBehavior::Resting | IdleBehavior::GoingToRest
        ) && resting_in.is_none()
            && !rest_cooldown_active
        {
            // 予約が無い GoingToRest は不整合。停止しやすいため通常アイドルへ戻す。
            if reserved_rest_area.is_none() {
                idle.behavior = IdleBehavior::Wandering;
                path.waypoints.clear();
                path.current_index = 0;
                dest.0 = current_pos;
            } else {
                let rest_area_target = resolve_rest_area_target(
                    reserved_rest_area,
                    dest.0,
                    current_pos,
                    &gq.q_rest_areas,
                    &local.pending_rest_reservations,
                );

                if let Some((rest_area_entity, rest_area_pos)) = rest_area_target {
                    let just_reserved = if reserved_rest_area != Some(rest_area_entity) {
                        request_writer.write(IdleBehaviorRequest {
                            entity,
                            operation: IdleBehaviorOperation::ReserveRestArea { rest_area_entity },
                        });
                        *local
                            .pending_rest_reservations
                            .entry(rest_area_entity)
                            .or_insert(0) += 1;
                        true
                    } else {
                        false
                    };

                    if rest_decision::process_resting_or_going_to_rest(
                        rest_decision::RestDecisionCtx {
                            entity,
                            rest_area_target: Some((rest_area_entity, rest_area_pos)),
                            participating_in,
                            current_pos,
                            just_reserved,
                        },
                        rest_decision::RestMoveState {
                            idle: &mut idle,
                            dest: &mut dest,
                            path: &mut path,
                        },
                        world_map.as_ref(),
                        &mut request_writer,
                    ) {
                        continue;
                    }
                } else {
                    if reserved_rest_area.is_some() {
                        request_writer.write(IdleBehaviorRequest {
                            entity,
                            operation: IdleBehaviorOperation::ReleaseRestArea,
                        });
                    }
                    idle.behavior = IdleBehavior::Wandering;
                }
            }
        }

        if matches!(
            idle.behavior,
            IdleBehavior::Escaping | IdleBehavior::Drifting
        ) {
            continue;
        }

        let wants_rest_area = soul.dream > 0.0
            && !rest_cooldown_active
            && (soul.laziness > LAZINESS_THRESHOLD_MID
                || soul.fatigue > FATIGUE_IDLE_THRESHOLD * 0.5
                || soul.stress > ESCAPE_STRESS_THRESHOLD
                || idle.total_idle_time > IDLE_TIME_TO_GATHERING * 0.3);

        if wants_rest_area {
            let rest_area_target = resolve_rest_area_target(
                reserved_rest_area,
                current_pos,
                current_pos,
                &gq.q_rest_areas,
                &local.pending_rest_reservations,
            );

            if let Some((rest_area_entity, rest_area_pos)) = rest_area_target {
                let just_reserved = if reserved_rest_area != Some(rest_area_entity) {
                    request_writer.write(IdleBehaviorRequest {
                        entity,
                        operation: IdleBehaviorOperation::ReserveRestArea { rest_area_entity },
                    });
                    *local
                        .pending_rest_reservations
                        .entry(rest_area_entity)
                        .or_insert(0) += 1;
                    true
                } else {
                    false
                };

                if rest_decision::process_wants_rest_area(
                    rest_decision::RestDecisionCtx {
                        entity,
                        rest_area_target: Some((rest_area_entity, rest_area_pos)),
                        participating_in,
                        current_pos,
                        just_reserved,
                    },
                    rest_decision::RestMoveState {
                        idle: &mut idle,
                        dest: &mut dest,
                        path: &mut path,
                    },
                    world_map.as_ref(),
                    &mut request_writer,
                ) {
                    continue;
                }
            } else if reserved_rest_area.is_some() {
                request_writer.write(IdleBehaviorRequest {
                    entity,
                    operation: IdleBehaviorOperation::ReleaseRestArea,
                });
            }
        } else if reserved_rest_area.is_some() {
            request_writer.write(IdleBehaviorRequest {
                entity,
                operation: IdleBehaviorOperation::ReleaseRestArea,
            });
        }

        idle.total_idle_time += dt;

        // dream=0で睡眠中なら強制起床
        if soul.dream <= 0.0 && idle.behavior == IdleBehavior::Sleeping {
            idle.behavior = IdleBehavior::Wandering;
            idle.idle_timer = 0.0;
            #[cfg(feature = "profiling")]
            let mut rng = SimulationRng::for_actor(
                audit_seed.as_deref(),
                random_state.as_deref_mut(),
                IDLE_BEHAVIOR_DURATION_STREAM,
            );
            #[cfg(feature = "profiling")]
            {
                idle.behavior_duration =
                    transitions::behavior_duration_for_with_rng(IdleBehavior::Wandering, &mut rng);
            }
            #[cfg(not(feature = "profiling"))]
            {
                idle.behavior_duration =
                    transitions::behavior_duration_for(IdleBehavior::Wandering);
            }
            path.waypoints.clear();
            path.current_index = 0;
            dest.0 = current_pos;
        }

        if soul.motivation > MOTIVATION_THRESHOLD && soul.fatigue < FATIGUE_IDLE_THRESHOLD {
            continue;
        }

        idle.idle_timer += dt;

        if idle.idle_timer >= idle.behavior_duration {
            idle.idle_timer = 0.0;
            let previous_behavior = idle.behavior;

            if soul.fatigue > FATIGUE_GATHERING_THRESHOLD
                || idle.total_idle_time > IDLE_TIME_TO_GATHERING
            {
                if idle.behavior != IdleBehavior::Gathering
                    && idle.behavior != IdleBehavior::ExhaustedGathering
                {
                    #[cfg(feature = "profiling")]
                    let mut gathering_behavior_rng = SimulationRng::for_actor(
                        audit_seed.as_deref(),
                        random_state.as_deref_mut(),
                        IDLE_GATHERING_BEHAVIOR_STREAM,
                    );
                    #[cfg(feature = "profiling")]
                    {
                        idle.gathering_behavior = transitions::random_gathering_behavior_with_rng(
                            soul.dream,
                            &mut gathering_behavior_rng,
                        );
                    }
                    #[cfg(not(feature = "profiling"))]
                    {
                        idle.gathering_behavior =
                            transitions::random_gathering_behavior(soul.dream);
                    }
                    idle.gathering_behavior_timer = 0.0;
                    #[cfg(feature = "profiling")]
                    let mut gathering_duration_rng = SimulationRng::for_actor(
                        audit_seed.as_deref(),
                        random_state.as_deref_mut(),
                        IDLE_GATHERING_DURATION_STREAM,
                    );
                    #[cfg(feature = "profiling")]
                    {
                        idle.gathering_behavior_duration =
                            transitions::random_gathering_duration_with_rng(
                                &mut gathering_duration_rng,
                            );
                    }
                    #[cfg(not(feature = "profiling"))]
                    {
                        idle.gathering_behavior_duration = transitions::random_gathering_duration();
                    }
                    idle.needs_separation = true;
                }
                idle.behavior = if soul.fatigue > FATIGUE_GATHERING_THRESHOLD {
                    IdleBehavior::ExhaustedGathering
                } else {
                    IdleBehavior::Gathering
                };
            } else {
                #[cfg(feature = "profiling")]
                let mut select_behavior_rng = SimulationRng::for_actor(
                    audit_seed.as_deref(),
                    random_state.as_deref_mut(),
                    IDLE_SELECT_BEHAVIOR_STREAM,
                );
                #[cfg(feature = "profiling")]
                {
                    idle.behavior = transitions::select_next_behavior_with_rng(
                        soul.laziness,
                        soul.fatigue,
                        idle.total_idle_time,
                        soul.dream,
                        &mut select_behavior_rng,
                    );
                }
                #[cfg(not(feature = "profiling"))]
                {
                    idle.behavior = transitions::select_next_behavior(
                        soul.laziness,
                        soul.fatigue,
                        idle.total_idle_time,
                        soul.dream,
                    );
                }
            }

            if matches!(
                idle.behavior,
                IdleBehavior::Sitting | IdleBehavior::Sleeping
            ) && idle.behavior != previous_behavior
            {
                // 睡眠/座り込み遷移時に残パスで歩き続けるのを防ぐ。
                path.waypoints.clear();
                path.current_index = 0;
                dest.0 = current_pos;
            }

            #[cfg(feature = "profiling")]
            let mut behavior_duration_rng = SimulationRng::for_actor(
                audit_seed.as_deref(),
                random_state.as_deref_mut(),
                IDLE_BEHAVIOR_DURATION_STREAM,
            );
            #[cfg(feature = "profiling")]
            {
                idle.behavior_duration = transitions::behavior_duration_for_with_rng(
                    idle.behavior,
                    &mut behavior_duration_rng,
                );
            }
            #[cfg(not(feature = "profiling"))]
            {
                idle.behavior_duration = transitions::behavior_duration_for(idle.behavior);
            }
        }

        motion_dispatch::update_motion_destinations(
            motion_dispatch::SoulPos {
                entity,
                pos: current_pos,
            },
            motion_dispatch::MotionGatheringCtx {
                center: gathering_center,
                target_spot_entity,
                participating_in,
            },
            motion_dispatch::MotionState {
                idle: &mut idle,
                dest: &mut dest,
                path: &mut path,
            },
            &*gq.soul_grid,
            world_map.as_ref(),
            &mut request_writer,
            motion_dispatch::MotionExtras {
                dt,
                dream: soul.dream,
                scratch: &mut local.nearby_buf,
                #[cfg(feature = "profiling")]
                audit_seed: audit_seed.as_deref(),
                #[cfg(feature = "profiling")]
                random_state: random_state.as_deref_mut(),
            },
        );
    }
}

fn resolve_gathering_target(
    participating_in: Option<&hw_core::relationships::ParticipatingIn>,
    q_spots: &Query<(Entity, &GatheringSpot, &GatheringParticipants)>,
    spot_grid: &GatheringSpotSpatialGrid,
    transform: &Transform,
    scratch: &mut Vec<Entity>,
) -> (Option<Vec2>, Option<Entity>) {
    if let Some(p) = participating_in {
        let center = q_spots.get(p.0).ok().map(|(_, s, _)| s.center);
        (center, Some(p.0))
    } else {
        let pos = transform.translation.truncate();
        spot_grid.get_nearby_in_radius_into(pos, GATHERING_LEAVE_RADIUS * 2.0, scratch);
        let nearest = scratch
            .iter()
            .filter_map(|&e| q_spots.get(e).ok())
            .filter(|item| item.2.len() < item.1.max_capacity)
            .min_by(|a, b| {
                a.1.center
                    .distance_squared(pos)
                    .partial_cmp(&b.1.center.distance_squared(pos))
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
        match nearest {
            Some((e, s, _)) => (Some(s.center), Some(e)),
            None => (None, None),
        }
    }
}

fn resolve_rest_area_target(
    reserved_rest_area: Option<Entity>,
    pos_a: Vec2,
    pos_b: Vec2,
    q_rest_areas: &RestAreasQuery,
    pending_rest_reservations: &HashMap<Entity, usize>,
) -> Option<(Entity, Vec2)> {
    reserved_rest_area
        .and_then(|reserved_entity| {
            q_rest_areas
                .get(reserved_entity)
                .ok()
                .map(|(_, t, _, _, _)| (reserved_entity, t.translation.truncate()))
        })
        .or_else(|| {
            find_nearest_available_rest_area(pos_a, q_rest_areas, pending_rest_reservations)
        })
        .or_else(|| {
            find_nearest_available_rest_area(pos_b, q_rest_areas, pending_rest_reservations)
        })
}
