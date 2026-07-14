//! 怠惰行動ごとの移動先更新（Wandering / Gathering）

use bevy::prelude::*;
use rand::Rng;

use super::{GATHERING_ARRIVAL_RADIUS, transitions};
use crate::soul_ai::helpers::gathering_motion;
use hw_core::constants::*;
use hw_core::events::{IdleBehaviorOperation, IdleBehaviorRequest};
use hw_core::relationships::ParticipatingIn;
#[cfg(feature = "profiling")]
use hw_core::simulation_rng::{FixedAuditSeed, SimulationRandomState, SimulationRng};
use hw_core::soul::{Destination, GatheringBehavior, IdleBehavior, IdleState, Path};
use hw_world::coords::grid_to_world;
use hw_world::coords::world_to_grid;
use hw_world::{SpatialGridOps, WorldMap};

/// `update_motion_destinations` に渡すエンティティ位置情報。
pub struct SoulPos {
    pub entity: Entity,
    pub pos: Vec2,
}

/// `update_motion_destinations` に渡す集会コンテキスト。
pub struct MotionGatheringCtx<'a> {
    pub center: Option<Vec2>,
    pub target_spot_entity: Option<Entity>,
    pub participating_in: Option<&'a ParticipatingIn>,
}

/// `update_motion_destinations` に渡す Soul 移動状態。
pub struct MotionState<'a> {
    pub idle: &'a mut IdleState,
    pub dest: &'a mut Destination,
    pub path: &'a mut Path,
}

/// `update_motion_destinations` に渡す時間・scratch バッファ。
pub struct MotionExtras<'a> {
    pub dt: f32,
    pub dream: f32,
    pub scratch: &'a mut Vec<Entity>,
    #[cfg(feature = "profiling")]
    pub audit_seed: Option<&'a FixedAuditSeed>,
    #[cfg(feature = "profiling")]
    pub random_state: Option<&'a mut SimulationRandomState>,
}

#[cfg(feature = "profiling")]
const WANDER_DESTINATION_STREAM: u64 = 0x6964_6c65_5f77_616e;
#[cfg(feature = "profiling")]
const GATHERING_SUB_BEHAVIOR_STREAM: u64 = 0x6761_7468_5f62_6568;
#[cfg(feature = "profiling")]
const GATHERING_SUB_DURATION_STREAM: u64 = 0x6761_7468_5f64_7572;
#[cfg(feature = "profiling")]
const GATHERING_INITIAL_POSITION_STREAM: u64 = 0x6761_7468_5f69_6e69;
#[cfg(feature = "profiling")]
const GATHERING_WANDERING_POSITION_STREAM: u64 = 0x6761_7468_5f77_616e;
#[cfg(feature = "profiling")]
const GATHERING_RETREAT_POSITION_STREAM: u64 = 0x6761_7468_5f72_6574;

