use std::collections::{HashMap, HashSet};

use bevy::prelude::*;

use crate::systems::logistics::ResourceType;

use super::helpers::{SupplyBucket, div_ceil_u32, drop_amount_for_resource};

pub(super) struct AutoGatherPlan {
    pub(super) target_auto_idle_count: HashMap<(Entity, ResourceType), u32>,
    pub(super) needed_new_auto_count: HashMap<(Entity, ResourceType), u32>,
}

pub(super) fn build_auto_gather_targets(
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

        let required_auto_yield =
            demand.saturating_sub(stats.ground_items.saturating_add(stats.pending_non_auto_yield));
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
