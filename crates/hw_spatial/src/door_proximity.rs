//! Door auto-open and auto-close adapters backed by the Soul spatial index.
//!
//! Candidate extraction belongs here; door policy and state application remain
//! owned by `hw_world`.

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use hw_core::constants::TILE_SIZE;
use hw_core::soul::{DamnedSoul, Path};
use hw_core::world::DoorState;
use hw_jobs::{Door, DoorCloseTimer};
use hw_world::{
    DoorVisualHandles, WorldMap, WorldMapWrite, apply_door_state, evaluate_door_auto_open,
    soul_keeps_door_open,
};

use crate::{SpatialGrid, SpatialGridOps};

/// Profiling counters for index candidate work performed by door automation.
#[cfg(feature = "profiling")]
#[derive(Resource, Debug, Default)]
pub struct DoorPerfMetrics {
    pub open_souls_scanned: u32,
    pub open_waypoints_scanned: u32,
    pub close_souls_scanned: u32,
}

const DOOR_NEARBY_RADIUS: f32 = TILE_SIZE * 1.5;

type DoorCloseSoulQuery<'w, 's> = Query<'w, 's, &'static Transform, With<DamnedSoul>>;
type DoorCloseQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static Transform,
        &'static mut Door,
        &'static mut Sprite,
        Option<&'static mut DoorCloseTimer>,
    ),
>;
type DoorOpenSoulQuery<'w, 's> =
    Query<'w, 's, (&'static Transform, &'static Path), With<DamnedSoul>>;
type DoorOpenQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static Transform,
        &'static mut Door,
        &'static mut Sprite,
    ),
>;

#[derive(SystemParam)]
pub struct DoorAutoOpenParams<'w, 's> {
    nearby_candidates: Local<'s, Vec<Entity>>,
    q_souls: DoorOpenSoulQuery<'w, 's>,
    q_doors: DoorOpenQuery<'w, 's>,
    #[cfg(feature = "profiling")]
    metrics: ResMut<'w, DoorPerfMetrics>,
}

#[derive(SystemParam)]
pub struct DoorAutoCloseParams<'w, 's> {
    nearby_candidates: Local<'s, Vec<Entity>>,
    q_souls: DoorCloseSoulQuery<'w, 's>,
    q_doors: DoorCloseQuery<'w, 's>,
    #[cfg(feature = "profiling")]
    metrics: ResMut<'w, DoorPerfMetrics>,
}

/// Closed doors inspect only nearby indexed Soul candidates and open when one
/// is on the door tile or has the door in its remaining path.
pub fn door_auto_open_nearby_system(
    mut commands: Commands,
    handles: Res<DoorVisualHandles>,
    soul_grid: Res<SpatialGrid>,
    mut world_map: WorldMapWrite,
    params: DoorAutoOpenParams,
) {
    let DoorAutoOpenParams {
        mut nearby_candidates,
        q_souls,
        mut q_doors,
        #[cfg(feature = "profiling")]
        mut metrics,
    } = params;

    #[cfg(feature = "profiling")]
    let mut souls_scanned = 0u32;
    #[cfg(feature = "profiling")]
    let mut waypoints_scanned = 0u32;

    for (entity, transform, mut door, mut sprite) in q_doors.iter_mut() {
        if door.state != DoorState::Closed {
            continue;
        }

        let door_grid = WorldMap::world_to_grid(transform.translation.truncate());
        soul_grid.get_nearby_in_radius_into(
            transform.translation.truncate(),
            DOOR_NEARBY_RADIUS,
            &mut nearby_candidates,
        );
        let should_open = nearby_candidates.iter().copied().any(|soul_entity| {
            #[cfg(feature = "profiling")]
            {
                souls_scanned = souls_scanned.saturating_add(1);
            }
            let Ok((soul_transform, path)) = q_souls.get(soul_entity) else {
                return false;
            };
            let evaluation = evaluate_door_auto_open(
                door.state,
                WorldMap::world_to_grid(soul_transform.translation.truncate()),
                path,
                door_grid,
            );
            #[cfg(feature = "profiling")]
            {
                waypoints_scanned = waypoints_scanned.saturating_add(evaluation.waypoints_scanned);
            }
            evaluation.should_open
        });

        if should_open {
            apply_door_state(
                &mut door,
                &mut sprite,
                &mut world_map,
                &handles,
                door_grid,
                DoorState::Open,
            );
            commands.entity(entity).remove::<DoorCloseTimer>();
        }
    }

    #[cfg(feature = "profiling")]
    {
        metrics.open_souls_scanned = metrics.open_souls_scanned.saturating_add(souls_scanned);
        metrics.open_waypoints_scanned = metrics
            .open_waypoints_scanned
            .saturating_add(waypoints_scanned);
    }
}

