//! Wall tile coating task execution

use crate::soul_ai::execute::task_execution::{
    common::*,
    context::{TaskExecutionContext, TaskHandlerControl},
    types::{AssignedTask, CoatWallData, CoatWallPhase},
};
use bevy::prelude::*;
use hw_core::constants::{FATIGUE_GAIN_ON_COMPLETION, WALL_COAT_DURATION_SECS, WALL_MUD_PER_TILE};
use hw_jobs::BuildingType;
use hw_jobs::WallTileState;
use hw_world::WorldMap;

fn cancel_coat_wall_task(
    ctx: &mut TaskExecutionContext,
    tile_entity: Entity,
    commands: &mut Commands,
    reason: &str,
) -> TaskHandlerControl {
    debug!(
        "COAT_WALL: Cancelled for {:?} - tile {:?} ({})",
        ctx.soul_entity, tile_entity, reason
    );
    ctx.abort_closed(commands, reason)
}

fn handle_legacy_coat_wall_task(
    ctx: &mut TaskExecutionContext,
    wall_entity: Entity,
    phase: CoatWallPhase,
    commands: &mut Commands,
) -> TaskHandlerControl {
    let soul_pos = ctx.soul_pos();

    match phase {
        CoatWallPhase::GoingToMaterialCenter | CoatWallPhase::GoingToTile => {
            let Ok((wall_transform, building, provisional_opt)) =
                ctx.queries.storage.buildings.get_mut(wall_entity)
            else {
                return cancel_coat_wall_task(ctx, wall_entity, commands, "legacy wall gone");
            };

            if building.kind != BuildingType::Wall
                || !building.is_provisional
                || provisional_opt.is_none_or(|provisional| !provisional.mud_delivered)
            {
                return cancel_coat_wall_task(ctx, wall_entity, commands, "legacy wall not ready");
            }

            let wall_pos = wall_transform.translation.truncate();
            let reachable = update_destination_to_adjacent(
                ctx.dest,
                wall_pos,
                ctx.path,
                soul_pos,
                ctx.env.world_map,
                ctx.pf_context,
            );
            if !reachable {
                return cancel_coat_wall_task(
                    ctx,
                    wall_entity,
                    commands,
                    "legacy wall unreachable",
                );
            }

            if is_near_target_or_dest(soul_pos, wall_pos, ctx.dest.0) {
                *ctx.task = AssignedTask::CoatWall(CoatWallData {
                    tile: wall_entity,
                    site: Entity::PLACEHOLDER,
                    wall: wall_entity,
                    phase: CoatWallPhase::PickingUpMud,
                });
                ctx.path.waypoints.clear();
            }
        }
        CoatWallPhase::PickingUpMud => {
            *ctx.task = AssignedTask::CoatWall(CoatWallData {
                tile: wall_entity,
                site: Entity::PLACEHOLDER,
                wall: wall_entity,
                phase: CoatWallPhase::Coating { progress_bp: 0 },
            });
            ctx.path.waypoints.clear();
        }
        CoatWallPhase::Coating { progress_bp } => {
            let Ok((_, mut building, provisional_opt)) =
                ctx.queries.storage.buildings.get_mut(wall_entity)
            else {
                return cancel_coat_wall_task(ctx, wall_entity, commands, "legacy wall gone");
            };

            if building.kind != BuildingType::Wall
                || !building.is_provisional
                || provisional_opt.is_none()
            {
                return cancel_coat_wall_task(
                    ctx,
                    wall_entity,
                    commands,
                    "legacy wall not provisional",
                );
            }

            const MAX_PROGRESS_BP: u16 = 10_000;
            let delta_bp = ((ctx.env.time.delta_secs() / WALL_COAT_DURATION_SECS
                * MAX_PROGRESS_BP as f32)
                .round()
                .max(1.0)) as u16;
            let new_progress_bp = progress_bp.saturating_add(delta_bp).min(MAX_PROGRESS_BP);

            if new_progress_bp >= MAX_PROGRESS_BP {
                building.is_provisional = false;
                commands
                    .entity(wall_entity)
                    .remove::<hw_jobs::ProvisionalWall>();
                ctx.soul.fatigue = (ctx.soul.fatigue + FATIGUE_GAIN_ON_COMPLETION).min(1.0);
                *ctx.task = AssignedTask::CoatWall(CoatWallData {
                    tile: wall_entity,
                    site: Entity::PLACEHOLDER,
                    wall: wall_entity,
                    phase: CoatWallPhase::Done,
                });
            } else {
                *ctx.task = AssignedTask::CoatWall(CoatWallData {
                    tile: wall_entity,
                    site: Entity::PLACEHOLDER,
                    wall: wall_entity,
                    phase: CoatWallPhase::Coating {
                        progress_bp: new_progress_bp,
                    },
                });
            }
        }
        CoatWallPhase::Done => {
            ctx.queue_reservation(hw_core::events::ResourceReservationOp::ReleaseSource {
                source: wall_entity,
                amount: 1,
            });
            return ctx.complete_task(commands, "legacy coat wall done");
        }
    }

    TaskHandlerControl::Continue
}

