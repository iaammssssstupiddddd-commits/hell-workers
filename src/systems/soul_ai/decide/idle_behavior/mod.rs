//! 怠惰行動の Decide Phase: 状態遷移・休憩・集会・移動先選定

mod exhausted_gathering;
mod gathering_motion;
mod motion_dispatch;
mod rest_area;
mod rest_decision;
mod task_override;
mod transitions;

use std::collections::HashMap;

use bevy::prelude::*;

use crate::constants::*;
use crate::entities::damned_soul::IdleBehavior;
use crate::events::{IdleBehaviorOperation, IdleBehaviorRequest};
use crate::relationships::RestAreaReservations;
use crate::systems::jobs::RestArea;
use crate::systems::soul_ai::helpers::gathering::{GATHERING_LEAVE_RADIUS, GatheringSpot};
use crate::systems::soul_ai::helpers::query_types::IdleDecisionSoulQuery;
use crate::systems::spatial::{GatheringSpotSpatialGrid, SpatialGridOps};
use crate::world::map::WorldMap;

pub use rest_area::{
    find_nearest_available_rest_area, has_arrived_at_rest_area,
    nearest_walkable_adjacent_to_rest_area, rest_area_has_capacity,
};

/// 集会エリアに「到着した」とみなす半径（escaping.rs 等から使用）
pub(crate) const GATHERING_ARRIVAL_RADIUS: f32 = TILE_SIZE * GATHERING_ARRIVAL_RADIUS_BASE;

