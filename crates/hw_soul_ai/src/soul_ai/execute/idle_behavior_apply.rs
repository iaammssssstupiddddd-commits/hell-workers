use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use std::collections::HashMap;

use hw_core::constants::{REST_AREA_RECRUIT_COOLDOWN_SECS, REST_AREA_RESTING_DURATION};
use hw_core::events::{
    IdleBehaviorOperation, IdleBehaviorRequest, OnGatheringJoined, OnGatheringParticipated,
};
use hw_core::relationships::{
    ParticipatingIn, RestAreaOccupants, RestAreaReservations, RestAreaReservedFor, RestingIn,
};
use hw_core::soul::{DamnedSoul, IdleBehavior, IdleState, Path, RestAreaCooldown};
use hw_jobs::{DeconstructionPending, RestArea};

type RestAreaEntities<'w, 's> = Query<
    'w,
    's,
    (
        &'static RestArea,
        Option<&'static RestAreaOccupants>,
        Option<&'static RestAreaReservations>,
        Option<&'static DeconstructionPending>,
    ),
>;

#[derive(SystemParam)]
pub struct RestAreaCheckQueries<'w, 's> {
    pub q_rest_reserved: Query<'w, 's, &'static RestAreaReservedFor>,
    pub q_rest_cooldown: Query<'w, 's, &'static RestAreaCooldown>,
    pub q_rest_areas: RestAreaEntities<'w, 's>,
}

fn set_soul_visibility(world: &mut World, soul: Entity, visibility: Visibility) {
    if world.get::<DamnedSoul>(soul).is_some()
        && let Some(mut current) = world.get_mut::<Visibility>(soul)
    {
        *current = visibility;
    }
}

