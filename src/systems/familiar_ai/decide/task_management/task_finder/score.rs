use crate::systems::jobs::{TargetBlueprint, WorkType};
use crate::systems::logistics::transport_request::TransportRequestKind;
use crate::systems::logistics::ResourceType;
use bevy::prelude::*;

pub(super) fn score_candidate(
    entity: Entity,
    work_type: WorkType,
    mut priority: i32,
    in_stockpile_none: bool,
    queries: &crate::systems::soul_ai::execute::task_execution::context::TaskAssignmentQueries,
    q_target_blueprints: &Query<&TargetBlueprint>,
) -> Option<i32> {
    if work_type == WorkType::Build {
        priority += 10;
    } else if work_type == WorkType::Haul {
        if q_target_blueprints.get(entity).is_ok() {
            priority += 10;
        }
        if queries.storage.target_mixers.get(entity).is_ok() {
            priority += 2;
        }
    } else if work_type == WorkType::GatherWater {
        priority += 5;

        let has_tank_space = if let Ok(req) = queries.transport_requests.get(entity) {
            if req.kind == TransportRequestKind::GatherWaterToTank {
                if let Ok((_, _, stock, stored)) = queries.storage.stockpiles.get(req.anchor) {
                    if stock.resource_type != Some(ResourceType::Water) {
                        false
                    } else {
                        let current_count = stored.map(|s| s.len()).unwrap_or(0);
                        let incoming = queries.reservation.incoming_deliveries_query.get(req.anchor).ok()
                            .map(|inc| inc.len()).unwrap_or(0);
                        (current_count + incoming) < stock.capacity
                    }
                } else {
                    false
                }
            } else {
                false
            }
        } else {
            let bucket_belongs = queries.designation.belongs.get(entity).ok();
            queries
                .storage
                .stockpiles
                .iter()
                .any(|(s_entity, _, stock, stored)| {
                    let is_tank = stock.resource_type == Some(ResourceType::Water);
                    let is_my_tank = bucket_belongs.map(|b| b.0) == Some(s_entity);
                    if is_tank && is_my_tank {
                        let current_count = stored.map(|s| s.len()).unwrap_or(0);
                        let incoming = queries.reservation.incoming_deliveries_query.get(s_entity).ok()
                            .map(|inc: &crate::relationships::IncomingDeliveries| inc.len())
                            .unwrap_or(0);
                        (current_count + incoming) < stock.capacity
                    } else {
                        false
                    }
                })
        };

        if !has_tank_space {
            return None;
        }

        if in_stockpile_none {
            priority += 2;
        }
    }

    Some(priority)
}
