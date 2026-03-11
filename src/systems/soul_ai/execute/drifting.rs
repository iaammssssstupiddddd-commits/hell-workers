use bevy::prelude::*;
use hw_ai::soul_ai::helpers::drifting::{drift_move_target, is_near_map_edge, random_wander_target};
use hw_core::events::SoulEscaped;
use rand::Rng;

use crate::entities::damned_soul::{DriftPhase, DriftingState, IdleBehavior, IdleState};
use crate::relationships::CommandedBy;
use crate::systems::soul_ai::execute::task_execution::AssignedTask;
use crate::world::map::{WorldMap, WorldMapRead};
use hw_core::constants::*;

/// 漂流（Drifting）中の Soul 行動更新
pub fn drifting_behavior_system(
    time: Res<Time>,
    world_map: WorldMapRead,
    mut commands: Commands,
    mut q_souls: Query<
        (
            Entity,
            &Transform,
            &mut IdleState,
            &mut crate::entities::damned_soul::Destination,
            &mut crate::entities::damned_soul::Path,
            &AssignedTask,
            Option<&CommandedBy>,
            &mut DriftingState,
        ),
        With<crate::entities::damned_soul::DamnedSoul>,
    >,
) {
    let dt = time.delta_secs();
    let mut rng = rand::thread_rng();

    for (
        entity,
        transform,
        mut idle,
        mut destination,
        mut path,
        task,
        under_command,
        mut drifting,
    ) in q_souls.iter_mut()
    {
        if idle.behavior != IdleBehavior::Drifting {
            commands.entity(entity).remove::<DriftingState>();
            continue;
        }

        if under_command.is_some() || !matches!(*task, AssignedTask::None) {
            idle.behavior = IdleBehavior::Wandering;
            idle.idle_timer = 0.0;
            idle.total_idle_time = 0.0;
            path.waypoints.clear();
            path.current_index = 0;
            commands.entity(entity).remove::<DriftingState>();
            continue;
        }

        let current_pos = transform.translation.truncate();
        let current_grid = WorldMap::world_to_grid(current_pos);
        drifting.phase_timer += dt;

        match drifting.phase {
            DriftPhase::Wandering => {
                let path_done =
                    path.waypoints.is_empty() || path.current_index >= path.waypoints.len();
                if path_done {
                    destination.0 =
                        random_wander_target(current_grid, world_map.as_ref(), &mut rng);
                    path.waypoints.clear();
                    path.current_index = 0;
                }

                if drifting.phase_timer >= drifting.phase_duration {
                    drifting.phase = DriftPhase::Moving;
                    drifting.phase_timer = 0.0;
                    destination.0 = drift_move_target(
                        current_grid,
                        drifting.target_edge,
                        world_map.as_ref(),
                        &mut rng,
                    );
                    path.waypoints.clear();
                    path.current_index = 0;
                }
            }
            DriftPhase::Moving => {
                let arrived = current_pos.distance(destination.0) <= TILE_SIZE * 0.75;
                let path_done =
                    path.waypoints.is_empty() || path.current_index >= path.waypoints.len();
                if arrived || (path_done && drifting.phase_timer > 1.0) {
                    drifting.phase = DriftPhase::Wandering;
                    drifting.phase_timer = 0.0;
                    drifting.phase_duration =
                        rng.gen_range(DRIFT_WANDER_DURATION_MIN..DRIFT_WANDER_DURATION_MAX);
                    destination.0 =
                        random_wander_target(current_grid, world_map.as_ref(), &mut rng);
                    path.waypoints.clear();
                    path.current_index = 0;
                }
            }
        }
    }
}

/// マップ端到達時に漂流中 Soul をデスポーン
pub fn despawn_at_edge_system(
    mut commands: Commands,
    q_souls: Query<
        (Entity, &Transform, &IdleState),
        (
            With<crate::entities::damned_soul::DamnedSoul>,
            With<DriftingState>,
        ),
    >,
) {
    for (entity, transform, idle) in q_souls.iter() {
        if idle.behavior != IdleBehavior::Drifting {
            continue;
        }

        let grid = WorldMap::world_to_grid(transform.translation.truncate());
        if !is_near_map_edge(grid) {
            continue;
        }

        commands.entity(entity).try_despawn();
        commands.trigger(SoulEscaped { entity, grid });
    }
}
