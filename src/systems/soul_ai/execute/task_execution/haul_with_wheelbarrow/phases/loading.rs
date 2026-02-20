//! アイテムを手押し車に積み込むフェーズ

use super::super::cancel;
use crate::relationships::LoadedIn;
use crate::systems::soul_ai::execute::task_execution::{
    common::{release_mixer_mud_storage_for_item, update_stockpile_on_item_removal},
    context::TaskExecutionContext,
    transport_common::{reservation, sand_collect},
    types::{AssignedTask, HaulWithWheelbarrowData, HaulWithWheelbarrowPhase},
};
use bevy::prelude::*;

pub fn handle(
    ctx: &mut TaskExecutionContext,
    data: HaulWithWheelbarrowData,
    commands: &mut Commands,
) {
    if let Some(source_entity) = data.collect_source {
        let collect_amount = data.collect_amount.max(1);
        let collected_items = match data.collect_resource_type {
            Some(crate::systems::logistics::ResourceType::Sand) => {
                if ctx.queries.designation.targets.get(source_entity).is_err() {
                    cancel::cancel_wheelbarrow_task(ctx, &data, commands);
                    return;
                }
                sand_collect::spawn_loaded_sand_items(
                    commands,
                    data.wheelbarrow,
                    data.source_pos,
                    collect_amount,
                )
            }
            Some(crate::systems::logistics::ResourceType::Bone) => {
                // 川タイルなどは Designation がない場合もあるが、とりあえずチェックなしで進めるか
                // あるいは find_collect_bone_source で Designation がないことを確認しているはず
                // ここではソースエンティティの存在チェックぐらいはすべきか？
                // しかし TileEntity は常に存在するはず
                // Bone の場合、sand_collect::clear_collect_sand_designation は呼ばなくて良い？
                // Sand の場合は `task_state` (Designation) を削除している。
                // Bone (River) の場合、Designation は付いていないはず (find_collect_bone_source で除外)。
                // したがって、Designation の削除も不要。

                sand_collect::spawn_loaded_bone_items(
                    commands,
                    data.wheelbarrow,
                    data.source_pos,
                    collect_amount,
                )
            }
            _ => {
                cancel::cancel_wheelbarrow_task(ctx, &data, commands);
                return;
            }
        };
        if collected_items.is_empty() {
            cancel::cancel_wheelbarrow_task(ctx, &data, commands);
            return;
        }

        sand_collect::clear_collect_sand_designation(commands, source_entity);
        reservation::release_source(ctx, source_entity, 1);

        let loaded_count = collected_items.len();
        for &item in &collected_items {
            if let Ok(mut item_commands) = commands.get_entity(item) {
                item_commands.try_insert(crate::relationships::DeliveringTo(
                    data.destination.stockpile_or_blueprint().unwrap(),
                ));
            }
        }
        let mut next_data = data;
        next_data.items = collected_items;
        next_data.collect_source = None;
        next_data.collect_amount = 0;
        next_data.collect_resource_type = None;
        next_data.phase = HaulWithWheelbarrowPhase::GoingToDestination;
        *ctx.task = AssignedTask::HaulWithWheelbarrow(next_data);

        info!(
            "WB_HAUL: Soul {:?} collected {} sand directly into wheelbarrow",
            ctx.soul_entity, loaded_count
        );
        return;
    }

    let mut deduped_items = std::collections::HashSet::new();
    // アイテム情報を先に収集（borrowing conflict 回避）
    let items_to_load: Vec<(Entity, Option<Entity>)> = data
        .items
        .iter()
        .filter_map(|&item_entity| {
            if !deduped_items.insert(item_entity) {
                return None;
            }
            let Ok((_, _, _, _, _, _, stored_in_opt)) =
                ctx.queries.designation.targets.get(item_entity)
            else {
                return None;
            };
            Some((item_entity, stored_in_opt.map(|si| si.0)))
        })
        .collect();

    if items_to_load.is_empty() {
        info!(
            "WB_HAUL: Soul {:?} found no loadable items, canceling",
            ctx.soul_entity
        );
        cancel::cancel_wheelbarrow_task(ctx, &data, commands);
        return;
    }

    let mut loaded_entities = std::collections::HashSet::new();
    for (item_entity, stored_in_stockpile) in &items_to_load {
        release_mixer_mud_storage_for_item(ctx, *item_entity, commands);
        let Ok(mut item_commands) = commands.get_entity(*item_entity) else {
            continue;
        };
        item_commands.try_insert((Visibility::Hidden, LoadedIn(data.wheelbarrow)));
        item_commands.try_remove::<crate::relationships::StoredIn>();
        item_commands.try_remove::<crate::systems::jobs::Designation>();
        item_commands.try_remove::<crate::systems::jobs::TaskSlots>();
        item_commands.try_remove::<crate::systems::jobs::Priority>();
        item_commands.try_remove::<crate::systems::logistics::ReservedForTask>();

        if let Some(stock_entity) = stored_in_stockpile {
            update_stockpile_on_item_removal(*stock_entity, &mut ctx.queries.storage.stockpiles);
        }

        reservation::record_picked_source(ctx, *item_entity, 1);
        loaded_entities.insert(*item_entity);
    }

    let loaded_count = loaded_entities.len();
    let total_count = data.items.len();
    if loaded_count < total_count {
        for &item_entity in &data.items {
            if !loaded_entities.contains(&item_entity) {
                reservation::release_source(ctx, item_entity, 1);
                if let Ok(mut item_commands) = commands.get_entity(item_entity) {
                    item_commands.try_remove::<crate::relationships::DeliveringTo>();
                }
            }
        }
        info!(
            "WB_HAUL: {} of {} items missing, released reservations",
            total_count - loaded_count,
            total_count
        );
    }

    *ctx.task = AssignedTask::HaulWithWheelbarrow(HaulWithWheelbarrowData {
        phase: HaulWithWheelbarrowPhase::GoingToDestination,
        ..data
    });

    info!(
        "WB_HAUL: Soul {:?} loaded {} items into wheelbarrow",
        ctx.soul_entity, loaded_count
    );
}
