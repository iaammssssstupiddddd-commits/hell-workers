use super::common::*;
use super::context::TaskExecutionContext;
use super::types::{AssignedTask, CollectSandPhase};
use crate::assets::GameAssets;
use crate::constants::*;
use crate::systems::logistics::{ResourceItem, ResourceType};
use crate::world::map::WorldMap;
use bevy::prelude::*;

pub fn handle_collect_sand_task(
    ctx: &mut TaskExecutionContext,
    target: Entity,
    phase: CollectSandPhase,
    commands: &mut Commands,
    game_assets: &Res<GameAssets>,
    time: &Res<Time>,
    world_map: &Res<WorldMap>,
) {
    let soul_pos = ctx.soul_pos();
    let q_targets = &ctx.queries.targets;

    match phase {
        CollectSandPhase::GoingToSand => {
            if let Ok((res_transform, _, _, _, des_opt, _)) = q_targets.get(target) {
                // 指定が解除されていたら中止
                if cancel_task_if_designation_missing(des_opt, ctx.task, ctx.path) {
                    return;
                }
                let res_pos = res_transform.translation.truncate();
                let reachable = update_destination_to_adjacent(ctx.dest, res_pos, ctx.path, soul_pos, world_map, ctx.pf_context);

                if !reachable {
                    // 到達不能: タスクをキャンセル
                    info!("COLLECT_SAND: Soul {:?} cannot reach SandPile {:?}, canceling", ctx.soul_entity, target);
                    commands.entity(target).remove::<crate::systems::jobs::Designation>();
                    commands.entity(target).remove::<crate::systems::jobs::TaskSlots>();
                    ctx.queries.resource_cache.release_source(target, 1);
                    clear_task_and_path(ctx.task, ctx.path);
                    return;
                }

                if is_near_target(soul_pos, res_pos) {
                    *ctx.task = AssignedTask::CollectSand(crate::systems::soul_ai::task_execution::types::CollectSandData {
                        target,
                        phase: CollectSandPhase::Collecting { progress: 0.0 },
                    });
                    ctx.path.waypoints.clear();
                }
            } else {
                // SandPile が存在しない場合も Designation を削除
                commands.entity(target).remove::<crate::systems::jobs::Designation>();
                commands.entity(target).remove::<crate::systems::jobs::TaskSlots>();
                ctx.queries.resource_cache.release_source(target, 1);
                clear_task_and_path(ctx.task, ctx.path);
            }
        }
        CollectSandPhase::Collecting { mut progress } => {
            if let Ok(target_data) = q_targets.get(target) {
                let (res_transform, _, _, _, des_opt, _) = target_data;
                // 指定が解除されていたら中止
                if cancel_task_if_designation_missing(des_opt, ctx.task, ctx.path) {
                    return;
                }
                
                // 進行度を更新
                progress += time.delta_secs() * GATHER_SPEED_BASE;

                if progress >= 1.0 {
                    // Sand をドロップ（MudMixer オートホールで自動的に運搬される）
                    let pos = res_transform.translation;
                    for i in 0..SAND_DROP_AMOUNT {
                        // 少しオフセットをつけて spawn
                        let offset = Vec3::new((i as f32) * 4.0, 0.0, 0.0);
                        commands.spawn((
                            ResourceItem(ResourceType::Sand),
                            Sprite {
                                image: game_assets.icon_sand_small.clone(),
                                custom_size: Some(Vec2::splat(TILE_SIZE * 0.5)),
                                ..default()
                            },
                            Transform::from_translation(pos.truncate().extend(Z_ITEM_PICKUP) + offset),
                            Name::new("Item (Sand)"),
                        ));
                    }
                    
                    info!("TASK_EXEC: Soul {:?} collected sand", ctx.soul_entity);

                    *ctx.task = AssignedTask::CollectSand(crate::systems::soul_ai::task_execution::types::CollectSandData {
                        target,
                        phase: CollectSandPhase::Done,
                    });
                    ctx.soul.fatigue = (ctx.soul.fatigue + FATIGUE_GAIN_ON_COMPLETION).min(1.0);
                } else {
                    // 進捗を保存
                    *ctx.task = AssignedTask::CollectSand(crate::systems::soul_ai::task_execution::types::CollectSandData {
                        target,
                        phase: CollectSandPhase::Collecting { progress },
                    });
                }
            } else {
                // SandPile が存在しない場合も Designation を削除
                commands.entity(target).remove::<crate::systems::jobs::Designation>();
                commands.entity(target).remove::<crate::systems::jobs::TaskSlots>();
                ctx.queries.resource_cache.release_source(target, 1);
                clear_task_and_path(ctx.task, ctx.path);
            }
        }
        CollectSandPhase::Done => {
            // SandPile の Designation を削除（次回必要なときに再発行される）
            commands.entity(target).remove::<crate::systems::jobs::Designation>();
            commands.entity(target).remove::<crate::systems::jobs::TaskSlots>();
            commands.entity(target).remove::<crate::systems::jobs::IssuedBy>();
            ctx.queries.resource_cache.release_source(target, 1);
            clear_task_and_path(ctx.task, ctx.path);
        }
    }
}
