//! 収集タスクの実行処理

use crate::assets::GameAssets;
use crate::constants::*;
use crate::systems::jobs::{Designation, Rock, Tree, WorkType};
use crate::systems::logistics::ResourceItem;
use crate::systems::soul_ai::task_execution::{
    common::*,
    context::TaskExecutionContext,
    types::{AssignedTask, GatherPhase},
};
use crate::world::map::WorldMap;
use bevy::prelude::*;

pub fn handle_gather_task(
    ctx: &mut TaskExecutionContext,
    target: Entity,
    work_type: &WorkType,
    phase: GatherPhase,
    q_targets: &Query<(
        &Transform,
        Option<&Tree>,
        Option<&Rock>,
        Option<&ResourceItem>,
        Option<&Designation>,
        Option<&crate::relationships::StoredIn>,
    )>,
    commands: &mut Commands,
    game_assets: &Res<GameAssets>,
    time: &Res<Time>,
    world_map: &Res<WorldMap>,
) {
    let soul_pos = ctx.soul_pos();
    match phase {
        GatherPhase::GoingToResource => {
            if let Ok((res_transform, _, _, _, des_opt, _)) = q_targets.get(target) {
                // 指定が解除されていたら中止
                if cancel_task_if_designation_missing(des_opt, ctx.task, ctx.path) {
                    return;
                }
                let res_pos = res_transform.translation.truncate();
                update_destination_to_adjacent(ctx.dest, res_pos, ctx.path, soul_pos, world_map);

                if is_near_target(soul_pos, res_pos) {
                    *ctx.task = AssignedTask::Gather {
                        target,
                        work_type: *work_type,
                        phase: GatherPhase::Collecting { progress: 0.0 },
                    };
                    ctx.path.waypoints.clear();
                }
            } else {
                clear_task_and_path(ctx.task, ctx.path);
            }
        }
        GatherPhase::Collecting { mut progress } => {
            if let Ok(target_data) = q_targets.get(target) {
                let (res_transform, tree, rock, _res_item, des_opt, _stored_in) = target_data;
                // 指定が解除されていたら中止
                if cancel_task_if_designation_missing(des_opt, ctx.task, ctx.path) {
                    return;
                }
                let pos = res_transform.translation;

                // 進行度を更新（岩は2倍の時間がかかる）
                let speed = if rock.is_some() {
                    GATHER_SPEED_BASE * crate::constants::GATHER_SPEED_ROCK_MULTIPLIER
                } else {
                    GATHER_SPEED_BASE
                };
                progress += time.delta_secs() * speed;

                if progress >= 1.0 {
                    if tree.is_some() {
                        // 木1本 → Wood × WOOD_DROP_AMOUNT
                        for i in 0..crate::constants::WOOD_DROP_AMOUNT {
                            // タイルサイズ 32 なので、中心から ±16 以内に収める。余裕を見て ±12
                            let offset = Vec3::new((i as f32 - 2.0) * 6.0, 0.0, 0.0);
                            commands.spawn((
                                ResourceItem(crate::systems::logistics::ResourceType::Wood),
                                Sprite {
                                    image: game_assets.wood.clone(),
                                    custom_size: Some(Vec2::splat(TILE_SIZE * 0.5)),
                                    color: Color::srgb(0.5, 0.35, 0.05),
                                    ..default()
                                },
                                Transform::from_translation(pos + offset),
                            ));
                        }
                        info!("TASK_EXEC: Soul {:?} chopped a tree (dropped {} wood)", ctx.soul_entity, crate::constants::WOOD_DROP_AMOUNT);
                    } else if rock.is_some() {
                        // 岩1つ → Rock × ROCK_DROP_AMOUNT
                        for i in 0..crate::constants::ROCK_DROP_AMOUNT {
                            // タイルサイズ 32 なので、中心から ±16 以内に収める。余裕を見て ±12
                            let offset = Vec3::new(((i % 5) as f32 - 2.0) * 6.0, ((i / 5) as f32 - 0.5) * 6.0, 0.0);
                            commands.spawn((
                                ResourceItem(crate::systems::logistics::ResourceType::Rock),
                                Sprite {
                                    image: game_assets.rock.clone(),
                                    custom_size: Some(Vec2::splat(TILE_SIZE * 0.5)),
                                    ..default()
                                },
                                Transform::from_translation(pos + offset),
                            ));
                        }
                        info!("TASK_EXEC: Soul {:?} mined a rock (dropped {} rock)", ctx.soul_entity, crate::constants::ROCK_DROP_AMOUNT);
                    }
                    commands.entity(target).despawn();

                    *ctx.task = AssignedTask::Gather {
                        target,
                        work_type: *work_type,
                        phase: GatherPhase::Done,
                    };
                    ctx.soul.fatigue = (ctx.soul.fatigue + FATIGUE_GAIN_ON_COMPLETION).min(1.0);
                } else {
                    // 進捗を保存
                    *ctx.task = AssignedTask::Gather {
                        target,
                        work_type: *work_type,
                        phase: GatherPhase::Collecting { progress },
                    };
                }
            } else {
                clear_task_and_path(ctx.task, ctx.path);
            }
        }
        GatherPhase::Done => {
            clear_task_and_path(ctx.task, ctx.path);
        }
    }
}
