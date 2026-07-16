use super::common::*;
use super::context::{TaskExecutionContext, TaskHandlerControl};
use super::transport_common::{cancel, reservation};
use super::types::{AssignedTask, HaulToMixerData, HaulToMixerPhase};
use bevy::prelude::*;
use hw_logistics::ResourceType;

pub fn handle_haul_to_mixer_task(
    ctx: &mut TaskExecutionContext,
    data: HaulToMixerData,
    commands: &mut Commands,
) -> TaskHandlerControl {
    let HaulToMixerData {
        item,
        mixer,
        resource_type,
        phase,
    } = data;
    let item_entity = item;
    let mixer_entity = mixer;
    let soul_pos = ctx.soul_pos();

    match phase {
        HaulToMixerPhase::GoingToItem => {
            // ミキサーのストレージが満杯かチェック
            if let Ok(mixer_data) = ctx.queries.storage.mixers.get(mixer_entity) {
                let (_, storage, _) = mixer_data;
                let is_full = storage.is_full(resource_type);
                if is_full {
                    debug!(
                        "HAUL_TO_MIXER: Soul {:?} - mixer {:?} storage full for {:?}, canceling",
                        ctx.soul_entity, mixer_entity, resource_type
                    );
                    return cancel::cancel_haul_to_mixer_before_pickup(ctx, commands);
                }
            } else {
                debug!(
                    "HAUL_TO_MIXER: Soul {:?} - mixer {:?} not found, canceling",
                    ctx.soul_entity, mixer_entity
                );
                return cancel::cancel_haul_to_mixer_before_pickup(ctx, commands);
            }

            if let Ok((res_transform, _, _, _, _, _, _)) =
                ctx.queries.designation.targets.get(item_entity)
            {
                let item_pos = res_transform.translation.truncate();
                // アイテムが障害物の上にある可能性があるため、隣接マスを目的地として設定
                match update_task_destination_to_adjacent(ctx, item_pos) {
                    PathSearchResult::Found(()) => {}
                    PathSearchResult::Deferred => return TaskHandlerControl::Continue,
                    PathSearchResult::Unreachable => {
                        // 到達不能: タスクをキャンセル
                        debug!(
                            "HAUL_TO_MIXER: Soul {:?} cannot reach item {:?}, canceling",
                            ctx.soul_entity, item_entity
                        );
                        return cancel::cancel_haul_to_mixer_before_pickup(ctx, commands);
                    }
                }

                if can_pickup_item(soul_pos, item_pos) {
                    pickup_item(commands, ctx.soul_entity, item_entity, &mut ctx.inventory);

                    *ctx.task = AssignedTask::HaulToMixer(
                        crate::soul_ai::execute::task_execution::types::HaulToMixerData {
                            item: item_entity,
                            mixer: mixer_entity,
                            resource_type,
                            phase: HaulToMixerPhase::GoingToMixer,
                        },
                    );
                    ctx.path.waypoints.clear();

                    reservation::record_picked_source(ctx, item_entity, 1);
                }
            } else {
                debug!(
                    "HAUL_TO_MIXER: Soul {:?} - item {:?} not found, canceling",
                    ctx.soul_entity, item_entity
                );
                return cancel::cancel_haul_to_mixer_before_pickup(ctx, commands);
            }
        }

        HaulToMixerPhase::GoingToMixer => {
            // インベントリにアイテムがあるか確認
            if ctx.inventory.0 != Some(item_entity) {
                debug!(
                    "HAUL_TO_MIXER: Soul {:?} - item not in inventory, canceling",
                    ctx.soul_entity
                );
                return cancel::cancel_haul_to_mixer(ctx, commands);
            }

            if let Ok(mixer_data) = ctx.queries.storage.mixers.get(mixer_entity) {
                let (mixer_transform, storage, _) = mixer_data;
                let mixer_pos = mixer_transform.translation.truncate();

                // ミキサーが満タンかチェック（移動中に満タンになる可能性）
                if storage.is_full(resource_type) {
                    // 満タン: 砂は無限にあるのでdespawn、それ以外はdrop
                    debug!(
                        "HAUL_TO_MIXER: Mixer {:?} full for {:?}, disposing item",
                        mixer_entity, resource_type
                    );
                    if resource_type == ResourceType::Sand {
                        commands.entity(item_entity).despawn();
                    } else {
                        drop_item(commands, ctx.soul_entity, item_entity, soul_pos);
                        commands
                            .entity(item_entity)
                            .remove::<hw_core::relationships::DeliveringTo>();
                    }
                    ctx.inventory.0 = None;
                    return ctx.abort_retryable_after_custom_cleanup(
                        commands,
                        "haul to mixer destination full",
                    );
                }

                // 到達可能かチェック
                match update_task_destination_to_adjacent(ctx, mixer_pos) {
                    PathSearchResult::Found(()) => {}
                    PathSearchResult::Deferred => return TaskHandlerControl::Continue,
                    PathSearchResult::Unreachable => {
                        // 到達不能: アイテムをドロップしてタスクをキャンセル
                        debug!(
                            "HAUL_TO_MIXER: Soul {:?} cannot reach mixer {:?}, dropping item",
                            ctx.soul_entity, mixer_entity
                        );
                        drop_item(commands, ctx.soul_entity, item_entity, soul_pos);
                        commands
                            .entity(item_entity)
                            .remove::<hw_core::relationships::DeliveringTo>();
                        ctx.inventory.0 = None;
                        return ctx.abort_retryable_after_custom_cleanup(
                            commands,
                            "haul to mixer destination unreachable",
                        );
                    }
                }

                if is_near_target_or_dest(soul_pos, mixer_pos, ctx.dest.0) {
                    *ctx.task = AssignedTask::HaulToMixer(
                        crate::soul_ai::execute::task_execution::types::HaulToMixerData {
                            item: item_entity,
                            mixer: mixer_entity,
                            resource_type,
                            phase: HaulToMixerPhase::Delivering,
                        },
                    );
                    ctx.path.waypoints.clear();
                }
            } else {
                // ミキサーが消失した場合はアイテムをドロップして終了
                return ctx.abort_closed(commands, "haul to mixer target gone");
            }
        }

        HaulToMixerPhase::Delivering => {
            let mut delivered = false;
            if let Ok(mixer_data) = ctx.queries.storage.mixers.get_mut(mixer_entity) {
                let (_, mut storage, _) = mixer_data;
                if storage.add_material(resource_type) {
                    delivered = true;
                }

                if delivered {
                    commands.entity(item_entity).despawn();
                    ctx.inventory.0 = None;
                    debug!(
                        "TASK_EXEC: Soul {:?} delivered {:?} to MudMixer",
                        ctx.soul_entity, resource_type
                    );
                    reservation::release_mixer_destination(ctx, mixer_entity, resource_type);
                    return ctx.complete_task(commands, "haul to mixer delivered");
                }

                if let Some(item) = ctx.inventory.0 {
                    drop_item(commands, ctx.soul_entity, item, soul_pos);
                    commands
                        .entity(item)
                        .remove::<hw_core::relationships::DeliveringTo>();
                    ctx.inventory.0 = None;
                }
            } else if let Some(item) = ctx.inventory.0 {
                drop_item(commands, ctx.soul_entity, item, soul_pos);
                commands
                    .entity(item)
                    .remove::<hw_core::relationships::DeliveringTo>();
                ctx.inventory.0 = None;
            }
            return ctx
                .abort_retryable_after_custom_cleanup(commands, "haul to mixer delivery canceled");
        }
    }

    TaskHandlerControl::Continue
}