/// Open doors inspect only nearby indexed Souls. A nearby Soul removes an
/// active close timer even when it does not currently have a `Path`.
pub fn door_auto_close_nearby_system(
    mut commands: Commands,
    time: Res<Time>,
    handles: Res<DoorVisualHandles>,
    soul_grid: Res<SpatialGrid>,
    mut world_map: WorldMapWrite,
    params: DoorAutoCloseParams,
) {
    let DoorAutoCloseParams {
        mut nearby_candidates,
        q_souls,
        mut q_doors,
        #[cfg(feature = "profiling")]
        mut metrics,
    } = params;

    #[cfg(feature = "profiling")]
    let mut souls_scanned = 0u32;

    for (entity, transform, mut door, mut sprite, timer_opt) in q_doors.iter_mut() {
        if door.state != DoorState::Open {
            continue;
        }

        let door_grid = WorldMap::world_to_grid(transform.translation.truncate());
        soul_grid.get_nearby_in_radius_into(
            transform.translation.truncate(),
            DOOR_NEARBY_RADIUS,
            &mut nearby_candidates,
        );
        let has_nearby_soul = nearby_candidates.iter().copied().any(|soul_entity| {
            #[cfg(feature = "profiling")]
            {
                souls_scanned = souls_scanned.saturating_add(1);
            }
            q_souls.get(soul_entity).is_ok_and(|soul_transform| {
                soul_keeps_door_open(
                    door.state,
                    WorldMap::world_to_grid(soul_transform.translation.truncate()),
                    door_grid,
                )
            })
        });

        if has_nearby_soul {
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
                    &handles,
                    door_grid,
                    DoorState::Closed,
                );
                commands.entity(entity).remove::<DoorCloseTimer>();
            }
        } else {
            commands.entity(entity).insert(DoorCloseTimer::new());
        }
    }

    #[cfg(feature = "profiling")]
    {
        metrics.close_souls_scanned = metrics.close_souls_scanned.saturating_add(souls_scanned);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_app() -> App {
        let mut app = App::new();
        app.init_resource::<SpatialGrid>()
            .init_resource::<WorldMap>()
            .init_resource::<Time>()
            .insert_resource(DoorVisualHandles {
                door_open: Handle::default(),
                door_closed: Handle::default(),
            });
        #[cfg(feature = "profiling")]
        app.init_resource::<DoorPerfMetrics>();
        app
    }

    fn spawn_door(app: &mut App, grid: (i32, i32), state: DoorState) -> Entity {
        let world = WorldMap::grid_to_world(grid.0, grid.1);
        let entity = app
            .world_mut()
            .spawn((
                Door { state },
                Transform::from_translation(world.extend(0.0)),
                Sprite::default(),
            ))
            .id();
        app.world_mut()
            .resource_mut::<WorldMap>()
            .register_door(grid, entity, state);
        entity
    }

    fn spawn_soul(app: &mut App, grid: (i32, i32), path: &[(i32, i32)]) -> Entity {
        let world = WorldMap::grid_to_world(grid.0, grid.1);
        let soul = app
            .world_mut()
            .spawn((
                DamnedSoul::default(),
                Transform::from_translation(world.extend(0.0)),
                Path {
                    waypoints: path
                        .iter()
                        .map(|&(x, y)| WorldMap::grid_to_world(x, y))
                        .collect(),
                    ..default()
                },
            ))
            .id();
        app.world_mut()
            .resource_mut::<SpatialGrid>()
            .insert(soul, world);
        soul
    }

    #[test]
    fn soul_path_to_door_opens_unlocked_door() {
        let mut app = test_app();
        app.add_systems(Update, door_auto_open_nearby_system);
        let door = spawn_door(&mut app, (5, 5), DoorState::Closed);
        spawn_soul(&mut app, (4, 5), &[(5, 5)]);

        app.update();

        assert_eq!(
            app.world().get::<Door>(door).unwrap().state,
            DoorState::Open
        );
    }

    #[test]
    fn locked_door_does_not_auto_open() {
        let mut app = test_app();
        app.add_systems(Update, door_auto_open_nearby_system);
        let door = spawn_door(&mut app, (5, 5), DoorState::Locked);
        spawn_soul(&mut app, (5, 5), &[(5, 5)]);

        app.update();

        assert_eq!(
            app.world().get::<Door>(door).unwrap().state,
            DoorState::Locked
        );
    }

    #[test]
    fn door_proximity_considers_souls_only() {
        let mut app = test_app();
        app.add_systems(Update, door_auto_open_nearby_system);
        let door = spawn_door(&mut app, (5, 5), DoorState::Closed);
        let world = WorldMap::grid_to_world(5, 5);
        let non_soul = app
            .world_mut()
            .spawn((
                Transform::from_translation(world.extend(0.0)),
                Path::default(),
            ))
            .id();
        app.world_mut()
            .resource_mut::<SpatialGrid>()
            .insert(non_soul, world);

        app.update();

        assert_eq!(
            app.world().get::<Door>(door).unwrap().state,
            DoorState::Closed
        );
    }

    #[test]
    fn door_close_timer_resets_for_nearby_soul() {
        let mut app = test_app();
        app.add_systems(Update, door_auto_close_nearby_system);
        let door = spawn_door(&mut app, (5, 5), DoorState::Open);
        app.world_mut()
            .entity_mut(door)
            .insert(DoorCloseTimer::new());

        let world = WorldMap::grid_to_world(4, 5);
        let soul = app
            .world_mut()
            .spawn((
                DamnedSoul::default(),
                Transform::from_translation(world.extend(0.0)),
            ))
            .id();
        app.world_mut()
            .resource_mut::<SpatialGrid>()
            .insert(soul, world);

        app.update();

        assert!(app.world().get::<DoorCloseTimer>(door).is_none());
        assert_eq!(
            app.world().get::<Door>(door).unwrap().state,
            DoorState::Open
        );
    }

    #[cfg(feature = "profiling")]
    #[test]
    fn indexed_door_query_does_not_scan_all_souls() {
        let mut app = test_app();
        app.add_systems(Update, door_auto_open_nearby_system);
        let _door = spawn_door(&mut app, (5, 5), DoorState::Closed);
        spawn_soul(&mut app, (4, 5), &[(9, 9)]);
        for x in 20..80 {
            spawn_soul(&mut app, (x, 20), &[(5, 5)]);
        }

        app.update();

        let metrics = app.world().resource::<DoorPerfMetrics>();
        assert_eq!(metrics.open_souls_scanned, 1);
        assert_eq!(metrics.open_waypoints_scanned, 1);
    }
}
