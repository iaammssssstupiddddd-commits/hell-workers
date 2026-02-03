use bevy::prelude::*;
use super::context::TaskExecutionContext;
use super::types::{AssignedTask, RefinePhase};
use super::common::*;
use crate::systems::logistics::{ResourceItem, ResourceType};
use crate::world::map::WorldMap;
use crate::constants::*;

pub fn handle_refine_task(
    ctx: &mut TaskExecutionContext,
    mixer_entity: Entity,
    phase: RefinePhase,
    commands: &mut Commands,
    game_assets: &Res<crate::assets::GameAssets>,
    time: &Res<Time>,
    world_map: &Res<WorldMap>,
) {
    let soul_pos = ctx.soul_pos();

    match phase {
        RefinePhase::GoingToMixer => {
            if let Ok(mixer_data) = ctx.queries.mixers.get(mixer_entity) {
                let (mixer_transform, _, _) = mixer_data;
                let mixer_pos = mixer_transform.translation.truncate();
                
                // 到達可能かチェック
                let reachable = update_destination_to_adjacent(ctx.dest, mixer_pos, ctx.path, soul_pos, world_map, ctx.pf_context);
                
                if !reachable {
                    // 到達不能: タスクをキャンセル
                    info!("REFINE: Soul {:?} cannot reach mixer {:?}, canceling", ctx.soul_entity, mixer_entity);
                    commands.entity(mixer_entity).remove::<crate::systems::jobs::Designation>();
                    commands.entity(mixer_entity).remove::<crate::systems::jobs::TaskSlots>();
                    clear_task_and_path(ctx.task, ctx.path);
                    return;
                }

                if is_near_target_or_dest(soul_pos, mixer_pos, ctx.dest.0) {
                    *ctx.task = AssignedTask::Refine(crate::systems::soul_ai::task_execution::types::RefineData {
                        mixer: mixer_entity,
                        phase: RefinePhase::Refining { progress: 0.0 },
                    });
                    ctx.path.waypoints.clear();
                }
            } else {
                // Mixer が存在しない場合も Designation を削除
                commands.entity(mixer_entity).remove::<crate::systems::jobs::Designation>();
                commands.entity(mixer_entity).remove::<crate::systems::jobs::TaskSlots>();
                clear_task_and_path(ctx.task, ctx.path);
            }
        }

        RefinePhase::Refining { mut progress } => {
            if let Ok(mixer_data) = ctx.queries.mixers.get_mut(mixer_entity) {
                let (mixer_transform, mut storage, _) = mixer_data;
                let water_count = match ctx.queries.stockpiles.get(mixer_entity) {
                    Ok((_, _, stockpile, Some(stored_items))) if stockpile.resource_type == Some(ResourceType::Water) => {
                        stored_items.len() as u32
                    }
                    _ => 0,
                };
                
                // 原料がまだあるか確認
                if !storage.has_materials_for_refining(water_count) {
                    info!("TASK_EXEC: Soul {:?} canceled refining due to lack of materials", ctx.soul_entity);
                    commands.entity(mixer_entity).remove::<crate::systems::jobs::Designation>();
                    commands.entity(mixer_entity).remove::<crate::systems::jobs::TaskSlots>();
                    clear_task_and_path(ctx.task, ctx.path);
                    return;
                }

                // 精製速度は伐採と同じ GATHER_SPEED_BASE を使用
                progress += time.delta_secs() * GATHER_SPEED_BASE;

                if progress >= 1.0 {
                    // 原料消費
                    let _ = storage.consume_materials_for_refining(water_count);
                    if let Some(water_entity) = ctx
                        .queries
                        .resource_items
                        .iter()
                        .find_map(|(res_entity, res_item, stored_in)| {
                            if res_item.0 == ResourceType::Water && stored_in.map(|s| s.0) == Some(mixer_entity) {
                                Some(res_entity)
                            } else {
                                None
                            }
                        })
                    {
                        commands.entity(water_entity).despawn();
                    } else {
                        warn!(
                            "TASK_EXEC: Soul {:?} could not find water item in mixer {:?} during refine",
                            ctx.soul_entity, mixer_entity
                        );
                    }

                    // StasisMud をドロップ（Stockpile オートホールで自動的に運搬される）
                    let pos = mixer_transform.translation;
                    for i in 0..STASIS_MUD_OUTPUT {
                        let offset = Vec3::new(((i % 3) as f32 - 1.0) * 8.0, ((i / 3) as f32) * 8.0, 0.0);
                        commands.spawn((
                            ResourceItem(ResourceType::StasisMud),
                            Sprite {
                                image: game_assets.icon_stasis_mud_small.clone(),
                                custom_size: Some(Vec2::splat(TILE_SIZE * 0.5)),
                                ..default()
                            },
                            Transform::from_translation(pos.truncate().extend(Z_ITEM_PICKUP) + offset),
                            Name::new("Item (StasisMud)"),
                        ));
                    }

                    info!("TASK_EXEC: Soul {:?} refined 5 StasisMud", ctx.soul_entity);

                    *ctx.task = AssignedTask::Refine(crate::systems::soul_ai::task_execution::types::RefineData {
                        mixer: mixer_entity,
                        phase: RefinePhase::Done,
                    });
                    ctx.soul.fatigue = (ctx.soul.fatigue + FATIGUE_GAIN_ON_COMPLETION).min(1.0);
                } else {
                    *ctx.task = AssignedTask::Refine(crate::systems::soul_ai::task_execution::types::RefineData {
                        mixer: mixer_entity,
                        phase: RefinePhase::Refining { progress },
                    });
                }
            } else {
                // Mixer が存在しない場合も Designation を削除
                commands.entity(mixer_entity).remove::<crate::systems::jobs::Designation>();
                commands.entity(mixer_entity).remove::<crate::systems::jobs::TaskSlots>();
                clear_task_and_path(ctx.task, ctx.path);
            }
        }
        RefinePhase::Done => {
            // 精製完了時に Designation を削除（次回必要なときに再発行される）
            commands.entity(mixer_entity).remove::<crate::systems::jobs::Designation>();
            commands.entity(mixer_entity).remove::<crate::systems::jobs::TaskSlots>();
            commands.entity(mixer_entity).remove::<crate::systems::jobs::IssuedBy>();
            clear_task_and_path(ctx.task, ctx.path);
        }
    }
}
