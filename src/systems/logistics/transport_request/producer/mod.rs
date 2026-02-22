pub mod blueprint;
pub mod bucket;
pub mod consolidation;
pub mod floor_construction;
pub mod mixer;
pub mod provisional_wall;
pub mod stockpile_group;
pub mod tank_water_request;
pub mod task_area;
pub mod upsert;
pub mod wall_construction;
pub mod wheelbarrow;

use crate::systems::command::TaskArea;
use bevy::math::Vec2;
use bevy::prelude::{Entity, Query, Transform, Visibility};

use crate::systems::logistics::{ResourceItem, ResourceType};
use crate::systems::spatial::{ResourceSpatialGrid, SpatialGridOps};

pub(crate) fn find_owner_familiar(
    pos: Vec2,
    familiars: &[(Entity, TaskArea)],
) -> Option<(Entity, &TaskArea)> {
    familiars
        .iter()
        .filter(|(_, area)| area.contains(pos))
        .min_by(|(_, area1), (_, area2)| {
            let d1 = area1.center().distance_squared(pos);
            let d2 = area2.center().distance_squared(pos);
            d1.partial_cmp(&d2).unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|(entity, area)| (*entity, area))
}

pub(crate) fn collect_nearby_resource_entities(
    center: Vec2,
    pickup_radius: f32,
    pickup_radius_sq: f32,
    target_resource: ResourceType,
    resource_grid: &ResourceSpatialGrid,
    q_resources: &Query<(
        Entity,
        &Transform,
        &Visibility,
        &ResourceItem,
        Option<&crate::relationships::StoredIn>,
    )>,
    resources_scanned: &mut u32,
) -> Vec<Entity> {
    let mut nearby_resources = Vec::new();
    for entity in resource_grid.get_nearby_in_radius(center, pickup_radius) {
        let Ok((_, transform, visibility, resource_item, stored_in_opt)) = q_resources.get(entity)
        else {
            continue;
        };
        *resources_scanned = resources_scanned.saturating_add(1);
        if *visibility != Visibility::Hidden
            && stored_in_opt.is_none()
            && resource_item.0 == target_resource
            && transform.translation.truncate().distance_squared(center) <= pickup_radius_sq
        {
            nearby_resources.push(entity);
        }
    }
    nearby_resources
}
