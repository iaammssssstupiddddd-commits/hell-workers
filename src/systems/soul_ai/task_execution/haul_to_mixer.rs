use bevy::prelude::*;
use super::context::TaskExecutionContext;
use super::types::{AssignedTask, HaulToMixerPhase};
use super::common::*;
use crate::systems::logistics::ResourceType;
use crate::world::map::WorldMap;
use crate::systems::familiar_ai::haul_cache::HaulReservationCache;

pub fn handle_haul_to_mixer_task(
    ctx: &mut TaskExecutionContext,
    item_entity: Entity,
    mixer_entity: Entity,
    resource_type: ResourceType,
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
                let is_full = storage.is_full(resource_type);
                if is_full {
                    info!("HAUL_TO_MIXER: Soul {:?} - mixer {:?} storage full for {:?}, canceling", ctx.soul_entity, mixer_entity, resource_type);
                    // 予約解除
                    haul_cache.release_mixer(mixer_entity, resource_type);
                    // アイテムのDesignationとTargetMixerを解除
                    commands.entity(item_entity).remove::<crate::systems::jobs::Designation>();
                    commands.entity(item_entity).remove::<crate::systems::jobs::TargetMixer>();
                    clear_task_and_path(ctx.task, ctx.path);
                    return;
                }
            } else {
                // ミキサーが存在しない
                info!("HAUL_TO_MIXER: Soul {:?} - mixer {:?} not found, canceling", ctx.soul_entity, mixer_entity);
                haul_cache.release_mixer(mixer_entity, resource_type);
                clear_task_and_path(ctx.task, ctx.path);
                return;
            }

            if let Ok((res_transform, _, _, _, des_opt, _)) = ctx.queries.targets.get(item_entity) {
                // 指定が解除されていたら中止
                if des_opt.is_none() {
                    info!("HAUL_TO_MIXER: Soul {:?} - item {:?} designation removed, canceling", ctx.soul_entity, item_entity);
                    haul_cache.release_mixer(mixer_entity, resource_type);
                    commands.entity(item_entity).remove::<crate::systems::jobs::TargetMixer>();
                    clear_task_and_path(ctx.task, ctx.path);
                    return;
                }

                let item_pos = res_transform.translation.truncate();
                // アイテムが障害物の上にある可能性があるため、隣接マスを目的地として設定
                let reachable = update_destination_to_adjacent(ctx.dest, item_pos, ctx.path, soul_pos, world_map, ctx.pf_context);

                if !reachable {
                    // 到達不能: タスクをキャンセル
                    info!("HAUL_TO_MIXER: Soul {:?} cannot reach item {:?}, canceling", ctx.soul_entity, item_entity);
                    haul_cache.release_mixer(mixer_entity, resource_type);
                    commands.entity(item_entity).remove::<crate::systems::jobs::Designation>();
                    commands.entity(item_entity).remove::<crate::systems::jobs::TargetMixer>();
                    clear_task_and_path(ctx.task, ctx.path);
                    return;
                }

                if is_near_target(soul_pos, item_pos) {
                    // アイテムを拾う（pickup_item が Designation, StoredIn などをクリア）
                    pickup_item(commands, ctx.soul_entity, item_entity, ctx.inventory);

                    *ctx.task = AssignedTask::HaulToMixer(crate::systems::soul_ai::task_execution::types::HaulToMixerData {
                        item: item_entity,
                        mixer: mixer_entity,
                        resource_type,
                        phase: HaulToMixerPhase::GoingToMixer,
                    });
                    ctx.path.waypoints.clear();
                }
            } else {
                info!("HAUL_TO_MIXER: Soul {:?} - item {:?} not found, canceling", ctx.soul_entity, item_entity);
                haul_cache.release_mixer(mixer_entity, resource_type);
                // アイテムが存在しない場合は削除できないが、念のためコマンドは発行しない（IDが無効なため）
                clear_task_and_path(ctx.task, ctx.path);
            }
        }


        HaulToMixerPhase::GoingToMixer => {
            // インベントリにアイテムがあるか確認
            if ctx.inventory.0 != Some(item_entity) {
                info!("HAUL_TO_MIXER: Soul {:?} - item not in inventory, canceling", ctx.soul_entity);
                haul_cache.release_mixer(mixer_entity, resource_type);
                clear_task_and_path(ctx.task, ctx.path);
                return;
            }

            if let Ok(mixer_data) = ctx.queries.mixers.get(mixer_entity) {
                let (mixer_transform, storage, _) = mixer_data;
                let mixer_pos = mixer_transform.translation.truncate();

                // ミキサーが満タンかチェック（移動中に満タンになる可能性）
                if storage.is_full(resource_type) {
                    // 満タン: 砂は無限にあるのでdespawn、それ以外はdrop
                    info!("HAUL_TO_MIXER: Mixer {:?} full for {:?}, disposing item", mixer_entity, resource_type);
                    haul_cache.release_mixer(mixer_entity, resource_type);

                    if resource_type == ResourceType::Sand {
                        commands.entity(item_entity).despawn();
                    } else {
                        commands.entity(item_entity).remove::<crate::systems::jobs::TargetMixer>();
                        drop_item(commands, ctx.soul_entity, item_entity, soul_pos);
                    }
                    ctx.inventory.0 = None;
                    clear_task_and_path(ctx.task, ctx.path);
                    return;
                }

                // 到達可能かチェック
                let reachable = update_destination_to_adjacent(ctx.dest, mixer_pos, ctx.path, soul_pos, world_map, ctx.pf_context);

                if !reachable {
                    // 到達不能: アイテムをドロップしてタスクをキャンセル
                    info!("HAUL_TO_MIXER: Soul {:?} cannot reach mixer {:?}, dropping item", ctx.soul_entity, mixer_entity);
                    haul_cache.release_mixer(mixer_entity, resource_type);
                    commands.entity(item_entity).remove::<crate::systems::jobs::TargetMixer>();
                    drop_item(commands, ctx.soul_entity, item_entity, soul_pos);
                    ctx.inventory.0 = None;
                    clear_task_and_path(ctx.task, ctx.path);
                    return;
                }

                if is_near_target(soul_pos, mixer_pos) || is_near_target(soul_pos, ctx.dest.0) {
                    *ctx.task = AssignedTask::HaulToMixer(crate::systems::soul_ai::task_execution::types::HaulToMixerData {
                        item: item_entity,
                        mixer: mixer_entity,
                        resource_type,
                        phase: HaulToMixerPhase::Delivering,
                    });
                    ctx.path.waypoints.clear();
                }

            } else {
                // ミキサーが消失した場合はアイテムをドロップして終了
                haul_cache.release_mixer(mixer_entity, resource_type);
                if let Some(item) = ctx.inventory.0 {
                    commands.entity(item).remove::<crate::systems::jobs::TargetMixer>();
                    drop_item(commands, ctx.soul_entity, item, soul_pos);
                    ctx.inventory.0 = None;
                }
                clear_task_and_path(ctx.task, ctx.path);
            }
        }

        HaulToMixerPhase::Delivering => {
            if let Ok(mixer_data) = ctx.queries.mixers.get_mut(mixer_entity) {
                let (_, mut storage, _) = mixer_data;
                let mut delivered = false;
                if storage.add_material(resource_type).is_ok() {
                    delivered = true;
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
            haul_cache.release_mixer(mixer_entity, resource_type);
            clear_task_and_path(ctx.task, ctx.path);
        }
    }
}


