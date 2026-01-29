//! Task area auto-haul system
//!
//! Automatically creates haul tasks for resources within the task area.

use bevy::prelude::*;

use crate::constants::*;
use crate::entities::familiar::ActiveCommand;
use crate::systems::command::TaskArea;
use crate::systems::jobs::{Designation, IssuedBy, TaskSlots, WorkType};
use crate::systems::logistics::ResourceItem;
use crate::systems::spatial::{ResourceSpatialGrid, SpatialGridOps};
use crate::relationships::TaskWorkers;

/// 指揮エリア内での自動運搬タスク生成システム
pub fn task_area_auto_haul_system(
    mut commands: Commands,
    resource_grid: Res<ResourceSpatialGrid>,
    q_familiars: Query<(Entity, &ActiveCommand, &TaskArea)>,
    q_stockpiles: Query<(
        &Transform,
        &crate::systems::logistics::Stockpile,
        Option<&crate::relationships::StoredItems>,
    )>,
    q_resources: Query<
        (Entity, &Transform, &Visibility, &ResourceItem),
        (
            Without<crate::relationships::StoredIn>,
            Without<Designation>,
            Without<TaskWorkers>,
            Without<crate::systems::jobs::TargetBlueprint>,
        ),
    >,
) {
    let mut already_assigned = std::collections::HashSet::new();

    for (fam_entity, _active_command, task_area) in q_familiars.iter() {
        let (fam_entity, _active_command, task_area): (Entity, &ActiveCommand, &TaskArea) = (fam_entity, _active_command, task_area);
        for (stock_transform, stockpile, stored_items_opt) in q_stockpiles.iter() {
            let (stock_transform, stockpile, stored_items_opt): (&Transform, &crate::systems::logistics::Stockpile, Option<&crate::relationships::StoredItems>) = (stock_transform, stockpile, stored_items_opt);
            let stock_pos = stock_transform.translation.truncate();
            if !task_area.contains(stock_pos) {
                continue;
            }

            let current_count = stored_items_opt.map(|si| si.len()).unwrap_or(0);
            if current_count >= stockpile.capacity {
                continue;
            }

            let search_radius = TILE_SIZE * 15.0;
            let nearby_resources = resource_grid.get_nearby_in_radius(stock_pos, search_radius);

            let nearest_resource = nearby_resources
                .iter()
                .filter(|&&entity| !already_assigned.contains(&entity))
                .filter_map(|&entity| {
                    let Ok((_, transform, visibility, res_item)) = q_resources.get(entity) else {
                        return None;
                    };
                    if *visibility == Visibility::Hidden {
                        return None;
                    }

                    if let Some(target_type) = stockpile.resource_type {
                        if res_item.0 != target_type {
                            return None;
                        }
                    }

                    let dist_sq = transform.translation.truncate().distance_squared(stock_pos);
                    if dist_sq < search_radius * search_radius {
                        Some((entity, dist_sq))
                    } else {
                        None
                    }
                })
                .min_by(|(_, d1), (_, d2)| d1.partial_cmp(d2).unwrap_or(std::cmp::Ordering::Equal))
                .map(|(entity, _)| entity);

            if let Some(item_entity) = nearest_resource {
                already_assigned.insert(item_entity);
                commands.entity(item_entity).insert((
                    Designation {
                        work_type: WorkType::Haul,
                    },
                    IssuedBy(fam_entity),
                    TaskSlots::new(1),
                    crate::systems::jobs::Priority(0),
                ));
            }
        }
    }
}
