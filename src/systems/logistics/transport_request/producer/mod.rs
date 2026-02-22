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
use bevy::prelude::{Commands, Entity, Query, Transform, Visibility};
use std::collections::HashMap;

use crate::systems::logistics::{ResourceItem, ResourceType};
use crate::systems::spatial::{ResourceSpatialGrid, SpatialGridOps};

pub(crate) fn to_u32_saturating(value: usize) -> u32 {
    u32::try_from(value).unwrap_or(u32::MAX)
}

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

pub(crate) fn group_tiles_by_site<T: bevy::prelude::Component>(
    q_tiles: &Query<(Entity, &T)>,
    mut parent_site_of: impl FnMut(&T) -> Entity,
    tiles_scanned: &mut u32,
) -> HashMap<Entity, Vec<Entity>> {
    let mut tiles_by_site = HashMap::<Entity, Vec<Entity>>::new();
    for (tile_entity, tile) in q_tiles.iter() {
        *tiles_scanned = tiles_scanned.saturating_add(1);
        tiles_by_site
            .entry(parent_site_of(tile))
            .or_default()
            .push(tile_entity);
    }
    tiles_by_site
}

pub(crate) fn consume_waiting_tile_resources<
    T: bevy::prelude::Component<Mutability = bevy::ecs::component::Mutable>,
>(
    commands: &mut Commands,
    site_tiles: &[Entity],
    q_tiles: &mut Query<&mut T>,
    nearby_resources: &mut Vec<Entity>,
    required_amount: u32,
    mut is_waiting: impl FnMut(&T) -> bool,
    mut delivered_mut: impl FnMut(&mut T) -> &mut u32,
    mut mark_ready: impl FnMut(&mut T),
) -> u32 {
    let mut consumed = 0u32;
    for tile_entity in site_tiles.iter().copied() {
        let Ok(mut tile) = q_tiles.get_mut(tile_entity) else {
            continue;
        };
        if !is_waiting(&tile) {
            continue;
        }

        let reached_required = {
            let delivered = delivered_mut(&mut tile);
            while *delivered < required_amount {
                let Some(resource_entity) = nearby_resources.pop() else {
                    break;
                };
                commands.entity(resource_entity).try_despawn();
                *delivered += 1;
                consumed += 1;
            }
            *delivered >= required_amount
        };

        if reached_required {
            mark_ready(&mut tile);
        }
        if nearby_resources.is_empty() {
            break;
        }
    }
    consumed
}
