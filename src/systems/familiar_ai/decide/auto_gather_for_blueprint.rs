use crate::constants::{
    BLUEPRINT_AUTO_GATHER_INTERVAL_SECS, BLUEPRINT_AUTO_GATHER_PATH_CHECK_LIMIT_PER_STAGE,
    BLUEPRINT_AUTO_GATHER_PRIORITY,
};
use crate::entities::familiar::{ActiveCommand, FamiliarCommand};
use crate::relationships::{LoadedIn, ManagedBy, StoredIn, TaskWorkers};
use crate::systems::command::TaskArea;
use crate::systems::jobs::{
    Blueprint, Designation, Priority, Rock, TargetBlueprint, TaskSlots, Tree,
};
use crate::systems::logistics::transport_request::{TransportRequest, TransportRequestKind};
use crate::systems::logistics::{ReservedForTask, ResourceItem, ResourceType};
use crate::world::map::WorldMap;
use crate::world::pathfinding::PathfindingContext;
use bevy::prelude::*;
use std::collections::{HashMap, HashSet};

mod helpers;
use self::helpers::{
    AutoIdleEntry, OwnerInfo, STAGE_COUNT, SourceCandidate, SupplyBucket,
    compare_auto_idle_for_cleanup, compare_source_candidates, div_ceil_u32,
    drop_amount_for_resource, is_reachable, resolve_owner, resource_rank,
    source_resource_from_components, stage_for_pos, work_type_for_resource,
};

#[derive(Resource)]
pub struct BlueprintAutoGatherTimer {
    pub timer: Timer,
    pub first_run_done: bool,
}

impl Default for BlueprintAutoGatherTimer {
    fn default() -> Self {
        Self {
            timer: Timer::from_seconds(BLUEPRINT_AUTO_GATHER_INTERVAL_SECS, TimerMode::Repeating),
            first_run_done: false,
        }
    }
}

#[derive(Component, Debug, Clone, Copy)]
pub struct AutoGatherForBlueprint {
    pub owner: Entity,
    pub resource_type: ResourceType,
}

