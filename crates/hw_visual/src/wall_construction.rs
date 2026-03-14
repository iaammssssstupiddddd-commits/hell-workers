//! Wall construction visual feedback

use bevy::prelude::*;
use hw_core::constants::{TILE_SIZE, Z_BAR_BG};
use hw_core::visual_mirror::construction::{
    WallSiteVisualState, WallTileStateMirror, WallTileVisualMirror,
};
use std::collections::HashSet;

use crate::progress_bar::{
    GenericProgressBar, ProgressBarBackground, ProgressBarConfig, ProgressBarFill,
    spawn_progress_bar, sync_progress_bar_fill_position, sync_progress_bar_position,
    update_progress_bar_fill,
};

const WALL_PROGRESS_BAR_WIDTH: f32 = 40.0;
const WALL_PROGRESS_BAR_HEIGHT: f32 = 5.0;
const WALL_PROGRESS_BAR_Y_OFFSET: f32 = TILE_SIZE * 1.25;
const WALL_PROGRESS_BAR_BG_COLOR: Color = Color::srgba(0.1, 0.1, 0.1, 0.9);

#[derive(Component)]
pub struct WallConstructionProgressBar;

fn progress_to_ratio(progress: u8) -> f32 {
    (progress as f32 / 100.0).clamp(0.0, 1.0)
}

fn site_phase_progress(site: &WallSiteVisualState) -> f32 {
    if site.tiles_total == 0 {
        return 1.0;
    }

    if site.phase_is_framing {
        (site.tiles_framed as f32 / site.tiles_total as f32).clamp(0.0, 1.0)
    } else {
        (site.tiles_coated as f32 / site.tiles_total as f32).clamp(0.0, 1.0)
    }
}

fn site_phase_fill_color(phase_is_framing: bool) -> Color {
    if phase_is_framing {
        Color::srgba(0.88, 0.66, 0.34, 1.0)
    } else {
        Color::srgba(0.58, 0.47, 0.36, 1.0)
    }
}

fn should_show_site_progress(site: &WallSiteVisualState) -> bool {
    if site.tiles_total == 0 {
        return false;
    }

    if site.phase_is_framing {
        site.tiles_framed < site.tiles_total
    } else {
        site.tiles_coated < site.tiles_total
    }
}

/// Update wall tile sprite color based on construction state.
pub fn update_wall_tile_visuals_system(
    mut q_tiles: Query<(&WallTileVisualMirror, &mut Sprite), Changed<WallTileVisualMirror>>,
) {
    for (mirror, mut sprite) in q_tiles.iter_mut() {
        sprite.color = match mirror.state {
            WallTileStateMirror::WaitingWood => Color::srgba(0.78, 0.56, 0.32, 0.25),
            WallTileStateMirror::FramingReady => Color::srgba(0.90, 0.68, 0.36, 0.40),
            WallTileStateMirror::Framing { progress } => {
                let t = progress_to_ratio(progress);
                Color::srgba(
                    0.86 - 0.20 * t,
                    0.66 - 0.20 * t,
                    0.38 - 0.12 * t,
                    0.40 + 0.35 * t,
                )
            }
            WallTileStateMirror::FramedProvisional => Color::srgba(0.58, 0.42, 0.30, 0.70),
            WallTileStateMirror::WaitingMud => Color::srgba(0.55, 0.44, 0.34, 0.30),
            WallTileStateMirror::CoatingReady => Color::srgba(0.62, 0.50, 0.37, 0.45),
            WallTileStateMirror::Coating { progress } => {
                let t = progress_to_ratio(progress);
                Color::srgba(
                    0.56 - 0.22 * t,
                    0.46 - 0.18 * t,
                    0.35 - 0.11 * t,
                    0.50 + 0.42 * t,
                )
            }
            WallTileStateMirror::Complete => Color::srgba(0.35, 0.35, 0.38, 0.95),
        };
    }
}

/// Spawn/remove phase progress bars for wall construction sites.
pub fn manage_wall_progress_bars_system(
    mut commands: Commands,
    q_sites: Query<
        (Entity, &Transform, &WallSiteVisualState),
        Without<WallConstructionProgressBar>,
    >,
    q_bars: Query<(Entity, &ChildOf), With<WallConstructionProgressBar>>,
) {
    let mut active_sites = HashSet::new();
    let mut bar_parents = HashSet::new();
    for (_, child_of) in q_bars.iter() {
        bar_parents.insert(child_of.parent());
    }

    for (site_entity, site_transform, site) in q_sites.iter() {
        if !should_show_site_progress(site) {
            continue;
        }

        active_sites.insert(site_entity);
        if bar_parents.contains(&site_entity) {
            continue;
        }

        let config = ProgressBarConfig {
            width: WALL_PROGRESS_BAR_WIDTH,
            height: WALL_PROGRESS_BAR_HEIGHT,
            y_offset: WALL_PROGRESS_BAR_Y_OFFSET,
            bg_color: WALL_PROGRESS_BAR_BG_COLOR,
            fill_color: site_phase_fill_color(site.phase_is_framing),
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
    q_sites: Query<(&Transform, &WallSiteVisualState), Without<WallConstructionProgressBar>>,
    q_generic_bars: Query<&GenericProgressBar>,
    mut q_bg_bars: Query<
        (Entity, &ChildOf, &mut Transform),
        (
            With<WallConstructionProgressBar>,
            With<ProgressBarBackground>,
            Without<WallSiteVisualState>,
            Without<ProgressBarFill>,
        ),
    >,
    mut q_fill_bars: Query<
        (Entity, &ChildOf, &mut Sprite, &mut Transform),
        (
            With<WallConstructionProgressBar>,
            With<ProgressBarFill>,
            Without<WallSiteVisualState>,
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
            Some(site_phase_fill_color(site.phase_is_framing)),
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
