//! Wall construction visual feedback

use bevy::prelude::*;
use hw_core::constants::{TILE_SIZE, Z_BAR_BG};
use hw_core::visual_mirror::construction::WallSiteVisualState;

use crate::progress_bar::{
    GenericProgressBar, ProgressBarBackground, ProgressBarConfig, ProgressBarFill,
    reconcile_site_progress_bars, sync_progress_bar_fill_position, sync_progress_bar_position,
    update_progress_bar_fill,
};

const WALL_PROGRESS_BAR_WIDTH: f32 = 40.0;
const WALL_PROGRESS_BAR_HEIGHT: f32 = 5.0;
const WALL_PROGRESS_BAR_Y_OFFSET: f32 = TILE_SIZE * 1.25;
const WALL_PROGRESS_BAR_BG_COLOR: Color = Color::srgba(0.1, 0.1, 0.1, 0.9);

#[derive(Component, Default)]
pub struct WallConstructionProgressBar;

type WallConstructionBgQuery<'w, 's> = Query<
    'w,
    's,
    (Entity, &'static ChildOf, &'static mut Transform),
    (
        With<WallConstructionProgressBar>,
        With<ProgressBarBackground>,
        Without<WallSiteVisualState>,
        Without<ProgressBarFill>,
    ),
>;

type WallConstructionFillQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static ChildOf,
        &'static mut Sprite,
        &'static mut Transform,
    ),
    (
        With<WallConstructionProgressBar>,
        With<ProgressBarFill>,
        Without<WallSiteVisualState>,
        Without<ProgressBarBackground>,
    ),
>;
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

/// Spawn/remove phase progress bars for wall construction sites.
pub fn manage_wall_progress_bars_system(
    mut commands: Commands,
    q_sites: Query<
        (Entity, &Transform, &WallSiteVisualState),
        Without<WallConstructionProgressBar>,
    >,
    q_bars: Query<(Entity, &ChildOf), With<WallConstructionProgressBar>>,
) {
    let active = q_sites.iter().filter_map(|(site_entity, _, site)| {
        if !should_show_site_progress(site) {
            return None;
        }
        let config = ProgressBarConfig {
            width: WALL_PROGRESS_BAR_WIDTH,
            height: WALL_PROGRESS_BAR_HEIGHT,
            y_offset: WALL_PROGRESS_BAR_Y_OFFSET,
            bg_color: WALL_PROGRESS_BAR_BG_COLOR,
            fill_color: site_phase_fill_color(site.phase_is_framing),
            z_index: Z_BAR_BG,
        };
        Some((site_entity, config))
    });
    reconcile_site_progress_bars::<WallConstructionProgressBar>(
        &mut commands,
        active,
        q_bars.iter().map(|(bar, parent)| (bar, parent.parent())),
    );
}

/// Update wall phase progress bar fill/position.
pub fn update_wall_progress_bars_system(
    q_sites: Query<(&Transform, &WallSiteVisualState), Without<WallConstructionProgressBar>>,
    q_generic_bars: Query<&GenericProgressBar>,
    mut q_bg_bars: WallConstructionBgQuery,
    mut q_fill_bars: WallConstructionFillQuery,
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
