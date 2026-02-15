//! Blueprint 向け運搬タスクの割り当て

use crate::constants::WHEELBARROW_CAPACITY;
use crate::constants::{MAP_HEIGHT, MAP_WIDTH};
use crate::systems::command::TaskArea;
use crate::systems::familiar_ai::decide::task_management::{AssignTaskContext, ReservationShadow};
use crate::systems::logistics::ResourceType;
use crate::systems::logistics::transport_request::{
    can_complete_pick_drop_to_blueprint, WheelbarrowDestination,
};
use crate::world::map::TerrainType;
use crate::world::map::WorldMap;
use bevy::prelude::*;

use super::super::super::builders::{
    issue_collect_sand_with_wheelbarrow_to_blueprint, issue_haul_to_blueprint_with_source,
    issue_haul_with_wheelbarrow, issue_collect_bone_with_wheelbarrow_to_blueprint,
};
use super::super::super::validator::{resolve_haul_to_blueprint_inputs, source_not_reserved};
use super::lease_validation;
use super::source_selector;
use super::wheelbarrow;

pub fn assign_haul_to_blueprint(
    task_pos: Vec2,
    already_commanded: bool,
    ctx: &AssignTaskContext<'_>,
    queries: &mut crate::systems::soul_ai::execute::task_execution::context::TaskAssignmentQueries,
    shadow: &mut ReservationShadow,
) -> bool {
    let Some((blueprint, resource_type)) =
        resolve_haul_to_blueprint_inputs(ctx.task_entity, queries)
    else {
        return false;
    };

    // 猫車不要のリソースは単品運搬
    if !resource_type.requires_wheelbarrow() {
        return assign_single_item_haul(
            blueprint,
            resource_type,
            task_pos,
            already_commanded,
            ctx,
            queries,
            shadow,
        );
    }

    // --- 猫車必須リソース ---
    let remaining_needed = compute_remaining_blueprint_wheelbarrow_amount(
        blueprint,
        resource_type,
        ctx.task_entity,
        queries,
        shadow,
    );
    if remaining_needed == 0 {
        return false;
    }

    // 1. Pick-drop チェック: リースがなく、最寄りアイテムが BP 隣接なら単品手運び
    if queries.wheelbarrow_leases.get(ctx.task_entity).is_err() {
        if try_pick_drop_to_blueprint(
            blueprint,
            resource_type,
            already_commanded,
            ctx,
            queries,
            shadow,
        ) {
            return true;
        }
    }

    // 2. リースあり → バリデーションして猫車運搬
    if let Ok(lease) = queries.wheelbarrow_leases.get(ctx.task_entity) {
        if lease_validation::validate_lease(lease, queries, shadow, 1) {
            let max_items = remaining_needed.min(WHEELBARROW_CAPACITY as u32) as usize;
            let lease_items: Vec<Entity> = lease.items.iter().copied().take(max_items).collect();
            if lease_items.is_empty() {
                return false;
            }
            issue_haul_with_wheelbarrow(
                lease.wheelbarrow,
                lease.source_pos,
                lease.destination,
                lease_items,
                task_pos,
                already_commanded,
                ctx,
                queries,
                shadow,
            );
            return true;
        }
    }

    // 3. リースなしフォールバック: 最寄り猫車 + 複数アイテム収集
    if let Some(wb_entity) = wheelbarrow::find_nearest_wheelbarrow(task_pos, queries, shadow) {
        let max_items = remaining_needed.min(WHEELBARROW_CAPACITY as u32) as usize;
        let mut items = source_selector::collect_nearby_items_for_wheelbarrow(
            resource_type,
            task_pos,
            max_items,
            queries,
            shadow,
        );
        if items.is_empty() {
            items = source_selector::collect_items_for_wheelbarrow_unbounded(
                resource_type,
                task_pos,
                max_items,
                queries,
                shadow,
            );
        }
        if !items.is_empty() {
            let source_pos = items
                .iter()
                .map(|(_, pos)| *pos)
                .reduce(|a, b| a + b)
                .unwrap()
                / items.len() as f32;
            let item_entities: Vec<Entity> = items.iter().map(|(e, _)| *e).collect();

            issue_haul_with_wheelbarrow(
                wb_entity,
                source_pos,
                WheelbarrowDestination::Blueprint(blueprint),
                item_entities,
                task_pos,
                already_commanded,
                ctx,
                queries,
                shadow,
            );
            return true;
        }
    }

    // 4. Sand 専用フォールバック: 砂ソースへ猫車で向かい、必要量を直接採取して搬入
    if resource_type == ResourceType::Sand
        && try_direct_sand_collect_with_wheelbarrow(
            blueprint,
            remaining_needed,
            task_pos,
            already_commanded,
            ctx,
            queries,
            shadow,
        )
    {
        return true;
    }

    // 5. Bone 専用フォールバック: 骨ソース（川など）へ猫車で向かい、必要量を直接採取して搬入
    if resource_type == ResourceType::Bone
        && try_direct_bone_collect_with_wheelbarrow(
            blueprint,
            remaining_needed,
            task_pos,
            already_commanded,
            ctx,
            queries,
            shadow,
        )
    {
        return true;
    }

    false
}