pub fn handle_coat_wall_task(
    ctx: &mut TaskExecutionContext,
    data: CoatWallData,
    commands: &mut Commands,
) -> TaskHandlerControl {
    let CoatWallData {
        tile,
        site,
        wall,
        phase,
    } = data;
    let tile_entity = tile;
    let site_entity = site;
    let wall_entity = wall;
    if site_entity == Entity::PLACEHOLDER {
        return handle_legacy_coat_wall_task(ctx, wall_entity, phase, commands);
    }

    let soul_pos = ctx.soul_pos();

    match phase {
        CoatWallPhase::GoingToMaterialCenter => {
            let Ok((site_transform, _site, _)) = ctx.queries.storage.wall_sites.get(site_entity)
            else {
                return cancel_coat_wall_task(ctx, tile_entity, commands, "site gone");
            };

            let material_center = site_transform.translation.truncate();
            let reachable = update_destination_to_adjacent(
                ctx.dest,
                material_center,
                ctx.path,
                soul_pos,
                ctx.env.world_map,
                ctx.pf_context,
            );
            if !reachable {
                return cancel_coat_wall_task(
                    ctx,
                    tile_entity,
                    commands,
                    "material center unreachable",
                );
            }

            if is_near_target_or_dest(soul_pos, material_center, ctx.dest.0) {
                *ctx.task = AssignedTask::CoatWall(CoatWallData {
                    tile: tile_entity,
                    site: site_entity,
                    wall: wall_entity,
                    phase: CoatWallPhase::PickingUpMud,
                });
                ctx.path.waypoints.clear();
            }
        }
        CoatWallPhase::PickingUpMud => {
            let Ok((_, tile_blueprint, _)) = ctx.queries.storage.wall_tiles.get(tile_entity) else {
                return cancel_coat_wall_task(ctx, tile_entity, commands, "tile gone");
            };
            let Some(actual_wall) = tile_blueprint.spawned_wall else {
                return cancel_coat_wall_task(ctx, tile_entity, commands, "spawned wall missing");
            };

            match tile_blueprint.state {
                WallTileState::WaitingMud => {
                    // 素材待ち - 搬入完了を待機
                }
                WallTileState::CoatingReady => {
                    *ctx.task = AssignedTask::CoatWall(CoatWallData {
                        tile: tile_entity,
                        site: site_entity,
                        wall: actual_wall,
                        phase: CoatWallPhase::GoingToTile,
                    });
                    ctx.path.waypoints.clear();
                }
                _ => {
                    // 他のソウルが先に作業を開始または完了した → 中断
                    return cancel_coat_wall_task(ctx, tile_entity, commands, "tile not coatable");
                }
            }
        }
        CoatWallPhase::GoingToTile => {
            let Ok((_, tile_blueprint, _)) = ctx.queries.storage.wall_tiles.get(tile_entity) else {
                return cancel_coat_wall_task(ctx, tile_entity, commands, "tile gone");
            };
            let Some(actual_wall) = tile_blueprint.spawned_wall else {
                return cancel_coat_wall_task(ctx, tile_entity, commands, "spawned wall missing");
            };
            if !matches!(
                tile_blueprint.state,
                WallTileState::CoatingReady | WallTileState::Coating { .. }
            ) {
                return cancel_coat_wall_task(ctx, tile_entity, commands, "tile not coatable");
            }

            let tile_pos =
                WorldMap::grid_to_world(tile_blueprint.grid_pos.0, tile_blueprint.grid_pos.1);
            let reachable = update_destination_to_adjacent(
                ctx.dest,
                tile_pos,
                ctx.path,
                soul_pos,
                ctx.env.world_map,
                ctx.pf_context,
            );
            if !reachable {
                return cancel_coat_wall_task(ctx, tile_entity, commands, "tile unreachable");
            }

            if is_near_target_or_dest(soul_pos, tile_pos, ctx.dest.0) {
                *ctx.task = AssignedTask::CoatWall(CoatWallData {
                    tile: tile_entity,
                    site: site_entity,
                    wall: actual_wall,
                    phase: CoatWallPhase::Coating { progress_bp: 0 },
                });
                ctx.path.waypoints.clear();
            }
        }
        CoatWallPhase::Coating { progress_bp } => {
            let Ok((_, mut tile_blueprint, _)) =
                ctx.queries.storage.wall_tiles.get_mut(tile_entity)
            else {
                return cancel_coat_wall_task(ctx, tile_entity, commands, "tile gone");
            };

            let Some(actual_wall) = tile_blueprint.spawned_wall else {
                return cancel_coat_wall_task(ctx, tile_entity, commands, "spawned wall missing");
            };

            let Ok((_, mut building, provisional_opt)) =
                ctx.queries.storage.buildings.get_mut(actual_wall)
            else {
                return cancel_coat_wall_task(ctx, tile_entity, commands, "wall gone");
            };

            if building.kind != BuildingType::Wall
                || !building.is_provisional
                || provisional_opt.is_none()
            {
                return cancel_coat_wall_task(ctx, tile_entity, commands, "wall not provisional");
            }

            const MAX_PROGRESS_BP: u16 = 10_000;
            let delta_bp = ((ctx.env.time.delta_secs() / WALL_COAT_DURATION_SECS
                * MAX_PROGRESS_BP as f32)
                .round()
                .max(1.0)) as u16;
            let new_progress_bp = progress_bp.saturating_add(delta_bp).min(MAX_PROGRESS_BP);
            let visual_progress =
                ((new_progress_bp as f32 / MAX_PROGRESS_BP as f32) * 100.0).round() as u8;

            tile_blueprint.state = WallTileState::Coating {
                progress: visual_progress.min(100),
            };

            if new_progress_bp >= MAX_PROGRESS_BP {
                tile_blueprint.mud_delivered = tile_blueprint.mud_delivered.max(WALL_MUD_PER_TILE);
                tile_blueprint.state = WallTileState::Complete;

                if let Ok((_, mut site, _)) = ctx.queries.storage.wall_sites.get_mut(site_entity) {
                    site.tiles_coated += 1;
                }

                building.is_provisional = false;
                commands
                    .entity(actual_wall)
                    .remove::<hw_jobs::ProvisionalWall>();

                ctx.soul.fatigue = (ctx.soul.fatigue + FATIGUE_GAIN_ON_COMPLETION).min(1.0);
                *ctx.task = AssignedTask::CoatWall(CoatWallData {
                    tile: tile_entity,
                    site: site_entity,
                    wall: actual_wall,
                    phase: CoatWallPhase::Done,
                });
            } else {
                *ctx.task = AssignedTask::CoatWall(CoatWallData {
                    tile: tile_entity,
                    site: site_entity,
                    wall: wall_entity,
                    phase: CoatWallPhase::Coating {
                        progress_bp: new_progress_bp,
                    },
                });
            }
        }
        CoatWallPhase::Done => {
            ctx.queue_reservation(hw_core::events::ResourceReservationOp::ReleaseSource {
                source: tile_entity,
                amount: 1,
            });
            return ctx.complete_task(commands, "coat wall done");
        }
    }

    TaskHandlerControl::Continue
}
