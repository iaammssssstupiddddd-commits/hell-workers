use std::cmp::Ordering;

use bevy::prelude::*;
use hw_core::area::AreaBounds;
use hw_core::constants::{ROCK_DROP_AMOUNT, WOOD_DROP_AMOUNT};
use hw_core::jobs::WorkType;
use hw_core::logistics::ResourceType;
use hw_world::Yard;

pub const STAGE_COUNT: usize = 3;

#[derive(Clone)]
pub struct OwnerInfo {
    pub area: AreaBounds,
    pub center: Vec2,
    pub path_start: (i32, i32),
    pub yard: Option<Yard>,
}

#[derive(Default)]
pub struct SupplyBucket {
    pub ground_items: u32,
    pub pending_non_auto_yield: u32,
    pub auto_active_count: u32,
    pub auto_idle: Vec<AutoIdleEntry>,
}

#[derive(Clone, Copy)]
pub struct SourceCandidate {
    pub entity: Entity,
    pub pos: Vec2,
    pub sort_dist_sq: f32,
    pub entity_bits: u64,
}

#[derive(Clone, Copy)]
pub struct AutoIdleEntry {
    pub entity: Entity,
    pub stage: usize,
    pub sort_dist_sq: f32,
    pub entity_bits: u64,
}

pub fn source_resource_from_components(has_tree: bool, has_rock: bool) -> Option<ResourceType> {
    if has_tree {
        Some(ResourceType::Wood)
    } else if has_rock {
        Some(ResourceType::Rock)
    } else {
        None
    }
}

pub fn drop_amount_for_resource(resource_type: ResourceType) -> u32 {
    match resource_type {
        ResourceType::Wood => WOOD_DROP_AMOUNT,
        ResourceType::Rock => ROCK_DROP_AMOUNT,
        _ => 0,
    }
}

pub fn work_type_for_resource(resource_type: ResourceType) -> WorkType {
    match resource_type {
        ResourceType::Wood => WorkType::Chop,
        ResourceType::Rock => WorkType::Mine,
        _ => WorkType::Chop,
    }
}

pub fn resource_rank(resource_type: ResourceType) -> u8 {
    match resource_type {
        ResourceType::Wood => 0,
        ResourceType::Rock => 1,
        _ => 255,
    }
}

pub fn stage_for_pos(pos: Vec2, owner: &OwnerInfo) -> usize {
    if owner.area.contains(pos) {
        return 0;
    }
    if let Some(yard) = owner.yard.as_ref()
        && yard.contains(pos)
    {
        return 1;
    }
    2
}

pub fn resolve_owner(
    pos: Vec2,
    owner_infos: &std::collections::HashMap<Entity, OwnerInfo>,
) -> Option<Entity> {
    if owner_infos.is_empty() {
        return None;
    }

    let mut inside_yard = Vec::<(Entity, &OwnerInfo)>::new();
    let mut inside_area = Vec::<(Entity, &OwnerInfo)>::new();

    for (owner, owner_info) in owner_infos {
        if owner_info.area.contains(pos) {
            inside_area.push((*owner, owner_info));
        }
        if let Some(yard) = owner_info.yard.as_ref()
            && yard.contains(pos)
        {
            inside_yard.push((*owner, owner_info));
        }
    }

    if !inside_area.is_empty() {
        return inside_area
            .into_iter()
            .min_by(|(owner_a, info_a), (owner_b, info_b)| {
                distance_sq_to_task_area_perimeter(pos, &info_a.area)
                    .partial_cmp(&distance_sq_to_task_area_perimeter(pos, &info_b.area))
                    .unwrap_or(std::cmp::Ordering::Equal)
                    .then(owner_a.to_bits().cmp(&owner_b.to_bits()))
            })
            .map(|(owner, _)| owner);
    }

    if !inside_yard.is_empty() {
        return inside_yard
            .into_iter()
            .min_by(|(owner_a, info_a), (owner_b, info_b)| {
                let yard_a = info_a.yard.as_ref().expect("inside_yard entries have yard");
                let yard_b = info_b.yard.as_ref().expect("inside_yard entries have yard");
                distance_sq_to_yard_perimeter(pos, yard_a)
                    .partial_cmp(&distance_sq_to_yard_perimeter(pos, yard_b))
                    .unwrap_or(std::cmp::Ordering::Equal)
                    .then(owner_a.to_bits().cmp(&owner_b.to_bits()))
            })
            .map(|(owner, _)| owner);
    }

    owner_infos
        .iter()
        .min_by(|(owner_a, info_a), (owner_b, info_b)| {
            let da = distance_sq_to_task_area_perimeter(pos, &info_a.area);
            let db = distance_sq_to_task_area_perimeter(pos, &info_b.area);
            da.partial_cmp(&db)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then(owner_a.to_bits().cmp(&owner_b.to_bits()))
        })
        .map(|(owner, _)| *owner)
}

