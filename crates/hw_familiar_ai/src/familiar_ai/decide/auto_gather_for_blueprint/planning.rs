use std::collections::{HashMap, HashSet};

use bevy::prelude::*;
use hw_core::constants::BLUEPRINT_AUTO_GATHER_PATH_CHECK_LIMIT_PER_STAGE;
use hw_core::logistics::ResourceType;

use super::demand::AutoGatherDemand;
use super::helpers::{
    OwnerInfo, STAGE_COUNT, SourceCandidate, SupplyBucket, div_ceil_u32, drop_amount_for_resource,
    is_reachable,
};

pub struct AutoGatherPlan {
    pub target_auto_idle_count: HashMap<(Entity, ResourceType), u32>,
    pub needed_new_auto_count: HashMap<(Entity, ResourceType), u32>,
}

pub fn resolve_raw_demand_by_owner(
    demand: AutoGatherDemand,
    supply_by_owner: &HashMap<(Entity, ResourceType), SupplyBucket>,
    candidate_sources: &HashMap<(Entity, ResourceType, usize), Vec<SourceCandidate>>,
    owner_infos: &HashMap<Entity, OwnerInfo>,
    world_map: &hw_world::WorldMap,
    connectivity_cache: &mut hw_world::WalkabilityConnectivityCache,
) -> HashMap<(Entity, ResourceType), u32> {
    let fixed_by_owner = demand.fixed_by_owner;
    let mut resolved = fixed_by_owner.clone();
    let mut keys = HashSet::new();
    keys.extend(fixed_by_owner.keys().copied());
    for flexible in &demand.flexible {
        keys.extend(
            flexible
                .accepted_types
                .iter()
                .map(|resource_type| (flexible.owner, *resource_type)),
        );
    }

    let mut surplus_supply_by_key = HashMap::new();
    let mut candidate_yield_by_key = HashMap::new();
    for key in keys {
        let drop_amount = drop_amount_for_resource(key.1);
        if drop_amount == 0 {
            continue;
        }

        let total_supply = supply_by_owner
            .get(&key)
            .map(|stats| {
                stats
                    .ground_items
                    .saturating_add(stats.pending_non_auto_yield)
                    .saturating_add(stats.auto_active_count.saturating_mul(drop_amount))
                    .saturating_add(
                        u32::try_from(stats.auto_idle.len())
                            .unwrap_or(u32::MAX)
                            .saturating_mul(drop_amount),
                    )
            })
            .unwrap_or(0);
        let fixed = fixed_by_owner.get(&key).copied().unwrap_or(0);
        let existing_surplus = total_supply.saturating_sub(fixed);

        let mut candidate_count = 0u32;
        if let Some(owner_info) = owner_infos.get(&key.0) {
            for stage in 0..STAGE_COUNT {
                let Some(candidates) = candidate_sources.get(&(key.0, key.1, stage)) else {
                    continue;
                };
                for candidate in candidates
                    .iter()
                    .take(BLUEPRINT_AUTO_GATHER_PATH_CHECK_LIMIT_PER_STAGE)
                {
                    if is_reachable(
                        owner_info.path_start,
                        candidate.pos,
                        world_map,
                        connectivity_cache,
                    ) {
                        candidate_count = candidate_count.saturating_add(1);
                    }
                }
            }
        }
        let fixed_uncovered = fixed.saturating_sub(total_supply);
        let fixed_source_count = div_ceil_u32(fixed_uncovered, drop_amount);
        let reserved_source_count = fixed_source_count.min(candidate_count);
        let reserved_yield = reserved_source_count.saturating_mul(drop_amount);
        let prospective_surplus = reserved_yield.saturating_sub(fixed_uncovered);
        surplus_supply_by_key.insert(key, existing_surplus.saturating_add(prospective_surplus));
        candidate_yield_by_key.insert(
            key,
            candidate_count
                .saturating_sub(reserved_source_count)
                .saturating_mul(drop_amount),
        );
    }

    for flexible in demand.flexible {
        let mut remaining = flexible.amount;

        for &resource_type in &flexible.accepted_types {
            if remaining == 0 {
                break;
            }
            let key = (flexible.owner, resource_type);
            let available = surplus_supply_by_key.get(&key).copied().unwrap_or(0);
            let allocated = remaining.min(available);
            add_demand(&mut resolved, key, allocated);
            surplus_supply_by_key.insert(key, available.saturating_sub(allocated));
            remaining = remaining.saturating_sub(allocated);
        }

        if remaining == 0 {
            continue;
        }

        let covering_resource = flexible
            .accepted_types
            .iter()
            .copied()
            .enumerate()
            .filter(|resource_type| {
                candidate_yield_by_key
                    .get(&(flexible.owner, resource_type.1))
                    .copied()
                    .unwrap_or(0)
                    >= remaining
            })
            .map(|(accepted_index, resource_type)| {
                (
                    div_ceil_u32(remaining, drop_amount_for_resource(resource_type)),
                    accepted_index,
                    resource_type,
                )
            })
            .min_by_key(|(source_count, accepted_index, _)| (*source_count, *accepted_index))
            .map(|(_, _, resource_type)| resource_type);
        if let Some(resource_type) = covering_resource {
            let key = (flexible.owner, resource_type);
            let surplus = consume_candidate_yield(
                &mut candidate_yield_by_key,
                key,
                remaining,
                drop_amount_for_resource(resource_type),
            );
            add_demand(&mut resolved, key, remaining);
            add_surplus(&mut surplus_supply_by_key, key, surplus);
            continue;
        }

        for &resource_type in &flexible.accepted_types {
            if remaining == 0 {
                break;
            }
            let key = (flexible.owner, resource_type);
            let available = candidate_yield_by_key.get(&key).copied().unwrap_or(0);
            let allocated = remaining.min(available);
            let surplus = consume_candidate_yield(
                &mut candidate_yield_by_key,
                key,
                allocated,
                drop_amount_for_resource(resource_type),
            );
            add_demand(&mut resolved, key, allocated);
            add_surplus(&mut surplus_supply_by_key, key, surplus);
            remaining = remaining.saturating_sub(allocated);
        }

        if remaining > 0
            && let Some(&resource_type) = flexible.accepted_types.first()
        {
            add_demand(&mut resolved, (flexible.owner, resource_type), remaining);
        }
    }

    resolved
}

