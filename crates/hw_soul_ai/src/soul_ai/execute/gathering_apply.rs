use bevy::prelude::*;

use hw_core::events::{GatheringManagementOp, GatheringManagementRequest, OnGatheringParticipated};
use hw_core::relationships::{GatheringParticipants, ParticipatingIn};
use std::collections::HashSet;

fn queue_gathering_join(commands: &mut Commands, soul: Entity, spot: Entity) {
    commands.queue(move |world: &mut World| {
        // The request may have outlived either entity while deferred commands from
        // other Execute systems were applied. Check both at the point that the
        // Relationship is inserted: Bevy warns when a Relationship target is gone.
        if world.get_entity(spot).is_err() {
            return;
        }

        let already_participating = world
            .get_entity(soul)
            .ok()
            .and_then(|soul_entity| soul_entity.get::<ParticipatingIn>())
            .is_some_and(|participating_in| participating_in.0 == spot);
        if already_participating {
            return;
        }

        {
            let Ok(mut soul_entity) = world.get_entity_mut(soul) else {
                return;
            };
            soul_entity.insert(ParticipatingIn(spot));
        }

        world.write_message(OnGatheringParticipated {
            entity: soul,
            spot_entity: spot,
        });
    });
}

fn detach_gathering_participants(
    commands: &mut Commands,
    q_gathering_participants: &Query<&GatheringParticipants>,
    spot: Entity,
) {
    if let Ok(participants) = q_gathering_participants.get(spot) {
        for soul_entity in participants.iter() {
            commands
                .entity(*soul_entity)
                .try_remove::<ParticipatingIn>();
        }
    }
}

