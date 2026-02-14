//! 運搬タスクのソースアイテム探索

use crate::systems::familiar_ai::decide::task_management::{
    ReservationShadow,
    validator::source_not_reserved,
};
use crate::systems::logistics::ResourceType;
use bevy::prelude::*;

type TaskQueries<'w, 's> =
    crate::systems::soul_ai::execute::task_execution::context::TaskAssignmentQueries<'w, 's>;

/// 共通: target_pos に最も近い未予約アイテムを検索（条件差分は extra_filter で指定）
fn find_nearest_source_item<'w, 's>(
    resource_type: ResourceType,
    target_pos: Vec2,
    queries: &TaskQueries<'w, 's>,
    shadow: &ReservationShadow,
    extra_filter: impl Fn(Entity) -> bool,
) -> Option<(Entity, Vec2)> {
    queries
        .free_resource_items
        .iter()
        .filter(|(_, _, visibility, res_item)| {
            **visibility != Visibility::Hidden && res_item.0 == resource_type
        })
        .filter(|(entity, _, _, _)| source_not_reserved(*entity, queries, shadow))
        .filter(|(entity, _, _, _)| extra_filter(*entity))
        .min_by(|(_, t1, _, _), (_, t2, _, _)| {
            let d1 = t1.translation.truncate().distance_squared(target_pos);
            let d2 = t2.translation.truncate().distance_squared(target_pos);
            d1.partial_cmp(&d2).unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|(e, t, _, _)| (e, t.translation.truncate()))
}

pub fn find_nearest_mixer_source_item<'w, 's>(
    item_type: ResourceType,
    mixer_pos: Vec2,
    queries: &TaskQueries<'w, 's>,
    shadow: &ReservationShadow,
) -> Option<(Entity, Vec2)> {
    find_nearest_source_item(item_type, mixer_pos, queries, shadow, |_| true)
}

pub fn find_nearest_stockpile_source_item<'w, 's>(
    resource_type: ResourceType,
    item_owner: Option<Entity>,
    stock_pos: Vec2,
    queries: &TaskQueries<'w, 's>,
    shadow: &ReservationShadow,
) -> Option<(Entity, Vec2)> {
    let extra_filter = |entity: Entity| {
        queries
            .designation
            .targets
            .get(entity)
            .ok()
            .is_some_and(|(_, _, _, _, _, _, stored_in_opt)| stored_in_opt.is_none())
            && {
                let belongs = queries.designation.belongs.get(entity).ok().map(|b| b.0);
                item_owner == belongs
            }
    };
    find_nearest_source_item(resource_type, stock_pos, queries, shadow, extra_filter)
}

pub fn find_fixed_stockpile_source_item<'w, 's>(
    source_item: Entity,
    resource_type: ResourceType,
    item_owner: Option<Entity>,
    queries: &TaskQueries<'w, 's>,
    shadow: &ReservationShadow,
) -> Option<(Entity, Vec2)> {
    if !source_not_reserved(source_item, queries, shadow) {
        return None;
    }

    let (transform, _, _, _, resource_opt, _, stored_in_opt) =
        queries.designation.targets.get(source_item).ok()?;
    if stored_in_opt.is_some() {
        return None;
    }
    if !resource_opt.is_some_and(|res| res.0 == resource_type) {
        return None;
    }

    let owner = queries.designation.belongs.get(source_item).ok().map(|b| b.0);
    if owner != item_owner {
        return None;
    }

    Some((source_item, transform.translation.truncate()))
}

pub fn find_nearest_blueprint_source_item<'w, 's>(
    resource_type: ResourceType,
    bp_pos: Vec2,
    queries: &TaskQueries<'w, 's>,
    shadow: &ReservationShadow,
) -> Option<(Entity, Vec2)> {
    find_nearest_source_item(resource_type, bp_pos, queries, shadow, |_| true)
}
