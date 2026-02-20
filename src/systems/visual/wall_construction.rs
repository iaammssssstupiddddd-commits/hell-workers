//! Wall construction visual feedback

use crate::constants::{TILE_SIZE, Z_BAR_BG};
use crate::systems::jobs::wall_construction::{
    WallConstructionPhase, WallConstructionSite, WallTileBlueprint, WallTileState,
};
use crate::systems::utils::progress_bar::{
    GenericProgressBar, ProgressBarBackground, ProgressBarConfig, ProgressBarFill,
    spawn_progress_bar, sync_progress_bar_fill_position, sync_progress_bar_position,
    update_progress_bar_fill,
};
use bevy::prelude::*;
use std::collections::HashSet;

const WALL_PROGRESS_BAR_WIDTH: f32 = 40.0;
const WALL_PROGRESS_BAR_HEIGHT: f32 = 5.0;
const WALL_PROGRESS_BAR_Y_OFFSET: f32 = TILE_SIZE * 1.25;
const WALL_PROGRESS_BAR_BG_COLOR: Color = Color::srgba(0.1, 0.1, 0.1, 0.9);

#[derive(Component)]
pub struct WallConstructionProgressBar;

fn progress_to_ratio(progress: u8) -> f32 {
    (progress as f32 / 100.0).clamp(0.0, 1.0)
}

fn site_phase_progress(site: &WallConstructionSite) -> f32 {
    if site.tiles_total == 0 {
        return 1.0;
    }

    match site.phase {
        WallConstructionPhase::Framing => {
            (site.tiles_framed as f32 / site.tiles_total as f32).clamp(0.0, 1.0)
        }
        WallConstructionPhase::Coating => {
            (site.tiles_coated as f32 / site.tiles_total as f32).clamp(0.0, 1.0)
        }
    }
}

fn site_phase_fill_color(phase: WallConstructionPhase) -> Color {
    match phase {
        WallConstructionPhase::Framing => Color::srgba(0.88, 0.66, 0.34, 1.0),
        WallConstructionPhase::Coating => Color::srgba(0.58, 0.47, 0.36, 1.0),
    }
}

/// Update wall tile sprite color based on construction state.
pub fn update_wall_tile_visuals_system(
    mut q_tiles: Query<(&WallTileBlueprint, &mut Sprite), Changed<WallTileBlueprint>>,
) {
    for (tile, mut sprite) in q_tiles.iter_mut() {
        sprite.color = match tile.state {
            WallTileState::WaitingWood => Color::srgba(0.78, 0.56, 0.32, 0.25),
            WallTileState::FramingReady => Color::srgba(0.90, 0.68, 0.36, 0.40),
            WallTileState::Framing { progress } => {
                let t = progress_to_ratio(progress);
                Color::srgba(
                    0.86 - 0.20 * t,
                    0.66 - 0.20 * t,
                    0.38 - 0.12 * t,
                    0.40 + 0.35 * t,
                )
            }
            WallTileState::FramedProvisional => Color::srgba(0.58, 0.42, 0.30, 0.70),
            WallTileState::WaitingMud => Color::srgba(0.55, 0.44, 0.34, 0.30),
            WallTileState::CoatingReady => Color::srgba(0.62, 0.50, 0.37, 0.45),
            WallTileState::Coating { progress } => {
                let t = progress_to_ratio(progress);
                Color::srgba(
                    0.56 - 0.22 * t,
                    0.46 - 0.18 * t,
                    0.35 - 0.11 * t,
                    0.50 + 0.42 * t,
                )
            }
            WallTileState::Complete => Color::srgba(0.35, 0.35, 0.38, 0.95),
        };
    }
}

/// Spawn/remove phase progress bars for wall construction sites.
pub fn manage_wall_progress_bars_system(
    mut commands: Commands,
    q_sites: Query<(Entity, &Transform, &WallConstructionSite), Without<WallConstructionProgressBar>>,
    q_bars: Query<(Entity, &ChildOf), With<WallConstructionProgressBar>>,
) {
    let mut active_sites = HashSet::new();
    let mut bar_parents = HashSet::new();
    for (_, child_of) in q_bars.iter() {
        bar_parents.insert(child_of.parent());
    }

    for (site_entity, site_transform, site) in q_sites.iter() {
        active_sites.insert(site_entity);
        if bar_parents.contains(&site_entity) {
            continue;
        }

        let config = ProgressBarConfig {
            width: WALL_PROGRESS_BAR_WIDTH,
            height: WALL_PROGRESS_BAR_HEIGHT,
            y_offset: WALL_PROGRESS_BAR_Y_OFFSET,
            bg_color: WALL_PROGRESS_BAR_BG_COLOR,
            fill_color: site_phase_fill_color(site.phase),
            z_index: Z_BAR_BG,
        };
        let (bg_entity, fill_entity) =
            spawn_progress_bar(&mut commands, site_entity, site_transform, config);

        commands
            .entity(bg_entity)
            .insert((WallConstructionProgressBar, ChildOf(site_entity)));
        commands
            .entity(fill_entity)
            .insert((WallConstructionProgressBar, ChildOf(site_entity)));
    }

    for (bar_entity, child_of) in q_bars.iter() {
        if !active_sites.contains(&child_of.parent()) {
            commands.entity(bar_entity).try_despawn();
        }
    }
}

/// Update wall phase progress bar fill/position.
pub fn update_wall_progress_bars_system(
    q_sites: Query<(&Transform, &WallConstructionSite), Without<WallConstructionProgressBar>>,
    q_generic_bars: Query<&GenericProgressBar>,
    mut q_bg_bars: Query<
        (Entity, &ChildOf, &mut Transform),
        (
            With<WallConstructionProgressBar>,
            With<ProgressBarBackground>,
            Without<WallConstructionSite>,
            Without<ProgressBarFill>,
        ),
    >,
    mut q_fill_bars: Query<
        (Entity, &ChildOf, &mut Sprite, &mut Transform),
        (
            With<WallConstructionProgressBar>,
            With<ProgressBarFill>,
            Without<WallConstructionSite>,
            Without<ProgressBarBackground>,
        ),
    >,
) {
    for (bg_entity, child_of, mut bg_transform) in q_bg_bars.iter_mut() {
        let Ok((site_transform, _site)) = q_sites.get(child_of.parent()) else {
            continue;
        };
        let Ok(generic_bar) = q_generic_bars.get(bg_entity) else {
            continue;
        };
        sync_progress_bar_position(site_transform, &generic_bar.config, &mut bg_transform);
    }

    for (fill_entity, child_of, mut sprite, mut fill_transform) in q_fill_bars.iter_mut() {
        let Ok((site_transform, site)) = q_sites.get(child_of.parent()) else {
            continue;
        };
        let Ok(generic_bar) = q_generic_bars.get(fill_entity) else {
            continue;
        };

        update_progress_bar_fill(
            site_phase_progress(site),
            &generic_bar.config,
            &mut sprite,
            &mut fill_transform,
            Some(site_phase_fill_color(site.phase)),
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