fn compute_remaining_blueprint_wheelbarrow_amount(
    blueprint: Entity,
    resource_type: ResourceType,
    task_entity: Entity,
    queries: &crate::systems::soul_ai::execute::task_execution::context::TaskAssignmentQueries,
    _shadow: &ReservationShadow,
) -> u32 {
    let Ok((_, blueprint_comp, _)) = queries.storage.blueprints.get(blueprint) else {
        return 0;
    };

    let required = *blueprint_comp
        .required_materials
        .get(&resource_type)
        .unwrap_or(&0);
    let delivered = *blueprint_comp
        .delivered_materials
        .get(&resource_type)
        .unwrap_or(&0);
    let needed_material = required.saturating_sub(delivered);
    if needed_material == 0 {
        return 0;
    }

    let _current_workers = queries
        .designation
        .designations
        .get(task_entity)
        .ok()
        .and_then(|(_, _, _, _, _, workers_opt, _, _)| workers_opt.map(|workers| workers.len()))
        .unwrap_or(0);

    // Relationship を利用して搬入予約数を取得
    let reserved_from_relationship = queries
        .reservation
        .incoming_deliveries_query
        .get(blueprint)
        .map(|inc| inc.len())
        .unwrap_or(0);

    let reserved_total = reserved_from_relationship;

    needed_material.saturating_sub(reserved_total as u32)
}

/// BP 隣接アイテムがあれば単品手運びで運ぶ（pick-drop）
fn try_pick_drop_to_blueprint(
    blueprint: Entity,
    resource_type: crate::systems::logistics::ResourceType,
    already_commanded: bool,
    ctx: &AssignTaskContext<'_>,
    queries: &mut crate::systems::soul_ai::execute::task_execution::context::TaskAssignmentQueries,
    shadow: &mut ReservationShadow,
) -> bool {
    let Ok((bp_transform, bp, _)) = queries.storage.blueprints.get(blueprint) else {
        return false;
    };
    let bp_pos = bp_transform.translation.truncate();
    let occupied_grids = bp.occupied_grids.clone();

    let Some((source_item, source_pos)) =
        source_selector::find_nearest_blueprint_source_item(resource_type, bp_pos, queries, shadow)
    else {
        return false;
    };

    if !can_complete_pick_drop_to_blueprint(source_pos, &occupied_grids) {
        return false;
    }

    issue_haul_to_blueprint_with_source(
        source_item,
        blueprint,
        source_pos,
        already_commanded,
        ctx,
        queries,
        shadow,
    );
    true
}

