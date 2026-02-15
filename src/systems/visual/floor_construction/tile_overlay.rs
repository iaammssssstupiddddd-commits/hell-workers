//! Floor tile visual overlay system
//!
//! Visualizes construction progress with icons and effects.

use crate::assets::GameAssets;
use crate::systems::jobs::floor_construction::{FloorTileBlueprint, FloorTileState};
use bevy::prelude::*;

/// Component tracking visual state of a floor tile
#[derive(Component, Default)]
pub struct FloorTileVisual {
    pub icon_entity: Option<Entity>,
    pub current_state: Option<FloorTileState>,
}

/// Attaches FloorTileVisual to new blueprints
pub fn attach_floor_tile_visual_system(
    mut commands: Commands,
    q_blueprints: Query<Entity, (With<FloorTileBlueprint>, Without<FloorTileVisual>)>,
) {
    for entity in q_blueprints.iter() {
        commands.entity(entity).insert(FloorTileVisual::default());
    }
}

/// Updates visual overlay based on tile state
pub fn update_floor_tile_visual_system(
    mut commands: Commands,
    mut q_tiles: Query<(Entity, &FloorTileBlueprint, &mut FloorTileVisual)>,
    mut q_icon_transforms: Query<&mut Transform>,
    game_assets: Res<GameAssets>,
    time: Res<Time>,
) {
    for (tile_entity, tile, mut visual) in q_tiles.iter_mut() {
        let is_state_changed = visual.current_state != Some(tile.state);
        
        // Always run update logic if animating, otherwise only on state change
        let is_animating = matches!(
            tile.state,
            FloorTileState::Reinforcing { .. } | FloorTileState::Pouring { .. }
        );

        if !is_state_changed && !is_animating {
            continue;
        }

        // Handle state change logic (spawn/despawn icons)
        if is_state_changed {
            visual.current_state = Some(tile.state);
            
            match tile.state {
                FloorTileState::WaitingBones => {
                    spawn_icon(
                        &mut commands,
                        tile_entity,
                        &mut visual,
                        game_assets.icon_bone_small.clone(),
                        Color::srgba(1.0, 1.0, 1.0, 0.5),
                        0.8,
                    );
                }
                FloorTileState::ReinforcingReady => {
                    spawn_icon(
                        &mut commands,
                        tile_entity,
                        &mut visual,
                        game_assets.icon_bone_small.clone(),
                        Color::WHITE,
                        1.0,
                    );
                }
                FloorTileState::ReinforcedComplete => {
                     spawn_icon(
                        &mut commands,
                        tile_entity,
                        &mut visual,
                        game_assets.icon_bone_small.clone(),
                        Color::WHITE,
                        1.0,
                    );
                }
                FloorTileState::WaitingMud | FloorTileState::PouringReady => {
                     spawn_icon(
                        &mut commands,
                        tile_entity,
                        &mut visual,
                        game_assets.icon_stasis_mud_small.clone(),
                        Color::srgba(1.0, 1.0, 1.0, 0.5),
                        0.8,
                    );
                }
                FloorTileState::Complete => {
                    cleanup_icon(&mut commands, &mut visual);
                }
                _ => {
                    // Animating states handled below or fallback
                    if visual.icon_entity.is_none() {
                         // Restore icon if missing during animation start
                         let (icon, color) = match tile.state {
                            FloorTileState::Reinforcing { .. } => (game_assets.icon_bone_small.clone(), Color::WHITE),
                            FloorTileState::Pouring { .. } => (game_assets.icon_stasis_mud_small.clone(), Color::WHITE),
                            _ => (game_assets.icon_bone_small.clone(), Color::WHITE),
                         };
                         spawn_icon(&mut commands, tile_entity, &mut visual, icon, color, 1.0);
                    }
                }
            }
        }

        // Handle animation logic (every frame)
        if let Some(icon_entity) = visual.icon_entity {
            if let Ok(mut transform) = q_icon_transforms.get_mut(icon_entity) {
                match tile.state {
                    FloorTileState::Reinforcing { progress } => {
                        let wobble = (time.elapsed_secs() * 10.0).sin() * 0.1;
                        transform.rotation = Quat::from_rotation_z(wobble);
                        
                        // Scale slightly up
                        let scale = 0.8 + (progress as f32 / 100.0) * 0.2;
                        transform.scale = Vec3::splat(scale);
                    }
                    FloorTileState::Pouring { .. } => {
                         // Scale pulse
                         let pulse = (time.elapsed_secs() * 5.0).sin() * 0.1 + 1.0;
                         transform.scale = Vec3::splat(pulse);
                    }
                    _ => {
                        // Reset transform if needed
                        transform.rotation = Quat::IDENTITY;
                        transform.scale = Vec3::ONE; // Or stored base scale
                    }
                }
            }
        }
    }
}

fn spawn_icon(
    commands: &mut Commands,
    parent: Entity,
    visual: &mut FloorTileVisual,
    image: Handle<Image>,
    color: Color,
    scale: f32,
) {
    cleanup_icon(commands, visual);

    let icon = commands.spawn((
        Sprite {
            image,
            color,
            custom_size: Some(Vec2::splat(16.0)), // Small icon
            ..default()
        },
        Transform::from_translation(Vec3::new(0.0, 0.0, 0.1)).with_scale(Vec3::splat(scale)),
    )).id();

    commands.entity(parent).add_child(icon);
    visual.icon_entity = Some(icon);
}

fn cleanup_icon(commands: &mut Commands, visual: &mut FloorTileVisual) {
    if let Some(entity) = visual.icon_entity {
        commands.entity(entity).despawn_recursive();
        visual.icon_entity = None;
    }
}
