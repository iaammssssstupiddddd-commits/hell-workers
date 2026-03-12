//! Floor construction phase transition system

use super::components::*;
use crate::systems::jobs::construction_shared::remove_tile_task_components;
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

            // Update all tile states to WaitingMud and collect entities for component removal
            let tile_entities: Vec<Entity> = q_tiles
                .iter_mut()
                .filter(|(_, t)| t.parent_site == site_entity)
                .map(|(tile_entity, mut tile)| {
                    tile.state = FloorTileState::WaitingMud;
                    tile_entity
                })
                .collect();

            // Remove task components (re-added by floor_tile_designation_system when mud is ready)
            remove_tile_task_components(&mut commands, &tile_entities);

            info!(
                "Floor site {:?} → Pouring phase (all {} tiles reinforced)",
                site_entity, site.tiles_total
            );
        }
    }
}
