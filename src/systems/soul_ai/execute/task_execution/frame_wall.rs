//! Wall tile framing task execution

use crate::constants::{WALL_FRAME_DURATION_SECS, WALL_WOOD_PER_TILE};
use crate::relationships::WorkingOn;
use crate::systems::jobs::wall_construction::WallTileState;
use crate::systems::soul_ai::execute::task_execution::{
    common::*,
    context::TaskExecutionContext,
    types::{AssignedTask, FrameWallPhase, FrameWallTileData},
};
use crate::world::map::WorldMap;
use bevy::prelude::*;

pub fn handle_frame_wall_task(
    ctx: &mut TaskExecutionContext,
    tile_entity: Entity,
    site_entity: Entity,
    phase: FrameWallPhase,
    commands: &mut Commands,
    time: &Res<Time>,
    world_map: &Res<WorldMap>,
) {
    let soul_pos = ctx.soul_pos();

    match phase {
        FrameWallPhase::GoingToMaterialCenter => {
            let Ok((site_transform, _site, _)) = ctx.queries.storage.wall_sites.get(site_entity) else {
                clear_task_and_path(ctx.task, ctx.path);
                commands.entity(ctx.soul_entity).remove::<WorkingOn>();
                return;
            };

            let material_center = site_transform.translation.truncate();
            update_destination_to_adjacent(
                ctx.dest,
                material_center,
                ctx.path,
                soul_pos,
                world_map,
                ctx.pf_context,
            );

            if is_near_target_or_dest(soul_pos, material_center, ctx.dest.0) {
                *ctx.task = AssignedTask::FrameWallTile(FrameWallTileData {
                    tile: tile_entity,
                    site: site_entity,
                    phase: FrameWallPhase::PickingUpWood,
                });
            }
        }
        FrameWallPhase::PickingUpWood => {
            let Ok(tile_blueprint) = ctx.queries.storage.wall_tiles.get(tile_entity) else {
                clear_task_and_path(ctx.task, ctx.path);
                commands.entity(ctx.soul_entity).remove::<WorkingOn>();
                return;
            };

            if matches!(tile_blueprint.state, WallTileState::FramingReady) {
                *ctx.task = AssignedTask::FrameWallTile(FrameWallTileData {
                    tile: tile_entity,
                    site: site_entity,
                    phase: FrameWallPhase::GoingToTile,
                });
                ctx.path.waypoints.clear();
            }
        }
        FrameWallPhase::GoingToTile => {
            let Ok(tile_blueprint) = ctx.queries.storage.wall_tiles.get(tile_entity) else {
                clear_task_and_path(ctx.task, ctx.path);
                commands.entity(ctx.soul_entity).remove::<WorkingOn>();
                return;
            };

            let tile_pos = WorldMap::grid_to_world(tile_blueprint.grid_pos.0, tile_blueprint.grid_pos.1);
            update_destination_to_adjacent(
                ctx.dest,
                tile_pos,
                ctx.path,
                soul_pos,
                world_map,
                ctx.pf_context,
            );

            if is_near_target_or_dest(soul_pos, tile_pos, ctx.dest.0) {
                *ctx.task = AssignedTask::FrameWallTile(FrameWallTileData {
                    tile: tile_entity,
                    site: site_entity,
                    phase: FrameWallPhase::Framing { progress_bp: 0 },
                });
                ctx.path.waypoints.clear();
            }
        }
        FrameWallPhase::Framing { progress_bp } => {
            let Ok(mut tile_blueprint) = ctx.queries.storage.wall_tiles.get_mut(tile_entity) else {
                clear_task_and_path(ctx.task, ctx.path);
                commands.entity(ctx.soul_entity).remove::<WorkingOn>();
                return;
            };

            const MAX_PROGRESS_BP: u16 = 10_000;
            let delta_bp = ((time.delta_secs() / WALL_FRAME_DURATION_SECS * MAX_PROGRESS_BP as f32)
                .round()
                .max(1.0)) as u16;
            let new_progress_bp = progress_bp.saturating_add(delta_bp).min(MAX_PROGRESS_BP);
            let visual_progress =
                ((new_progress_bp as f32 / MAX_PROGRESS_BP as f32) * 100.0).round() as u8;

            tile_blueprint.state = WallTileState::Framing {
                progress: visual_progress.min(100),
            };

            if new_progress_bp >= MAX_PROGRESS_BP {
                tile_blueprint.wood_delivered = tile_blueprint.wood_delivered.max(WALL_WOOD_PER_TILE);
                tile_blueprint.state = WallTileState::FramedProvisional;

                if let Ok((_, mut site, _)) = ctx.queries.storage.wall_sites.get_mut(site_entity) {
                    site.tiles_framed += 1;
                }

                ctx.soul.fatigue = (ctx.soul.fatigue + 0.15).min(1.0);
                *ctx.task = AssignedTask::FrameWallTile(FrameWallTileData {
                    tile: tile_entity,
                    site: site_entity,
                    phase: FrameWallPhase::Done,
                });
            } else {
                *ctx.task = AssignedTask::FrameWallTile(FrameWallTileData {
                    tile: tile_entity,
                    site: site_entity,
                    phase: FrameWallPhase::Framing {
                        progress_bp: new_progress_bp,
                    },
                });
            }
        }
        FrameWallPhase::Done => {
            ctx.queue_reservation(crate::events::ResourceReservationOp::ReleaseSource {
                source: tile_entity,
                amount: 1,
            });
            commands.entity(ctx.soul_entity).remove::<WorkingOn>();
            clear_task_and_path(ctx.task, ctx.path);
        }
    }
}
