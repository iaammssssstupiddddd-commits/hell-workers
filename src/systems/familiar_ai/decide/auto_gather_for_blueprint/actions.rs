use std::collections::{HashMap, HashSet};

use bevy::prelude::*;

use crate::constants::{
    BLUEPRINT_AUTO_GATHER_PATH_CHECK_LIMIT_PER_STAGE, BLUEPRINT_AUTO_GATHER_PRIORITY,
};
use crate::relationships::ManagedBy;
use crate::systems::jobs::{Designation, Priority, TaskSlots};
use crate::systems::logistics::ResourceType;
use crate::world::map::WorldMap;
use crate::world::pathfinding::PathfindingContext;

use super::AutoGatherDesignation;
use super::helpers::{
    OwnerInfo, STAGE_COUNT, SourceCandidate, SupplyBucket, compare_auto_idle_for_cleanup,
    is_reachable, resource_rank, work_type_for_resource,
};

fn assign_auto_gather_designation(
    commands: &mut Commands,
    source: Entity,
    owner: Entity,
    resource_type: ResourceType,
    work_type: crate::systems::jobs::WorkType,
) {
    commands.entity(source).insert((
        Designation { work_type },
        ManagedBy(owner),
        TaskSlots::new(1),
        Priority(BLUEPRINT_AUTO_GATHER_PRIORITY),
        AutoGatherDesignation {
            owner,
            resource_type,
        },
    ));
}

fn clear_auto_gather_designation(commands: &mut Commands, source: Entity) {
    commands.entity(source).remove::<(
        Designation,
        TaskSlots,
        Priority,
        ManagedBy,
        AutoGatherDesignation,
    )>();
}

pub(super) fn assign_needed_auto_designations(
    commands: &mut Commands,
    needed_new_auto_count: &HashMap<(Entity, ResourceType), u32>,
    owner_infos: &HashMap<Entity, OwnerInfo>,
    candidate_sources: &HashMap<(Entity, ResourceType, usize), Vec<SourceCandidate>>,
    world_map: &WorldMap,
    pf_context: &mut PathfindingContext,
) {
    let mut needed_keys: Vec<(Entity, ResourceType)> = needed_new_auto_count
        .iter()
        .filter_map(|(key, count)| if *count > 0 { Some(*key) } else { None })
        .collect();
    needed_keys.sort_by(|(owner_a, res_a), (owner_b, res_b)| {
        owner_a
            .to_bits()
            .cmp(&owner_b.to_bits())
            .then(resource_rank(*res_a).cmp(&resource_rank(*res_b)))
    });

    for (owner, resource_type) in needed_keys {
        let Some(owner_info) = owner_infos.get(&owner) else {
            continue;
        };
        let mut remaining = needed_new_auto_count
            .get(&(owner, resource_type))
            .copied()
            .unwrap_or(0) as usize;
        if remaining == 0 {
            continue;
        }

        let work_type = work_type_for_resource(resource_type);
        for stage in 0..STAGE_COUNT {
            if remaining == 0 {
                break;
            }

            let Some(candidates) = candidate_sources.get(&(owner, resource_type, stage)) else {
                continue;
            };

            let mut path_checks = 0usize;
            for candidate in candidates {
                if remaining == 0 {
                    break;
                }
                if path_checks >= BLUEPRINT_AUTO_GATHER_PATH_CHECK_LIMIT_PER_STAGE {
                    break;
                }
                path_checks += 1;

                if !is_reachable(owner_info.path_start, candidate.pos, world_map, pf_context) {
                    continue;
                }

                assign_auto_gather_designation(
                    commands,
                    candidate.entity,
                    owner,
                    resource_type,
                    work_type,
                );
                remaining -= 1;
            }
        }
    }
}

pub(super) fn cleanup_auto_gather_markers(
    commands: &mut Commands,
    stale_marker_only: HashSet<Entity>,
    invalid_auto_idle: HashSet<Entity>,
    supply_by_owner: &mut HashMap<(Entity, ResourceType), SupplyBucket>,
    target_auto_idle_count: &HashMap<(Entity, ResourceType), u32>,
) {
    for entity in stale_marker_only {
        commands.entity(entity).remove::<AutoGatherDesignation>();
    }

    for entity in invalid_auto_idle {
        clear_auto_gather_designation(commands, entity);
    }

    for (key, stats) in supply_by_owner {
        let current_idle = stats.auto_idle.len() as u32;
        if current_idle == 0 {
            continue;
        }

        let target_idle = target_auto_idle_count.get(key).copied().unwrap_or(0);
        if current_idle <= target_idle {
            continue;
        }

        let remove_count = (current_idle - target_idle) as usize;
        stats.auto_idle.sort_by(compare_auto_idle_for_cleanup);
        for entry in stats.auto_idle.iter().take(remove_count) {
            clear_auto_gather_designation(commands, entry.entity);
        }
    }
}
