use bevy::prelude::*;
use hw_jobs::{
    DeconstructionTargetMarkers, deconstruction_marker_matches, supports_deconstruction_cleanup,
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
    if pending.order != order {
        return Err(CandidateRejectReason::MalformedTask);
    }
    if queries.deconstruction_claims.get(target).is_ok() {
        return Err(CandidateRejectReason::TemporaryContention);
    }
    if queries.move_planned.get(target).is_ok()
        || queries.pending_building_moves.get(target).is_ok()
        || queries
            .move_plant_tasks
            .iter()
            .any(|move_task| move_task.building == target)
        || active_move_targets.contains(&target)
        || queries
            .deconstruction_blockers
            .get(order)
            .is_ok_and(|blocker| blocker.active)
    {
        return Err(CandidateRejectReason::DependencyWaiting);
    }

    let Ok((_, building, provisional_wall)) = queries.storage.buildings.get(target) else {
        return Err(CandidateRejectReason::StaleInput);
    };
    if building.is_provisional || provisional_wall.is_some() {
        return Err(CandidateRejectReason::DependencyWaiting);
    }
    if !supports_deconstruction_cleanup(building.kind) {
        return Err(CandidateRejectReason::DependencyWaiting);
    }
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
    if !deconstruction_marker_matches(building.kind, markers) {
        return Err(CandidateRejectReason::MalformedTask);
    }

    Ok(target)
}
