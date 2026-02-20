//! Floor construction phase transition system

use super::components::*;
use bevy::prelude::*;

/// Handles transition from Reinforcing to Pouring phase
pub fn floor_construction_phase_transition_system(
    mut q_sites: Query<(Entity, &mut FloorConstructionSite)>,
    mut q_tiles: Query<(Entity, &mut FloorTileBlueprint)>,
    mut commands: Commands,
) {
    for (site_entity, mut site) in q_sites.iter_mut() {
        if site.phase != FloorConstructionPhase::Reinforcing {
            continue;
        }

        // Check if all tiles are reinforced
        let all_reinforced = q_tiles
            .iter()
            .filter(|(_, tile)| tile.parent_site == site_entity)
            .all(|(_, tile)| matches!(tile.state, FloorTileState::ReinforcedComplete));

        if all_reinforced {
            // Transition to Pouring phase
            site.phase = FloorConstructionPhase::Pouring;

            // Update all tile states to WaitingMud
            for (tile_entity, mut tile) in q_tiles
                .iter_mut()
                .filter(|(_, t)| t.parent_site == site_entity)
            {
                tile.state = FloorTileState::WaitingMud;

                // Remove any existing Designation (from reinforcing phase)
                // This will be re-added by floor_tile_designation_system when mud is ready
                commands.entity(tile_entity).remove::<(
                    crate::systems::jobs::Designation,
                    crate::systems::jobs::TaskSlots,
                    crate::systems::jobs::Priority,
                )>();
            }

            info!(
                "Floor site {:?} → Pouring phase (all {} tiles reinforced)",
                site_entity, site.tiles_total
            );
        }
    }
}
