use bevy::prelude::*;
use rand::Rng;

use crate::constants::*;
use crate::entities::damned_soul::{
    DriftEdge, DriftPhase, DriftingState, IdleBehavior, IdleState,
};
use crate::entities::damned_soul::spawn::PopulationManager;
use crate::relationships::CommandedBy;
use crate::systems::soul_ai::execute::task_execution::AssignedTask;
use crate::world::map::WorldMap;

fn is_near_map_edge(grid: (i32, i32)) -> bool {
    grid.0 <= SOUL_DESPAWN_EDGE_MARGIN_TILES
        || grid.0 >= MAP_WIDTH - 1 - SOUL_DESPAWN_EDGE_MARGIN_TILES
        || grid.1 <= SOUL_DESPAWN_EDGE_MARGIN_TILES
        || grid.1 >= MAP_HEIGHT - 1 - SOUL_DESPAWN_EDGE_MARGIN_TILES
}

fn random_wander_target(grid: (i32, i32), world_map: &WorldMap, rng: &mut impl Rng) -> Vec2 {
    for _ in 0..24 {
        let dx = rng.gen_range(-4..=4);
        let dy = rng.gen_range(-4..=4);
        let target = (grid.0 + dx, grid.1 + dy);
        if world_map.is_walkable(target.0, target.1) {
            return WorldMap::grid_to_world(target.0, target.1);
        }
    }
    WorldMap::grid_to_world(grid.0, grid.1)
}

fn drift_move_target(
    current_grid: (i32, i32),
    edge: DriftEdge,
    world_map: &WorldMap,
    rng: &mut impl Rng,
) -> Vec2 {
    let drift_tiles = rng.gen_range(DRIFT_MOVE_TILES_MIN..=DRIFT_MOVE_TILES_MAX);
    let lateral = rng.gen_range(-DRIFT_LATERAL_OFFSET_MAX..=DRIFT_LATERAL_OFFSET_MAX);

    let desired = match edge {
        DriftEdge::North => (current_grid.0 + lateral, current_grid.1 - drift_tiles),
        DriftEdge::South => (current_grid.0 + lateral, current_grid.1 + drift_tiles),
        DriftEdge::East => (current_grid.0 + drift_tiles, current_grid.1 + lateral),
        DriftEdge::West => (current_grid.0 - drift_tiles, current_grid.1 + lateral),
    };

    let clamped = (
        desired.0.clamp(0, MAP_WIDTH - 1),
        desired.1.clamp(0, MAP_HEIGHT - 1),
    );

    if world_map.is_walkable(clamped.0, clamped.1) {
        return WorldMap::grid_to_world(clamped.0, clamped.1);
    }

    let desired_world = WorldMap::grid_to_world(clamped.0, clamped.1);
    world_map
        .get_nearest_walkable_grid(desired_world)
        .map(|(gx, gy)| WorldMap::grid_to_world(gx, gy))
        .unwrap_or_else(|| WorldMap::grid_to_world(current_grid.0, current_grid.1))
}

/// 漂流（Drifting）中の Soul 行動更新
pub fn drifting_behavior_system(
    time: Res<Time>,
    world_map: Res<WorldMap>,
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

    for (entity, transform, mut idle, mut destination, mut path, task, under_command, mut drifting) in
        q_souls.iter_mut()
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
                let path_done = path.waypoints.is_empty() || path.current_index >= path.waypoints.len();
                if path_done {
                    destination.0 = random_wander_target(current_grid, &world_map, &mut rng);
                    path.waypoints.clear();
                    path.current_index = 0;
                }

                if drifting.phase_timer >= drifting.phase_duration {
                    drifting.phase = DriftPhase::Moving;
                    drifting.phase_timer = 0.0;
                    destination.0 =
                        drift_move_target(current_grid, drifting.target_edge, &world_map, &mut rng);
                    path.waypoints.clear();
                    path.current_index = 0;
                }
            }
            DriftPhase::Moving => {
                let arrived = current_pos.distance(destination.0) <= TILE_SIZE * 0.75;
                let path_done = path.waypoints.is_empty() || path.current_index >= path.waypoints.len();
                if arrived || (path_done && drifting.phase_timer > 1.0) {
                    drifting.phase = DriftPhase::Wandering;
                    drifting.phase_timer = 0.0;
                    drifting.phase_duration =
                        rng.gen_range(DRIFT_WANDER_DURATION_MIN..DRIFT_WANDER_DURATION_MAX);
                    destination.0 = random_wander_target(current_grid, &world_map, &mut rng);
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
    mut population: ResMut<PopulationManager>,
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

        population.total_escaped += 1;
        commands.entity(entity).try_despawn();
        info!(
            "SOUL_DRIFT: {:?} despawned at edge {:?} (total_escaped={})",
            entity, grid, population.total_escaped
        );
    }
}