/// GatheringManagementRequest を適用する（Execute Phase）
pub fn gathering_apply_system(
    mut commands: Commands,
    mut request_reader: MessageReader<GatheringManagementRequest>,
    q_gathering_participants: Query<&GatheringParticipants>,
) {
    let requests = request_reader.read().cloned().collect::<Vec<_>>();
    let dissolved_spots = requests
        .iter()
        .filter_map(|request| match &request.operation {
            GatheringManagementOp::Dissolve { spot_entity, .. } => Some(*spot_entity),
            _ => None,
        })
        .collect::<HashSet<_>>();
    let retired_spots = requests
        .iter()
        .filter_map(|request| match &request.operation {
            GatheringManagementOp::Dissolve { spot_entity, .. } => Some(*spot_entity),
            GatheringManagementOp::Merge { absorbed, .. } => Some(*absorbed),
            _ => None,
        })
        .collect::<HashSet<_>>();

    for request in &requests {
        match &request.operation {
            GatheringManagementOp::Dissolve {
                spot_entity,
                aura_entity,
                object_entity,
            } => {
                // Keep the source side clean before retiring the target. The
                // request batch has already ruled out new joins to this spot.
                detach_gathering_participants(
                    &mut commands,
                    &q_gathering_participants,
                    *spot_entity,
                );

                commands.entity(*aura_entity).try_despawn();
                if let Some(obj) = object_entity {
                    commands.entity(*obj).try_despawn();
                }
                commands.entity(*spot_entity).try_despawn();
            }
            GatheringManagementOp::Merge {
                absorber,
                absorbed,
                participants_to_move,
                absorbed_aura,
                absorbed_object,
            } => {
                // A merge whose survivor is being retired in the same batch
                // cannot produce a valid relation. If the source is explicitly
                // dissolved too, its dissolve owns the cleanup instead.
                if retired_spots.contains(absorber) || dissolved_spots.contains(absorbed) {
                    continue;
                }

                // Clear the old target before the queued joins. This covers
                // participants added after the decision snapshot as well.
                detach_gathering_participants(&mut commands, &q_gathering_participants, *absorbed);
                for soul_entity in participants_to_move {
                    queue_gathering_join(&mut commands, *soul_entity, *absorber);
                }

                commands.entity(*absorbed_aura).try_despawn();
                if let Some(obj) = absorbed_object {
                    commands.entity(*obj).try_despawn();
                }
                commands.entity(*absorbed).try_despawn();
            }
            GatheringManagementOp::Recruit { soul, spot } => {
                // Decide systems run independently, so a spot can be recruited
                // and retired in one Message batch. A retired target must never
                // receive a ParticipatingIn insert.
                if retired_spots.contains(spot) {
                    continue;
                }

                queue_gathering_join(&mut commands, *soul, *spot);
            }
            GatheringManagementOp::Leave { soul, spot: _ } => {
                commands.entity(*soul).try_remove::<ParticipatingIn>();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::app::ScheduleRunnerPlugin;

    #[derive(Resource, Default)]
    struct ParticipationCount(u32);

    fn count_gathering_participation(
        mut reader: MessageReader<OnGatheringParticipated>,
        mut count: ResMut<ParticipationCount>,
    ) {
        count.0 += reader.read().count() as u32;
    }

    fn test_app() -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins.set(ScheduleRunnerPlugin::run_once()))
            .add_message::<GatheringManagementRequest>()
            .add_message::<OnGatheringParticipated>()
            .init_resource::<ParticipationCount>()
            .add_systems(
                Update,
                (gathering_apply_system, count_gathering_participation).chain(),
            );
        app
    }

    #[test]
    fn dissolve_discards_same_batch_recruit_before_relation_insert() {
        let mut app = test_app();
        let soul = app.world_mut().spawn_empty().id();
        let spot = app.world_mut().spawn(GatheringParticipants::default()).id();
        let aura = app.world_mut().spawn_empty().id();

        app.world_mut().write_message(GatheringManagementRequest {
            operation: GatheringManagementOp::Dissolve {
                spot_entity: spot,
                aura_entity: aura,
                object_entity: None,
            },
        });
        app.world_mut().write_message(GatheringManagementRequest {
            operation: GatheringManagementOp::Recruit { soul, spot },
        });

        app.update();

        assert!(app.world().get_entity(spot).is_err());
        assert!(
            app.world()
                .get_entity(soul)
                .unwrap()
                .get::<ParticipatingIn>()
                .is_none()
        );
        assert_eq!(app.world().resource::<ParticipationCount>().0, 0);
    }

    #[test]
    fn merge_into_a_retired_absorber_is_discarded() {
        let mut app = test_app();
        let soul = app.world_mut().spawn_empty().id();
        let absorber = app.world_mut().spawn(GatheringParticipants::default()).id();
        let absorbed = app.world_mut().spawn(GatheringParticipants::default()).id();
        let absorber_aura = app.world_mut().spawn_empty().id();
        let absorbed_aura = app.world_mut().spawn_empty().id();

        app.world_mut().write_message(GatheringManagementRequest {
            operation: GatheringManagementOp::Dissolve {
                spot_entity: absorber,
                aura_entity: absorber_aura,
                object_entity: None,
            },
        });
        app.world_mut().write_message(GatheringManagementRequest {
            operation: GatheringManagementOp::Merge {
                absorber,
                absorbed,
                participants_to_move: vec![soul],
                absorbed_aura,
                absorbed_object: None,
            },
        });

        app.update();

        assert!(app.world().get_entity(absorber).is_err());
        assert!(app.world().get_entity(absorbed).is_ok());
        assert!(
            app.world()
                .get_entity(soul)
                .unwrap()
                .get::<ParticipatingIn>()
                .is_none()
        );
        assert_eq!(app.world().resource::<ParticipationCount>().0, 0);
    }

    #[test]
    fn stale_recruit_checks_target_at_command_application_time() {
        let mut app = test_app();
        let soul = app.world_mut().spawn_empty().id();
        let spot = app.world_mut().spawn(GatheringParticipants::default()).id();
        assert!(app.world_mut().despawn(spot));

        app.world_mut().write_message(GatheringManagementRequest {
            operation: GatheringManagementOp::Recruit { soul, spot },
        });

        app.update();

        assert!(
            app.world()
                .get_entity(soul)
                .unwrap()
                .get::<ParticipatingIn>()
                .is_none()
        );
        assert_eq!(app.world().resource::<ParticipationCount>().0, 0);
    }

    #[test]
    fn recruit_writes_participation_message_after_inserting_relation() {
        let mut app = test_app();
        let soul = app.world_mut().spawn_empty().id();
        let spot = app.world_mut().spawn(GatheringParticipants::default()).id();

        app.world_mut().write_message(GatheringManagementRequest {
            operation: GatheringManagementOp::Recruit { soul, spot },
        });

        app.update();

        assert_eq!(
            app.world()
                .get_entity(soul)
                .unwrap()
                .get::<ParticipatingIn>()
                .map(|participating_in| participating_in.0),
            Some(spot)
        );
        assert_eq!(app.world().resource::<ParticipationCount>().0, 1);
    }
}
