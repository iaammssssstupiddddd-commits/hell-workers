use bevy::prelude::*;

use crate::constants::{REST_AREA_RECRUIT_COOLDOWN_SECS, REST_AREA_RESTING_DURATION};
use crate::entities::damned_soul::{IdleBehavior, IdleState, Path, RestAreaCooldown};
use crate::events::{IdleBehaviorOperation, IdleBehaviorRequest};
use crate::relationships::{
    ParticipatingIn, RestAreaOccupants, RestAreaReservations, RestAreaReservedFor, RestingIn,
};
use crate::systems::jobs::RestArea;
use std::collections::HashMap;

/// アイドル行動の適用システム (Execute Phase)
///
/// IdleBehaviorRequestを読み取り、実際のエンティティ操作を行う。
/// - ParticipatingInの追加/削除
/// - イベントのトリガー
pub fn idle_behavior_apply_system(
    mut commands: Commands,
    mut request_reader: MessageReader<IdleBehaviorRequest>,
    mut q_idle: Query<&mut IdleState>,
    mut q_path: Query<&mut Path>,
    mut q_visibility: Query<&mut Visibility, With<crate::entities::damned_soul::DamnedSoul>>,
    q_rest_reserved: Query<&RestAreaReservedFor>,
    q_participating: Query<(), With<ParticipatingIn>>,
    q_rest_areas: Query<(
        &RestArea,
        Option<&RestAreaOccupants>,
        Option<&RestAreaReservations>,
    )>,
) {
    let mut pending_rest_reservations: HashMap<Entity, usize> = HashMap::new();
    let mut pending_rest_entries: HashMap<Entity, usize> = HashMap::new();
    for request in request_reader.read() {
        match &request.operation {
            IdleBehaviorOperation::JoinGathering { spot_entity } => {
                commands
                    .entity(request.entity)
                    .remove::<(RestingIn, RestAreaReservedFor)>();
                if let Ok(mut visibility) = q_visibility.get_mut(request.entity) {
                    *visibility = Visibility::Visible;
                }
                commands
                    .entity(request.entity)
                    .insert(ParticipatingIn(*spot_entity));
                commands.trigger(crate::events::OnGatheringParticipated {
                    entity: request.entity,
                    spot_entity: *spot_entity,
                });
            }
            IdleBehaviorOperation::LeaveGathering { spot_entity: _ } => {
                commands.entity(request.entity).remove::<ParticipatingIn>();
                commands.trigger(crate::events::OnGatheringLeft {
                    entity: request.entity,
                });
            }
            IdleBehaviorOperation::ArriveAtGathering { spot_entity } => {
                commands
                    .entity(request.entity)
                    .remove::<(RestingIn, RestAreaReservedFor)>();
                if let Ok(mut visibility) = q_visibility.get_mut(request.entity) {
                    *visibility = Visibility::Visible;
                }
                commands
                    .entity(request.entity)
                    .insert(ParticipatingIn(*spot_entity));
                commands.trigger(crate::events::OnGatheringParticipated {
                    entity: request.entity,
                    spot_entity: *spot_entity,
                });
                commands.trigger(crate::events::OnGatheringJoined {
                    entity: request.entity,
                });
            }
            IdleBehaviorOperation::ReserveRestArea { rest_area_entity } => {
                let can_reserve = q_rest_areas
                    .get(*rest_area_entity)
                    .map(|(rest_area, occupants, reservations)| {
                        let current =
                            occupants.map_or(0, crate::relationships::RestAreaOccupants::len);
                        let reserved = reservations
                            .map_or(0, crate::relationships::RestAreaReservations::len);
                        let pending = pending_rest_reservations
                            .get(rest_area_entity)
                            .copied()
                            .unwrap_or(0);
                        current + reserved + pending < rest_area.capacity
                    })
                    .unwrap_or(false);
                if !can_reserve {
                    continue;
                }

                *pending_rest_reservations
                    .entry(*rest_area_entity)
                    .or_insert(0) += 1;
                commands
                    .entity(request.entity)
                    .insert(RestAreaReservedFor(*rest_area_entity));
            }
            IdleBehaviorOperation::ReleaseRestArea => {
                commands.entity(request.entity).remove::<RestAreaReservedFor>();
            }
            IdleBehaviorOperation::EnterRestArea { rest_area_entity } => {
                let has_reservation_for_target = q_rest_reserved
                    .get(request.entity)
                    .map(|reserved| reserved.0 == *rest_area_entity)
                    .unwrap_or(false);
                let can_enter = q_rest_areas
                    .get(*rest_area_entity)
                    .map(|(rest_area, occupants, reservations)| {
                        let current =
                            occupants.map_or(0, crate::relationships::RestAreaOccupants::len);
                        let mut reserved = reservations
                            .map_or(0, crate::relationships::RestAreaReservations::len);
                        if has_reservation_for_target {
                            reserved = reserved.saturating_sub(1);
                        }
                        let pending_reservations = pending_rest_reservations
                            .get(rest_area_entity)
                            .copied()
                            .unwrap_or(0);
                        let pending = pending_rest_entries.get(rest_area_entity).copied().unwrap_or(0);
                        current + reserved + pending_reservations + pending < rest_area.capacity
                    })
                    .unwrap_or(false);
                if !can_enter {
                    continue;
                }
                *pending_rest_entries.entry(*rest_area_entity).or_insert(0) += 1;

                if q_participating.get(request.entity).is_ok() {
                    commands.entity(request.entity).remove::<ParticipatingIn>();
                    commands.trigger(crate::events::OnGatheringLeft {
                        entity: request.entity,
                    });
                }

                commands.entity(request.entity).remove::<RestAreaCooldown>();
                commands
                    .entity(request.entity)
                    .remove::<RestAreaReservedFor>()
                    .insert(RestingIn(*rest_area_entity));
                if let Ok(mut visibility) = q_visibility.get_mut(request.entity) {
                    *visibility = Visibility::Hidden;
                }

                if let Ok(mut idle) = q_idle.get_mut(request.entity) {
                    idle.behavior = IdleBehavior::Resting;
                    idle.idle_timer = 0.0;
                    idle.total_idle_time = 0.0;
                    idle.behavior_duration = REST_AREA_RESTING_DURATION;
                    idle.needs_separation = false;
                }
                if let Ok(mut path) = q_path.get_mut(request.entity) {
                    path.waypoints.clear();
                    path.current_index = 0;
                }
            }
            IdleBehaviorOperation::LeaveRestArea => {
                commands
                    .entity(request.entity)
                    .remove::<(RestingIn, RestAreaReservedFor)>();
                commands.entity(request.entity).insert(RestAreaCooldown {
                    remaining_secs: REST_AREA_RECRUIT_COOLDOWN_SECS,
                });
                if let Ok(mut visibility) = q_visibility.get_mut(request.entity) {
                    *visibility = Visibility::Visible;
                }

                if let Ok(mut idle) = q_idle.get_mut(request.entity) {
                    if matches!(idle.behavior, IdleBehavior::Resting | IdleBehavior::GoingToRest) {
                        idle.behavior = IdleBehavior::Wandering;
                    }
                    idle.idle_timer = 0.0;
                }
                if let Ok(mut path) = q_path.get_mut(request.entity) {
                    path.waypoints.clear();
                    path.current_index = 0;
                }
            }
        }
    }
}
