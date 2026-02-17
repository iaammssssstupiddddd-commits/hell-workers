//! Floor tile reinforcement task execution

use crate::constants::{FLOOR_BONES_PER_TILE, FLOOR_REINFORCE_DURATION_SECS};
use crate::relationships::WorkingOn;
use crate::systems::jobs::floor_construction::FloorTileState;
use crate::systems::soul_ai::execute::task_execution::{
    common::*,
    context::TaskExecutionContext,
    types::{AssignedTask, ReinforceFloorPhase},
};
use crate::world::map::WorldMap;
use bevy::prelude::*;

pub fn handle_reinforce_floor_task(
    ctx: &mut TaskExecutionContext,
    tile_entity: Entity,
    site_entity: Entity,
    phase: ReinforceFloorPhase,
    commands: &mut Commands,
    time: &Res<Time>,
    world_map: &Res<WorldMap>,
) {
    let soul_pos = ctx.soul_pos();

    match phase {
        ReinforceFloorPhase::GoingToMaterialCenter => {
            // Get site material center position
            let Ok((site_transform, _site, _)) = ctx.queries.storage.floor_sites.get(site_entity)
            else {
                // Site disappeared
                info!(
                    "REINFORCE_FLOOR: Cancelled for {:?} - Site {:?} gone",
                    ctx.soul_entity, site_entity
                );
                clear_task_and_path(ctx.task, ctx.path);
                commands.entity(ctx.soul_entity).remove::<WorkingOn>();
                return;
            };

            let material_center = site_transform.translation.truncate();

            // Navigate to material center
            update_destination_to_adjacent(
                ctx.dest,
                material_center,
                ctx.path,
                soul_pos,
                world_map,
                ctx.pf_context,
            );

            // Check if near material center (target or adjacent destination)
            if is_near_target_or_dest(soul_pos, material_center, ctx.dest.0) {
                *ctx.task = AssignedTask::ReinforceFloorTile(
                    crate::systems::soul_ai::execute::task_execution::types::ReinforceFloorTileData {
                        tile: tile_entity,
                        site: site_entity,
                        phase: ReinforceFloorPhase::PickingUpBones,
                    },
                );
                info!(
                    "REINFORCE_FLOOR: Soul {:?} arrived at material center",
                    ctx.soul_entity
                );
            }
        }

        ReinforceFloorPhase::PickingUpBones => {
            let Ok(tile_blueprint) = ctx.queries.storage.floor_tiles.get(tile_entity) else {
                info!(
                    "REINFORCE_FLOOR: Cancelled for {:?} - Tile {:?} gone",
                    ctx.soul_entity, tile_entity
                );
                clear_task_and_path(ctx.task, ctx.path);
                commands.entity(ctx.soul_entity).remove::<WorkingOn>();
                return;
            };

            if matches!(tile_blueprint.state, FloorTileState::ReinforcingReady) {
                *ctx.task = AssignedTask::ReinforceFloorTile(
                    crate::systems::soul_ai::execute::task_execution::types::ReinforceFloorTileData {
                        tile: tile_entity,
                        site: site_entity,
                        phase: ReinforceFloorPhase::GoingToTile,
                    },
                );
                ctx.path.waypoints.clear();
                info!(
                    "REINFORCE_FLOOR: Soul {:?} material ready, heading to tile {:?}",
                    ctx.soul_entity, tile_entity
                );
            }
        }

        ReinforceFloorPhase::GoingToTile => {
            // Get tile position
            let Ok(tile_blueprint) = ctx.queries.storage.floor_tiles.get(tile_entity) else {
                // Tile disappeared
                info!(
                    "REINFORCE_FLOOR: Cancelled for {:?} - Tile {:?} gone",
                    ctx.soul_entity, tile_entity
                );
                clear_task_and_path(ctx.task, ctx.path);
                commands.entity(ctx.soul_entity).remove::<WorkingOn>();
                return;
            };

            let tile_pos =
                WorldMap::grid_to_world(tile_blueprint.grid_pos.0, tile_blueprint.grid_pos.1);

            // Navigate to tile
            update_destination_to_adjacent(
                ctx.dest,
                tile_pos,
                ctx.path,
                soul_pos,
                world_map,
                ctx.pf_context,
            );

            // Check if near tile (target or adjacent destination)
            if is_near_target_or_dest(soul_pos, tile_pos, ctx.dest.0) {
                *ctx.task = AssignedTask::ReinforceFloorTile(
                    crate::systems::soul_ai::execute::task_execution::types::ReinforceFloorTileData {
                        tile: tile_entity,
                        site: site_entity,
                        phase: ReinforceFloorPhase::Reinforcing { progress_bp: 0 },
                    },
                );
                ctx.path.waypoints.clear();
                info!(
                    "REINFORCE_FLOOR: Soul {:?} started reinforcing tile {:?}",
                    ctx.soul_entity, tile_entity
                );
            }
        }

        ReinforceFloorPhase::Reinforcing { progress_bp } => {
            // Get tile and update state
            let Ok(mut tile_blueprint) = ctx.queries.storage.floor_tiles.get_mut(tile_entity)
            else {
                info!(
                    "REINFORCE_FLOOR: Cancelled for {:?} - Tile {:?} gone",
                    ctx.soul_entity, tile_entity
                );
                clear_task_and_path(ctx.task, ctx.path);
                commands.entity(ctx.soul_entity).remove::<WorkingOn>();
                return;
            };

            // Update progress (basis points) to avoid truncation at 1x speed.
            const MAX_PROGRESS_BP: u16 = 10_000;
            let delta_bp = ((time.delta_secs() / FLOOR_REINFORCE_DURATION_SECS
                * MAX_PROGRESS_BP as f32)
                .round()
                .max(1.0)) as u16;
            let new_progress_bp = progress_bp.saturating_add(delta_bp).min(MAX_PROGRESS_BP);
            let visual_progress =
                ((new_progress_bp as f32 / MAX_PROGRESS_BP as f32) * 100.0).round() as u8;

            // Update tile visual state
            tile_blueprint.state = FloorTileState::Reinforcing {
                progress: visual_progress.min(100),
            };

            if new_progress_bp >= MAX_PROGRESS_BP {
                // Update tile state
                tile_blueprint.bones_delivered =
                    tile_blueprint.bones_delivered.max(FLOOR_BONES_PER_TILE);
                tile_blueprint.state = FloorTileState::ReinforcedComplete;

                // Update site counter
                if let Ok((_, mut site, _)) = ctx.queries.storage.floor_sites.get_mut(site_entity) {
                    site.tiles_reinforced += 1;
                    info!(
                        "REINFORCE_FLOOR: Tile {:?} reinforced ({}/{})",
                        tile_entity, site.tiles_reinforced, site.tiles_total
                    );
                }

                // Add fatigue
                ctx.soul.fatigue = (ctx.soul.fatigue + 0.15).min(1.0);

                *ctx.task = AssignedTask::ReinforceFloorTile(
                    crate::systems::soul_ai::execute::task_execution::types::ReinforceFloorTileData {
                        tile: tile_entity,
                        site: site_entity,
                        phase: ReinforceFloorPhase::Done,
                    },
                );
                info!(
                    "REINFORCE_FLOOR: Soul {:?} completed reinforcing tile {:?}",
                    ctx.soul_entity, tile_entity
                );
            } else {
                *ctx.task = AssignedTask::ReinforceFloorTile(
                    crate::systems::soul_ai::execute::task_execution::types::ReinforceFloorTileData {
                        tile: tile_entity,
                        site: site_entity,
                        phase: ReinforceFloorPhase::Reinforcing {
                            progress_bp: new_progress_bp,
                        },
                    },
                );
            }
        }

        ReinforceFloorPhase::Done => {
            // Release task slot
            ctx.queue_reservation(crate::events::ResourceReservationOp::ReleaseSource {
                source: tile_entity,
                amount: 1,
            });
            commands.entity(ctx.soul_entity).remove::<WorkingOn>();
            clear_task_and_path(ctx.task, ctx.path);
            info!(
                "REINFORCE_FLOOR: Soul {:?} finished reinforcing task",
                ctx.soul_entity
            );
        }
    }
}
