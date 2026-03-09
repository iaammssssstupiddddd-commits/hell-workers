use std::collections::HashSet;

use bevy::prelude::*;

use crate::entities::familiar::{ActiveCommand, FamiliarCommand};
use crate::relationships::{ManagedBy, TaskWorkers};
use crate::systems::jobs::{Designation, WorkType};
use crate::systems::logistics::ResourceType;
use crate::systems::logistics::transport_request::TransportRequestKind;
use crate::systems::logistics::transport_request::{TransportDemand, TransportRequest};
use crate::systems::world::zones::{AreaBounds, Yard};

pub(crate) fn collect_active_familiars(
    q_familiars: &Query<(Entity, &ActiveCommand, &crate::systems::command::TaskArea)>,
) -> Vec<(Entity, AreaBounds)> {
    q_familiars
        .iter()
        .filter(|(_, active_command, _)| !matches!(active_command.command, FamiliarCommand::Idle))
        .map(|(entity, _, area)| (entity, area.bounds()))
        .collect()
}

pub(crate) fn collect_active_yards(q_yards: &Query<(Entity, &Yard)>) -> Vec<(Entity, Yard)> {
    q_yards
        .iter()
        .map(|(entity, yard)| (entity, yard.clone()))
        .collect()
}

pub(crate) fn collect_collect_sand_familiar_states(
    q_requests_for_demand: &Query<(
        &TransportRequest,
        Option<&TaskWorkers>,
        Option<&TransportDemand>,
    )>,
    q_collect_sand_tasks: &Query<(&Designation, &ManagedBy, Option<&TaskWorkers>)>,
) -> (HashSet<Entity>, HashSet<Entity>) {
    let mut familiar_with_collect_sand_demand = HashSet::<Entity>::new();
    for (request, workers_opt, demand_opt) in q_requests_for_demand.iter() {
        if !request_is_collect_sand_demand(request) {
            continue;
        }

        let desired_slots = demand_opt.map(|d| d.desired_slots).unwrap_or(0);
        let workers = workers_opt.map(|w| w.len() as u32).unwrap_or(0);
        if desired_slots == 0 && workers == 0 {
            continue;
        }

        familiar_with_collect_sand_demand.insert(request.issued_by);
    }

    let mut familiar_with_collect_sand_task = HashSet::<Entity>::new();
    for (designation, managed_by, _workers_opt) in q_collect_sand_tasks.iter() {
        if designation.work_type != WorkType::CollectSand {
            continue;
        }
        familiar_with_collect_sand_task.insert(managed_by.0);
    }

    (
        familiar_with_collect_sand_demand,
        familiar_with_collect_sand_task,
    )
}

pub(crate) fn collect_inflight_mixer_requests(
    q_mixer_requests: &Query<(
        Entity,
        &crate::systems::jobs::TargetMixer,
        &TransportRequest,
        Option<&Designation>,
        Option<&TaskWorkers>,
    )>,
) -> (
    std::collections::HashMap<Entity, u32>,
    std::collections::HashMap<Entity, u32>,
) {
    let mut water_inflight_by_mixer = std::collections::HashMap::<Entity, u32>::new();
    let mut sand_inflight_by_mixer = std::collections::HashMap::<Entity, u32>::new();

    for (_, target_mixer, request, _, workers_opt) in q_mixer_requests.iter() {
        let workers = workers_opt.map(|w| w.len() as u32).unwrap_or(0);
        if workers == 0 {
            continue;
        }

        match (request.kind, request.resource_type) {
            (TransportRequestKind::DeliverWaterToMixer, _) => {
                *water_inflight_by_mixer.entry(target_mixer.0).or_insert(0) += workers;
            }
            (TransportRequestKind::DeliverToMixerSolid, ResourceType::Sand) => {
                *sand_inflight_by_mixer.entry(target_mixer.0).or_insert(0) += workers;
            }
            _ => {}
        }
    }

    (water_inflight_by_mixer, sand_inflight_by_mixer)
}

fn request_is_collect_sand_demand(request: &TransportRequest) -> bool {
    matches!(
        (request.kind, request.resource_type),
        (
            TransportRequestKind::DeliverToMixerSolid,
            ResourceType::Sand
        ) | (TransportRequestKind::DeliverToBlueprint, ResourceType::Sand)
            | (
                TransportRequestKind::DeliverToBlueprint,
                ResourceType::StasisMud
            )
            | (
                TransportRequestKind::DeliverToFloorConstruction,
                ResourceType::StasisMud
            )
            | (
                TransportRequestKind::DeliverToWallConstruction,
                ResourceType::StasisMud
            )
            | (
                TransportRequestKind::DeliverToProvisionalWall,
                ResourceType::StasisMud
            )
    )
}
