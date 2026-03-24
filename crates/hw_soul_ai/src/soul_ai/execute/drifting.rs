//! 漂流（Drifting）実行システム
//!
//! `PopulationManager` など root 固有リソースに依存しない純粋な実行ロジック。
//! Decide フェーズの判定（`decide/drifting.rs`）は root 側に残る。

use bevy::prelude::*;
use hw_core::constants::TILE_SIZE;
use hw_core::events::SoulEscaped;
use hw_core::relationships::CommandedBy;
use hw_core::soul::{DamnedSoul, DriftPhase, DriftingState, IdleBehavior, IdleState};
use hw_jobs::AssignedTask;
use hw_world::map::WorldMapRead;
use rand::Rng;

use crate::soul_ai::helpers::drifting::{
    drift_move_target, is_near_map_edge, random_wander_target,
};

type DriftingBehaviorQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static Transform,
        &'static mut IdleState,
        &'static mut hw_core::soul::Destination,
        &'static mut hw_core::soul::Path,
        &'static AssignedTask,
        Option<&'static CommandedBy>,
        &'static mut DriftingState,
    ),
    With<DamnedSoul>,
>;

type DespawnAtEdgeQuery<'w, 's> =
    Query<'w, 's, (Entity, &'static Transform, &'static IdleState), (With<DamnedSoul>, With<DriftingState>)>;

/// 漂流（Drifting）中の Soul 行動更新
pub fn drifting_behavior_system(
    time: Res<Time>,
    world_map: WorldMapRead,
    mut commands: Commands,
    mut q_souls: DriftingBehaviorQuery,
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
        let current_grid = hw_world::map::WorldMap::world_to_grid(current_pos);
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
                    drifting.phase_duration = rng.gen_range(
                        hw_core::constants::DRIFT_WANDER_DURATION_MIN
                            ..hw_core::constants::DRIFT_WANDER_DURATION_MAX,
                    );
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
    q_souls: DespawnAtEdgeQuery,
) {
    for (entity, transform, idle) in q_souls.iter() {
        if idle.behavior != IdleBehavior::Drifting {
            continue;
        }

        let grid = hw_world::map::WorldMap::world_to_grid(transform.translation.truncate());
        if !is_near_map_edge(grid) {
            continue;
        }

        commands.entity(entity).try_despawn();
        commands.trigger(SoulEscaped { entity, grid });
    }
}
