//! 運搬タスクの割り当てポリシー

mod blueprint;
mod consolidation;
mod lease_validation;
mod source_selector;
mod stockpile;
mod wheelbarrow;

use crate::systems::familiar_ai::decide::task_management::{AssignTaskContext, ReservationShadow};
use bevy::prelude::*;

use super::super::builders::{
    issue_collect_bone_with_wheelbarrow_to_floor, issue_haul_to_mixer,
    issue_haul_to_stockpile_with_source, issue_haul_with_wheelbarrow,
};
use super::super::validator::{
    find_bucket_return_assignment, resolve_haul_to_floor_construction_inputs,
    resolve_haul_to_mixer_inputs, resolve_return_bucket_tank,
};

fn mixer_can_accept_item(
    mixer_entity: Entity,
    item_type: crate::systems::logistics::ResourceType,
    mixer_already_reserved: bool,
    queries: &crate::systems::soul_ai::execute::task_execution::context::TaskAssignmentQueries,
    shadow: &ReservationShadow,
) -> bool {
    let Ok((_, storage, _)) = queries.storage.mixers.get(mixer_entity) else {
        return false;
    };
    let reserved = queries
        .reservation
        .resource_cache
        .get_mixer_destination_reservation(mixer_entity, item_type)
        + shadow.mixer_reserved(mixer_entity, item_type);
    let required = if mixer_already_reserved {
        reserved as u32
    } else {
        (reserved + 1) as u32
    };
    storage.can_accept(item_type, required)
}

pub fn assign_haul_to_mixer(
    task_pos: Vec2,
    already_commanded: bool,
    ctx: &AssignTaskContext<'_>,
    queries: &mut crate::systems::soul_ai::execute::task_execution::context::TaskAssignmentQueries,
    shadow: &mut ReservationShadow,
) -> bool {
    let Some((mixer_entity, item_type)) = resolve_haul_to_mixer_inputs(ctx.task_entity, queries)
    else {
        debug!(
            "ASSIGN: HaulToMixer request {:?} has no resolver input",
            ctx.task_entity
        );
        return false;
    };

    // --- 全固体リソース共通: リースがあれば猫車、なければ単品手運び ---

    // 1. Arbitration がリースを付与していれば猫車で一括運搬
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

    // 2. リースなし → 単品手運び
    assign_single_item_haul_to_mixer(
        mixer_entity,
        item_type,
        task_pos,
        already_commanded,
        ctx,
        queries,
        shadow,
    )
}

/// 単品運搬（Mixer向け）
fn assign_single_item_haul_to_mixer(
    mixer_entity: Entity,
    item_type: crate::systems::logistics::ResourceType,
    task_pos: Vec2,
    already_commanded: bool,
    ctx: &AssignTaskContext<'_>,
    queries: &mut crate::systems::soul_ai::execute::task_execution::context::TaskAssignmentQueries,
    shadow: &mut ReservationShadow,
) -> bool {
    let Some((source_item, source_pos)) =
        source_selector::find_nearest_mixer_source_item(item_type, task_pos, queries, shadow)
    else {
        debug!(
            "ASSIGN: HaulToMixer request {:?} has no available {:?} source",
            ctx.task_entity, item_type
        );
        return false;
    };

    if !mixer_can_accept_item(mixer_entity, item_type, false, queries, shadow) {
        debug!(
            "ASSIGN: Mixer {:?} cannot accept item {:?} (Full or Reserved)",
            mixer_entity, item_type
        );
        return false;
    }

    issue_haul_to_mixer(
        source_item,
        mixer_entity,
        item_type,
        false,
        source_pos,
        already_commanded,
        ctx,
        queries,
        shadow,
    );
    true
}

fn assign_haul_to_floor_construction(
    _task_pos: Vec2,
    already_commanded: bool,
    ctx: &AssignTaskContext<'_>,
    queries: &mut crate::systems::soul_ai::execute::task_execution::context::TaskAssignmentQueries,
    shadow: &mut ReservationShadow,
) -> bool {
    let Some((site_entity, resource_type)) =
        resolve_haul_to_floor_construction_inputs(ctx.task_entity, queries)
    else {
        return false;
    };

    let site_pos = if let Ok((site_transform, _, _)) = queries.storage.floor_sites.get(site_entity) {
        site_transform.translation.truncate()
    } else {
        debug!(
            "ASSIGN: Floor request {:?} site {:?} not found",
            ctx.task_entity, site_entity
        );
        return false;
    };

    // Floor construction requests deliver items onto the site material center.
    // Reuse Haul task path and let execution drop the item near the site anchor.
    if let Some((source_item, source_pos)) =
        source_selector::find_nearest_blueprint_source_item(resource_type, site_pos, queries, shadow)
    {
        issue_haul_to_stockpile_with_source(
            source_item,
            site_entity,
            source_pos,
            already_commanded,
            ctx,
            queries,
            shadow,
        );
        return true;
    }

    // Bone は地面アイテムが無いことが多いため、直接採取フォールバックを許可する。
    if resource_type == crate::systems::logistics::ResourceType::Bone
        && try_direct_bone_collect_to_floor(
            site_entity,
            ctx.task_entity,
            site_pos,
            already_commanded,
            ctx,
            queries,
            shadow,
        )
    {
        return true;
    }

    debug!(
        "ASSIGN: Floor request {:?} has no available {:?} source",
        ctx.task_entity, resource_type
    );
    false
}

