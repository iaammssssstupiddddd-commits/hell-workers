//! 手押し車による一括運搬のポリシーロジック

use crate::constants::*;
use crate::systems::familiar_ai::decide::task_management::{ReservationShadow, validator};
use crate::systems::jobs::WorkType;
use bevy::prelude::*;

pub use validator::{find_nearest_wheelbarrow, remaining_stockpile_capacity};

use validator::source_not_reserved;

/// 指定アイテムの近くにある、手押し車に積載可能な未予約 Haul アイテムを収集
pub fn collect_nearby_haulable_items(
    primary_item: Entity,
    task_pos: Vec2,
    queries: &crate::systems::soul_ai::execute::task_execution::context::TaskAssignmentQueries,
    shadow: &ReservationShadow,
) -> Vec<Entity> {
    let search_radius_sq = (TILE_SIZE * 10.0) * (TILE_SIZE * 10.0);

    let mut items: Vec<(Entity, f32)> = queries
        .designation
        .designations
        .iter()
        .filter_map(
            |(entity, transform, designation, _, _, task_workers, _, _)| {
                if designation.work_type != WorkType::Haul {
                    return None;
                }
                if task_workers.is_some_and(|tw| !tw.is_empty()) {
                    return None;
                }
                if !source_not_reserved(entity, queries, shadow) {
                    return None;
                }
                let item_type = queries.items.get(entity).ok().map(|(it, _)| it.0)?;
                if !item_type.is_loadable() {
                    return None;
                }
                let pos = transform.translation.truncate();
                let dist_sq = pos.distance_squared(task_pos);
                if dist_sq > search_radius_sq {
                    return None;
                }
                Some((entity, dist_sq))
            },
        )
        .collect();

    items.sort_by(|(_, d1), (_, d2)| d1.partial_cmp(d2).unwrap_or(std::cmp::Ordering::Equal));

    let mut result: Vec<Entity> = Vec::new();
    result.push(primary_item);
    for (entity, _) in items {
        if entity == primary_item {
            continue;
        }
        result.push(entity);
    }

    result
}
