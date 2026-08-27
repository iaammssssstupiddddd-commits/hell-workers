//! アイテムを手押し車に積み込むフェーズ

use super::super::cancel;
use crate::soul_ai::execute::task_execution::{
    common::{release_mixer_mud_storage_for_item, update_stockpile_on_item_removal},
    context::{TaskExecutionContext, TaskHandlerControl},
    transport_common::{reservation, sand_collect},
    types::{AssignedTask, HaulWithWheelbarrowData, HaulWithWheelbarrowPhase},
};
use bevy::prelude::*;
use hw_core::logistics::{ResourceSourceKey, ResourceType};
use hw_core::relationships::LoadedIn;
use hw_world::TerrainType;

pub fn handle(
    ctx: &mut TaskExecutionContext,
    data: HaulWithWheelbarrowData,
    commands: &mut Commands,
) -> TaskHandlerControl {
    if let Some(source) = data.collect_source {
        let collect_amount = data.collect_amount.max(1);
        let collected_items = match data.collect_resource_type {
            Some(ResourceType::Sand) => {
                let source_available = match source {
                    ResourceSourceKey::Entity(entity) => {
                        ctx.queries.designation.targets.get(entity).is_ok()
                    }
                    ResourceSourceKey::Terrain {
                        grid,
                        resource_type: ResourceType::Sand,
                    } => {
                        ctx.env
                            .world_map
                            .pos_to_idx(grid.0, grid.1)
                            .and_then(|idx| ctx.env.world_map.terrain_at_idx(idx))
                            == Some(TerrainType::Sand)
                    }
                    _ => false,
                };
                if !source_available {
                    return cancel::cancel_wheelbarrow_task(ctx, &data, commands);
                }
                sand_collect::spawn_loaded_sand_items(
                    commands,
                    data.wheelbarrow,
                    data.source_pos,
                    collect_amount,
                )
            }
            Some(ResourceType::Bone) => {
                let source_available = match source {
                    // BonePile は従来どおり loading 時の存在確認を行わない。
                    ResourceSourceKey::Entity(_) => true,
                    ResourceSourceKey::Terrain {
                        grid,
                        resource_type: ResourceType::Bone,
                    } => {
                        ctx.env
                            .world_map
                            .pos_to_idx(grid.0, grid.1)
                            .and_then(|idx| ctx.env.world_map.terrain_at_idx(idx))
                            == Some(TerrainType::River)
                    }
                    _ => false,
                };
                if !source_available {
                    return cancel::cancel_wheelbarrow_task(ctx, &data, commands);
                }
                sand_collect::spawn_loaded_bone_items(
                    commands,
                    data.wheelbarrow,
                    data.source_pos,
                    collect_amount,
                )
            }
            _ => {
                return cancel::cancel_wheelbarrow_task(ctx, &data, commands);
            }
        };
        if collected_items.is_empty() {
            return cancel::cancel_wheelbarrow_task(ctx, &data, commands);
        }

        if let ResourceSourceKey::Entity(source_entity) = source {
            sand_collect::clear_collect_source_designation(commands, source_entity);
        }
        reservation::release_source(ctx, source, 1);

        let loaded_count = collected_items.len();
        if let Some(dest_entity) = data.destination.stockpile_or_blueprint() {
            for &item in &collected_items {
                if let Ok(mut item_commands) = commands.get_entity(item) {
                    item_commands.try_insert(hw_core::relationships::DeliveringTo(dest_entity));
                }
            }
        }
        let mut next_data = data;
        next_data.items = collected_items;
        next_data.collect_source = None;
        next_data.collect_amount = 0;
        next_data.collect_resource_type = None;
        next_data.phase = HaulWithWheelbarrowPhase::GoingToDestination;
        *ctx.task = AssignedTask::HaulWithWheelbarrow(next_data);

        debug!(
            "WB_HAUL: Soul {:?} collected {} items into wheelbarrow",
            ctx.soul_entity, loaded_count
        );
        return TaskHandlerControl::Continue;
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
        debug!(
            "WB_HAUL: Soul {:?} found no loadable items, canceling",
            ctx.soul_entity
        );
        return cancel::cancel_wheelbarrow_task(ctx, &data, commands);
    }

    let mut loaded_entities = std::collections::HashSet::new();
    for (item_entity, stored_in_stockpile) in &items_to_load {
        release_mixer_mud_storage_for_item(ctx, *item_entity, commands);
        let Ok(mut item_commands) = commands.get_entity(*item_entity) else {
            continue;
        };
        item_commands.try_insert((Visibility::Hidden, LoadedIn(data.wheelbarrow)));
        item_commands.try_remove::<hw_core::relationships::StoredIn>();
        item_commands.try_remove::<hw_jobs::Designation>();
        item_commands.try_remove::<hw_jobs::TaskSlots>();
        item_commands.try_remove::<hw_jobs::Priority>();

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
            if !loaded_entities.contains(&item_entity)
                && let Ok(mut item_commands) = commands.get_entity(item_entity)
            {
                item_commands.try_remove::<hw_core::relationships::DeliveringTo>();
            }
        }
        debug!(
            "WB_HAUL: {} of {} items missing",
            total_count - loaded_count,
            total_count
        );
    }

    *ctx.task = AssignedTask::HaulWithWheelbarrow(HaulWithWheelbarrowData {
        phase: HaulWithWheelbarrowPhase::GoingToDestination,
        ..data
    });

    debug!(
        "WB_HAUL: Soul {:?} loaded {} items into wheelbarrow",
        ctx.soul_entity, loaded_count
    );

    TaskHandlerControl::Continue
}