fn queue_idle_gathering_join(commands: &mut Commands, soul: Entity, spot: Entity, arrived: bool) {
    commands.queue(move |world: &mut World| {
        // A different Execute command can retire the target after Decide (or
        // after this system runs). Keep relation, visibility and notifications
        // together behind the live-target check at command application time.
        if world.get_entity(spot).is_err() {
            return;
        }
        let Ok(mut entity) = world.get_entity_mut(soul) else {
            return;
        };
        entity.remove::<(RestingIn, RestAreaReservedFor)>();
        entity.insert(ParticipatingIn(spot));
        set_soul_visibility(world, soul, Visibility::Visible);
        world.write_message(OnGatheringParticipated {
            entity: soul,
            spot_entity: spot,
        });
        if arrived {
            world.write_message(OnGatheringJoined { entity: soul });
        }
    });
}

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
    rest_queries: RestAreaCheckQueries,
) {
    let mut pending_rest_reservations: HashMap<Entity, usize> = HashMap::new();
    let mut pending_rest_entries: HashMap<Entity, usize> = HashMap::new();
    for request in request_reader.read() {
        match &request.operation {
            IdleBehaviorOperation::JoinGathering { spot_entity } => {
                queue_idle_gathering_join(&mut commands, request.entity, *spot_entity, false);
            }
            IdleBehaviorOperation::LeaveGathering { spot_entity: _ } => {
                commands.entity(request.entity).remove::<ParticipatingIn>();
            }
            IdleBehaviorOperation::ArriveAtGathering { spot_entity } => {
                queue_idle_gathering_join(&mut commands, request.entity, *spot_entity, true);
            }
            IdleBehaviorOperation::ReserveRestArea { rest_area_entity } => {
                let cooldown_active = rest_queries
                    .q_rest_cooldown
                    .get(request.entity)
                    .map(|cooldown| cooldown.remaining_secs > f32::EPSILON)
                    .unwrap_or(false);
                if cooldown_active {
                    commands
                        .entity(request.entity)
                        .remove::<RestAreaReservedFor>();
                    continue;
                }
                let can_reserve = rest_queries
                    .q_rest_areas
                    .get(*rest_area_entity)
                    .map(
                        |(rest_area, occupants, reservations, deconstruction_pending)| {
                            let current = occupants.map_or(0, RestAreaOccupants::len);
                            let reserved = reservations.map_or(0, RestAreaReservations::len);
                            let pending = pending_rest_reservations
                                .get(rest_area_entity)
                                .copied()
                                .unwrap_or(0);
                            deconstruction_pending.is_none()
                                && current + reserved + pending < rest_area.capacity
                        },
                    )
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
                commands
                    .entity(request.entity)
                    .remove::<RestAreaReservedFor>();
            }
            IdleBehaviorOperation::EnterRestArea { rest_area_entity } => {
                let cooldown_active = rest_queries
                    .q_rest_cooldown
                    .get(request.entity)
                    .map(|cooldown| cooldown.remaining_secs > f32::EPSILON)
                    .unwrap_or(false);
                if cooldown_active {
                    commands
                        .entity(request.entity)
                        .remove::<RestAreaReservedFor>();
                    continue;
                }
                let has_reservation_for_target = rest_queries
                    .q_rest_reserved
                    .get(request.entity)
                    .map(|reserved| reserved.0 == *rest_area_entity)
                    .unwrap_or(false);
                let can_enter = rest_queries
                    .q_rest_areas
                    .get(*rest_area_entity)
                    .map(
                        |(rest_area, occupants, reservations, deconstruction_pending)| {
                            let current = occupants.map_or(0, RestAreaOccupants::len);
                            let mut reserved = reservations.map_or(0, RestAreaReservations::len);
                            if has_reservation_for_target {
                                reserved = reserved.saturating_sub(1);
                            }
                            let pending_reservations = pending_rest_reservations
                                .get(rest_area_entity)
                                .copied()
                                .unwrap_or(0);
                            let pending = pending_rest_entries
                                .get(rest_area_entity)
                                .copied()
                                .unwrap_or(0);
                            deconstruction_pending.is_none()
                                && current + reserved + pending_reservations + pending
                                    < rest_area.capacity
                        },
                    )
                    .unwrap_or(false);
                if !can_enter {
                    continue;
                }
                *pending_rest_entries.entry(*rest_area_entity).or_insert(0) += 1;

                // A preceding request can have queued a join in this batch.
                commands.entity(request.entity).remove::<ParticipatingIn>();

                commands.entity(request.entity).remove::<RestAreaCooldown>();
                commands
                    .entity(request.entity)
                    .remove::<RestAreaReservedFor>()
                    .insert(RestingIn(*rest_area_entity));
                let soul = request.entity;
                commands.queue(move |world: &mut World| {
                    set_soul_visibility(world, soul, Visibility::Hidden);
                });

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
                let soul = request.entity;
                commands.queue(move |world: &mut World| {
                    set_soul_visibility(world, soul, Visibility::Visible);
                });

                if let Ok(mut idle) = q_idle.get_mut(request.entity) {
                    if matches!(
                        idle.behavior,
                        IdleBehavior::Resting | IdleBehavior::GoingToRest
                    ) {
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

#[cfg(test)]
mod tests {
    use super::*;
    use hw_core::events::{IdleBehaviorOperation, IdleBehaviorRequest};

    #[test]
    fn gathering_and_rest_requests_preserve_command_order_and_visibility() {
        for arrived in [false, true] {
            for join_first in [false, true] {
                let mut app = App::new();
                app.add_plugins(MinimalPlugins)
                    .add_message::<IdleBehaviorRequest>()
                    .add_message::<OnGatheringJoined>()
                    .add_message::<OnGatheringParticipated>()
                    .add_systems(Update, (idle_behavior_apply_system, ApplyDeferred).chain());
                let rest_area = app.world_mut().spawn(RestArea { capacity: 2 }).id();
                let spot = app.world_mut().spawn_empty().id();
                let soul = app
                    .world_mut()
                    .spawn((DamnedSoul::default(), Visibility::Visible))
                    .id();
                let join = if arrived {
                    IdleBehaviorOperation::ArriveAtGathering { spot_entity: spot }
                } else {
                    IdleBehaviorOperation::JoinGathering { spot_entity: spot }
                };
                let enter = IdleBehaviorOperation::EnterRestArea {
                    rest_area_entity: rest_area,
                };
                for operation in if join_first {
                    [join, enter]
                } else {
                    [enter, join]
                } {
                    app.world_mut().write_message(IdleBehaviorRequest {
                        entity: soul,
                        operation,
                    });
                }
                app.update();
                assert_eq!(app.world().get::<RestingIn>(soul).is_some(), join_first);
                assert_eq!(
                    app.world().get::<ParticipatingIn>(soul).is_some(),
                    !join_first
                );
                assert_eq!(
                    app.world().get::<Visibility>(soul),
                    Some(if join_first {
                        &Visibility::Hidden
                    } else {
                        &Visibility::Visible
                    })
                );
            }
        }
    }

    #[test]
    fn gathering_join_rechecks_both_entities_when_commands_are_applied() {
        for arrived in [false, true] {
            for missing in [None, Some("spot"), Some("soul")] {
                let mut world = World::new();
                world.init_resource::<Messages<OnGatheringJoined>>();
                world.init_resource::<Messages<OnGatheringParticipated>>();
                let rest_area = world.spawn(RestArea { capacity: 2 }).id();
                let soul = world
                    .spawn((
                        DamnedSoul::default(),
                        Visibility::Hidden,
                        RestingIn(rest_area),
                    ))
                    .id();
                let spot = world.spawn_empty().id();
                let mut queue = bevy::ecs::world::CommandQueue::default();
                {
                    let mut commands = Commands::new(&mut queue, &world);
                    // Simulate a target/source retiring in another Execute
                    // command before this already-decided join is applied.
                    if let Some(missing) = missing {
                        commands
                            .entity(if missing == "spot" { spot } else { soul })
                            .despawn();
                    }
                    queue_idle_gathering_join(&mut commands, soul, spot, arrived);
                }
                queue.apply(&mut world);

                let valid = missing.is_none();
                assert_eq!(
                    world.resource::<Messages<OnGatheringParticipated>>().len(),
                    usize::from(valid)
                );
                assert_eq!(
                    world.resource::<Messages<OnGatheringJoined>>().len(),
                    usize::from(valid && arrived)
                );
                if missing != Some("soul") {
                    assert_eq!(
                        world.get::<ParticipatingIn>(soul).map(|p| p.0),
                        valid.then_some(spot)
                    );
                    assert_eq!(world.get::<RestingIn>(soul).is_some(), !valid);
                    assert_eq!(
                        world.get::<Visibility>(soul),
                        Some(if valid {
                            &Visibility::Visible
                        } else {
                            &Visibility::Hidden
                        })
                    );
                }
            }
        }
    }

    #[test]
    fn pending_rest_area_rejects_queued_reserve_and_enter_operations() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_message::<IdleBehaviorRequest>()
            .add_message::<OnGatheringJoined>()
            .add_message::<OnGatheringParticipated>()
            .add_systems(Update, (idle_behavior_apply_system, ApplyDeferred).chain());

        let order = app.world_mut().spawn_empty().id();
        let rest_area = app
            .world_mut()
            .spawn((RestArea { capacity: 2 }, DeconstructionPending { order }))
            .id();
        let souls = [
            app.world_mut()
                .spawn((
                    DamnedSoul::default(),
                    IdleState::default(),
                    Path::default(),
                    Visibility::Visible,
                ))
                .id(),
            app.world_mut()
                .spawn((
                    DamnedSoul::default(),
                    IdleState::default(),
                    Path::default(),
                    Visibility::Visible,
                ))
                .id(),
        ];
        app.world_mut().write_message(IdleBehaviorRequest {
            entity: souls[0],
            operation: IdleBehaviorOperation::ReserveRestArea {
                rest_area_entity: rest_area,
            },
        });
        app.world_mut().write_message(IdleBehaviorRequest {
            entity: souls[1],
            operation: IdleBehaviorOperation::EnterRestArea {
                rest_area_entity: rest_area,
            },
        });

        app.update();

        assert!(app.world().get::<RestAreaReservedFor>(souls[0]).is_none());
        assert!(app.world().get::<RestingIn>(souls[1]).is_none());
        assert_eq!(
            app.world().get::<Visibility>(souls[1]),
            Some(&Visibility::Visible)
        );
    }
}
