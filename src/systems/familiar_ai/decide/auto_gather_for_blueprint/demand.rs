use std::collections::HashMap;

use bevy::prelude::*;

use crate::relationships::TaskWorkers;
use crate::systems::jobs::wall_construction::TargetWallConstructionSite;
use crate::systems::jobs::{Blueprint, TargetBlueprint};
use crate::systems::logistics::ResourceType;
use crate::systems::logistics::transport_request::{
    TransportDemand, TransportRequest, TransportRequestKind,
};

use super::helpers::OwnerInfo;

type RawDemandKey = (Entity, ResourceType);

#[derive(Default)]
struct RawDemandAccumulator {
    by_owner: HashMap<RawDemandKey, u32>,
}

impl RawDemandAccumulator {
    fn add_if_supported(
        &mut self,
        owner_infos: &HashMap<Entity, OwnerInfo>,
        owner: Entity,
        resource_type: ResourceType,
        amount: u32,
    ) {
        if amount == 0
            || !owner_infos.contains_key(&owner)
            || !is_auto_gather_resource(resource_type)
        {
            return;
        }
        *self.by_owner.entry((owner, resource_type)).or_insert(0) += amount;
    }

    fn into_inner(self) -> HashMap<RawDemandKey, u32> {
        self.by_owner
    }
}

#[inline]
fn is_auto_gather_resource(resource_type: ResourceType) -> bool {
    matches!(resource_type, ResourceType::Wood | ResourceType::Rock)
}

pub(super) fn collect_raw_demand_by_owner(
    owner_infos: &HashMap<Entity, OwnerInfo>,
    q_bp_requests: &Query<(&TransportRequest, &TargetBlueprint, Option<&TaskWorkers>)>,
    q_wall_requests: &Query<(
        &TransportRequest,
        &TargetWallConstructionSite,
        Option<&TaskWorkers>,
        Option<&TransportDemand>,
    )>,
    q_mixer_solid_requests: &Query<(
        &TransportRequest,
        Option<&TaskWorkers>,
        Option<&TransportDemand>,
    )>,
    q_blueprints: &Query<&Blueprint>,
) -> HashMap<(Entity, ResourceType), u32> {
    let mut raw_demand = RawDemandAccumulator::default();
    let mut inflight_by_blueprint = HashMap::<(Entity, Entity), HashMap<ResourceType, u32>>::new();

    for (req, target_bp, workers_opt) in q_bp_requests.iter() {
        if !matches!(req.kind, TransportRequestKind::DeliverToBlueprint) {
            continue;
        }
        if !is_auto_gather_resource(req.resource_type) {
            continue;
        }
        if !owner_infos.contains_key(&req.issued_by) {
            continue;
        }

        let inflight = workers_opt.map(|workers| workers.len() as u32).unwrap_or(0);
        *inflight_by_blueprint
            .entry((req.issued_by, target_bp.0))
            .or_default()
            .entry(req.resource_type)
            .or_insert(0) += inflight;
    }

    for ((owner, blueprint_entity), inflight_by_resource) in inflight_by_blueprint {
        let Ok(blueprint) = q_blueprints.get(blueprint_entity) else {
            continue;
        };

        for (&resource_type, &required) in &blueprint.required_materials {
            if !is_auto_gather_resource(resource_type) {
                continue;
            }
            if required == 0 {
                continue;
            }

            let delivered = *blueprint
                .delivered_materials
                .get(&resource_type)
                .unwrap_or(&0);
            let inflight = *inflight_by_resource.get(&resource_type).unwrap_or(&0);
            let needed = required.saturating_sub(delivered.saturating_add(inflight));
            raw_demand.add_if_supported(owner_infos, owner, resource_type, needed);
        }

        if let Some(flexible) = &blueprint.flexible_material_requirement {
            if flexible.accepted_types.is_empty() {
                continue;
            }

            let total_inflight_flexible: u32 = flexible
                .accepted_types
                .iter()
                .map(|resource_type| *inflight_by_resource.get(resource_type).unwrap_or(&0))
                .sum();
            let needed = flexible.remaining().saturating_sub(total_inflight_flexible);
            if needed == 0 {
                continue;
            }

            let preferred_resource = if flexible.accepted_types.contains(&ResourceType::Wood) {
                ResourceType::Wood
            } else {
                flexible.accepted_types[0]
            };
            raw_demand.add_if_supported(owner_infos, owner, preferred_resource, needed);
        }
    }

    for (req, _target_site, workers_opt, demand_opt) in q_wall_requests.iter() {
        if !matches!(req.kind, TransportRequestKind::DeliverToWallConstruction) {
            continue;
        }
        if !is_auto_gather_resource(req.resource_type) {
            continue;
        }

        let Some(demand) = demand_opt else {
            continue;
        };
        let inflight = workers_opt.map(|workers| workers.len() as u32).unwrap_or(0);
        let needed = demand.desired_slots.saturating_sub(inflight);
        raw_demand.add_if_supported(owner_infos, req.issued_by, req.resource_type, needed);
    }

    let mut desired_and_inflight_by_mixer =
        HashMap::<(Entity, Entity, ResourceType), (u32, u32)>::new();
    for (req, workers_opt, demand_opt) in q_mixer_solid_requests.iter() {
        if !matches!(req.kind, TransportRequestKind::DeliverToMixerSolid) {
            continue;
        }
        if !is_auto_gather_resource(req.resource_type) {
            continue;
        }

        let entry = desired_and_inflight_by_mixer
            .entry((req.issued_by, req.anchor, req.resource_type))
            .or_insert((0, 0));
        let desired = demand_opt.map(|d| d.desired_slots).unwrap_or(0);
        let inflight = workers_opt.map(|workers| workers.len() as u32).unwrap_or(0);
        entry.0 = entry.0.max(desired);
        entry.1 = entry.1.saturating_add(inflight);
    }

    for ((owner, _mixer, resource_type), (desired_slots, inflight)) in desired_and_inflight_by_mixer
    {
        let needed = desired_slots.saturating_sub(inflight);
        raw_demand.add_if_supported(owner_infos, owner, resource_type, needed);
    }

    raw_demand.into_inner()
}
