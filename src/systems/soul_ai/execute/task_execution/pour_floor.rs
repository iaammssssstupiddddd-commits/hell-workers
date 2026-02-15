//! Floor tile pouring task execution

use crate::relationships::WorkingOn;
use crate::constants::{FLOOR_MUD_PER_TILE, FLOOR_POUR_DURATION_SECS};
use crate::systems::jobs::floor_construction::FloorTileState;
use crate::systems::logistics::ResourceType;
use crate::systems::soul_ai::execute::task_execution::{
    common::*,
    context::TaskExecutionContext,
    types::{AssignedTask, PourFloorPhase, PourFloorTileData},
};
use crate::world::map::WorldMap;
use bevy::prelude::*;

pub fn handle_pour_floor_task(
    ctx: &mut TaskExecutionContext,
    tile_entity: Entity,
    site_entity: Entity,
    phase: PourFloorPhase,
    commands: &mut Commands,
    time: &Res<Time>,
    world_map: &Res<WorldMap>,
) {
    let soul_pos = ctx.soul_pos();

    match phase {
        PourFloorPhase::GoingToMaterialCenter => {
            // Get site material center position
            let Ok((site_transform, _site, _)) = ctx
                .queries
                .storage
                .floor_sites
                .get(site_entity)
            else {
                // Site disappeared
                info!(
                    "POUR_FLOOR: Cancelled for {:?} - Site {:?} gone",
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

            // Check if near material center
            if soul_pos.distance(material_center) < 32.0 {
                *ctx.task = AssignedTask::PourFloorTile(PourFloorTileData {
                    tile: tile_entity,
                    site: site_entity,
                    phase: PourFloorPhase::PickingUpMud,
                });
                info!(
                    "POUR_FLOOR: Soul {:?} arrived at material center",
                    ctx.soul_entity
                );
            }
        }

        PourFloorPhase::PickingUpMud => {
            // Find nearby mud at material center
            let nearby_mud: Vec<Entity> = ctx
                .queries
                .resource_items
                .iter()
                .filter(|(_, item, _)| item.0 == ResourceType::StasisMud)
                .filter(|(_, _, stored_opt)| stored_opt.is_none()) // Not stored
                .map(|(entity, _, _)| entity)
                .take(FLOOR_MUD_PER_TILE as usize)
                .collect();

            if nearby_mud.len() >= FLOOR_MUD_PER_TILE as usize {
                // Transition to going to tile
                *ctx.task = AssignedTask::PourFloorTile(PourFloorTileData {
                    tile: tile_entity,
                    site: site_entity,
                    phase: PourFloorPhase::GoingToTile,
                });
                ctx.path.waypoints.clear();
                info!(
                    "POUR_FLOOR: Soul {:?} found mud, heading to tile {:?}",
                    ctx.soul_entity, tile_entity
                );
            } else {
                // Not enough mud, wait
                info!(
                    "POUR_FLOOR: Soul {:?} waiting for mud at material center (found {}/{})",
                    ctx.soul_entity, nearby_mud.len(), FLOOR_MUD_PER_TILE
                );
            }
        }

        PourFloorPhase::GoingToTile => {
            // Get tile position
            let Ok(tile_blueprint) = ctx.queries.storage.floor_tiles.get(tile_entity) else {
                // Tile disappeared
                info!(
                    "POUR_FLOOR: Cancelled for {:?} - Tile {:?} gone",
                    ctx.soul_entity, tile_entity
                );
                clear_task_and_path(ctx.task, ctx.path);
                commands.entity(ctx.soul_entity).remove::<WorkingOn>();
                return;
            };

            let tile_pos = WorldMap::grid_to_world(tile_blueprint.grid_pos.0, tile_blueprint.grid_pos.1);

            // Navigate to tile
            update_destination_to_adjacent(
                ctx.dest,
                tile_pos,
                ctx.path,
                soul_pos,
                world_map,
                ctx.pf_context,
            );

            // Check if near tile
            if soul_pos.distance(tile_pos) < 32.0 {
                *ctx.task = AssignedTask::PourFloorTile(PourFloorTileData {
                    tile: tile_entity,
                    site: site_entity,
                    phase: PourFloorPhase::Pouring { progress: 0 },
                });
                ctx.path.waypoints.clear();
                info!(
                    "POUR_FLOOR: Soul {:?} started pouring tile {:?}",
                    ctx.soul_entity, tile_entity
                );
            }
        }

        PourFloorPhase::Pouring { progress } => {
            // Get tile and update state
            let Ok(mut tile_blueprint) = ctx.queries.storage.floor_tiles.get_mut(tile_entity)
            else {
                info!(
                    "POUR_FLOOR: Cancelled for {:?} - Tile {:?} gone",
                    ctx.soul_entity, tile_entity
                );
                clear_task_and_path(ctx.task, ctx.path);
                commands.entity(ctx.soul_entity).remove::<WorkingOn>();
                return;
            };

            // Update progress
            let delta = (time.delta_secs() / FLOOR_POUR_DURATION_SECS * 100.0) as u8;
            let new_progress = progress.saturating_add(delta).min(100);

            // Update tile visual state
            tile_blueprint.state = FloorTileState::Pouring {
                progress: new_progress,
            };

            if new_progress >= 100 {
                // Pouring complete
                // Consume mud from nearby material center
                let site_pos = if let Ok((site_transform, _, _)) =
                    ctx.queries.storage.floor_sites.get(site_entity)
                {
                    site_transform.translation.truncate()
                } else {
                    Vec2::ZERO
                };

                let nearby_mud: Vec<Entity> = ctx
                    .queries
                    .resource_items
                    .iter()
                    .filter(|(_entity, item, stored_opt)| {
                        item.0 == ResourceType::StasisMud && stored_opt.is_none()
                    })
                    .filter(|(entity, _, _)| {
                        if let Ok((_mud_entity, _, _)) = ctx.queries.resource_items.get(*entity) {
                            // Check proximity to material center
                            if let Ok((t, _, _, _, _, _, _)) = ctx.queries.designation.targets.get(*entity) {
                                t.translation.truncate().distance(site_pos) < 64.0
                            } else {
                                false
                            }
                        } else {
                            false
                        }
                    })
                    .map(|(entity, _, _)| entity)
                    .take(FLOOR_MUD_PER_TILE as usize)
                    .collect();

                // Despawn consumed mud
                for mud_entity in nearby_mud.iter().take(FLOOR_MUD_PER_TILE as usize) {
                    commands.entity(*mud_entity).despawn();
                }

                // Update tile state
                tile_blueprint.mud_delivered += FLOOR_MUD_PER_TILE;
                tile_blueprint.state = FloorTileState::Complete;

                // Update site counter
                if let Ok((_, mut site, _)) = ctx.queries.storage.floor_sites.get_mut(site_entity) {
                    site.tiles_poured += 1;
                    info!(
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
                info!(
                    "POUR_FLOOR: Soul {:?} completed pouring tile {:?}",
                    ctx.soul_entity, tile_entity
                );
            } else {
                *ctx.task = AssignedTask::PourFloorTile(PourFloorTileData {
                    tile: tile_entity,
                    site: site_entity,
                    phase: PourFloorPhase::Pouring {
                        progress: new_progress,
                    },
                });
            }
        }

        PourFloorPhase::Done => {
            // Release task slot (if needed, but usually handled by completion system)
            // For floor tiles, workers are assigned to a tile. 
            // We should release the reservation here.
            ctx.queue_reservation(crate::events::ResourceReservationOp::ReleaseSource {
                source: tile_entity,
                amount: 1,
            });
            commands.entity(ctx.soul_entity).remove::<WorkingOn>();
            clear_task_and_path(ctx.task, ctx.path);
            info!(
                "POUR_FLOOR: Soul {:?} finished pouring task",
                ctx.soul_entity
            );
        }
    }
}