fn add_demand(
    demand_by_owner: &mut HashMap<(Entity, ResourceType), u32>,
    key: (Entity, ResourceType),
    amount: u32,
) {
    if amount > 0 {
        let entry = demand_by_owner.entry(key).or_insert(0);
        *entry = entry.saturating_add(amount);
    }
}

fn consume_candidate_yield(
    candidate_yield_by_key: &mut HashMap<(Entity, ResourceType), u32>,
    key: (Entity, ResourceType),
    amount: u32,
    drop_amount: u32,
) -> u32 {
    if amount == 0 || drop_amount == 0 {
        return 0;
    }
    let consumed_yield = div_ceil_u32(amount, drop_amount).saturating_mul(drop_amount);
    let available = candidate_yield_by_key.get(&key).copied().unwrap_or(0);
    candidate_yield_by_key.insert(key, available.saturating_sub(consumed_yield));
    consumed_yield.saturating_sub(amount)
}

fn add_surplus(
    surplus_supply_by_key: &mut HashMap<(Entity, ResourceType), u32>,
    key: (Entity, ResourceType),
    amount: u32,
) {
    if amount > 0 {
        let entry = surplus_supply_by_key.entry(key).or_insert(0);
        *entry = entry.saturating_add(amount);
    }
}

pub fn build_auto_gather_targets(
    raw_demand_by_owner: &HashMap<(Entity, ResourceType), u32>,
    supply_by_owner: &HashMap<(Entity, ResourceType), SupplyBucket>,
) -> AutoGatherPlan {
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

    AutoGatherPlan {
        target_auto_idle_count,
        needed_new_auto_count,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hw_core::area::AreaBounds;
    use hw_world::WorldMap;

    fn owner_info() -> OwnerInfo {
        OwnerInfo {
            area: AreaBounds::new(Vec2::ZERO, WorldMap::grid_to_world(30, 30)),
            center: WorldMap::grid_to_world(15, 15),
            path_start: (15, 15),
            yard: None,
        }
    }

    fn candidate(entity_bits: u64, grid: (i32, i32)) -> SourceCandidate {
        let pos = WorldMap::grid_to_world(grid.0, grid.1);
        SourceCandidate {
            entity: Entity::from_bits(entity_bits),
            pos,
            sort_dist_sq: pos.distance_squared(WorldMap::grid_to_world(15, 15)),
            entity_bits,
        }
    }

    #[test]
    fn fixed_drop_surplus_satisfies_flexible_demand() {
        let owner = Entity::from_bits(1);
        let demand = AutoGatherDemand {
            fixed_by_owner: HashMap::from([((owner, ResourceType::Rock), 1)]),
            flexible: vec![super::super::demand::FlexibleAutoGatherDemand {
                owner,
                accepted_types: vec![ResourceType::Wood, ResourceType::Rock],
                amount: 6,
            }],
        };
        let candidate_sources =
            HashMap::from([((owner, ResourceType::Rock, 0), vec![candidate(2, (20, 20))])]);
        let owner_infos = HashMap::from([(owner, owner_info())]);
        let world_map = WorldMap::default();
        let mut connectivity_cache = hw_world::WalkabilityConnectivityCache::default();

        let resolved = resolve_raw_demand_by_owner(
            demand,
            &HashMap::new(),
            &candidate_sources,
            &owner_infos,
            &world_map,
            &mut connectivity_cache,
        );

        assert_eq!(resolved.get(&(owner, ResourceType::Rock)), Some(&7));
        assert!(!resolved.contains_key(&(owner, ResourceType::Wood)));
    }

    #[test]
    fn flexible_demands_share_candidate_drop_surplus() {
        let owner = Entity::from_bits(1);
        let flexible = || super::super::demand::FlexibleAutoGatherDemand {
            owner,
            accepted_types: vec![ResourceType::Wood, ResourceType::Rock],
            amount: 6,
        };
        let demand = AutoGatherDemand {
            fixed_by_owner: HashMap::new(),
            flexible: vec![flexible(), flexible()],
        };
        let candidate_sources = HashMap::from([
            (
                (owner, ResourceType::Wood, 0),
                vec![candidate(2, (20, 20)), candidate(3, (21, 20))],
            ),
            ((owner, ResourceType::Rock, 0), vec![candidate(4, (22, 20))]),
        ]);
        let owner_infos = HashMap::from([(owner, owner_info())]);
        let world_map = WorldMap::default();
        let mut connectivity_cache = hw_world::WalkabilityConnectivityCache::default();

        let resolved = resolve_raw_demand_by_owner(
            demand,
            &HashMap::new(),
            &candidate_sources,
            &owner_infos,
            &world_map,
            &mut connectivity_cache,
        );
        let plan = build_auto_gather_targets(&resolved, &HashMap::new());

        assert_eq!(resolved.get(&(owner, ResourceType::Rock)), Some(&10));
        assert_eq!(resolved.get(&(owner, ResourceType::Wood)), Some(&2));
        assert_eq!(
            plan.needed_new_auto_count.get(&(owner, ResourceType::Rock)),
            Some(&1)
        );
        assert_eq!(
            plan.needed_new_auto_count.get(&(owner, ResourceType::Wood)),
            Some(&1)
        );
    }
}