/// 猫車不要リソースの単品運搬
fn assign_single_item_haul(
    blueprint: Entity,
    resource_type: crate::systems::logistics::ResourceType,
    task_pos: Vec2,
    already_commanded: bool,
    ctx: &AssignTaskContext<'_>,
    queries: &mut crate::systems::soul_ai::execute::task_execution::context::TaskAssignmentQueries,
    shadow: &mut ReservationShadow,
) -> bool {
    let Some((source_item, source_pos)) =
        source_selector::find_nearest_blueprint_source_item(resource_type, task_pos, queries, shadow)
    else {
        debug!(
            "ASSIGN: Blueprint request {:?} has no available {:?} source",
            ctx.task_entity, resource_type
        );
        return false;
    };
    issue_haul_to_blueprint_with_source(
        source_item,
        blueprint,
        source_pos,
        already_commanded,
        ctx,
        queries,
        shadow,
    );
    true
}

fn try_direct_sand_collect_with_wheelbarrow(
    blueprint: Entity,
    remaining_needed: u32,
    task_pos: Vec2,
    already_commanded: bool,
    ctx: &AssignTaskContext<'_>,
    queries: &mut crate::systems::soul_ai::execute::task_execution::context::TaskAssignmentQueries,
    shadow: &mut ReservationShadow,
) -> bool {
    let Some((source_entity, source_pos)) =
        find_collect_sand_source(task_pos, ctx.task_area_opt, queries, shadow)
    else {
        return false;
    };

    let Some(wb_entity) = wheelbarrow::find_nearest_wheelbarrow(source_pos, queries, shadow) else {
        return false;
    };

    let amount = remaining_needed.max(1).min(WHEELBARROW_CAPACITY as u32);

    issue_collect_sand_with_wheelbarrow_to_blueprint(
        wb_entity,
        source_entity,
        source_pos,
        blueprint,
        amount,
        task_pos,
        already_commanded,
        ctx,
        queries,
        shadow,
    );
    true
}

fn find_collect_sand_source(
    target_pos: Vec2,
    task_area_opt: Option<&TaskArea>,
    queries: &crate::systems::soul_ai::execute::task_execution::context::TaskAssignmentQueries,
    shadow: &ReservationShadow,
) -> Option<(Entity, Vec2)> {
    let find_sand_pile = |area_filter: Option<&TaskArea>| -> Option<(Entity, Vec2)> {
        queries
            .sand_piles
            .iter()
            .filter(|(entity, transform, designation_opt, workers_opt)| {
                if designation_opt.is_some() {
                    return false;
                }
                if workers_opt.map(|workers| workers.len()).unwrap_or(0) > 0 {
                    return false;
                }
                if !source_not_reserved(*entity, queries, shadow) {
                    return false;
                }
                if let Some(area) = area_filter {
                    area.contains(transform.translation.truncate())
                } else {
                    true
                }
            })
            .min_by(|(_, t1, _, _), (_, t2, _, _)| {
                let d1 = t1.translation.truncate().distance_squared(target_pos);
                let d2 = t2.translation.truncate().distance_squared(target_pos);
                d1.partial_cmp(&d2).unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|(entity, transform, _, _)| (entity, transform.translation.truncate()))
    };

    if let Some(best) = find_sand_pile(task_area_opt) {
        return Some(best);
    }
    if task_area_opt.is_some() {
        if let Some(best) = find_sand_pile(None) {
            return Some(best);
        }
    }

    let scan_sand_tiles = |area_filter: Option<&TaskArea>| -> Option<(Entity, Vec2)> {
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
                if queries.world_map.tiles[idx] != TerrainType::Sand {
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
                if workers_opt.map(|workers| workers.len()).unwrap_or(0) > 0 {
                    continue;
                }
                if !source_not_reserved(tile_entity, queries, shadow) {
                    continue;
                }

                let tile_pos = WorldMap::grid_to_world(gx, gy);
                if let Some(area) = area_filter && !area.contains(tile_pos) {
                    continue;
                }

                let dist_sq = tile_pos.distance_squared(target_pos);
                match best {
                    Some((_, _, best_dist)) if best_dist <= dist_sq => {}
                    _ => best = Some((tile_entity, tile_pos, dist_sq)),
                }
            }
        }

        best.map(|(entity, pos, _)| (entity, pos))
    };

    if let Some(best) = scan_sand_tiles(task_area_opt) {
        return Some(best);
    }
    if task_area_opt.is_some() {
        return scan_sand_tiles(None);
    }

    None
}

