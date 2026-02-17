//! 直接採取: Sand/Bone の pile 優先 → 地形タイル走査の共通ロジック

use crate::constants::{MAP_HEIGHT, MAP_WIDTH};
use crate::systems::command::TaskArea;
use crate::systems::familiar_ai::decide::task_management::ReservationShadow;
use crate::systems::familiar_ai::decide::task_management::validator::source_not_reserved;
use crate::world::map::TerrainType;
use crate::world::map::WorldMap;
use bevy::prelude::*;

type TaskAssignmentQueries<'w, 's> =
    crate::systems::soul_ai::execute::task_execution::context::TaskAssignmentQueries<'w, 's>;

/// Sand 採取ソースを探索（pile 優先 → 砂地形タイル走査）
pub fn find_collect_sand_source(
    target_pos: Vec2,
    task_area_opt: Option<&TaskArea>,
    queries: &TaskAssignmentQueries<'_, '_>,
    shadow: &ReservationShadow,
) -> Option<(Entity, Vec2)> {
    if let Some(best) = find_sand_pile(target_pos, task_area_opt, queries, shadow) {
        return Some(best);
    }
    if task_area_opt.is_some() {
        if let Some(best) = find_sand_pile(target_pos, None, queries, shadow) {
            return Some(best);
        }
    }
    if let Some(best) = scan_terrain_tiles(
        target_pos,
        task_area_opt,
        TerrainType::Sand,
        queries,
        shadow,
    ) {
        return Some(best);
    }
    if task_area_opt.is_some() {
        return scan_terrain_tiles(target_pos, None, TerrainType::Sand, queries, shadow);
    }
    None
}

/// Bone 採取ソースを探索（pile 優先 → 川タイル走査）
pub fn find_collect_bone_source(
    target_pos: Vec2,
    task_area_opt: Option<&TaskArea>,
    queries: &TaskAssignmentQueries<'_, '_>,
    shadow: &ReservationShadow,
) -> Option<(Entity, Vec2)> {
    if let Some(best) = find_bone_pile(target_pos, task_area_opt, queries, shadow) {
        return Some(best);
    }
    if task_area_opt.is_some() {
        if let Some(best) = find_bone_pile(target_pos, None, queries, shadow) {
            return Some(best);
        }
    }
    if let Some(best) = scan_terrain_tiles(
        target_pos,
        task_area_opt,
        TerrainType::River,
        queries,
        shadow,
    ) {
        return Some(best);
    }
    if task_area_opt.is_some() {
        return scan_terrain_tiles(target_pos, None, TerrainType::River, queries, shadow);
    }
    None
}

fn find_sand_pile(
    target_pos: Vec2,
    area_filter: Option<&TaskArea>,
    queries: &TaskAssignmentQueries<'_, '_>,
    shadow: &ReservationShadow,
) -> Option<(Entity, Vec2)> {
    queries
        .sand_piles
        .iter()
        .filter(|(entity, transform, designation_opt, workers_opt)| {
            designation_opt.is_none()
                && workers_opt.map(|w| w.len()).unwrap_or(0) == 0
                && source_not_reserved(*entity, queries, shadow)
                && area_filter.map_or(true, |a| a.contains(transform.translation.truncate()))
        })
        .min_by(|(_, t1, _, _), (_, t2, _, _)| {
            let d1 = t1.translation.truncate().distance_squared(target_pos);
            let d2 = t2.translation.truncate().distance_squared(target_pos);
            d1.partial_cmp(&d2).unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|(entity, transform, _, _)| (entity, transform.translation.truncate()))
}

fn find_bone_pile(
    target_pos: Vec2,
    area_filter: Option<&TaskArea>,
    queries: &TaskAssignmentQueries<'_, '_>,
    shadow: &ReservationShadow,
) -> Option<(Entity, Vec2)> {
    queries
        .bone_piles
        .iter()
        .filter(|(entity, transform, designation_opt, workers_opt)| {
            designation_opt.is_none()
                && workers_opt.map(|w| w.len()).unwrap_or(0) == 0
                && source_not_reserved(*entity, queries, shadow)
                && area_filter.map_or(true, |a| a.contains(transform.translation.truncate()))
        })
        .min_by(|(_, t1, _, _), (_, t2, _, _)| {
            let d1 = t1.translation.truncate().distance_squared(target_pos);
            let d2 = t2.translation.truncate().distance_squared(target_pos);
            d1.partial_cmp(&d2).unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|(entity, transform, _, _)| (entity, transform.translation.truncate()))
}

fn scan_terrain_tiles(
    target_pos: Vec2,
    area_filter: Option<&TaskArea>,
    terrain_type: TerrainType,
    queries: &TaskAssignmentQueries<'_, '_>,
    shadow: &ReservationShadow,
) -> Option<(Entity, Vec2)> {
    let (x0, y0, x1, y1) = if let Some(area) = area_filter {
        let (ax0, ay0) = WorldMap::world_to_grid(area.min);
        let (ax1, ay1) = WorldMap::world_to_grid(area.max);
        (ax0, ay0, ax1, ay1)
    } else {
        (0, 0, MAP_WIDTH - 1, MAP_HEIGHT - 1)
    };

    let min_x = x0.min(x1);
    let max_x = x0.max(x1);
    let min_y = y0.min(y1);
    let max_y = y0.max(y1);

    let mut best: Option<(Entity, Vec2, f32)> = None;
    for gy in min_y..=max_y {
        for gx in min_x..=max_x {
            let Some(idx) = queries.world_map.pos_to_idx(gx, gy) else {
                continue;
            };
            if queries.world_map.tiles[idx] != terrain_type {
                continue;
            }

            let Some(tile_entity) = queries.world_map.tile_entities[idx] else {
                continue;
            };
            let Ok((designation_opt, workers_opt)) = queries.task_state.get(tile_entity) else {
                continue;
            };
            if designation_opt.is_some() {
                continue;
            }
            if workers_opt.map(|w| w.len()).unwrap_or(0) > 0 {
                continue;
            }
            if !source_not_reserved(tile_entity, queries, shadow) {
                continue;
            }

            let tile_pos = WorldMap::grid_to_world(gx, gy);
            if let Some(area) = area_filter {
                if !area.contains(tile_pos) {
                    continue;
                }
            }

            let dist_sq = tile_pos.distance_squared(target_pos);
            match best {
                Some((_, _, best_dist)) if best_dist <= dist_sq => {}
                _ => best = Some((tile_entity, tile_pos, dist_sq)),
            }
        }
    }

    best.map(|(entity, pos, _)| (entity, pos))
}
