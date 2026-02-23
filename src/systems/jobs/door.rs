use crate::assets::GameAssets;
use crate::constants::DOOR_CLOSE_DELAY_SECS;
use crate::entities::damned_soul::{DamnedSoul, Path};
use crate::world::map::WorldMap;
use bevy::prelude::*;

#[derive(Component, Debug, Clone, Copy, Reflect)]
#[reflect(Component)]
pub struct Door {
    pub state: DoorState,
}

impl Default for Door {
    fn default() -> Self {
        Self {
            state: DoorState::Closed,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Reflect)]
pub enum DoorState {
    Open,
    Closed,
    Locked,
}

impl Door {
    pub fn is_passable(&self) -> bool {
        self.state != DoorState::Locked
    }

    pub fn is_open(&self) -> bool {
        self.state == DoorState::Open
    }
}

#[derive(Component)]
pub struct DoorCloseTimer {
    pub timer: Timer,
}

pub fn apply_door_state(
    door: &mut Door,
    sprite: &mut Sprite,
    world_map: &mut WorldMap,
    game_assets: &GameAssets,
    door_grid: (i32, i32),
    next_state: DoorState,
) {
    door.state = next_state;
    sprite.image = if next_state == DoorState::Open {
        game_assets.door_open.clone()
    } else {
        game_assets.door_closed.clone()
    };

    match next_state {
        DoorState::Open => world_map.remove_obstacle(door_grid.0, door_grid.1),
        DoorState::Closed | DoorState::Locked => world_map.add_obstacle(door_grid.0, door_grid.1),
    }
    world_map.set_door_state(door_grid.0, door_grid.1, next_state);
}

fn soul_nearby_and_heading_to_door(
    soul_grid: (i32, i32),
    path: &Path,
    door_grid: (i32, i32),
) -> bool {
    let nearby = (soul_grid.0 - door_grid.0).abs() <= 1 && (soul_grid.1 - door_grid.1).abs() <= 1;
    if !nearby {
        return false;
    }
    if soul_grid == door_grid {
        return true;
    }
    if path.current_index >= path.waypoints.len() {
        return false;
    }

    path.waypoints[path.current_index..]
        .iter()
        .map(|&wp| WorldMap::world_to_grid(wp))
        .any(|grid| grid == door_grid)
}

fn any_soul_touching_or_adjacent(
    q_souls: &Query<(&Transform, &Path), With<DamnedSoul>>,
    door_grid: (i32, i32),
) -> bool {
    q_souls.iter().any(|(transform, _)| {
        let soul_grid = WorldMap::world_to_grid(transform.translation.truncate());
        (soul_grid.0 - door_grid.0).abs() <= 1 && (soul_grid.1 - door_grid.1).abs() <= 1
    })
}

pub fn door_auto_open_system(
    mut commands: Commands,
    game_assets: Res<GameAssets>,
    mut world_map: ResMut<WorldMap>,
    q_souls: Query<(&Transform, &Path), With<DamnedSoul>>,
    mut q_doors: Query<(Entity, &Transform, &mut Door, &mut Sprite)>,
) {
    for (entity, transform, mut door, mut sprite) in q_doors.iter_mut() {
        if door.state != DoorState::Closed {
            continue;
        }
        let door_grid = WorldMap::world_to_grid(transform.translation.truncate());
        let should_open = q_souls.iter().any(|(soul_transform, path)| {
            let soul_grid = WorldMap::world_to_grid(soul_transform.translation.truncate());
            soul_nearby_and_heading_to_door(soul_grid, path, door_grid)
        });
        if should_open {
            apply_door_state(
                &mut door,
                &mut sprite,
                &mut world_map,
                &game_assets,
                door_grid,
                DoorState::Open,
            );
            commands.entity(entity).remove::<DoorCloseTimer>();
        }
    }
}

pub fn door_auto_close_system(
    mut commands: Commands,
    time: Res<Time>,
    game_assets: Res<GameAssets>,
    mut world_map: ResMut<WorldMap>,
    q_souls: Query<(&Transform, &Path), With<DamnedSoul>>,
    mut q_doors: Query<(
        Entity,
        &Transform,
        &mut Door,
        &mut Sprite,
        Option<&mut DoorCloseTimer>,
    )>,
) {
    for (entity, transform, mut door, mut sprite, timer_opt) in q_doors.iter_mut() {
        if door.state != DoorState::Open {
            continue;
        }

        let door_grid = WorldMap::world_to_grid(transform.translation.truncate());
        if any_soul_touching_or_adjacent(&q_souls, door_grid) {
            if timer_opt.is_some() {
                commands.entity(entity).remove::<DoorCloseTimer>();
            }
            continue;
        }

        if let Some(mut close_timer) = timer_opt {
            close_timer.timer.tick(time.delta());
            if close_timer.timer.just_finished() {
                apply_door_state(
                    &mut door,
                    &mut sprite,
                    &mut world_map,
                    &game_assets,
                    door_grid,
                    DoorState::Closed,
                );
                commands.entity(entity).remove::<DoorCloseTimer>();
            }
        } else {
            commands.entity(entity).insert(DoorCloseTimer {
                timer: Timer::from_seconds(DOOR_CLOSE_DELAY_SECS, TimerMode::Once),
            });
        }
    }
}