fn try_direct_bone_collect_to_floor(
    site_entity: Entity,
    task_entity: Entity,
    site_pos: Vec2,
    already_commanded: bool,
    ctx: &AssignTaskContext<'_>,
    queries: &mut crate::systems::soul_ai::execute::task_execution::context::TaskAssignmentQueries,
    shadow: &mut ReservationShadow,
) -> bool {
    let Some((source_entity, source_pos)) =
        blueprint::find_collect_bone_source(site_pos, ctx.task_area_opt, queries, shadow)
    else {
        debug!(
            "ASSIGN: Floor request {:?} has no available Bone collect source",
            task_entity
        );
        return false;
    };

    let Some(wheelbarrow) = wheelbarrow::find_nearest_wheelbarrow(source_pos, queries, shadow)
    else {
        debug!(
            "ASSIGN: Floor request {:?} has no available wheelbarrow for Bone collect",
            task_entity
        );
        return false;
    };

    let remaining_needed = compute_remaining_floor_bones(site_entity, queries);
    if remaining_needed == 0 {
        debug!(
            "ASSIGN: Floor request {:?} already satisfied before direct collect assignment",
            task_entity
        );
        return false;
    }
    let amount = remaining_needed.min(crate::constants::WHEELBARROW_CAPACITY as u32);

    issue_collect_bone_with_wheelbarrow_to_floor(
        wheelbarrow,
        source_entity,
        source_pos,
        site_entity,
        amount,
        source_pos,
        already_commanded,
        ctx,
        queries,
        shadow,
    );
    info!(
        "ASSIGN: Floor request {:?} assigned direct Bone collect via wheelbarrow {:?} from {:?} to site {:?} (amount {})",
        task_entity,
        wheelbarrow,
        source_entity,
        site_entity,
        amount
    );
    true
}

fn compute_remaining_floor_bones(
    site_entity: Entity,
    queries: &crate::systems::soul_ai::execute::task_execution::context::TaskAssignmentQueries,
) -> u32 {
    let mut needed = 0u32;

    for tile in queries
        .storage
        .floor_tiles
        .iter()
        .filter(|tile| tile.parent_site == site_entity)
    {
        if tile.state == crate::systems::jobs::floor_construction::FloorTileState::WaitingBones {
            needed += crate::constants::FLOOR_BONES_PER_TILE.saturating_sub(tile.bones_delivered);
        }
    }

    let incoming = queries
        .reservation
        .incoming_deliveries_query
        .get(site_entity)
        .map(|inc| inc.len() as u32)
        .unwrap_or(0);

    needed.saturating_sub(incoming)
}

pub fn assign_haul(
    task_pos: Vec2,
    already_commanded: bool,
    ctx: &AssignTaskContext<'_>,
    queries: &mut crate::systems::soul_ai::execute::task_execution::context::TaskAssignmentQueries,
    shadow: &mut ReservationShadow,
) -> bool {
    if blueprint::assign_haul_to_blueprint(task_pos, already_commanded, ctx, queries, shadow) {
        return true;
    }

    if let Some(tank) = resolve_return_bucket_tank(ctx.task_entity, queries) {
        let Some((source_item, source_pos, destination_stockpile)) =
            find_bucket_return_assignment(tank, task_pos, queries, shadow)
        else {
            debug!(
                "ASSIGN: ReturnBucket request {:?} has no valid source/destination for tank {:?}",
                ctx.task_entity, tank
            );
            return false;
        };
        issue_haul_to_stockpile_with_source(
            source_item,
            destination_stockpile,
            source_pos,
            already_commanded,
            ctx,
            queries,
            shadow,
        );
        return true;
    }

    if assign_haul_to_floor_construction(task_pos, already_commanded, ctx, queries, shadow) {
        return true;
    }

    if stockpile::assign_haul_to_stockpile(task_pos, already_commanded, ctx, queries, shadow) {
        return true;
    }

    if consolidation::assign_consolidation_to_stockpile(
        task_pos,
        already_commanded,
        ctx,
        queries,
        shadow,
    ) {
        return true;
    }

    debug!(
        "ASSIGN: Haul task {:?} is not a valid transport request candidate",
        ctx.task_entity
    );
    false
}
