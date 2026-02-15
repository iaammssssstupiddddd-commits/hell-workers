//! Floor tile reinforcement task execution

use crate::relationships::WorkingOn;
use crate::systems::jobs::floor_construction::FloorTileState;
use crate::systems::logistics::ResourceType;
use crate::systems::soul_ai::execute::task_execution::{
    common::*,
    context::TaskExecutionContext,
    types::{AssignedTask, ReinforceFloorPhase},
};
use crate::world::map::WorldMap;
use bevy::prelude::*;

const FLOOR_BONES_PER_TILE: u32 = 2;
const FLOOR_REINFORCE_DURATION_SECS: f32 = 3.0;

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
            let Ok((site_transform, _site, _)) = ctx
                .queries
                .storage
                .floor_sites
                .get(site_entity)
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

            // Check if near material center
            if soul_pos.distance(material_center) < 32.0 {
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
            // Find nearby bones at material center
            let nearby_bones: Vec<Entity> = ctx
                .queries
                .resource_items
                .iter()
                .filter(|(_, item, _)| item.0 == ResourceType::Bone)
                .filter(|(_, _, stored_opt)| stored_opt.is_none()) // Not stored
                .map(|(entity, _, _)| entity)
                .take(FLOOR_BONES_PER_TILE as usize)
                .collect();

            if nearby_bones.len() >= FLOOR_BONES_PER_TILE as usize {
                // Transition to going to tile
                *ctx.task = AssignedTask::ReinforceFloorTile(
                    crate::systems::soul_ai::execute::task_execution::types::ReinforceFloorTileData {
                        tile: tile_entity,
                        site: site_entity,
                        phase: ReinforceFloorPhase::GoingToTile,
                    },
                );
                ctx.path.waypoints.clear();
                info!(
                    "REINFORCE_FLOOR: Soul {:?} found bones, heading to tile {:?}",
                    ctx.soul_entity, tile_entity
                );
            } else {
                // Not enough bones, wait
                info!(
                    "REINFORCE_FLOOR: Soul {:?} waiting for bones at material center (found {}/{})",
                    ctx.soul_entity, nearby_bones.len(), FLOOR_BONES_PER_TILE
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
                *ctx.task = AssignedTask::ReinforceFloorTile(
                    crate::systems::soul_ai::execute::task_execution::types::ReinforceFloorTileData {
                        tile: tile_entity,
                        site: site_entity,
                        phase: ReinforceFloorPhase::Reinforcing { progress: 0 },
                    },
                );
                ctx.path.waypoints.clear();
                info!(
                    "REINFORCE_FLOOR: Soul {:?} started reinforcing tile {:?}",
                    ctx.soul_entity, tile_entity
                );
            }
        }

        ReinforceFloorPhase::Reinforcing { progress } => {
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

            // Update progress
            let delta = (time.delta_secs() / FLOOR_REINFORCE_DURATION_SECS * 100.0) as u8;
            let new_progress = progress.saturating_add(delta).min(100);

            // Update tile visual state
            tile_blueprint.state = FloorTileState::Reinforcing {
                progress: new_progress,
            };

            if new_progress >= 100 {
                // Reinforcing complete
                // Consume bones from nearby material center
                let site_pos = if let Ok((site_transform, _, _)) =
                    ctx.queries.storage.floor_sites.get(site_entity)
                {
                    site_transform.translation.truncate()
                } else {
                    Vec2::ZERO
                };

                let nearby_bones: Vec<Entity> = ctx
                    .queries
                    .resource_items
                    .iter()
                    .filter(|(_entity, item, stored_opt)| {
                        item.0 == ResourceType::Bone && stored_opt.is_none()
                    })
                    .filter(|(entity, _, _)| {
                        if let Ok((_bone_entity, _, _)) = ctx.queries.resource_items.get(*entity) {
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
                    .take(FLOOR_BONES_PER_TILE as usize)
                    .collect();

                // Despawn consumed bones
                for bone_entity in nearby_bones.iter().take(FLOOR_BONES_PER_TILE as usize) {
                    commands.entity(*bone_entity).despawn();
                }

                // Update tile state
                tile_blueprint.bones_delivered += FLOOR_BONES_PER_TILE;
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
                            progress: new_progress,
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
