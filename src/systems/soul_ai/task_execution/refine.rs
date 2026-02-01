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
                let reachable = update_destination_to_adjacent(ctx.dest, mixer_pos, ctx.path, soul_pos, world_map);
                
                if !reachable {
                    // 到達不能: タスクをキャンセル
                    info!("REFINE: Soul {:?} cannot reach mixer {:?}, canceling", ctx.soul_entity, mixer_entity);
                    clear_task_and_path(ctx.task, ctx.path);
                    return;
                }

                if is_near_target(soul_pos, mixer_pos) {
                    *ctx.task = AssignedTask::Refine(crate::systems::soul_ai::task_execution::types::RefineData {
                        mixer: mixer_entity,
                        phase: RefinePhase::Refining { progress: 0.0 },
                    });
                    ctx.path.waypoints.clear();
                }
            } else {
                clear_task_and_path(ctx.task, ctx.path);
            }
        }

        RefinePhase::Refining { mut progress } => {
            if let Ok(mixer_data) = ctx.queries.mixers.get_mut(mixer_entity) {
                let (mixer_transform, mut storage, _) = mixer_data;
                
                // 原料がまだあるか確認
                if storage.sand == 0 || storage.water == 0 || storage.rock == 0 {
                    info!("TASK_EXEC: Soul {:?} canceled refining due to lack of materials", ctx.soul_entity);
                    clear_task_and_path(ctx.task, ctx.path);
                    return;
                }

                // 精製速度は伐採と同じ GATHER_SPEED_BASE を使用
                progress += time.delta_secs() * GATHER_SPEED_BASE;

                if progress >= 1.0 {
                    // 原料消費
                    storage.sand -= 1;
                    storage.water -= 1;
                    storage.rock -= 1;

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
                clear_task_and_path(ctx.task, ctx.path);
            }
        }
        RefinePhase::Done => {
            clear_task_and_path(ctx.task, ctx.path);
        }
    }
}
