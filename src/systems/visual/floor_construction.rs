//! Floor construction visual feedback

use crate::assets::GameAssets;
use crate::constants::{FLOOR_BONES_PER_TILE, FLOOR_CURING_DURATION_SECS, TILE_SIZE, Z_BAR_BG};
use crate::systems::jobs::floor_construction::{
    FloorConstructionPhase, FloorConstructionSite, FloorTileBlueprint, FloorTileState,
};
use crate::systems::utils::progress_bar::{
    GenericProgressBar, ProgressBarBackground, ProgressBarConfig, ProgressBarFill,
    spawn_progress_bar, sync_progress_bar_fill_position, sync_progress_bar_position,
    update_progress_bar_fill,
};
use bevy::prelude::*;
use std::collections::HashSet;

const MAX_BONE_VISUAL_SLOTS: u8 = 2;
const FLOOR_CURING_BAR_WIDTH: f32 = 40.0;
const FLOOR_CURING_BAR_HEIGHT: f32 = 5.0;
const FLOOR_CURING_BAR_Y_OFFSET: f32 = TILE_SIZE * 0.75;
const FLOOR_CURING_BAR_BG_COLOR: Color = Color::srgba(0.1, 0.1, 0.1, 0.9);
const FLOOR_CURING_BAR_FILL_COLOR: Color = Color::srgba(0.2, 0.9, 0.3, 1.0);

#[derive(Component)]
pub struct FloorTileBoneVisual {
    slot: u8,
}

#[derive(Component)]
pub struct FloorCuringProgressBar;

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

fn curing_progress_ratio(site: &FloorConstructionSite) -> f32 {
    if FLOOR_CURING_DURATION_SECS <= f32::EPSILON {
        return 1.0;
    }
    (1.0 - site.curing_remaining_secs / FLOOR_CURING_DURATION_SECS).clamp(0.0, 1.0)
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

/// Spawn/remove curing progress bars for floor construction sites.
pub fn manage_floor_curing_progress_bars_system(
    mut commands: Commands,
    q_sites: Query<
        (Entity, &Transform, &FloorConstructionSite),
        Without<FloorCuringProgressBar>,
    >,
    q_bars: Query<(Entity, &ChildOf), With<FloorCuringProgressBar>>,
) {
    let mut curing_sites = HashSet::new();
    let mut bar_parents = HashSet::new();
    for (_, child_of) in q_bars.iter() {
        bar_parents.insert(child_of.parent());
    }

    for (site_entity, site_transform, site) in q_sites.iter() {
        if site.phase != FloorConstructionPhase::Curing {
            continue;
        }

        curing_sites.insert(site_entity);
        if bar_parents.contains(&site_entity) {
            continue;
        }

        let config = ProgressBarConfig {
            width: FLOOR_CURING_BAR_WIDTH,
            height: FLOOR_CURING_BAR_HEIGHT,
            y_offset: FLOOR_CURING_BAR_Y_OFFSET,
            bg_color: FLOOR_CURING_BAR_BG_COLOR,
            fill_color: FLOOR_CURING_BAR_FILL_COLOR,
            z_index: Z_BAR_BG,
        };
        let (bg_entity, fill_entity) =
            spawn_progress_bar(&mut commands, site_entity, site_transform, config);

        commands
            .entity(bg_entity)
            .insert((FloorCuringProgressBar, ChildOf(site_entity)));
        commands
            .entity(fill_entity)
            .insert((FloorCuringProgressBar, ChildOf(site_entity)));
    }

    for (bar_entity, child_of) in q_bars.iter() {
        if !curing_sites.contains(&child_of.parent()) {
            commands.entity(bar_entity).try_despawn();
        }
    }
}

/// Update curing progress bar fill/position.
pub fn update_floor_curing_progress_bars_system(
    q_sites: Query<(&Transform, &FloorConstructionSite), Without<FloorCuringProgressBar>>,
    q_generic_bars: Query<&GenericProgressBar>,
    mut q_bg_bars: Query<
        (Entity, &ChildOf, &mut Transform),
        (
            With<FloorCuringProgressBar>,
            With<ProgressBarBackground>,
            Without<FloorConstructionSite>,
            Without<ProgressBarFill>,
        ),
    >,
    mut q_fill_bars: Query<
        (Entity, &ChildOf, &mut Sprite, &mut Transform),
        (
            With<FloorCuringProgressBar>,
            With<ProgressBarFill>,
            Without<FloorConstructionSite>,
            Without<ProgressBarBackground>,
        ),
    >,
) {
    for (bg_entity, child_of, mut bg_transform) in q_bg_bars.iter_mut() {
        let Ok((site_transform, site)) = q_sites.get(child_of.parent()) else {
            continue;
        };
        if site.phase != FloorConstructionPhase::Curing {
            continue;
        }
        let Ok(generic_bar) = q_generic_bars.get(bg_entity) else {
            continue;
        };
        sync_progress_bar_position(site_transform, &generic_bar.config, &mut bg_transform);
    }

    for (fill_entity, child_of, mut sprite, mut fill_transform) in q_fill_bars.iter_mut() {
        let Ok((site_transform, site)) = q_sites.get(child_of.parent()) else {
            continue;
        };
        if site.phase != FloorConstructionPhase::Curing {
            continue;
        }
        let Ok(generic_bar) = q_generic_bars.get(fill_entity) else {
            continue;
        };

        let progress = curing_progress_ratio(site);
        update_progress_bar_fill(
            progress,
            &generic_bar.config,
            &mut sprite,
            &mut fill_transform,
            Some(FLOOR_CURING_BAR_FILL_COLOR),
        );
        let fill_width = sprite.custom_size.map(|s| s.x).unwrap_or(0.0);
        sync_progress_bar_fill_position(
            site_transform,
            &generic_bar.config,
            fill_width,
            &mut fill_transform,
        );
    }
}
