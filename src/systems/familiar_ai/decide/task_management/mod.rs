//! 使い魔のタスク管理モジュール
//!
//! タスクの検索・割り当てロジックを提供します。

mod builders;
mod delegation;
mod policy;
mod task_assigner;
mod task_finder;
mod validator;

use crate::systems::logistics::ResourceType;
use bevy::prelude::*;
use std::collections::HashMap;

pub use delegation::TaskManager;
pub(crate) use delegation::take_reachable_with_cache_calls;
pub(crate) use policy::take_source_selector_scan_snapshot;
pub use task_assigner::AssignTaskContext;
pub use task_assigner::ReservationShadow;
pub type FamiliarTaskAssignmentQueries<'w, 's> =
    crate::systems::soul_ai::execute::task_execution::context::TaskAssignmentQueries<'w, 's>;
pub use task_assigner::assign_task_to_worker;
pub(crate) use task_assigner::{CachedSourceItem, SourceSelectorFrameCache};
pub use task_finder::DelegationCandidate;
pub use task_finder::ScoredDelegationCandidate;
pub use task_finder::collect_scored_candidates;

#[derive(Default)]
pub struct IncomingDeliverySnapshot {
    by_destination: HashMap<Entity, HashMap<ResourceType, u32>>,
}

impl IncomingDeliverySnapshot {
    pub fn build<'w, 's>(
        queries: &crate::systems::familiar_ai::decide::task_management::FamiliarTaskAssignmentQueries<
            'w, 's,
        >,
    ) -> Self {
        let mut snapshot = Self {
            by_destination: HashMap::new(),
        };

        for (destination, incoming_deliveries) in
            queries.reservation.incoming_deliveries_query.iter()
        {
            let destination_map = snapshot.by_destination.entry(destination).or_default();
            for item in incoming_deliveries.iter() {
                let Ok(resource_item) = queries.reservation.resources.get(*item) else {
                    continue;
                };
                *destination_map.entry(resource_item.0).or_insert(0) += 1;
            }
        }

        snapshot
    }

    pub fn count_exact(&self, target: Entity, resource_type: ResourceType) -> u32 {
        self.by_destination
            .get(&target)
            .and_then(|map| map.get(&resource_type).copied())
            .unwrap_or(0)
    }

    pub fn count_total(&self, target: Entity) -> u32 {
        self.by_destination
            .get(&target)
            .map(|counts| counts.values().copied().sum())
            .unwrap_or(0)
    }

    pub fn iter_counts(
        &self,
        target: Entity,
    ) -> impl Iterator<Item = (ResourceType, u32)> + '_ {
        self.by_destination
            .get(&target)
            .into_iter()
            .flat_map(|counts| counts.iter().map(|(&resource_type, &count)| (resource_type, count)))
    }
}
