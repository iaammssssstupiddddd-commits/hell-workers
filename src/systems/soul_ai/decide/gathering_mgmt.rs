use bevy::prelude::*;

use crate::constants::{ESCAPE_GATHERING_JOIN_RADIUS, ESCAPE_SAFE_DISTANCE_MULTIPLIER};
use crate::entities::damned_soul::{DamnedSoul, IdleBehavior, IdleState};
use crate::entities::familiar::Familiar;
use crate::events::GatheringManagementOp;
use crate::relationships::CommandedBy;
use crate::systems::soul_ai::decide::SoulDecideOutput;
use crate::systems::soul_ai::execute::task_execution::AssignedTask;
use crate::systems::soul_ai::helpers::gathering::*;
use crate::systems::spatial::{SpatialGrid, SpatialGridOps};

fn is_gathering_spot_safe_from_familiars(
    spot_pos: Vec2,
    q_familiars: &Query<(&Transform, &Familiar)>,
) -> bool {
    let mut nearest: Option<(f32, f32)> = None;
    for (transform, familiar) in q_familiars.iter() {
        let dist = spot_pos.distance(transform.translation.truncate());
        if nearest.map_or(true, |(best_dist, _)| dist < best_dist) {
            nearest = Some((dist, familiar.command_radius));
        }
    }

    match nearest {
        None => true,
        Some((dist, command_radius)) => dist > command_radius * ESCAPE_SAFE_DISTANCE_MULTIPLIER,
    }
}

/// 人数不足で猶予切れの集会を Dissolve 要求に変換する
pub fn gathering_maintenance_decision(
    q_spots: Query<(Entity, &GatheringSpot, &GatheringVisuals)>,
    update_timer: Res<GatheringUpdateTimer>,
    mut decide_output: SoulDecideOutput,
) {
    if !update_timer.timer.just_finished() {
        return;
    }

    for (spot_entity, spot, visuals) in q_spots.iter() {
        if spot.participants < GATHERING_MIN_PARTICIPANTS
            && spot.grace_active
            && spot.grace_timer <= 0.0
        {
            decide_output
                .gathering_requests
                .write(crate::events::GatheringManagementRequest {
                    operation: GatheringManagementOp::Dissolve {
                        spot_entity,
                        aura_entity: visuals.aura_entity,
                        object_entity: visuals.object_entity,
                    },
                });
        }
    }
}

/// 近接する集会の統合を Merge 要求に変換する
pub fn gathering_merge_decision(
    time: Res<Time>,
    q_spots: Query<(Entity, &GatheringSpot, &GatheringVisuals)>,
    q_participants: Query<(Entity, &ParticipatingIn)>,
    update_timer: Res<GatheringUpdateTimer>,
    mut decide_output: SoulDecideOutput,
) {
    if !update_timer.timer.just_finished() {
        return;
    }

    let current_time = time.elapsed_secs();
    let spots: Vec<_> = q_spots.iter().collect();

    for i in 0..spots.len() {
        for j in (i + 1)..spots.len() {
            let (entity_a, spot_a, visuals_a) = &spots[i];
            let (entity_b, spot_b, visuals_b) = &spots[j];

            let combined_participants = spot_a.participants + spot_b.participants;
            if combined_participants > GATHERING_MAX_CAPACITY {
                continue;
            }

            let distance = (spot_a.center - spot_b.center).length();
            let elapsed_a = current_time - spot_a.created_at;
            let elapsed_b = current_time - spot_b.created_at;
            let merge_distance_a = calculate_merge_distance(spot_a.participants, elapsed_a);
            let merge_distance_b = calculate_merge_distance(spot_b.participants, elapsed_b);

            if distance < merge_distance_a.max(merge_distance_b) {
                let (absorber, absorbed, absorbed_visuals) =
                    if spot_a.participants > spot_b.participants {
                        (*entity_a, *entity_b, visuals_b)
                    } else if spot_b.participants > spot_a.participants {
                        (*entity_b, *entity_a, visuals_a)
                    } else if spot_a.created_at < spot_b.created_at {
                        (*entity_a, *entity_b, visuals_b)
                    } else {
                        (*entity_b, *entity_a, visuals_a)
                    };

                let participants_to_move = q_participants
                    .iter()
                    .filter_map(|(soul_entity, participating)| {
                        if participating.0 == absorbed {
                            Some(soul_entity)
                        } else {
                            None
                        }
                    })
                    .collect();

                decide_output
                    .gathering_requests
                    .write(crate::events::GatheringManagementRequest {
                        operation: GatheringManagementOp::Merge {
                            absorber,
                            absorbed,
                            participants_to_move,
                            absorbed_aura: absorbed_visuals.aura_entity,
                            absorbed_object: absorbed_visuals.object_entity,
                        },
                    });

                return;
            }
        }
    }
}

