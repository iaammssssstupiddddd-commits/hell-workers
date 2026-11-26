use bevy::prelude::*;
use hw_jobs::{
    DeconstructionAssignmentFacts, DeconstructionAssignmentRejection, DeconstructionTargetMarkers,
    validate_deconstruction_assignment_facts,
};
use hw_logistics::ResourceType;
use std::collections::HashSet;

use crate::familiar_ai::decide::task_management::{
    CandidateRejectReason, FamiliarTaskAssignmentQueries,
};

/// Resolves and validates the M2 deconstruction target without changing the
/// durable order or its one-slot worker relationship.
pub fn resolve_assignable_deconstruction_target(
    order: Entity,
    queries: &FamiliarTaskAssignmentQueries<'_, '_>,
    active_move_targets: &HashSet<Entity>,
) -> Result<Entity, CandidateRejectReason> {
    let target = queries
        .deconstruction_order_targets
        .get(order)
        .map_err(|_| CandidateRejectReason::StaleInput)?
        .0;

    let pending = queries
        .deconstruction_pending
        .get(target)
        .map_err(|_| CandidateRejectReason::StaleInput)?;
    let move_conflict = queries.move_planned.get(target).is_ok()
        || queries.pending_building_moves.get(target).is_ok()
        || queries
            .move_plant_tasks
            .iter()
            .any(|move_task| move_task.building == target)
        || active_move_targets.contains(&target);

    let Ok((_, building, provisional_wall)) = queries.storage.buildings.get(target) else {
        return Err(CandidateRejectReason::StaleInput);
    };
    let water_storage = queries
        .storage
        .stockpiles
        .get(target)
        .is_ok_and(|(_, _, stockpile, _)| stockpile.resource_type == Some(ResourceType::Water));
    let markers = DeconstructionTargetMarkers {
        water_storage,
        mud_mixer_storage: queries.storage.mixers.get(target).is_ok(),
        rest_area: queries.rest_areas.get(target).is_ok(),
        wheelbarrow_parking: queries.wheelbarrow_parkings.get(target).is_ok(),
        sand_pile: queries.sand_piles.get(target).is_ok(),
        bone_pile: queries.bone_piles.get(target).is_ok(),
        door: queries.doors.get(target).is_ok(),
        bridge: queries.bridges.get(target).is_ok(),
        operational_soul_spa: queries
            .soul_spa_sites
            .get(target)
            .is_ok_and(|site| site.phase == hw_energy::SoulSpaPhase::Operational),
        power_consumer: queries.power_consumers.get(target).is_ok(),
        power_generator: queries.power_generators.get(target).is_ok(),
    };
    validate_deconstruction_assignment_facts(DeconstructionAssignmentFacts {
        pending_matches_order: pending.order == order,
        claim_present: queries.deconstruction_claims.get(target).is_ok(),
        move_conflict,
        blocker_active: queries
            .deconstruction_blockers
            .get(order)
            .is_ok_and(|blocker| blocker.active),
        provisional: building.is_provisional || provisional_wall.is_some(),
        kind: building.kind,
        markers,
    })
    .map_err(|reason| match reason {
        DeconstructionAssignmentRejection::Claimed => CandidateRejectReason::TemporaryContention,
        DeconstructionAssignmentRejection::DependencyWaiting
        | DeconstructionAssignmentRejection::UnsupportedTarget => {
            CandidateRejectReason::DependencyWaiting
        }
        DeconstructionAssignmentRejection::MalformedOrder
        | DeconstructionAssignmentRejection::MalformedTarget => {
            CandidateRejectReason::MalformedTask
        }
    })?;

    Ok(target)
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;
    use hw_energy::{PowerConsumer, PowerGenerator, SoulSpaPhase, SoulSpaSite};
    use hw_jobs::mud_mixer::MudMixerStorage;
    use hw_jobs::{
        BonePile, BridgeMarker, Building, BuildingType, DeconstructionOrder, DeconstructionPending,
        Designation, Door, RestArea, SandPile, TargetDeconstructionRoot, TaskSlots, WorkType,
    };
    use hw_logistics::types::WheelbarrowParking;
    use hw_logistics::zone::Stockpile;

    fn remove_required_marker(world: &mut World, target: Entity, kind: BuildingType) -> bool {
        let mut entity = world.entity_mut(target);
        match kind {
            BuildingType::Wall | BuildingType::Floor | BuildingType::ScreamingVent => return false,
            BuildingType::Door => {
                entity.remove::<Door>();
            }
            BuildingType::Bridge => {
                entity.remove::<BridgeMarker>();
            }
            BuildingType::Tank => {
                entity.remove::<Stockpile>();
            }
            BuildingType::MudMixer => {
                entity.remove::<MudMixerStorage>();
            }
            BuildingType::RestArea => {
                entity.remove::<RestArea>();
            }
            BuildingType::WheelbarrowParking => {
                entity.remove::<WheelbarrowParking>();
            }
            BuildingType::SandPile => {
                entity.remove::<SandPile>();
            }
            BuildingType::BonePile => {
                entity.remove::<BonePile>();
            }
            BuildingType::SoulSpa => {
                entity.remove::<PowerGenerator>();
            }
            BuildingType::Equipment(kind) => {
                if kind.demand() == 0.0 {
                    return false;
                }
                entity.remove::<PowerConsumer>();
            }
            BuildingType::OutdoorLamp => {
                entity.remove::<PowerConsumer>();
            }
        }
        true
    }

    fn target_for(world: &mut World, kind: BuildingType) -> Entity {
        let mut entity = world.spawn((
            Building {
                kind,
                is_provisional: false,
            },
            Transform::default(),
        ));
        match kind {
            BuildingType::Wall | BuildingType::Floor | BuildingType::ScreamingVent => {}
            BuildingType::Door => {
                entity.insert(Door::default());
            }
            BuildingType::Bridge => {
                entity.insert(BridgeMarker);
            }
            BuildingType::Tank => {
                entity.insert(Stockpile {
                    capacity: 50,
                    resource_type: Some(ResourceType::Water),
                });
            }
            BuildingType::MudMixer => {
                entity.insert((
                    Stockpile {
                        capacity: 50,
                        resource_type: Some(ResourceType::Water),
                    },
                    MudMixerStorage::default(),
                ));
            }
            BuildingType::RestArea => {
                entity.insert(RestArea { capacity: 4 });
            }
            BuildingType::WheelbarrowParking => {
                entity.insert(WheelbarrowParking { capacity: 2 });
            }
            BuildingType::SandPile => {
                entity.insert(SandPile);
            }
            BuildingType::BonePile => {
                entity.insert(BonePile);
            }
            BuildingType::SoulSpa => {
                entity.insert(SoulSpaSite {
                    phase: SoulSpaPhase::Operational,
                    ..default()
                });
            }
            BuildingType::Equipment(kind) => {
                if kind.demand() > 0.0 {
                    entity.insert(PowerConsumer {
                        demand: kind.demand(),
                    });
                }
            }
            BuildingType::OutdoorLamp => {
                entity.insert(PowerConsumer { demand: 1.0 });
            }
        }
        entity.id()
    }

    fn inspect(world: &mut World, order: Entity) -> Result<Entity, CandidateRejectReason> {
        world
            .run_system_once(move |queries: FamiliarTaskAssignmentQueries| {
                resolve_assignable_deconstruction_target(order, &queries, &HashSet::new())
            })
            .unwrap()
    }

    #[test]
    fn every_kind_checks_live_marker_shape_before_candidate_assignment() {
        for kind in BuildingType::ALL {
            let mut app = App::new();
            app.init_resource::<hw_world::WorldMap>()
                .init_resource::<hw_logistics::SharedResourceCache>()
                .init_resource::<hw_logistics::transport_request::WheelbarrowArbitrationDiagnostics>()
                .add_message::<hw_core::events::ResourceReservationRequest>()
                .add_message::<hw_jobs::events::TaskAssignmentRequest>();
            let target = target_for(app.world_mut(), kind);
            let order = app
                .world_mut()
                .spawn((
                    DeconstructionOrder,
                    TargetDeconstructionRoot(target),
                    Designation {
                        work_type: WorkType::Deconstruct,
                    },
                    TaskSlots::new(1),
                ))
                .id();
            app.world_mut()
                .entity_mut(target)
                .insert(DeconstructionPending { order });
            assert_eq!(inspect(app.world_mut(), order), Ok(target), "{kind:?}");
            if kind == BuildingType::Tank {
                app.world_mut()
                    .get_mut::<Stockpile>(target)
                    .unwrap()
                    .resource_type = Some(ResourceType::Wood);
                assert_eq!(
                    inspect(app.world_mut(), order),
                    Err(CandidateRejectReason::MalformedTask)
                );
                app.world_mut()
                    .get_mut::<Stockpile>(target)
                    .unwrap()
                    .resource_type = Some(ResourceType::Water);
            }
            if kind == BuildingType::SoulSpa {
                app.world_mut()
                    .get_mut::<SoulSpaSite>(target)
                    .unwrap()
                    .phase = SoulSpaPhase::Constructing;
                assert_eq!(
                    inspect(app.world_mut(), order),
                    Err(CandidateRejectReason::MalformedTask)
                );
                app.world_mut()
                    .get_mut::<SoulSpaSite>(target)
                    .unwrap()
                    .phase = SoulSpaPhase::Operational;
            }
            if remove_required_marker(app.world_mut(), target, kind) {
                assert_eq!(
                    inspect(app.world_mut(), order),
                    Err(CandidateRejectReason::MalformedTask),
                    "missing marker: {kind:?}"
                );
                app.world_mut().despawn(target);
                let replacement = target_for(app.world_mut(), kind);
                app.world_mut()
                    .entity_mut(replacement)
                    .insert(DeconstructionPending { order });
                app.world_mut()
                    .entity_mut(order)
                    .insert(TargetDeconstructionRoot(replacement));
                assert_eq!(
                    inspect(app.world_mut(), order),
                    Ok(replacement),
                    "replacement: {kind:?}"
                );
                if kind == BuildingType::SandPile {
                    app.world_mut().entity_mut(replacement).insert(BonePile);
                } else {
                    app.world_mut().entity_mut(replacement).insert(SandPile);
                }
                assert_eq!(
                    inspect(app.world_mut(), order),
                    Err(CandidateRejectReason::MalformedTask),
                    "extra marker: {kind:?}"
                );
                continue;
            }
            if kind == BuildingType::SandPile {
                app.world_mut().entity_mut(target).insert(BonePile);
            } else {
                app.world_mut().entity_mut(target).insert(SandPile);
            }
            assert_eq!(
                inspect(app.world_mut(), order),
                Err(CandidateRejectReason::MalformedTask),
                "{kind:?}"
            );
        }
    }
}
