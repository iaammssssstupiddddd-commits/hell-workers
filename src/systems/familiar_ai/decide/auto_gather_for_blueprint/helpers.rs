use crate::constants::{
    BLUEPRINT_AUTO_GATHER_STAGE1_RADIUS_TILES, BLUEPRINT_AUTO_GATHER_STAGE2_RADIUS_TILES,
    BLUEPRINT_AUTO_GATHER_STAGE3_RADIUS_TILES, ROCK_DROP_AMOUNT, TILE_SIZE, WOOD_DROP_AMOUNT,
};
use crate::systems::command::TaskArea;
use crate::systems::jobs::WorkType;
use crate::systems::logistics::ResourceType;
use crate::world::map::WorldMap;
use crate::world::pathfinding::{self, PathfindingContext};
use bevy::prelude::*;
use std::cmp::Ordering;

pub(super) const STAGE_COUNT: usize = 5;

#[derive(Clone)]
pub(super) struct OwnerInfo {
    pub(super) area: TaskArea,
    pub(super) center: Vec2,
    pub(super) path_start: (i32, i32),
}

#[derive(Default)]
pub(super) struct SupplyBucket {
    pub(super) ground_items: u32,
    pub(super) pending_non_auto_yield: u32,
    pub(super) auto_active_count: u32,
    pub(super) auto_idle: Vec<AutoIdleEntry>,
}

#[derive(Clone, Copy)]
pub(super) struct SourceCandidate {
    pub(super) entity: Entity,
    pub(super) pos: Vec2,
    pub(super) sort_dist_sq: f32,
    pub(super) entity_bits: u64,
}

#[derive(Clone, Copy)]
pub(super) struct AutoIdleEntry {
    pub(super) entity: Entity,
    pub(super) stage: usize,
    pub(super) sort_dist_sq: f32,
    pub(super) entity_bits: u64,
}

pub(super) fn source_resource_from_components(
    has_tree: bool,
    has_rock: bool,
) -> Option<ResourceType> {
    if has_tree {
        Some(ResourceType::Wood)
    } else if has_rock {
        Some(ResourceType::Rock)
    } else {
        None
    }
}

pub(super) fn drop_amount_for_resource(resource_type: ResourceType) -> u32 {
    match resource_type {
        ResourceType::Wood => WOOD_DROP_AMOUNT,
        ResourceType::Rock => ROCK_DROP_AMOUNT,
        _ => 0,
    }
}

pub(super) fn work_type_for_resource(resource_type: ResourceType) -> WorkType {
    match resource_type {
        ResourceType::Wood => WorkType::Chop,
        ResourceType::Rock => WorkType::Mine,
        _ => WorkType::Chop,
    }
}

pub(super) fn resource_rank(resource_type: ResourceType) -> u8 {
    match resource_type {
        ResourceType::Wood => 0,
        ResourceType::Rock => 1,
        _ => 255,
    }
}

pub(super) fn stage_for_pos(pos: Vec2, area: &TaskArea) -> usize {
    if area.contains(pos) {
        return 0;
    }

    let dist_sq = distance_sq_to_task_area_outside(pos, area);
    let radius1 = BLUEPRINT_AUTO_GATHER_STAGE1_RADIUS_TILES * TILE_SIZE;
    let radius2 = BLUEPRINT_AUTO_GATHER_STAGE2_RADIUS_TILES * TILE_SIZE;
    let radius3 = BLUEPRINT_AUTO_GATHER_STAGE3_RADIUS_TILES * TILE_SIZE;

    if dist_sq <= radius1 * radius1 {
        1
    } else if dist_sq <= radius2 * radius2 {
        2
    } else if dist_sq <= radius3 * radius3 {
        3
    } else {
        4
    }
}

pub(super) fn resolve_owner(pos: Vec2, owner_areas: &[(Entity, TaskArea)]) -> Option<Entity> {
    if owner_areas.is_empty() {
        return None;
    }

    let mut containing = Vec::new();
    for (owner, area) in owner_areas {
        if area.contains(pos) {
            containing.push((*owner, area));
        }
    }

    let candidates: Vec<(Entity, &TaskArea)> = if containing.is_empty() {
        owner_areas
            .iter()
            .map(|(owner, area)| (*owner, area))
            .collect()
    } else {
        containing
    };

    candidates
        .into_iter()
        .min_by(|(owner_a, area_a), (owner_b, area_b)| {
            let da = distance_sq_to_task_area_perimeter(pos, area_a);
            let db = distance_sq_to_task_area_perimeter(pos, area_b);
            da.partial_cmp(&db)
                .unwrap_or(Ordering::Equal)
                .then(owner_a.to_bits().cmp(&owner_b.to_bits()))
        })
        .map(|(owner, _)| owner)
}

fn distance_sq_to_task_area_outside(pos: Vec2, area: &TaskArea) -> f32 {
    if area.contains(pos) {
        return 0.0;
    }

    let clamped_x = pos.x.clamp(area.min.x, area.max.x);
    let clamped_y = pos.y.clamp(area.min.y, area.max.y);
    let dx = pos.x - clamped_x;
    let dy = pos.y - clamped_y;
    dx * dx + dy * dy
}

fn distance_sq_to_task_area_perimeter(pos: Vec2, area: &TaskArea) -> f32 {
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

pub(super) fn compare_source_candidates(a: &SourceCandidate, b: &SourceCandidate) -> Ordering {
    a.sort_dist_sq
        .partial_cmp(&b.sort_dist_sq)
        .unwrap_or(Ordering::Equal)
        .then(a.entity_bits.cmp(&b.entity_bits))
}

pub(super) fn compare_auto_idle_for_cleanup(a: &AutoIdleEntry, b: &AutoIdleEntry) -> Ordering {
    b.stage
        .cmp(&a.stage)
        .then(
            b.sort_dist_sq
                .partial_cmp(&a.sort_dist_sq)
                .unwrap_or(Ordering::Equal),
        )
        .then(b.entity_bits.cmp(&a.entity_bits))
}

pub(super) fn is_reachable(
    start_grid: (i32, i32),
    target_pos: Vec2,
    world_map: &WorldMap,
    pf_context: &mut PathfindingContext,
) -> bool {
    let target_grid = WorldMap::world_to_grid(target_pos);
    pathfinding::find_path_to_adjacent(world_map, pf_context, start_grid, target_grid).is_some()
}

pub(super) fn div_ceil_u32(value: u32, divisor: u32) -> u32 {
    if value == 0 {
        0
    } else {
        (value + divisor - 1) / divisor
    }
}
