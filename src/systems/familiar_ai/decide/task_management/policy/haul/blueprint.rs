//! Blueprint 向け運搬タスクの割り当て

use crate::constants::WHEELBARROW_CAPACITY;
use crate::systems::familiar_ai::decide::task_management::{AssignTaskContext, ReservationShadow};
use crate::systems::logistics::transport_request::{
    can_complete_pick_drop_to_blueprint, WheelbarrowDestination,
};
use bevy::prelude::*;

use super::super::super::builders::{
    issue_haul_to_blueprint_with_source, issue_haul_with_wheelbarrow,
};
use super::super::super::validator::resolve_haul_to_blueprint_inputs;
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
            issue_haul_with_wheelbarrow(
                lease.wheelbarrow,
                lease.source_pos,
                lease.destination,
                lease.items.clone(),
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
        let items = source_selector::collect_nearby_items_for_wheelbarrow(
            resource_type,
            task_pos,
            WHEELBARROW_CAPACITY,
            queries,
            shadow,
        );
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

    false
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
