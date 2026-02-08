use crate::systems::jobs::{TargetBlueprint, WorkType};
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

        let bucket_belongs = queries.designation.belongs.get(entity).ok();
        let has_tank_space =
            queries
                .storage
                .stockpiles
                .iter()
                .any(|(s_entity, _, stock, stored)| {
                    let is_tank = stock.resource_type == Some(ResourceType::Water);
                    let is_my_tank = bucket_belongs.map(|b| b.0) == Some(s_entity);
                    if is_tank && is_my_tank {
                        let current_count = stored.map(|s| s.len()).unwrap_or(0);
                        let reserved = queries
                            .reservation
                            .resource_cache
                            .get_destination_reservation(s_entity);
                        (current_count + reserved) < stock.capacity
                    } else {
                        false
                    }
                });

        if !has_tank_space {
            return None;
        }

        if in_stockpile_none {
            priority += 2;
        }
    }

    Some(priority)
}
