use bevy::prelude::*;

use crate::constants::FLOOR_CONSTRUCTION_PRIORITY;
use crate::relationships::TaskWorkers;
use crate::systems::jobs::floor_construction::{FloorTileBlueprint, FloorTileState};
use crate::systems::jobs::{Designation, Priority, TaskSlots, WorkType};

/// System to assign Designation to FloorTileBlueprint based on their state.
///
/// This system runs in TransportRequestSet::Decide phase (after material delivery logic)
/// to prepare tiles for worker assignment.
pub fn floor_tile_designation_system(
    mut commands: Commands,
    mut q_tiles: Query<(
        Entity,
        &Transform,
        &mut FloorTileBlueprint,
        Option<&Designation>,
        Option<&TaskWorkers>,
        &mut Visibility,
    )>,
) {
    for (tile_entity, tile_transform, mut tile, designation_opt, workers_opt, mut visibility) in
        q_tiles.iter_mut()
    {
        if *visibility == Visibility::Hidden {
            *visibility = Visibility::Visible;
        }

        // If a worker abandoned an in-progress tile, return it to Ready so designation can be reissued.
        if workers_opt.map(|w| w.len()).unwrap_or(0) == 0 {
            match tile.state {
                FloorTileState::Reinforcing { .. } => {
                    tile.state = FloorTileState::ReinforcingReady;
                    debug!(
                        "FLOOR_DESIGNATION: reset abandoned reinforcing tile {:?} to ReinforcingReady",
                        tile_entity
                    );
                }
                FloorTileState::Pouring { .. } => {
                    tile.state = FloorTileState::PouringReady;
                    debug!(
                        "FLOOR_DESIGNATION: reset abandoned pouring tile {:?} to PouringReady",
                        tile_entity
                    );
                }
                _ => {}
            }
        }

        let desired_work_type = match tile.state {
            FloorTileState::ReinforcingReady => Some(WorkType::ReinforceFloorTile),
            FloorTileState::PouringReady => Some(WorkType::PourFloorTile),
            _ => None,
        };

        match (desired_work_type, designation_opt) {
            // Need to add designation
            (Some(work_type), None) => {
                commands.entity(tile_entity).try_insert((
                    Transform::from_xyz(
                        tile_transform.translation.x,
                        tile_transform.translation.y,
                        tile_transform.translation.z,
                    ),
                    Visibility::Visible,
                    Designation { work_type },
                    TaskSlots::new(1), // Only 1 worker per tile
                    Priority(FLOOR_CONSTRUCTION_PRIORITY),
                ));
            }
            // Need to remove designation
            (None, Some(_)) => {
                commands.entity(tile_entity).remove::<Designation>();
                commands.entity(tile_entity).remove::<TaskSlots>();
                commands.entity(tile_entity).remove::<Priority>();
            }
            // Already correct or no change needed
            _ => {}
        }
    }
}
