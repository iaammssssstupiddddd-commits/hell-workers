//! Floor construction visual feedback

use crate::assets::GameAssets;
use crate::constants::{FLOOR_BONES_PER_TILE, TILE_SIZE};
use crate::systems::jobs::floor_construction::{FloorTileBlueprint, FloorTileState};
use bevy::prelude::*;

const MAX_BONE_VISUAL_SLOTS: u8 = 2;

#[derive(Component)]
pub struct FloorTileBoneVisual {
    slot: u8,
}

fn progress_to_ratio(progress: u8) -> f32 {
    (progress as f32 / 100.0).clamp(0.0, 1.0)
}

fn desired_bone_visual_count(tile: &FloorTileBlueprint) -> u8 {
    if matches!(tile.state, FloorTileState::Complete) {
        return 0;
    }

    tile.bones_delivered
        .min(FLOOR_BONES_PER_TILE)
        .min(MAX_BONE_VISUAL_SLOTS as u32) as u8
}

fn bone_visual_offset(slot: u8) -> Vec3 {
    match slot {
        0 => Vec3::new(-TILE_SIZE * 0.18, -TILE_SIZE * 0.10, 0.05),
        1 => Vec3::new(TILE_SIZE * 0.18, TILE_SIZE * 0.10, 0.05),
        _ => Vec3::new(0.0, 0.0, 0.05),
    }
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

/// Sync per-tile bone marker sprites from `bones_delivered`.
pub fn sync_floor_tile_bone_visuals_system(
    mut commands: Commands,
    game_assets: Res<GameAssets>,
    q_tiles: Query<(Entity, &FloorTileBlueprint, Option<&Children>)>,
    q_bone_visuals: Query<&FloorTileBoneVisual>,
) {
    for (tile_entity, tile, children_opt) in q_tiles.iter() {
        let desired_count = desired_bone_visual_count(tile);
        let mut has_slot = [false; MAX_BONE_VISUAL_SLOTS as usize];

        if let Some(children) = children_opt {
            for child in children.iter() {
                let Ok(marker) = q_bone_visuals.get(child) else {
                    continue;
                };

                if marker.slot >= MAX_BONE_VISUAL_SLOTS || marker.slot >= desired_count {
                    commands.entity(child).try_despawn();
                    continue;
                }

                has_slot[marker.slot as usize] = true;
            }
        }

        for slot in 0..desired_count {
            if has_slot[slot as usize] {
                continue;
            }

            let icon = game_assets.icon_bone_small.clone();
            commands.entity(tile_entity).with_children(|parent| {
                parent.spawn((
                    FloorTileBoneVisual { slot },
                    Sprite {
                        image: icon,
                        custom_size: Some(Vec2::splat(TILE_SIZE * 0.34)),
                        color: Color::srgba(1.0, 1.0, 1.0, 0.95),
                        ..default()
                    },
                    Transform::from_translation(bone_visual_offset(slot)),
                    Name::new(format!("FloorTileBoneMarker{}", slot + 1)),
                ));
            });
        }
    }
}
