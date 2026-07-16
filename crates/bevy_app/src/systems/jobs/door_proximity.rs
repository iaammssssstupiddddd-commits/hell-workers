//! Door の自動開閉を Soul 空間インデックスの近傍候補だけで判定する。
//!
//! `hw_world` は `hw_spatial` に依存できないため、map mutation は既存の
//! `apply_door_state` に委譲し、候補抽出だけを root adapter が所有する。

use crate::systems::jobs::{Door, DoorCloseTimer, DoorState, apply_door_state};
use crate::world::map::{WorldMap, WorldMapWrite};
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use hw_core::constants::TILE_SIZE;
use hw_core::soul::{DamnedSoul, Path};
use hw_spatial::{SpatialGrid, SpatialGridOps};
use hw_world::DoorVisualHandles;

/// profiling capture で door の候補絞り込みを確認するための集計値。
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

fn soul_nearby_and_heading_to_door(
    soul_grid: (i32, i32),
    path: &Path,
    door_grid: (i32, i32),
    #[cfg(feature = "profiling")] waypoints_scanned: &mut u32,
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

    for waypoint in &path.waypoints[path.current_index..] {
        #[cfg(feature = "profiling")]
        {
            *waypoints_scanned = waypoints_scanned.saturating_add(1);
        }
        if WorldMap::world_to_grid(*waypoint) == door_grid {
            return true;
        }
    }
    false
}

fn soul_touches_or_is_adjacent(soul_grid: (i32, i32), door_grid: (i32, i32)) -> bool {
    (soul_grid.0 - door_grid.0).abs() <= 1 && (soul_grid.1 - door_grid.1).abs() <= 1
}

/// Closed Door は近傍 Soul の残り path だけを確認して開く。
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
            soul_nearby_and_heading_to_door(
                WorldMap::world_to_grid(soul_transform.translation.truncate()),
                path,
                door_grid,
                #[cfg(feature = "profiling")]
                &mut waypoints_scanned,
            )
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

/// Open Door は近傍 Soul だけで close timer を維持する。
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
                soul_touches_or_is_adjacent(
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

    #[test]
    fn path_waypoint_check_requires_a_nearby_soul() {
        let path = Path {
            waypoints: vec![WorldMap::grid_to_world(5, 5)],
            ..default()
        };
        #[cfg(feature = "profiling")]
        let mut waypoints_scanned = 0;

        assert!(!soul_nearby_and_heading_to_door(
            (1, 1),
            &path,
            (5, 5),
            #[cfg(feature = "profiling")]
            &mut waypoints_scanned,
        ));
    }
}