pub fn blueprint_auto_gather_system(
    mut commands: Commands,
    time: Res<Time>,
    mut timer: ResMut<BlueprintAutoGatherTimer>,
    world_map: Res<WorldMap>,
    mut pf_context: Local<PathfindingContext>,
    q_familiars: Query<(Entity, &ActiveCommand, &TaskArea, &Transform)>,
    q_bp_requests: Query<(&TransportRequest, &TargetBlueprint, Option<&TaskWorkers>)>,
    q_blueprints: Query<&Blueprint>,
    q_ground_items: Query<
        (&Transform, &Visibility, &ResourceItem),
        (
            Without<Designation>,
            Without<TaskWorkers>,
            Without<ReservedForTask>,
            Without<StoredIn>,
            Without<LoadedIn>,
        ),
    >,
    q_sources: Query<
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
) {
    let timer_finished = timer.timer.tick(time.delta()).just_finished();
    if timer.first_run_done && !timer_finished {
        return;
    }
    timer.first_run_done = true;

    let mut owner_infos = HashMap::<Entity, OwnerInfo>::new();
    for (fam_entity, active_command, area, transform) in q_familiars.iter() {
        if matches!(active_command.command, FamiliarCommand::Idle) {
            continue;
        }

        let start_grid = world_map
            .get_nearest_walkable_grid(transform.translation.truncate())
            .or_else(|| world_map.get_nearest_walkable_grid(area.center()));
        let Some(path_start) = start_grid else {
            continue;
        };

        owner_infos.insert(
            fam_entity,
            OwnerInfo {
                area: area.clone(),
                center: area.center(),
                path_start,
            },
        );
    }

    let mut owner_areas: Vec<(Entity, TaskArea)> = owner_infos
        .iter()
        .map(|(entity, info)| (*entity, info.area.clone()))
        .collect();
    owner_areas.sort_by_key(|(entity, _)| entity.to_bits());

    let mut raw_demand_by_owner = HashMap::<(Entity, ResourceType), u32>::new();
    let mut demand_by_blueprint = HashMap::<(Entity, Entity, ResourceType), u32>::new();

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
        *demand_by_blueprint
            .entry((req.issued_by, target_bp.0, req.resource_type))
            .or_insert(0) += inflight;
    }

    for ((owner, blueprint_entity, resource_type), inflight) in demand_by_blueprint {
        let Ok(blueprint) = q_blueprints.get(blueprint_entity) else {
            continue;
        };
        let required = *blueprint
            .required_materials
            .get(&resource_type)
            .unwrap_or(&0);
        if required == 0 {
            continue;
        }
        let delivered = *blueprint
            .delivered_materials
            .get(&resource_type)
            .unwrap_or(&0);
        let needed = required.saturating_sub(delivered.saturating_add(inflight));
        if needed == 0 {
            continue;
        }

        *raw_demand_by_owner
            .entry((owner, resource_type))
            .or_insert(0) += needed;
    }

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
        let Some(owner) = resolve_owner(pos, &owner_areas) else {
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
                let Some(resolved_owner) = resolve_owner(pos, &owner_areas) else {
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

        let Some(owner) = resolve_owner(pos, &owner_areas) else {
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

    let mut all_keys = HashSet::<(Entity, ResourceType)>::new();
    all_keys.extend(raw_demand_by_owner.keys().copied());
    all_keys.extend(supply_by_owner.keys().copied());

    let mut target_auto_idle_count = HashMap::<(Entity, ResourceType), u32>::new();
    let mut needed_new_auto_count = HashMap::<(Entity, ResourceType), u32>::new();

    for key in all_keys {
        let demand = raw_demand_by_owner.get(&key).copied().unwrap_or(0);
        let drop_amount = drop_amount_for_resource(key.1);
        if drop_amount == 0 {
            continue;
        }

        let Some(stats) = supply_by_owner.get(&key) else {
            if demand == 0 {
                target_auto_idle_count.insert(key, 0);
                continue;
            }
            let target_idle = div_ceil_u32(demand, drop_amount);
            target_auto_idle_count.insert(key, target_idle);
            needed_new_auto_count.insert(key, target_idle);
            continue;
        };

        let required_auto_yield = demand.saturating_sub(
            stats
                .ground_items
                .saturating_add(stats.pending_non_auto_yield),
        );
        let auto_active_yield = stats.auto_active_count.saturating_mul(drop_amount);
        let required_idle_or_new_yield = required_auto_yield.saturating_sub(auto_active_yield);

        let target_idle = div_ceil_u32(required_idle_or_new_yield, drop_amount);
        target_auto_idle_count.insert(key, target_idle);

        let current_idle = stats.auto_idle.len() as u32;
        if target_idle > current_idle {
            needed_new_auto_count.insert(key, target_idle - current_idle);
        }
    }

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

                if !is_reachable(
                    owner_info.path_start,
                    candidate.pos,
                    &world_map,
                    &mut pf_context,
                ) {
                    continue;
                }

                commands.entity(candidate.entity).insert((
                    Designation { work_type },
                    ManagedBy(owner),
                    TaskSlots::new(1),
                    Priority(BLUEPRINT_AUTO_GATHER_PRIORITY),
                    AutoGatherForBlueprint {
                        owner,
                        resource_type,
                    },
                ));
                remaining -= 1;
            }
        }
    }

    for entity in stale_marker_only {
        commands.entity(entity).remove::<AutoGatherForBlueprint>();
    }

    for entity in invalid_auto_idle {
        commands.entity(entity).remove::<(
            Designation,
            TaskSlots,
            Priority,
            ManagedBy,
            AutoGatherForBlueprint,
        )>();
    }

    for (key, stats) in &mut supply_by_owner {
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
            commands.entity(entry.entity).remove::<(
                Designation,
                TaskSlots,
                Priority,
                ManagedBy,
                AutoGatherForBlueprint,
            )>();
        }
    }
}
