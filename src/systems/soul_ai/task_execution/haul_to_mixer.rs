use bevy::prelude::*;
use super::context::TaskExecutionContext;
use super::types::{AssignedTask, HaulToMixerPhase};
use super::common::*;
use crate::systems::logistics::ResourceType;
use crate::world::map::WorldMap;
use crate::constants::*;
use crate::systems::familiar_ai::haul_cache::HaulReservationCache;

pub fn handle_haul_to_mixer_task(
    ctx: &mut TaskExecutionContext,
    item_entity: Entity,
    mixer_entity: Entity,
    phase: HaulToMixerPhase,
    commands: &mut Commands,
    haul_cache: &mut HaulReservationCache,
    world_map: &Res<WorldMap>,
) {

    let soul_pos = ctx.soul_pos();
    
    match phase {
        HaulToMixerPhase::GoingToItem => {
            // ミキサーのストレージが満杯かチェック
            if let Ok(mixer_data) = ctx.queries.mixers.get(mixer_entity) {
                let (_, storage, _) = mixer_data;
                if let Ok(res_item) = ctx.queries.resources.get(item_entity) {
                    let current = match res_item.0 {
                        ResourceType::Sand => storage.sand,
                        ResourceType::Rock => storage.rock,
                        _ => 0,
                    };
                    if current >= MUD_MIXER_CAPACITY {
                        info!("HAUL_TO_MIXER: Soul {:?} - mixer {:?} storage full for {:?}, canceling", ctx.soul_entity, mixer_entity, res_item.0);
                        // 予約解除
                        haul_cache.release_mixer(mixer_entity, res_item.0);
                        // アイテムのDesignationを解除
                        commands.entity(item_entity).remove::<crate::systems::jobs::Designation>();
                        clear_task_and_path(ctx.task, ctx.path);
                        return;
                    }
                }
            } else {
                // ミキサーが存在しない
                info!("HAUL_TO_MIXER: Soul {:?} - mixer {:?} not found, canceling", ctx.soul_entity, mixer_entity);
                // リソースタイプが不明なため、予約解除できない（アイテムが残っていれば解除したいが...）
                if let Ok(res_item) = ctx.queries.resources.get(item_entity) {
                    haul_cache.release_mixer(mixer_entity, res_item.0);
                }
                clear_task_and_path(ctx.task, ctx.path);
                return;
            }

            if let Ok((res_transform, _, _, _, des_opt, _)) = ctx.queries.targets.get(item_entity) {
                // 指定が解除されていたら中止
                if des_opt.is_none() {
                    info!("HAUL_TO_MIXER: Soul {:?} - item {:?} designation removed, canceling", ctx.soul_entity, item_entity);
                    if let Ok(res_item) = ctx.queries.resources.get(item_entity) {
                        haul_cache.release_mixer(mixer_entity, res_item.0);
                    }
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
                info!("HAUL_TO_MIXER: Soul {:?} - item {:?} not found, canceling", ctx.soul_entity, item_entity);
                // アイテムがないのでリソースタイプ不明。予約解除できない可能性があるが、アイテム消失時に別途処理が必要か
                clear_task_and_path(ctx.task, ctx.path);
            }
        }


        HaulToMixerPhase::GoingToMixer => {
            // インベントリにアイテムがあるか確認
            if ctx.inventory.0 != Some(item_entity) {
                info!("HAUL_TO_MIXER: Soul {:?} - item not in inventory, canceling", ctx.soul_entity);
                // アイテム紛失？予約解除できない
                clear_task_and_path(ctx.task, ctx.path);
                return;
            }
            
            // 手持ちアイテムのリソースタイプを取得
            let resource_type = if let Ok(res) = ctx.queries.resources.get(item_entity) {
                Some(res.0)
            } else {
                None 
            };

            if let Ok(mixer_data) = ctx.queries.mixers.get(mixer_entity) {
                let (mixer_transform, storage, _) = mixer_data;
                let mixer_pos = mixer_transform.translation.truncate();

                if let Some(res_type) = resource_type {
                    // ミキサーが満タンかチェック（移動中に満タンになる可能性）
                    let current = match res_type {
                        ResourceType::Sand => storage.sand,
                        ResourceType::Rock => storage.rock,
                        _ => 0,
                    };
                    if current >= MUD_MIXER_CAPACITY {
                        // 満タン: 砂は無限にあるのでdespawn、それ以外はdrop
                        info!("HAUL_TO_MIXER: Mixer {:?} full for {:?}, disposing item", mixer_entity, res_type);
                        haul_cache.release_mixer(mixer_entity, res_type);
                        
                        if res_type == ResourceType::Sand {
                            commands.entity(item_entity).despawn();
                        } else {
                            drop_item(commands, ctx.soul_entity, item_entity, soul_pos);
                        }
                        ctx.inventory.0 = None;
                        clear_task_and_path(ctx.task, ctx.path);
                        return;
                    }
                }
                
                // 到達可能かチェック
                let reachable = update_destination_to_adjacent(ctx.dest, mixer_pos, ctx.path, soul_pos, world_map);
                
                if !reachable {
                    // 到達不能: アイテムをドロップしてタスクをキャンセル
                    info!("HAUL_TO_MIXER: Soul {:?} cannot reach mixer {:?}, dropping item", ctx.soul_entity, mixer_entity);
                    if let Some(res_type) = resource_type {
                        haul_cache.release_mixer(mixer_entity, res_type);
                    }
                    drop_item(commands, ctx.soul_entity, item_entity, soul_pos);
                    ctx.inventory.0 = None;
                    clear_task_and_path(ctx.task, ctx.path);
                    return;
                }

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
                if let Some(res_type) = resource_type {
                    haul_cache.release_mixer(mixer_entity, res_type);
                }
                if let Some(item) = ctx.inventory.0 {
                    drop_item(commands, ctx.soul_entity, item, soul_pos);
                    ctx.inventory.0 = None;
                }
                clear_task_and_path(ctx.task, ctx.path);
            }
        }

        HaulToMixerPhase::Delivering => {
            // 手持ちアイテムのリソースタイプを取得
            let resource_type = if let Ok(res) = ctx.queries.resources.get(item_entity) {
                Some(res.0)
            } else {
                None
            };

            
            if let Ok(mixer_data) = ctx.queries.mixers.get_mut(mixer_entity) {
                let (_, mut storage, _) = mixer_data;
                let mut delivered = false;
                match resource_type {
                    Some(ResourceType::Sand) => {
                        if storage.sand < MUD_MIXER_CAPACITY {
                            storage.sand += 1;
                            delivered = true;
                        }
                    }
                    Some(ResourceType::Rock) => {
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
                    info!("TASK_EXEC: Soul {:?} delivered {:?} to MudMixer", ctx.soul_entity, resource_type);
                } else {
                    // ストレージがいっぱいなら足元にドロップ
                    if let Some(item) = ctx.inventory.0 {
                        drop_item(commands, ctx.soul_entity, item, soul_pos);
                        ctx.inventory.0 = None;
                    }
                }
            } else {
                if let Some(item) = ctx.inventory.0 {
                    drop_item(commands, ctx.soul_entity, item, soul_pos);
                    ctx.inventory.0 = None;
                }
            }
            // 完了したので予約解除
            if let Some(res_type) = resource_type {
                haul_cache.release_mixer(mixer_entity, res_type);
            }
            clear_task_and_path(ctx.task, ctx.path);
        }
    }
}


