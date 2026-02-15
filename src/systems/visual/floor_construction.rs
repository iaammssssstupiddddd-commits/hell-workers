//! Floor construction visual feedback

use crate::systems::jobs::floor_construction::{FloorTileBlueprint, FloorTileState};
use bevy::prelude::*;

fn progress_to_ratio(progress: u8) -> f32 {
    (progress as f32 / 100.0).clamp(0.0, 1.0)
}

/// Update floor tile sprite color based on construction state.
pub fn update_floor_tile_visuals_system(
    mut q_tiles: Query<(&FloorTileBlueprint, &mut Sprite), Changed<FloorTileBlueprint>>,
) {
    for (tile, mut sprite) in q_tiles.iter_mut() {
        sprite.color = match tile.state {
            FloorTileState::WaitingBones => Color::srgba(0.50, 0.50, 0.80, 0.20),
            FloorTileState::ReinforcingReady => Color::srgba(0.65, 0.65, 0.90, 0.35),
            FloorTileState::Reinforcing { progress } => {
                let t = progress_to_ratio(progress);
                Color::srgba(0.60 + 0.18 * t, 0.58 + 0.14 * t, 0.52 + 0.10 * t, 0.35 + 0.25 * t)
            }
            FloorTileState::ReinforcedComplete => Color::srgba(0.78, 0.72, 0.60, 0.60),
            FloorTileState::WaitingMud => Color::srgba(0.55, 0.44, 0.34, 0.30),
            FloorTileState::PouringReady => Color::srgba(0.60, 0.48, 0.36, 0.45),
            FloorTileState::Pouring { progress } => {
                let t = progress_to_ratio(progress);
                Color::srgba(0.52 - 0.18 * t, 0.44 - 0.14 * t, 0.34 - 0.10 * t, 0.50 + 0.40 * t)
            }
            FloorTileState::Complete => Color::srgba(0.33, 0.33, 0.35, 0.95),
        };
    }
}
