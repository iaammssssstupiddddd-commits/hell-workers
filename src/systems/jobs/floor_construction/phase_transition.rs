//! Floor construction phase transition system

use super::components::*;
use bevy::prelude::*;

/// Handles transition from Reinforcing to Pouring phase
pub fn floor_construction_phase_transition_system(
    mut q_sites: Query<(Entity, &mut FloorConstructionSite)>,
    q_tiles: Query<&FloorTileBlueprint>,
    _commands: Commands,
) {
    for (site_entity, mut site) in q_sites.iter_mut() {
        if site.phase != FloorConstructionPhase::Reinforcing {
            continue;
        }

        // Check if all tiles are reinforced
        let all_reinforced = q_tiles
            .iter()
            .filter(|tile| tile.parent_site == site_entity)
            .all(|tile| matches!(tile.state, FloorTileState::ReinforcedComplete));

        if all_reinforced && site.tiles_reinforced == site.tiles_total {
            // Transition to Pouring phase
            site.phase = FloorConstructionPhase::Pouring;

            // TODO: Update all tile states to WaitingMud
            // This will be properly implemented in Phase 6

            info!("Floor site {:?} → Pouring phase", site_entity);
        }
    }
}
