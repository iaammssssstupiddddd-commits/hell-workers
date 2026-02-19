//! Wall tile coating task execution

use crate::constants::{FATIGUE_GAIN_ON_COMPLETION, WALL_COAT_DURATION_SECS, WALL_MUD_PER_TILE};
use crate::relationships::WorkingOn;
use crate::systems::jobs::BuildingType;
use crate::systems::jobs::wall_construction::WallTileState;
use crate::systems::soul_ai::execute::task_execution::{
    common::*,
    context::TaskExecutionContext,
    types::{AssignedTask, CoatWallData, CoatWallPhase},
};
use crate::world::map::WorldMap;
use bevy::prelude::*;

fn cancel_coat_wall_task(
    ctx: &mut TaskExecutionContext,
    tile_entity: Entity,
    commands: &mut Commands,
    reason: &str,
) {
    info!(
        "COAT_WALL: Cancelled for {:?} - tile {:?} ({})",
        ctx.soul_entity, tile_entity, reason
    );
    ctx.queue_reservation(crate::events::ResourceReservationOp::ReleaseSource {
        source: tile_entity,
        amount: 1,
    });
    clear_task_and_path(ctx.task, ctx.path);
    commands.entity(ctx.soul_entity).remove::<WorkingOn>();
}

fn handle_legacy_coat_wall_task(
    ctx: &mut TaskExecutionContext,
    wall_entity: Entity,
    phase: CoatWallPhase,
    commands: &mut Commands,
    time: &Res<Time>,
    world_map: &Res<WorldMap>,
) {
    let soul_pos = ctx.soul_pos();

    match phase {
        CoatWallPhase::GoingToMaterialCenter | CoatWallPhase::GoingToTile => {
            let Ok((wall_transform, building, provisional_opt)) =
                ctx.queries.storage.buildings.get_mut(wall_entity)
            else {
                cancel_coat_wall_task(ctx, wall_entity, commands, "legacy wall gone");
                return;
            };

            if building.kind != BuildingType::Wall
                || !building.is_provisional
                || provisional_opt.is_none_or(|provisional| !provisional.mud_delivered)
            {
                cancel_coat_wall_task(ctx, wall_entity, commands, "legacy wall not ready");
                return;
            }

            let wall_pos = wall_transform.translation.truncate();
            let reachable = update_destination_to_adjacent(
                ctx.dest,
                wall_pos,
                ctx.path,
                soul_pos,
                world_map,
                ctx.pf_context,
            );
            if !reachable {
                cancel_coat_wall_task(ctx, wall_entity, commands, "legacy wall unreachable");
                return;
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
                cancel_coat_wall_task(ctx, wall_entity, commands, "legacy wall gone");
                return;
            };

            if building.kind != BuildingType::Wall || !building.is_provisional || provisional_opt.is_none() {
                cancel_coat_wall_task(ctx, wall_entity, commands, "legacy wall not provisional");
                return;
            }

            const MAX_PROGRESS_BP: u16 = 10_000;
            let delta_bp = ((time.delta_secs() / WALL_COAT_DURATION_SECS * MAX_PROGRESS_BP as f32)
                .round()
                .max(1.0)) as u16;
            let new_progress_bp = progress_bp.saturating_add(delta_bp).min(MAX_PROGRESS_BP);

            if new_progress_bp >= MAX_PROGRESS_BP {
                building.is_provisional = false;
                commands
                    .entity(wall_entity)
                    .remove::<crate::systems::jobs::ProvisionalWall>();
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
            ctx.queue_reservation(crate::events::ResourceReservationOp::ReleaseSource {
                source: wall_entity,
                amount: 1,
            });
            commands.entity(ctx.soul_entity).remove::<WorkingOn>();
            clear_task_and_path(ctx.task, ctx.path);
        }
    }
}

pub fn handle_coat_wall_task(
    ctx: &mut TaskExecutionContext,
    tile_entity: Entity,
    site_entity: Entity,
    wall_entity: Entity,
    phase: CoatWallPhase,
    commands: &mut Commands,
    time: &Res<Time>,
    world_map: &Res<WorldMap>,
) {
    if site_entity == Entity::PLACEHOLDER {
        handle_legacy_coat_wall_task(ctx, wall_entity, phase, commands, time, world_map);
        return;
    }

    let soul_pos = ctx.soul_pos();

    match phase {
        CoatWallPhase::GoingToMaterialCenter => {
            let Ok((site_transform, _site, _)) = ctx.queries.storage.wall_sites.get(site_entity) else {
                cancel_coat_wall_task(ctx, tile_entity, commands, "site gone");
                return;
            };

            let material_center = site_transform.translation.truncate();
            let reachable = update_destination_to_adjacent(
                ctx.dest,
                material_center,
                ctx.path,
                soul_pos,
                world_map,
                ctx.pf_context,
            );
            if !reachable {
                cancel_coat_wall_task(ctx, tile_entity, commands, "material center unreachable");
                return;
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
            let Ok(tile_blueprint) = ctx.queries.storage.wall_tiles.get(tile_entity) else {
                cancel_coat_wall_task(ctx, tile_entity, commands, "tile gone");
                return;
            };
            let Some(actual_wall) = tile_blueprint.spawned_wall else {
                cancel_coat_wall_task(ctx, tile_entity, commands, "spawned wall missing");
                return;
            };
            if !matches!(tile_blueprint.state, WallTileState::CoatingReady) {
                cancel_coat_wall_task(ctx, tile_entity, commands, "tile not ready");
                return;
            }

            *ctx.task = AssignedTask::CoatWall(CoatWallData {
                tile: tile_entity,
                site: site_entity,
                wall: actual_wall,
                phase: CoatWallPhase::GoingToTile,
            });
            ctx.path.waypoints.clear();
        }
        CoatWallPhase::GoingToTile => {
            let Ok(tile_blueprint) = ctx.queries.storage.wall_tiles.get(tile_entity) else {
                cancel_coat_wall_task(ctx, tile_entity, commands, "tile gone");
                return;
            };
            let Some(actual_wall) = tile_blueprint.spawned_wall else {
                cancel_coat_wall_task(ctx, tile_entity, commands, "spawned wall missing");
                return;
            };
            if !matches!(
                tile_blueprint.state,
                WallTileState::CoatingReady | WallTileState::Coating { .. }
            ) {
                cancel_coat_wall_task(ctx, tile_entity, commands, "tile not coatable");
                return;
            }

            let tile_pos = WorldMap::grid_to_world(tile_blueprint.grid_pos.0, tile_blueprint.grid_pos.1);
            let reachable = update_destination_to_adjacent(
                ctx.dest,
                tile_pos,
                ctx.path,
                soul_pos,
                world_map,
                ctx.pf_context,
            );
            if !reachable {
                cancel_coat_wall_task(ctx, tile_entity, commands, "tile unreachable");
                return;
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
            let Ok(mut tile_blueprint) = ctx.queries.storage.wall_tiles.get_mut(tile_entity) else {
                cancel_coat_wall_task(ctx, tile_entity, commands, "tile gone");
                return;
            };

            let Some(actual_wall) = tile_blueprint.spawned_wall else {
                cancel_coat_wall_task(ctx, tile_entity, commands, "spawned wall missing");
                return;
            };

            let Ok((_, mut building, provisional_opt)) =
                ctx.queries.storage.buildings.get_mut(actual_wall)
            else {
                cancel_coat_wall_task(ctx, tile_entity, commands, "wall gone");
                return;
            };

            if building.kind != BuildingType::Wall || !building.is_provisional || provisional_opt.is_none() {
                cancel_coat_wall_task(ctx, tile_entity, commands, "wall not provisional");
                return;
            }

            const MAX_PROGRESS_BP: u16 = 10_000;
            let delta_bp = ((time.delta_secs() / WALL_COAT_DURATION_SECS * MAX_PROGRESS_BP as f32)
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
                    .remove::<crate::systems::jobs::ProvisionalWall>();

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
            ctx.queue_reservation(crate::events::ResourceReservationOp::ReleaseSource {
                source: tile_entity,
                amount: 1,
            });
            commands.entity(ctx.soul_entity).remove::<WorkingOn>();
            clear_task_and_path(ctx.task, ctx.path);
        }
    }
}