/// 条件を満たすSoulの集会参加を Recruit 要求に変換する
pub fn gathering_recruitment_decision(
    q_spots: Query<(Entity, &GatheringSpot)>,
    soul_grid: Res<SpatialGrid>,
    q_souls: Query<
        (Entity, &Transform, &AssignedTask, &IdleState),
        (
            With<DamnedSoul>,
            Without<ParticipatingIn>,
            Without<CommandedBy>,
        ),
    >,
    q_familiars: Query<(&Transform, &Familiar)>,
    update_timer: Res<GatheringUpdateTimer>,
    mut decide_output: SoulDecideOutput,
) {
    if !update_timer.timer.just_finished() {
        return;
    }

    for (spot_entity, spot) in q_spots.iter() {
        if spot.participants >= spot.max_capacity {
            continue;
        }

        let spot_is_safe_for_escape =
            is_gathering_spot_safe_from_familiars(spot.center, &q_familiars);

        let search_radius = GATHERING_DETECTION_RADIUS.max(ESCAPE_GATHERING_JOIN_RADIUS);
        let nearby_souls = soul_grid.get_nearby_in_radius(spot.center, search_radius);

        let mut current_participants = spot.participants;
        for soul_entity in nearby_souls {
            if current_participants >= spot.max_capacity {
                break;
            }

            if let Ok((_, transform, task, idle)) = q_souls.get(soul_entity) {
                if !matches!(task, AssignedTask::None) {
                    continue;
                }

                let dist_to_spot = spot.center.distance(transform.translation.truncate());
                if idle.behavior == IdleBehavior::Escaping {
                    if dist_to_spot > ESCAPE_GATHERING_JOIN_RADIUS || !spot_is_safe_for_escape {
                        continue;
                    }
                } else if dist_to_spot > GATHERING_DETECTION_RADIUS {
                    continue;
                }

                current_participants += 1;
                decide_output
                    .gathering_requests
                    .write(crate::events::GatheringManagementRequest {
                        operation: GatheringManagementOp::Recruit {
                            soul: soul_entity,
                            spot: spot_entity,
                        },
                    });
            }
        }
    }
}

/// 離脱条件を満たす参加者を Leave 要求に変換する
pub fn gathering_leave_decision(
    q_spots: Query<&GatheringSpot>,
    q_participants: Query<(Entity, &Transform, &IdleState, &ParticipatingIn), With<DamnedSoul>>,
    update_timer: Res<GatheringUpdateTimer>,
    mut decide_output: SoulDecideOutput,
) {
    if !update_timer.timer.just_finished() {
        return;
    }

    for (entity, transform, idle, participating_in) in q_participants.iter() {
        if matches!(
            idle.behavior,
            IdleBehavior::Gathering | IdleBehavior::ExhaustedGathering
        ) {
            continue;
        }

        if let Ok(spot) = q_spots.get(participating_in.0) {
            let dist = (spot.center - transform.translation.truncate()).length();
            if dist > GATHERING_LEAVE_RADIUS {
                decide_output
                    .gathering_requests
                    .write(crate::events::GatheringManagementRequest {
                        operation: GatheringManagementOp::Leave {
                            soul: entity,
                            spot: participating_in.0,
                        },
                    });
            }
        } else {
            decide_output
                .gathering_requests
                .write(crate::events::GatheringManagementRequest {
                    operation: GatheringManagementOp::Leave {
                        soul: entity,
                        spot: participating_in.0,
                    },
                });
        }
    }
}
