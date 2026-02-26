//! 怠惰行動ごとの移動先更新（Wandering / Gathering）

use bevy::prelude::*;
use rand::Rng;

use crate::constants::*;
use crate::entities::damned_soul::{GatheringBehavior, IdleBehavior};
use crate::events::{IdleBehaviorOperation, IdleBehaviorRequest};
use crate::relationships::ParticipatingIn;
use crate::systems::spatial::SpatialGridOps;
use crate::world::map::WorldMap;

use super::GATHERING_ARRIVAL_RADIUS;
use super::gathering_motion;
use super::transitions;

/// 現在の行動に応じて移動先を更新
pub fn update_motion_destinations(
    entity: Entity,
    current_pos: Vec2,
    gathering_center: Option<Vec2>,
    target_spot_entity: Option<Entity>,
    participating_in: Option<&ParticipatingIn>,
    idle: &mut crate::entities::damned_soul::IdleState,
    dest: &mut crate::entities::damned_soul::Destination,
    path: &mut crate::entities::damned_soul::Path,
    soul_grid: &impl SpatialGridOps,
    world_map: &WorldMap,
    request_writer: &mut MessageWriter<IdleBehaviorRequest>,
    dt: f32,
    dream: f32,
) {
    match idle.behavior {
        IdleBehavior::Wandering => {
            if path.waypoints.is_empty() || path.current_index >= path.waypoints.len() {
                let current_grid = WorldMap::world_to_grid(current_pos);
                let mut rng = rand::thread_rng();
                for _ in 0..10 {
                    let dx: i32 = rng.gen_range(-5..=5);
                    let dy: i32 = rng.gen_range(-5..=5);
                    let new_grid = (current_grid.0 + dx, current_grid.1 + dy);
                    if world_map.is_walkable(new_grid.0, new_grid.1) {
                        dest.0 = WorldMap::grid_to_world(new_grid.0, new_grid.1);
                        break;
                    }
                }
            }
        }
        IdleBehavior::Gathering | IdleBehavior::ExhaustedGathering => {
            if let Some(center) = gathering_center {
                let dist_from_center = (center - current_pos).length();

                idle.gathering_behavior_timer += dt;
                if idle.gathering_behavior_timer >= idle.gathering_behavior_duration {
                    idle.gathering_behavior_timer = 0.0;
                    idle.gathering_behavior = transitions::random_gathering_behavior(dream);
                    idle.gathering_behavior_duration = transitions::random_gathering_duration();
                    idle.needs_separation = true;
                }

                // dream=0で集会中Sleeping → Sleeping以外に切り替え
                if idle.gathering_behavior == GatheringBehavior::Sleeping && dream <= 0.0 {
                    idle.gathering_behavior = transitions::random_gathering_behavior(dream);
                    idle.gathering_behavior_timer = 0.0;
                    idle.gathering_behavior_duration = transitions::random_gathering_duration();
                    idle.needs_separation = true;
                }

                if dist_from_center > GATHERING_ARRIVAL_RADIUS {
                    if path.waypoints.is_empty() || path.current_index >= path.waypoints.len() {
                        dest.0 = center;
                    }
                } else {
                    let just_arrived = participating_in.is_none();
                    if just_arrived {
                        if let Some(spot_entity) = target_spot_entity {
                            request_writer.write(IdleBehaviorRequest {
                                entity,
                                operation: IdleBehaviorOperation::JoinGathering { spot_entity },
                            });
                        }
                    }
                    if idle.behavior == IdleBehavior::ExhaustedGathering {
                        idle.behavior = IdleBehavior::Gathering;
                    }

                    let is_moving =
                        !path.waypoints.is_empty() && path.current_index < path.waypoints.len();

                    if !is_moving
                        && (just_arrived
                            || dist_from_center < TILE_SIZE * GATHERING_KEEP_DISTANCE_MIN)
                    {
                        if let Some(new_target) = gathering_motion::find_initial_gathering_position(
                            center,
                            current_pos,
                            entity,
                            soul_grid,
                            world_map,
                        ) {
                            dest.0 = new_target;
                            path.waypoints.clear();
                            path.current_index = 0;
                        }
                    }

                    match idle.gathering_behavior {
                        GatheringBehavior::Wandering => {
                            let path_complete = path.waypoints.is_empty()
                                || path.current_index >= path.waypoints.len();
                            if path_complete {
                                if let Some(new_target) =
                                    gathering_motion::find_gathering_wandering_target(
                                        center,
                                        current_pos,
                                        entity,
                                        soul_grid,
                                        world_map,
                                    )
                                {
                                    dest.0 = new_target;
                                    path.waypoints.clear();
                                    path.current_index = 0;
                                }
                            }
                        }
                        GatheringBehavior::Sleeping
                        | GatheringBehavior::Standing
                        | GatheringBehavior::Dancing => {
                            let path_complete = path.waypoints.is_empty()
                                || path.current_index >= path.waypoints.len();
                            if dist_from_center < TILE_SIZE * GATHERING_KEEP_DISTANCE_MIN
                                && path_complete
                            {
                                if let Some(target) =
                                    gathering_motion::find_gathering_still_retreat_target(
                                        center,
                                        current_pos,
                                        entity,
                                        soul_grid,
                                        world_map,
                                    )
                                {
                                    dest.0 = target;
                                    path.waypoints.clear();
                                    path.current_index = 0;
                                }
                            } else {
                                const MIN_SEPARATION: f32 = TILE_SIZE * 1.2;
                                let nearby_souls =
                                    soul_grid.get_nearby_in_radius(current_pos, MIN_SEPARATION);
                                let has_overlap = nearby_souls.iter().any(|&other| other != entity);
                                let dist_to_dest = (dest.0 - current_pos).length();
                                if !has_overlap && dist_to_dest < TILE_SIZE * 0.5 {
                                    path.waypoints.clear();
                                    path.current_index = 0;
                                }
                            }
                        }
                    }
                }
            } else {
                idle.behavior = IdleBehavior::Wandering;
            }
        }
        IdleBehavior::Sitting
        | IdleBehavior::Sleeping
        | IdleBehavior::Resting
        | IdleBehavior::GoingToRest
        | IdleBehavior::Escaping
        | IdleBehavior::Drifting => {}
    }
}
