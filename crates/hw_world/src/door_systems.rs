//! ドアの自動開閉規則とビジュアルアセットハンドル。

use crate::map::WorldMap;
use bevy::prelude::*;
use hw_core::soul::Path;
use hw_core::world::DoorState;
use hw_jobs::Door;

/// bevy_app から注入されるドア系ビジュアルアセットハンドル。
#[derive(Resource)]
pub struct DoorVisualHandles {
    pub door_open: Handle<Image>,
    pub door_closed: Handle<Image>,
}

pub fn apply_door_state(
    door: &mut Door,
    sprite: &mut Sprite,
    world_map: &mut WorldMap,
    handles: &DoorVisualHandles,
    door_grid: (i32, i32),
    next_state: DoorState,
) {
    door.state = next_state;
    sprite.image = if next_state == DoorState::Open {
        handles.door_open.clone()
    } else {
        handles.door_closed.clone()
    };

    world_map.sync_door_passability(door_grid, next_state);
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DoorOpenEvaluation {
    pub should_open: bool,
    pub waypoints_scanned: u32,
}

/// Evaluates one indexed Soul candidate for an automatic door open.
///
/// The returned waypoint count is always available so profiling features in a
/// caller crate never change this function's signature.
pub fn evaluate_door_auto_open(
    door_state: DoorState,
    soul_grid: (i32, i32),
    path: &Path,
    door_grid: (i32, i32),
) -> DoorOpenEvaluation {
    if door_state != DoorState::Closed {
        return DoorOpenEvaluation::default();
    }
    let nearby = (soul_grid.0 - door_grid.0).abs() <= 1 && (soul_grid.1 - door_grid.1).abs() <= 1;
    if !nearby {
        return DoorOpenEvaluation::default();
    }
    if soul_grid == door_grid {
        return DoorOpenEvaluation {
            should_open: true,
            waypoints_scanned: 0,
        };
    }
    if path.current_index >= path.waypoints.len() {
        return DoorOpenEvaluation::default();
    }

    let mut evaluation = DoorOpenEvaluation::default();
    for waypoint in &path.waypoints[path.current_index..] {
        evaluation.waypoints_scanned = evaluation.waypoints_scanned.saturating_add(1);
        if WorldMap::world_to_grid(*waypoint) == door_grid {
            evaluation.should_open = true;
            break;
        }
    }
    evaluation
}

/// Returns whether an indexed Soul candidate should keep an open door open.
/// Closing intentionally does not require a `Path`; mere proximity is enough.
pub fn soul_keeps_door_open(
    door_state: DoorState,
    soul_grid: (i32, i32),
    door_grid: (i32, i32),
) -> bool {
    door_state == DoorState::Open
        && (soul_grid.0 - door_grid.0).abs() <= 1
        && (soul_grid.1 - door_grid.1).abs() <= 1
}

#[cfg(test)]
mod tests {
    use super::*;

    fn path(grids: &[(i32, i32)], current_index: usize) -> Path {
        Path {
            waypoints: grids
                .iter()
                .map(|&(x, y)| WorldMap::grid_to_world(x, y))
                .collect(),
            current_index,
            ..default()
        }
    }

    #[test]
    fn distant_soul_does_not_scan_path_or_open() {
        let result =
            evaluate_door_auto_open(DoorState::Closed, (0, 0), &path(&[(5, 5)], 0), (5, 5));

        assert_eq!(result, DoorOpenEvaluation::default());
    }

    #[test]
    fn adjacent_soul_opens_when_remaining_path_reaches_door() {
        let result = evaluate_door_auto_open(
            DoorState::Closed,
            (4, 5),
            &path(&[(1, 1), (5, 5)], 1),
            (5, 5),
        );

        assert_eq!(
            result,
            DoorOpenEvaluation {
                should_open: true,
                waypoints_scanned: 1,
            }
        );
    }

    #[test]
    fn waypoints_before_current_index_are_ignored() {
        let result = evaluate_door_auto_open(
            DoorState::Closed,
            (4, 5),
            &path(&[(5, 5), (7, 7)], 1),
            (5, 5),
        );

        assert!(!result.should_open);
        assert_eq!(result.waypoints_scanned, 1);
    }

    #[test]
    fn same_tile_opens_without_a_path() {
        let result = evaluate_door_auto_open(DoorState::Closed, (5, 5), &path(&[], 0), (5, 5));

        assert!(result.should_open);
        assert_eq!(result.waypoints_scanned, 0);
    }

    #[test]
    fn locked_door_never_auto_opens() {
        let result =
            evaluate_door_auto_open(DoorState::Locked, (5, 5), &path(&[(5, 5)], 0), (5, 5));

        assert_eq!(result, DoorOpenEvaluation::default());
    }

    #[test]
    fn only_open_door_is_kept_open_by_proximity() {
        assert!(soul_keeps_door_open(DoorState::Open, (4, 5), (5, 5)));
        assert!(!soul_keeps_door_open(DoorState::Closed, (4, 5), (5, 5)));
        assert!(!soul_keeps_door_open(DoorState::Open, (3, 5), (5, 5)));
    }
}