fn try_direct_bone_collect_with_wheelbarrow(
    blueprint: Entity,
    remaining_needed: u32,
    task_pos: Vec2,
    already_commanded: bool,
    ctx: &AssignTaskContext<'_>,
    queries: &mut crate::systems::soul_ai::execute::task_execution::context::TaskAssignmentQueries,
    shadow: &mut ReservationShadow,
) -> bool {
    let Some((source_entity, source_pos)) =
        find_collect_bone_source(task_pos, ctx.task_area_opt, queries, shadow)
    else {
        debug!(
            "ASSIGN: Blueprint {:?} has no available Bone source for direct collect",
            blueprint
        );
        return false;
    };

    let Some(wb_entity) = wheelbarrow::find_nearest_wheelbarrow(source_pos, queries, shadow) else {
        debug!(
            "ASSIGN: Blueprint {:?} has no available wheelbarrow for direct Bone collect",
            blueprint
        );
        return false;
    };

    let amount = remaining_needed.max(1).min(WHEELBARROW_CAPACITY as u32);

    issue_collect_bone_with_wheelbarrow_to_blueprint(
        wb_entity,
        source_entity,
        source_pos,
        blueprint,
        amount,
        task_pos,
        already_commanded,
        ctx,
        queries,
        shadow,
    );
    true
}

pub(super) fn find_collect_bone_source(
    target_pos: Vec2,
    task_area_opt: Option<&TaskArea>,
    queries: &crate::systems::soul_ai::execute::task_execution::context::TaskAssignmentQueries,
    shadow: &ReservationShadow,
) -> Option<(Entity, Vec2)> {
    let find_bone_pile = |area_filter: Option<&TaskArea>| -> Option<(Entity, Vec2)> {
        queries
            .bone_piles
            .iter()
            .filter(|(entity, transform, designation_opt, workers_opt)| {
                if designation_opt.is_some() {
                    return false;
                }
                if workers_opt.map(|workers| workers.len()).unwrap_or(0) > 0 {
                    return false;
                }
                if !source_not_reserved(*entity, queries, shadow) {
                    return false;
                }
                if let Some(area) = area_filter {
                    area.contains(transform.translation.truncate())
                } else {
                    true
                }
            })
            .min_by(|(_, t1, _, _), (_, t2, _, _)| {
                let d1 = t1.translation.truncate().distance_squared(target_pos);
                let d2 = t2.translation.truncate().distance_squared(target_pos);
                d1.partial_cmp(&d2).unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|(entity, transform, _, _)| (entity, transform.translation.truncate()))
    };

    let scan_river_tiles = |area_filter: Option<&TaskArea>| -> Option<(Entity, Vec2)> {
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
                
                // 川タイル (River) を対象とする
                if queries.world_map.tiles[idx] != TerrainType::River {
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
                if workers_opt.map(|workers| workers.len()).unwrap_or(0) > 0 {
                    continue;
                }
                if !source_not_reserved(tile_entity, queries, shadow) {
                    continue;
                }

                let tile_pos = WorldMap::grid_to_world(gx, gy);
                if let Some(area) = area_filter && !area.contains(tile_pos) {
                    continue;
                }

                let dist_sq = tile_pos.distance_squared(target_pos);
                match best {
                    Some((_, _, best_dist)) if best_dist <= dist_sq => {}
                    _ => best = Some((tile_entity, tile_pos, dist_sq)),
                }
            }
        }

        best.map(|(entity, pos, _)| (entity, pos))
    };

    if let Some(best) = find_bone_pile(task_area_opt) {
        return Some(best);
    }
    if task_area_opt.is_some() {
        if let Some(best) = find_bone_pile(None) {
            return Some(best);
        }
    }

    if let Some(best) = scan_river_tiles(task_area_opt) {
        return Some(best);
    }
    if task_area_opt.is_some() {
        return scan_river_tiles(None);
    }

    None
}
