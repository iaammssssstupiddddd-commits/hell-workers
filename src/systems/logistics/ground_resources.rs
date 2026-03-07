//! 地面資材数カウント helper.

use bevy::prelude::*;

use crate::relationships::{LoadedIn, StoredIn};
use crate::systems::logistics::ResourceItem;
use crate::systems::logistics::ResourceType;

pub fn count_nearby_ground_resources<'a>(
    resource_items: impl Iterator<
        Item = (
            Entity,
            &'a Transform,
            &'a Visibility,
            &'a ResourceItem,
            Option<&'a StoredIn>,
            Option<&'a LoadedIn>,
        ),
    >,
    center: Vec2,
    radius_sq: f32,
    resource_type: ResourceType,
    exclude_item: Option<Entity>,
) -> usize {
    let excluded = exclude_item.unwrap_or(Entity::PLACEHOLDER);
    resource_items
        .filter(|(entity, transform, visibility, resource_item, stored_in_opt, loaded_in_opt)| {
            *entity != excluded
                && **visibility != Visibility::Hidden
                && stored_in_opt.is_none()
                && loaded_in_opt.is_none()
                && resource_item.0 == resource_type
                && transform.translation.truncate().distance_squared(center) <= radius_sq
        })
        .count()
}
