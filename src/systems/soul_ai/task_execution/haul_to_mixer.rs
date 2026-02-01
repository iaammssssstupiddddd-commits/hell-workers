use bevy::prelude::*;
use super::context::TaskExecutionContext;
use super::types::{AssignedTask, HaulToMixerPhase};
use super::common::*;
use crate::systems::logistics::ResourceType;
use crate::world::map::WorldMap;
use crate::constants::*;

pub fn handle_haul_to_mixer_task(
    ctx: &mut TaskExecutionContext,
    item_entity: Entity,
    mixer_entity: Entity,
    phase: HaulToMixerPhase,
    commands: &mut Commands,
    world_map: &Res<WorldMap>,
) {
    let soul_pos = ctx.soul_pos();
    
    match phase {
        HaulToMixerPhase::GoingToItem => {
            if let Ok((res_transform, _, _, _, des_opt, _)) = ctx.queries.targets.get(item_entity) {
                // 指定が解除されていたら中止
                if des_opt.is_none() {
                    clear_task_and_path(ctx.task, ctx.path);
                    return;
                }

                let item_pos = res_transform.translation.truncate();
                update_destination_if_needed(ctx.dest, item_pos, ctx.path);

                if is_near_target(soul_pos, item_pos) {
                    // アイテムを拾う（pickup_item が Designation, StoredIn などをクリア）
                    pickup_item(commands, ctx.soul_entity, item_entity, ctx.inventory);
                    
                    *ctx.task = AssignedTask::HaulToMixer(crate::systems::soul_ai::task_execution::types::HaulToMixerData {
                        item: item_entity,
                        mixer: mixer_entity,
                        phase: HaulToMixerPhase::GoingToMixer,
                    });
                    ctx.path.waypoints.clear();
                }
            } else {
                clear_task_and_path(ctx.task, ctx.path);
            }
        }
        HaulToMixerPhase::GoingToMixer => {
            if let Ok(mixer_data) = ctx.queries.mixers.get(mixer_entity) {
                let (mixer_transform, _, _) = mixer_data;
                let mixer_pos = mixer_transform.translation.truncate();
                update_destination_to_adjacent(ctx.dest, mixer_pos, ctx.path, soul_pos, world_map);

                if is_near_target(soul_pos, mixer_pos) {
                    *ctx.task = AssignedTask::HaulToMixer(crate::systems::soul_ai::task_execution::types::HaulToMixerData {
                        item: item_entity,
                        mixer: mixer_entity,
                        phase: HaulToMixerPhase::Delivering,
                    });
                    ctx.path.waypoints.clear();
                }
            } else {
                // ミキサーが消失した場合はアイテムをドロップして終了
                if let Some(item) = ctx.inventory.0 {
                    drop_item(commands, ctx.soul_entity, item, soul_pos);
                    ctx.inventory.0 = None;
                }
                clear_task_and_path(ctx.task, ctx.path);
            }
        }
        HaulToMixerPhase::Delivering => {
            if let Ok(mixer_data) = ctx.queries.mixers.get_mut(mixer_entity) {
                let (_, mut storage, _) = mixer_data;
                if let Ok(res_item) = ctx.queries.resources.get(item_entity) {
                    let mut delivered = false;
                    match res_item.0 {
                        ResourceType::Sand => {
                            if storage.sand < MUD_MIXER_CAPACITY {
                                storage.sand += 1;
                                delivered = true;
                            }
                        }
                        ResourceType::Rock => {
                            if storage.rock < MUD_MIXER_CAPACITY {
                                storage.rock += 1;
                                delivered = true;
                            }
                        }
                        _ => {}
                    }

                    if delivered {
                        commands.entity(item_entity).despawn();
                        ctx.inventory.0 = None;
                        info!("TASK_EXEC: Soul {:?} delivered {:?} to MudMixer", ctx.soul_entity, res_item.0);
                    } else {
                        // ストレージがいっぱいなら足元にドロップ
                        if let Some(item) = ctx.inventory.0 {
                            drop_item(commands, ctx.soul_entity, item, soul_pos);
                            ctx.inventory.0 = None;
                        }
                    }
                }
            } else {
                if let Some(item) = ctx.inventory.0 {
                    drop_item(commands, ctx.soul_entity, item, soul_pos);
                    ctx.inventory.0 = None;
                }
            }
            clear_task_and_path(ctx.task, ctx.path);
        }
    }
}
