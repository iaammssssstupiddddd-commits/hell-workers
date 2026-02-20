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

pub(super) fn collect_raw_demand_by_owner(
    owner_infos: &HashMap<Entity, OwnerInfo>,
    q_bp_requests: &Query<(&TransportRequest, &TargetBlueprint, Option<&TaskWorkers>)>,
    q_wall_requests: &Query<(
        &TransportRequest,
        &TargetWallConstructionSite,
        Option<&TaskWorkers>,
        Option<&TransportDemand>,
    )>,
    q_blueprints: &Query<&Blueprint>,
) -> HashMap<(Entity, ResourceType), u32> {
    let mut raw_demand_by_owner = HashMap::<(Entity, ResourceType), u32>::new();
    let mut inflight_by_blueprint = HashMap::<(Entity, Entity), HashMap<ResourceType, u32>>::new();

    for (req, target_bp, workers_opt) in q_bp_requests.iter() {
        if !matches!(req.kind, TransportRequestKind::DeliverToBlueprint) {
            continue;
        }
        if !matches!(req.resource_type, ResourceType::Wood | ResourceType::Rock) {
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
            if !matches!(resource_type, ResourceType::Wood | ResourceType::Rock) {
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
            if needed == 0 {
                continue;
            }

            *raw_demand_by_owner
                .entry((owner, resource_type))
                .or_insert(0) += needed;
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
            *raw_demand_by_owner
                .entry((owner, preferred_resource))
                .or_insert(0) += needed;
        }
    }

    for (req, _target_site, workers_opt, demand_opt) in q_wall_requests.iter() {
        if !matches!(req.kind, TransportRequestKind::DeliverToWallConstruction) {
            continue;
        }
        if !matches!(req.resource_type, ResourceType::Wood) {
            continue;
        }
        if !owner_infos.contains_key(&req.issued_by) {
            continue;
        }

        let Some(demand) = demand_opt else {
            continue;
        };
        let inflight = workers_opt.map(|workers| workers.len() as u32).unwrap_or(0);
        let needed = demand.desired_slots.saturating_sub(inflight);
        if needed == 0 {
            continue;
        }

        *raw_demand_by_owner
            .entry((req.issued_by, req.resource_type))
            .or_insert(0) += needed;
    }

    raw_demand_by_owner
}