/// 現在の行動に応じて移動先を更新
pub fn update_motion_destinations(
    soul_pos: SoulPos,
    gathering: MotionGatheringCtx<'_>,
    state: MotionState<'_>,
    soul_grid: &impl SpatialGridOps,
    world_map: &WorldMap,
    request_writer: &mut MessageWriter<IdleBehaviorRequest>,
    extras: MotionExtras<'_>,
) {
    let entity = soul_pos.entity;
    let current_pos = soul_pos.pos;
    #[cfg(feature = "profiling")]
    let MotionExtras {
        dt,
        dream,
        scratch,
        audit_seed,
        mut random_state,
    } = extras;
    #[cfg(not(feature = "profiling"))]
    let MotionExtras { dt, dream, scratch } = extras;
    let idle = state.idle;
    let dest = state.dest;
    let path = state.path;
    let gathering_center = gathering.center;
    let target_spot_entity = gathering.target_spot_entity;
    let participating_in = gathering.participating_in;
    match idle.behavior {
        IdleBehavior::Wandering => {
            if path.waypoints.is_empty() || path.current_index >= path.waypoints.len() {
                #[cfg(feature = "profiling")]
                let mut rng = SimulationRng::for_actor(
                    audit_seed,
                    random_state.as_deref_mut(),
                    WANDER_DESTINATION_STREAM,
                );
                #[cfg(not(feature = "profiling"))]
                let mut rng = rand::thread_rng();
                set_wandering_destination(current_pos, dest, world_map, &mut rng);
            }
        }
        IdleBehavior::Gathering | IdleBehavior::ExhaustedGathering => {
            if let Some(center) = gathering_center {
                let dist_from_center = (center - current_pos).length();

                idle.gathering_behavior_timer += dt;
                if idle.gathering_behavior_timer >= idle.gathering_behavior_duration {
                    idle.gathering_behavior_timer = 0.0;
                    #[cfg(feature = "profiling")]
                    let mut behavior_rng = SimulationRng::for_actor(
                        audit_seed,
                        random_state.as_deref_mut(),
                        GATHERING_SUB_BEHAVIOR_STREAM,
                    );
                    #[cfg(feature = "profiling")]
                    {
                        idle.gathering_behavior = transitions::random_gathering_behavior_with_rng(
                            dream,
                            &mut behavior_rng,
                        );
                    }
                    #[cfg(not(feature = "profiling"))]
                    {
                        idle.gathering_behavior = transitions::random_gathering_behavior(dream);
                    }
                    #[cfg(feature = "profiling")]
                    let mut duration_rng = SimulationRng::for_actor(
                        audit_seed,
                        random_state.as_deref_mut(),
                        GATHERING_SUB_DURATION_STREAM,
                    );
                    #[cfg(feature = "profiling")]
                    {
                        idle.gathering_behavior_duration =
                            transitions::random_gathering_duration_with_rng(&mut duration_rng);
                    }
                    #[cfg(not(feature = "profiling"))]
                    {
                        idle.gathering_behavior_duration = transitions::random_gathering_duration();
                    }
                    idle.needs_separation = true;
                }

                // dream=0で集会中Sleeping → Sleeping以外に切り替え
                if idle.gathering_behavior == GatheringBehavior::Sleeping && dream <= 0.0 {
                    #[cfg(feature = "profiling")]
                    let mut behavior_rng = SimulationRng::for_actor(
                        audit_seed,
                        random_state.as_deref_mut(),
                        GATHERING_SUB_BEHAVIOR_STREAM,
                    );
                    #[cfg(feature = "profiling")]
                    {
                        idle.gathering_behavior = transitions::random_gathering_behavior_with_rng(
                            dream,
                            &mut behavior_rng,
                        );
                    }
                    #[cfg(not(feature = "profiling"))]
                    {
                        idle.gathering_behavior = transitions::random_gathering_behavior(dream);
                    }
                    idle.gathering_behavior_timer = 0.0;
                    #[cfg(feature = "profiling")]
                    let mut duration_rng = SimulationRng::for_actor(
                        audit_seed,
                        random_state.as_deref_mut(),
                        GATHERING_SUB_DURATION_STREAM,
                    );
                    #[cfg(feature = "profiling")]
                    {
                        idle.gathering_behavior_duration =
                            transitions::random_gathering_duration_with_rng(&mut duration_rng);
                    }
                    #[cfg(not(feature = "profiling"))]
                    {
                        idle.gathering_behavior_duration = transitions::random_gathering_duration();
                    }
                    idle.needs_separation = true;
                }

                if dist_from_center > GATHERING_ARRIVAL_RADIUS {
                    if path.waypoints.is_empty() || path.current_index >= path.waypoints.len() {
                        dest.0 = center;
                    }
                } else {
                    let just_arrived = participating_in.is_none();
                    if just_arrived && let Some(spot_entity) = target_spot_entity {
                        request_writer.write(IdleBehaviorRequest {
                            entity,
                            operation: IdleBehaviorOperation::JoinGathering { spot_entity },
                        });
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
                        #[cfg(feature = "profiling")]
                        let new_target = {
                            let mut rng = SimulationRng::for_actor(
                                audit_seed,
                                random_state.as_deref_mut(),
                                GATHERING_INITIAL_POSITION_STREAM,
                            );
                            gathering_motion::find_initial_gathering_position_with_rng(
                                center,
                                current_pos,
                                entity,
                                soul_grid,
                                world_map,
                                scratch,
                                &mut rng,
                            )
                        };
                        #[cfg(not(feature = "profiling"))]
                        let new_target = gathering_motion::find_initial_gathering_position(
                            center,
                            current_pos,
                            entity,
                            soul_grid,
                            world_map,
                            scratch,
                        );
                        if let Some(new_target) = new_target {
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
                                #[cfg(feature = "profiling")]
                                let new_target = {
                                    let mut rng = SimulationRng::for_actor(
                                        audit_seed,
                                        random_state.as_deref_mut(),
                                        GATHERING_WANDERING_POSITION_STREAM,
                                    );
                                    gathering_motion::find_gathering_wandering_target_with_rng(
                                        center,
                                        current_pos,
                                        entity,
                                        soul_grid,
                                        world_map,
                                        scratch,
                                        &mut rng,
                                    )
                                };
                                #[cfg(not(feature = "profiling"))]
                                let new_target = gathering_motion::find_gathering_wandering_target(
                                    center,
                                    current_pos,
                                    entity,
                                    soul_grid,
                                    world_map,
                                    scratch,
                                );
                                if let Some(new_target) = new_target {
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
                                #[cfg(feature = "profiling")]
                                let target = {
                                    let mut rng = SimulationRng::for_actor(
                                        audit_seed,
                                        random_state.as_deref_mut(),
                                        GATHERING_RETREAT_POSITION_STREAM,
                                    );
                                    gathering_motion::find_gathering_still_retreat_target_with_rng(
                                        center,
                                        current_pos,
                                        entity,
                                        soul_grid,
                                        world_map,
                                        scratch,
                                        &mut rng,
                                    )
                                };
                                #[cfg(not(feature = "profiling"))]
                                let target = gathering_motion::find_gathering_still_retreat_target(
                                    center,
                                    current_pos,
                                    entity,
                                    soul_grid,
                                    world_map,
                                    scratch,
                                );
                                if let Some(target) = target {
                                    dest.0 = target;
                                    path.waypoints.clear();
                                    path.current_index = 0;
                                }
                            } else {
                                const MIN_SEPARATION: f32 = TILE_SIZE * 1.2;
                                soul_grid.get_nearby_in_radius_into(
                                    current_pos,
                                    MIN_SEPARATION,
                                    scratch,
                                );
                                let has_overlap = scratch.iter().any(|&other| other != entity);
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

fn set_wandering_destination(
    current_pos: Vec2,
    destination: &mut Destination,
    world_map: &WorldMap,
    rng: &mut impl Rng,
) {
    let current_grid = world_to_grid(current_pos);
    for _ in 0..10 {
        let dx: i32 = rng.gen_range(-5..=5);
        let dy: i32 = rng.gen_range(-5..=5);
        let new_grid = (current_grid.0 + dx, current_grid.1 + dy);
        if world_map.is_walkable(new_grid.0, new_grid.1) {
            destination.0 = grid_to_world(new_grid.0, new_grid.1);
            break;
        }
    }
}
