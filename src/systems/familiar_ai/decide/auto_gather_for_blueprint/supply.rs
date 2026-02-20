use std::collections::{HashMap, HashSet};

use bevy::prelude::*;

use crate::relationships::{LoadedIn, ManagedBy, StoredIn, TaskWorkers};
use crate::systems::command::TaskArea;
use crate::systems::jobs::{Designation, Rock, Tree};
use crate::systems::logistics::{ReservedForTask, ResourceItem, ResourceType};

use super::AutoGatherForBlueprint;
use super::helpers::{
    AutoIdleEntry, OwnerInfo, STAGE_COUNT, SourceCandidate, SupplyBucket,
    compare_source_candidates, drop_amount_for_resource, resolve_owner,
    source_resource_from_components, stage_for_pos, work_type_for_resource,
};

pub(super) struct SupplyState {
    pub(super) supply_by_owner: HashMap<(Entity, ResourceType), SupplyBucket>,
    pub(super) candidate_sources: HashMap<(Entity, ResourceType, usize), Vec<SourceCandidate>>,
    pub(super) stale_marker_only: HashSet<Entity>,
    pub(super) invalid_auto_idle: HashSet<Entity>,
}

pub(super) fn collect_supply_state(
    owner_infos: &HashMap<Entity, OwnerInfo>,
    owner_areas: &[(Entity, TaskArea)],
    q_ground_items: &Query<
        (&Transform, &Visibility, &ResourceItem),
        (
            Without<Designation>,
            Without<TaskWorkers>,
            Without<ReservedForTask>,
            Without<StoredIn>,
            Without<LoadedIn>,
        ),
    >,
    q_sources: &Query<
        (
            Entity,
            &Transform,
            Option<&Tree>,
            Option<&Rock>,
            Option<&Designation>,
            Option<&TaskWorkers>,
            Option<&ManagedBy>,
            Option<&AutoGatherForBlueprint>,
        ),
        Or<(With<Tree>, With<Rock>, With<AutoGatherForBlueprint>)>,
    >,
) -> SupplyState {
    let mut supply_by_owner = HashMap::<(Entity, ResourceType), SupplyBucket>::new();
    let mut candidate_sources =
        HashMap::<(Entity, ResourceType, usize), Vec<SourceCandidate>>::new();
    let mut stale_marker_only = HashSet::<Entity>::new();
    let mut invalid_auto_idle = HashSet::<Entity>::new();

    for (transform, visibility, item) in q_ground_items.iter() {
        if *visibility == Visibility::Hidden {
            continue;
        }
        if !matches!(item.0, ResourceType::Wood | ResourceType::Rock) {
            continue;
        }

        let pos = transform.translation.truncate();
        let Some(owner) = resolve_owner(pos, owner_areas) else {
            continue;
        };
        let bucket = supply_by_owner.entry((owner, item.0)).or_default();
        bucket.ground_items = bucket.ground_items.saturating_add(1);
    }

    for (
        entity,
        transform,
        tree_opt,
        rock_opt,
        designation_opt,
        workers_opt,
        managed_by_opt,
        auto_opt,
    ) in q_sources.iter()
    {
        let pos = transform.translation.truncate();
        let workers = workers_opt.map(|workers| workers.len()).unwrap_or(0);

        let source_resource =
            source_resource_from_components(tree_opt.is_some(), rock_opt.is_some());
        let Some(resource_type) = source_resource else {
            if auto_opt.is_some() {
                if designation_opt.is_none() {
                    stale_marker_only.insert(entity);
                } else if workers == 0 {
                    invalid_auto_idle.insert(entity);
                }
            }
            continue;
        };

        let expected_work_type = work_type_for_resource(resource_type);
        let drop_amount = drop_amount_for_resource(resource_type);

        if let Some(designation) = designation_opt {
            if designation.work_type != expected_work_type {
                if auto_opt.is_some() && workers == 0 {
                    invalid_auto_idle.insert(entity);
                }
                continue;
            }

            let owner = if let Some(marker) = auto_opt {
                marker.owner
            } else if let Some(managed_by) =
                managed_by_opt.filter(|m| owner_infos.contains_key(&m.0))
            {
                managed_by.0
            } else {
                let Some(resolved_owner) = resolve_owner(pos, owner_areas) else {
                    continue;
                };
                resolved_owner
            };

            let bucket = supply_by_owner.entry((owner, resource_type)).or_default();
            if auto_opt.is_some() {
                if workers > 0 {
                    bucket.auto_active_count = bucket.auto_active_count.saturating_add(1);
                } else {
                    let (stage, sort_dist_sq) = if let Some(owner_info) = owner_infos.get(&owner) {
                        (
                            stage_for_pos(pos, &owner_info.area),
                            pos.distance_squared(owner_info.center),
                        )
                    } else {
                        (STAGE_COUNT - 1, f32::INFINITY)
                    };
                    bucket.auto_idle.push(AutoIdleEntry {
                        entity,
                        stage,
                        sort_dist_sq,
                        entity_bits: entity.to_bits(),
                    });
                }
            } else {
                bucket.pending_non_auto_yield =
                    bucket.pending_non_auto_yield.saturating_add(drop_amount);
            }
            continue;
        }

        if auto_opt.is_some() {
            stale_marker_only.insert(entity);
            continue;
        }

        if workers > 0 {
            continue;
        }

        let Some(owner) = resolve_owner(pos, owner_areas) else {
            continue;
        };
        let Some(owner_info) = owner_infos.get(&owner) else {
            continue;
        };

        let stage = stage_for_pos(pos, &owner_info.area);
        let sort_dist_sq = pos.distance_squared(owner_info.center);

        candidate_sources
            .entry((owner, resource_type, stage))
            .or_default()
            .push(SourceCandidate {
                entity,
                pos,
                sort_dist_sq,
                entity_bits: entity.to_bits(),
            });
    }

    for candidates in candidate_sources.values_mut() {
        candidates.sort_by(compare_source_candidates);
    }

    SupplyState {
        supply_by_owner,
        candidate_sources,
        stale_marker_only,
        invalid_auto_idle,
    }
}
