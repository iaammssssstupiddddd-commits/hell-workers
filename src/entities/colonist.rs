use bevy::prelude::*;
use crate::constants::*;
use crate::assets::GameAssets;
use crate::world::map::WorldMap;
use crate::world::pathfinding::find_path;

#[derive(Component)]
pub struct Colonist;

#[derive(Component)]
pub struct Destination(pub Vec2);

#[derive(Component, Default)]
pub struct Path {
    pub waypoints: Vec<Vec2>,
    pub current_index: usize,
}

#[derive(Component)]
pub struct AnimationState {
    pub is_moving: bool,
    pub facing_right: bool,
    pub bob_timer: f32,
}

impl Default for AnimationState {
    fn default() -> Self {
        Self {
            is_moving: false,
            facing_right: true,
            bob_timer: 0.0,
        }
    }
}

pub fn spawn_colonists(
    mut commands: Commands,
    game_assets: Res<GameAssets>,
    world_map: Res<WorldMap>,
    inventory: crate::systems::logistics::Inventory,
    current_job: crate::systems::jobs::CurrentJob,
) {
    let spawn_pos = Vec2::new(0.0, 0.0);
    let spawn_grid = WorldMap::world_to_grid(spawn_pos);
    
    let mut dest_grid = (spawn_grid.0 + 5, spawn_grid.1 + 5);
    for dx in 0..10 {
        for dy in 0..10 {
            let test = (spawn_grid.0 + dx, spawn_grid.1 + dy);
            if world_map.is_walkable(test.0, test.1) {
                dest_grid = test;
                break;
            }
        }
    }
    let dest_pos = WorldMap::grid_to_world(dest_grid.0, dest_grid.1);
    
    commands.spawn((
        Colonist,
        current_job,
        inventory,
        Sprite {
            image: game_assets.colonist.clone(),
            custom_size: Some(Vec2::splat(TILE_SIZE * 0.8)),
            ..default()
        },
        Transform::from_xyz(spawn_pos.x, spawn_pos.y, 1.0),
        Destination(dest_pos),
        Path::default(),
        AnimationState::default(),
    ));
    
    info!("BEVY_STARTUP: Colonist spawned at {:?}, destination {:?}", spawn_pos, dest_pos);
}

pub fn pathfinding_system(
    world_map: Res<WorldMap>,
    mut query: Query<(&Transform, &Destination, &mut Path), Changed<Destination>>,
) {
    for (transform, destination, mut path) in query.iter_mut() {
        let current_pos = transform.translation.truncate();
        let start_grid = WorldMap::world_to_grid(current_pos);
        let goal_grid = WorldMap::world_to_grid(destination.0);
        
        if let Some(grid_path) = find_path(&world_map, start_grid, goal_grid) {
            path.waypoints = grid_path.iter()
                .map(|&(x, y)| WorldMap::grid_to_world(x, y))
                .collect();
            path.current_index = 0;
            info!("PATH_FOUND: {} waypoints", path.waypoints.len());
        } else {
            info!("PATH_NOT_FOUND");
            path.waypoints.clear();
        }
    }
}

pub fn colonist_movement(
    time: Res<Time>,
    mut query: Query<(&mut Transform, &mut Path, &mut AnimationState), With<Colonist>>,
) {
    for (mut transform, mut path, mut anim) in query.iter_mut() {
        if path.current_index < path.waypoints.len() {
            let target = path.waypoints[path.current_index];
            let current_pos = transform.translation.truncate();
            let to_target = target - current_pos;
            let distance = to_target.length();
            
            if distance > 1.0 {
                let speed = 100.0;
                let move_dist = (speed * time.delta_secs()).min(distance);
                let direction = to_target.normalize();
                let velocity = direction * move_dist;
                transform.translation += velocity.extend(0.0);
                
                anim.is_moving = true;
                if direction.x.abs() > 0.1 {
                    anim.facing_right = direction.x > 0.0;
                }
            } else {
                path.current_index += 1;
            }
        } else {
            anim.is_moving = false;
        }
    }
}

pub fn animation_system(
    time: Res<Time>,
    mut query: Query<(&mut Transform, &mut Sprite, &mut AnimationState), With<Colonist>>,
) {
    for (mut transform, mut sprite, mut anim) in query.iter_mut() {
        sprite.flip_x = !anim.facing_right;
        
        if anim.is_moving {
            anim.bob_timer += time.delta_secs() * 10.0;
            let bob = (anim.bob_timer.sin() * 0.05) + 1.0;
            transform.scale = Vec3::new(1.0, bob, 1.0);
        } else {
            anim.bob_timer += time.delta_secs() * 2.0;
            let breath = (anim.bob_timer.sin() * 0.02) + 1.0;
            transform.scale = Vec3::splat(breath);
        }
    }
}