/// アイドル行動の決定システム (Decide Phase)
///
/// 怠惰行動のAIロジック。やる気が低い魂は怠惰な行動をする。
/// タスクがある魂は怠惰行動をしない。
///
/// このシステムはIdleState, Destination, Pathの更新と、
/// IdleBehaviorRequestの発行を行う。実際のエンティティ操作は
/// idle_behavior_apply_systemで行われる。
pub fn idle_behavior_decision_system(
    time: Res<Time>,
    mut request_writer: MessageWriter<IdleBehaviorRequest>,
    world_map: Res<WorldMap>,
    q_spots: Query<(
        Entity,
        &GatheringSpot,
        &crate::relationships::GatheringParticipants,
    )>,
    q_rest_areas: Query<(
        Entity,
        &Transform,
        &RestArea,
        Option<&crate::relationships::RestAreaOccupants>,
        Option<&RestAreaReservations>,
    )>,
    mut query: IdleDecisionSoulQuery,
    spot_grid: Res<GatheringSpotSpatialGrid>,
    soul_grid: Res<crate::systems::spatial::SpatialGrid>,
) {
    let dt = time.delta_secs();
    let mut pending_rest_reservations: HashMap<Entity, usize> = HashMap::new();

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
    ) in query.iter_mut()
    {
        let (gathering_center, target_spot_entity) =
            resolve_gathering_target(participating_in, &q_spots, &spot_grid, &transform);

        if exhausted_gathering::process_exhausted_gathering(
            entity,
            transform.translation.truncate(),
            gathering_center,
            target_spot_entity,
            participating_in,
            &mut idle,
            &mut dest,
            &mut path,
            &mut request_writer,
        ) {
            continue;
        }

        if task_override::process_task_override(
            entity,
            &task,
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

        if matches!(
            idle.behavior,
            IdleBehavior::Resting | IdleBehavior::GoingToRest
        ) && resting_in.is_none()
        {
            let rest_area_target = resolve_rest_area_target(
                reserved_rest_area,
                dest.0,
                current_pos,
                &q_rest_areas,
                &pending_rest_reservations,
            );

            if let Some((rest_area_entity, rest_area_pos)) = rest_area_target {
                let just_reserved = if reserved_rest_area != Some(rest_area_entity) {
                    request_writer.write(IdleBehaviorRequest {
                        entity,
                        operation: IdleBehaviorOperation::ReserveRestArea { rest_area_entity },
                    });
                    *pending_rest_reservations
                        .entry(rest_area_entity)
                        .or_insert(0) += 1;
                    true
                } else {
                    false
                };

                if rest_decision::process_resting_or_going_to_rest(
                    entity,
                    Some((rest_area_entity, rest_area_pos)),
                    reserved_rest_area,
                    participating_in,
                    &mut idle,
                    &mut dest,
                    &mut path,
                    &world_map,
                    &mut request_writer,
                    current_pos,
                    just_reserved,
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

        if matches!(
            idle.behavior,
            IdleBehavior::Escaping | IdleBehavior::Drifting
        ) {
            continue;
        }

        let wants_rest_area = soul.laziness > LAZINESS_THRESHOLD_MID
            || soul.fatigue > FATIGUE_IDLE_THRESHOLD * 0.5
            || soul.stress > ESCAPE_STRESS_THRESHOLD
            || idle.total_idle_time > IDLE_TIME_TO_GATHERING * 0.3;

        if wants_rest_area {
            let rest_area_target = resolve_rest_area_target(
                reserved_rest_area,
                current_pos,
                current_pos,
                &q_rest_areas,
                &pending_rest_reservations,
            );

            if let Some((rest_area_entity, rest_area_pos)) = rest_area_target {
                let just_reserved = if reserved_rest_area != Some(rest_area_entity) {
                    request_writer.write(IdleBehaviorRequest {
                        entity,
                        operation: IdleBehaviorOperation::ReserveRestArea { rest_area_entity },
                    });
                    *pending_rest_reservations
                        .entry(rest_area_entity)
                        .or_insert(0) += 1;
                    true
                } else {
                    false
                };

                if rest_decision::process_wants_rest_area(
                    entity,
                    Some((rest_area_entity, rest_area_pos)),
                    reserved_rest_area,
                    participating_in,
                    &mut idle,
                    &mut dest,
                    &mut path,
                    &world_map,
                    &mut request_writer,
                    current_pos,
                    just_reserved,
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
                    idle.gathering_behavior = transitions::random_gathering_behavior();
                    idle.gathering_behavior_timer = 0.0;
                    idle.gathering_behavior_duration = transitions::random_gathering_duration();
                    idle.needs_separation = true;
                }
                idle.behavior = if soul.fatigue > FATIGUE_GATHERING_THRESHOLD {
                    IdleBehavior::ExhaustedGathering
                } else {
                    IdleBehavior::Gathering
                };
            } else {
                idle.behavior = transitions::select_next_behavior(
                    soul.laziness,
                    soul.fatigue,
                    idle.total_idle_time,
                );
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

            idle.behavior_duration = transitions::behavior_duration_for(idle.behavior);
        }

        motion_dispatch::update_motion_destinations(
            entity,
            current_pos,
            gathering_center,
            target_spot_entity,
            participating_in,
            &mut idle,
            &mut dest,
            &mut path,
            &*soul_grid,
            &world_map,
            &mut request_writer,
            dt,
        );
    }
}

fn resolve_gathering_target(
    participating_in: Option<&crate::relationships::ParticipatingIn>,
    q_spots: &Query<(
        Entity,
        &GatheringSpot,
        &crate::relationships::GatheringParticipants,
    )>,
    spot_grid: &GatheringSpotSpatialGrid,
    transform: &Transform,
) -> (Option<Vec2>, Option<Entity>) {
    if let Some(p) = participating_in {
        let center = q_spots.get(p.0).ok().map(|(_, s, _)| s.center);
        (center, Some(p.0))
    } else {
        let pos = transform.translation.truncate();
        let spot_entities = spot_grid.get_nearby_in_radius(pos, GATHERING_LEAVE_RADIUS * 2.0);
        let nearest = spot_entities
            .iter()
            .filter_map(|&e| q_spots.get(e).ok())
            .filter(|item| item.2.len() < item.1.max_capacity)
            .min_by(|a, b| {
                a.1.center
                    .distance_squared(pos)
                    .partial_cmp(&b.1.center.distance_squared(pos))
                    .unwrap()
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
    q_rest_areas: &Query<(
        Entity,
        &Transform,
        &RestArea,
        Option<&crate::relationships::RestAreaOccupants>,
        Option<&RestAreaReservations>,
    )>,
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
