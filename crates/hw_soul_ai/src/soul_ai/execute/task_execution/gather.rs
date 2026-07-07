//! 収集タスクの実行処理

use crate::soul_ai::execute::task_execution::{
    chain::{self, GatherHaulChain},
    common::*,
    context::TaskExecutionContext,
    types::{
        AssignedTask, GatherData, GatherPhase, HaulData, HaulPhase, HaulToBlueprintData,
        HaulToBpPhase, HaulToMixerData, HaulToMixerPhase,
    },
};
use bevy::prelude::*;
use hw_core::constants::*;
use hw_core::relationships::WorkingOn;
use hw_core::visual::FadeOut;
use hw_jobs::{Designation, WorkType};
use hw_logistics::{ResourceItem, ResourceType};

pub fn handle_gather_task(
    ctx: &mut TaskExecutionContext,
    data: GatherData,
    commands: &mut Commands,
) {
    let GatherData {
        target,
        work_type,
        phase,
    } = data;
    let work_type = &work_type;
    let soul_pos = ctx.soul_pos();
    let q_targets = &ctx.queries.designation.targets;
    match phase {
        GatherPhase::GoingToResource => {
            let (res_pos, has_designation) = {
                let Ok((res_transform, _, _, _, _, des_opt, _)) = q_targets.get(target) else {
                    ctx.abort_closed(commands, "gather target entity missing");
                    return;
                };
                (res_transform.translation.truncate(), des_opt.is_some())
            };
            match navigate_to_adjacent(
                ctx,
                has_designation,
                res_pos,
                soul_pos,
                ctx.env.world_map,
                commands,
            ) {
                NavOutcome::Moving | NavOutcome::Cancelled => {}
                NavOutcome::Unreachable => {
                    debug!(
                        "GATHER: Soul {:?} cannot reach target {:?}, canceling",
                        ctx.soul_entity, target
                    );
                    ctx.queue_reservation(hw_core::events::ResourceReservationOp::ReleaseSource {
                        source: target,
                        amount: 1,
                    });
                    ctx.clear_soul_assignment(
                        commands,
                        crate::soul_ai::execute::task_execution::context::TaskEndDisposition::AbortedRetryable,
                    );
                }
                NavOutcome::Arrived => {
                    *ctx.task = AssignedTask::Gather(
                        crate::soul_ai::execute::task_execution::types::GatherData {
                            target,
                            work_type: *work_type,
                            phase: GatherPhase::Collecting { progress: 0.0 },
                        },
                    );
                    ctx.path.waypoints.clear();
                }
            }
        }

        GatherPhase::Collecting { mut progress } => {
            if let Ok(target_data) = q_targets.get(target) {
                let (res_transform, tree, tree_variant, rock, _res_item, des_opt, _stored_in) =
                    target_data;
                if des_opt.is_none() {
                    ctx.queue_reservation(hw_core::events::ResourceReservationOp::ReleaseSource {
                        source: target,
                        amount: 1,
                    });
                    ctx.abort_closed(commands, "designation missing");
                    return;
                }
                let pos = res_transform.translation;

                let speed = if rock.is_some() {
                    GATHER_SPEED_BASE * hw_core::constants::GATHER_SPEED_ROCK_MULTIPLIER
                } else {
                    GATHER_SPEED_BASE
                };
                progress += ctx.env.time.delta_secs() * speed;

                if progress >= 1.0 {
                    if tree.is_some() {
                        for i in 0..hw_core::constants::WOOD_DROP_AMOUNT {
                            let offset = Vec3::new((i as f32 - 2.0) * 6.0, 0.0, 0.0);
                            commands.spawn((
                                ResourceItem(hw_logistics::ResourceType::Wood),
                                Sprite {
                                    image: ctx.env.soul_handles.wood.clone(),
                                    custom_size: Some(Vec2::splat(TILE_SIZE * 0.5)),
                                    ..default()
                                },
                                Transform::from_translation(pos + offset),
                            ));
                        }
                        debug!(
                            "TASK_EXEC: Soul {:?} chopped a tree (dropped {} wood)",
                            ctx.soul_entity,
                            hw_core::constants::WOOD_DROP_AMOUNT
                        );

                        commands
                            .entity(target)
                            .remove::<hw_jobs::ObstaclePosition>();
                        commands.entity(target).remove::<hw_jobs::Tree>();
                        commands.entity(target).remove::<Designation>();

                        let variant_index = if let Some(variant) = tree_variant {
                            variant.0
                        } else {
                            0
                        };

                        if let Some(anime_image) = ctx.env.soul_handles.tree_animes.get(variant_index) {
                            commands.entity(target).insert(Sprite {
                                image: anime_image.clone(),
                                custom_size: Some(Vec2::splat(TILE_SIZE * 1.5)),
                                ..default()
                            });
                        }

                        commands.entity(target).insert(FadeOut { speed: 1.0 });
                    } else if rock.is_some() {
                        for i in 0..hw_core::constants::ROCK_DROP_AMOUNT {
                            let offset = Vec3::new(
                                ((i % 5) as f32 - 2.0) * 6.0,
                                ((i / 5) as f32 - 0.5) * 6.0,
                                0.0,
                            );
                            commands.spawn((
                                ResourceItem(hw_logistics::ResourceType::Rock),
                                Sprite {
                                    image: ctx.env.soul_handles.rock.clone(),
                                    custom_size: Some(Vec2::splat(TILE_SIZE * 0.5)),
                                    ..default()
                                },
                                Transform::from_translation(pos + offset),
                            ));
                        }
                        debug!(
                            "TASK_EXEC: Soul {:?} mined a rock (dropped {} rock)",
                            ctx.soul_entity,
                            hw_core::constants::ROCK_DROP_AMOUNT
                        );
                        commands
                            .entity(ctx.soul_entity)
                            .remove::<hw_core::relationships::WorkingOn>();
                        commands.entity(target).despawn();
                    } else {
                        commands
                            .entity(ctx.soul_entity)
                            .remove::<hw_core::relationships::WorkingOn>();
                        commands.entity(target).despawn();
                    }

                    ctx.queue_reservation(hw_core::events::ResourceReservationOp::ReleaseSource {
                        source: target,
                        amount: 1,
                    });

                    *ctx.task = AssignedTask::Gather(
                        crate::soul_ai::execute::task_execution::types::GatherData {
                            target,
                            work_type: *work_type,
                            phase: GatherPhase::Done,
                        },
                    );
                    ctx.soul.fatigue = (ctx.soul.fatigue + FATIGUE_GAIN_ON_COMPLETION).min(1.0);
                } else {
                    *ctx.task = AssignedTask::Gather(
                        crate::soul_ai::execute::task_execution::types::GatherData {
                            target,
                            work_type: *work_type,
                            phase: GatherPhase::Collecting { progress },
                        },
                    );
                }
            } else {
                ctx.abort_closed(commands, "gather target missing during collect");
            }
        }
        GatherPhase::Done => {
            let resource_type = match work_type {
                WorkType::Chop => Some(ResourceType::Wood),
                WorkType::Mine => Some(ResourceType::Rock),
                _ => None,
            };

            if let Some(resource_type) = resource_type
                && let Some(chain) =
                    chain::find_haul_chain_after_gather(resource_type, soul_pos, ctx)
            {
                commands.entity(ctx.soul_entity).remove::<WorkingOn>();
                ctx.path.waypoints.clear();
                match chain {
                    GatherHaulChain::Storage { item, destination } => {
                        commands.entity(ctx.soul_entity).insert(WorkingOn(item));
                        *ctx.task = AssignedTask::Haul(HaulData {
                            item,
                            stockpile: destination,
                            phase: HaulPhase::GoingToItem,
                        });
                        debug!(
                            "GATHER_CHAIN: Soul {:?} chained to haul {:?} ({:?}) to storage {:?}",
                            ctx.soul_entity, item, resource_type, destination
                        );
                    }
                    GatherHaulChain::Blueprint { item, blueprint } => {
                        commands.entity(ctx.soul_entity).insert(WorkingOn(item));
                        *ctx.task = AssignedTask::HaulToBlueprint(HaulToBlueprintData {
                            item,
                            blueprint,
                            phase: HaulToBpPhase::GoingToItem,
                        });
                        debug!(
                            "GATHER_CHAIN: Soul {:?} chained to haul {:?} ({:?}) to blueprint {:?}",
                            ctx.soul_entity, item, resource_type, blueprint
                        );
                    }
                    GatherHaulChain::Mixer { item, mixer } => {
                        commands.entity(ctx.soul_entity).insert(WorkingOn(item));
                        *ctx.task = AssignedTask::HaulToMixer(HaulToMixerData {
                            item,
                            mixer,
                            resource_type,
                            phase: HaulToMixerPhase::GoingToItem,
                        });
                        debug!(
                            "GATHER_CHAIN: Soul {:?} chained to haul {:?} ({:?}) to mixer {:?}",
                            ctx.soul_entity, item, resource_type, mixer
                        );
                    }
                }
                return;
            }

            ctx.complete_task(commands, "gather done without chain");
        }
    }
}