fn distance_sq_to_task_area_perimeter(pos: Vec2, area: &AreaBounds) -> f32 {
    let inside_x = pos.x >= area.min.x && pos.x <= area.max.x;
    let inside_y = pos.y >= area.min.y && pos.y <= area.max.y;

    if inside_x && inside_y {
        let dist_to_left = pos.x - area.min.x;
        let dist_to_right = area.max.x - pos.x;
        let dist_to_bottom = pos.y - area.min.y;
        let dist_to_top = area.max.y - pos.y;
        let min_dist = dist_to_left
            .min(dist_to_right)
            .min(dist_to_bottom)
            .min(dist_to_top);
        min_dist * min_dist
    } else {
        let clamped_x = pos.x.clamp(area.min.x, area.max.x);
        let clamped_y = pos.y.clamp(area.min.y, area.max.y);
        let dx = pos.x - clamped_x;
        let dy = pos.y - clamped_y;
        dx * dx + dy * dy
    }
}

fn distance_sq_to_yard_perimeter(pos: Vec2, yard: &Yard) -> f32 {
    let inside_x = pos.x >= yard.min.x && pos.x <= yard.max.x;
    let inside_y = pos.y >= yard.min.y && pos.y <= yard.max.y;

    if inside_x && inside_y {
        let dist_to_left = pos.x - yard.min.x;
        let dist_to_right = yard.max.x - pos.x;
        let dist_to_bottom = pos.y - yard.min.y;
        let dist_to_top = yard.max.y - pos.y;
        let min_dist = dist_to_left
            .min(dist_to_right)
            .min(dist_to_bottom)
            .min(dist_to_top);
        min_dist * min_dist
    } else {
        let clamped_x = pos.x.clamp(yard.min.x, yard.max.x);
        let clamped_y = pos.y.clamp(yard.min.y, yard.max.y);
        let dx = pos.x - clamped_x;
        let dy = pos.y - clamped_y;
        dx * dx + dy * dy
    }
}

pub fn compare_source_candidates(a: &SourceCandidate, b: &SourceCandidate) -> Ordering {
    a.sort_dist_sq
        .partial_cmp(&b.sort_dist_sq)
        .unwrap_or(Ordering::Equal)
        .then(a.entity_bits.cmp(&b.entity_bits))
}

pub fn compare_auto_idle_for_cleanup(a: &AutoIdleEntry, b: &AutoIdleEntry) -> Ordering {
    b.stage
        .cmp(&a.stage)
        .then(
            b.sort_dist_sq
                .partial_cmp(&a.sort_dist_sq)
                .unwrap_or(Ordering::Equal),
        )
        .then(b.entity_bits.cmp(&a.entity_bits))
}

pub fn div_ceil_u32(value: u32, divisor: u32) -> u32 {
    if value == 0 {
        0
    } else {
        value.div_ceil(divisor)
    }
}

/// パス到達可能性チェック（WorldMap + PathfindingContext 依存）
pub fn is_reachable(
    start_grid: (i32, i32),
    target_pos: Vec2,
    world_map: &hw_world::WorldMap,
    pf_context: &mut hw_world::pathfinding::PathfindingContext,
) -> bool {
    let target_grid = hw_world::WorldMap::world_to_grid(target_pos);
    hw_world::pathfinding::find_path_to_adjacent(
        world_map,
        pf_context,
        start_grid,
        target_grid,
        true,
    )
    .is_some()
}
