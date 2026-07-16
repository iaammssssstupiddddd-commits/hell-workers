//! Floor tile pouring task execution

use crate::soul_ai::execute::task_execution::{
    common::*,
    context::{TaskExecutionContext, TaskHandlerControl},
    types::{AssignedTask, PourFloorPhase, PourFloorTileData},
};
use bevy::prelude::*;
use hw_core::constants::{FLOOR_MUD_PER_TILE, FLOOR_POUR_DURATION_SECS};
use hw_jobs::FloorTileState;
use hw_world::WorldMap;

pub fn handle_pour_floor_task(
    ctx: &mut TaskExecutionContext,
    data: PourFloorTileData,
    commands: &mut Commands,
) -> TaskHandlerControl {
    let PourFloorTileData { tile, site, phase } = data;
    let tile_entity = tile;
    let site_entity = site;
    let soul_pos = ctx.soul_pos();

    match phase {
        PourFloorPhase::GoingToMaterialCenter => {
            // Get site material center position
            let Ok((site_transform, _site, _)) = ctx.queries.storage.floor_sites.get(site_entity)
            else {
                // Site disappeared
                debug!(
                    "POUR_FLOOR: Cancelled for {:?} - Site {:?} gone",
                    ctx.soul_entity, site_entity
                );
                return ctx.abort_closed(commands, "construction cancelled");
            };

            let material_center = site_transform.translation.truncate();

            // Navigate to material center
            if matches!(
                update_task_destination_to_adjacent(ctx, material_center),
                PathSearchResult::Deferred
            ) {
                return TaskHandlerControl::Continue;
            }

            // Check if near material center (target or adjacent destination)
            if is_near_target_or_dest(soul_pos, material_center, ctx.dest.0) {
                *ctx.task = AssignedTask::PourFloorTile(PourFloorTileData {
                    tile: tile_entity,
                    site: site_entity,
                    phase: PourFloorPhase::PickingUpMud,
                });
                debug!(
                    "POUR_FLOOR: Soul {:?} arrived at material center",
                    ctx.soul_entity
                );
            }
        }

        PourFloorPhase::PickingUpMud => {
            let Ok((_, tile_blueprint, _)) = ctx.queries.storage.floor_tiles.get(tile_entity)
            else {
                debug!(
                    "POUR_FLOOR: Cancelled for {:?} - Tile {:?} gone",
                    ctx.soul_entity, tile_entity
                );
                return ctx.abort_closed(commands, "construction cancelled");
            };

            match tile_blueprint.state {
                FloorTileState::WaitingMud => {
                    // 素材待ち - 搬入完了を待機
                }
                FloorTileState::PouringReady => {
                    *ctx.task = AssignedTask::PourFloorTile(PourFloorTileData {
                        tile: tile_entity,
                        site: site_entity,
                        phase: PourFloorPhase::GoingToTile,
                    });
                    ctx.path.waypoints.clear();
                    debug!(
                        "POUR_FLOOR: Soul {:?} material ready, heading to tile {:?}",
                        ctx.soul_entity, tile_entity
                    );
                }
                _ => {
                    // 他のソウルが先に作業を開始または完了した → 中断
                    debug!(
                        "POUR_FLOOR: Cancelled for {:?} - Tile {:?} state changed unexpectedly in PickingUpMud",
                        ctx.soul_entity, tile_entity
                    );
                    return ctx.abort_retryable(commands, "tile state changed unexpectedly");
                }
            }
        }

        PourFloorPhase::GoingToTile => {
            // Get tile position
            let Ok((_, tile_blueprint, _)) = ctx.queries.storage.floor_tiles.get(tile_entity)
            else {
                // Tile disappeared
                debug!(
                    "POUR_FLOOR: Cancelled for {:?} - Tile {:?} gone",
                    ctx.soul_entity, tile_entity
                );
                return ctx.abort_closed(commands, "construction cancelled");
            };

            let tile_pos =
                WorldMap::grid_to_world(tile_blueprint.grid_pos.0, tile_blueprint.grid_pos.1);

            // Navigate to tile
            if matches!(
                update_task_destination_to_adjacent(ctx, tile_pos),
                PathSearchResult::Deferred
            ) {
                return TaskHandlerControl::Continue;
            }

            // Check if near tile (target or adjacent destination)
            if is_near_target_or_dest(soul_pos, tile_pos, ctx.dest.0) {
                *ctx.task = AssignedTask::PourFloorTile(PourFloorTileData {
                    tile: tile_entity,
                    site: site_entity,
                    phase: PourFloorPhase::Pouring { progress_bp: 0 },
                });
                ctx.path.waypoints.clear();
                debug!(
                    "POUR_FLOOR: Soul {:?} started pouring tile {:?}",
                    ctx.soul_entity, tile_entity
                );
            }
        }

        PourFloorPhase::Pouring { progress_bp } => {
            // Get tile and update state
            let Ok((_, mut tile_blueprint, _)) =
                ctx.queries.storage.floor_tiles.get_mut(tile_entity)
            else {
                debug!(
                    "POUR_FLOOR: Cancelled for {:?} - Tile {:?} gone",
                    ctx.soul_entity, tile_entity
                );
                return ctx.abort_closed(commands, "construction cancelled");
            };

            // Update progress (basis points) to avoid truncation at 1x speed.
            const MAX_PROGRESS_BP: u16 = 10_000;
            let delta_bp = ((ctx.env.time.delta_secs() / FLOOR_POUR_DURATION_SECS
                * MAX_PROGRESS_BP as f32)
                .round()
                .max(1.0)) as u16;
            let new_progress_bp = progress_bp.saturating_add(delta_bp).min(MAX_PROGRESS_BP);
            let visual_progress =
                ((new_progress_bp as f32 / MAX_PROGRESS_BP as f32) * 100.0).round() as u8;

            // Update tile visual state
            tile_blueprint.state = FloorTileState::Pouring {
                progress: visual_progress.min(100),
            };

            if new_progress_bp >= MAX_PROGRESS_BP {
                // Update tile state
                tile_blueprint.mud_delivered = tile_blueprint.mud_delivered.max(FLOOR_MUD_PER_TILE);
                tile_blueprint.state = FloorTileState::Complete;

                // Update site counter
                if let Ok((_, mut site, _)) = ctx.queries.storage.floor_sites.get_mut(site_entity) {
                    site.tiles_poured += 1;
                    debug!(
                        "POUR_FLOOR: Tile {:?} poured ({}/{})",
                        tile_entity, site.tiles_poured, site.tiles_total
                    );
                }

                // Add fatigue
                ctx.soul.fatigue = (ctx.soul.fatigue + 0.10).min(1.0);

                *ctx.task = AssignedTask::PourFloorTile(PourFloorTileData {
                    tile: tile_entity,
                    site: site_entity,
                    phase: PourFloorPhase::Done,
                });
                debug!(
                    "POUR_FLOOR: Soul {:?} completed pouring tile {:?}",
                    ctx.soul_entity, tile_entity
                );
            } else {
                *ctx.task = AssignedTask::PourFloorTile(PourFloorTileData {
                    tile: tile_entity,
                    site: site_entity,
                    phase: PourFloorPhase::Pouring {
                        progress_bp: new_progress_bp,
                    },
                });
            }
        }

        PourFloorPhase::Done => {
            // Release task slot (if needed, but usually handled by completion system)
            // For floor tiles, workers are assigned to a tile.
            // We should release the reservation here.
            ctx.queue_reservation(hw_core::events::ResourceReservationOp::ReleaseSource {
                source: tile_entity,
                amount: 1,
            });
            debug!(
                "POUR_FLOOR: Soul {:?} finished pouring task",
                ctx.soul_entity
            );
            return ctx.complete_task(commands, "construction done");
        }
    }

    TaskHandlerControl::Continue
}
