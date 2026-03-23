//! 建築ビジュアルシステム

mod components;
mod effects;
mod material_display;
mod progress_bar;
mod worker_indicator;

use bevy::prelude::*;

use crate::animations::{PulseAnimation, update_pulse_animation};
use hw_core::visual_mirror::construction::BlueprintVisualState;

pub use components::{
    BlueprintState, BlueprintVisual, BuildingBounceEffect, CompletionText, DeliveryPopup,
    HasWorkerIndicator, MaterialCounter, MaterialIcon, ProgressBar, WorkerHammerIcon,
};
pub use effects::{
    building_bounce_animation_system, material_delivery_vfx_system, update_completion_text_system,
    update_delivery_popup_system,
};
pub use material_display::{
    cleanup_material_display_system, spawn_material_display_system, update_material_counter_system,
};
pub use progress_bar::{
    cleanup_progress_bars_system, spawn_progress_bar_system, sync_progress_bar_position_system,
    update_progress_bar_fill_system,
};
pub use worker_indicator::{spawn_worker_indicators_system, update_worker_indicators_system};

pub const PROGRESS_BAR_WIDTH: f32 = 24.0;
pub const PROGRESS_BAR_HEIGHT: f32 = 4.0;
pub const PROGRESS_BAR_Y_OFFSET: f32 = -18.0;

pub const MATERIAL_ICON_X_OFFSET: f32 = 20.0;
pub const MATERIAL_ICON_Y_OFFSET: f32 = 10.0;
pub const COUNTER_TEXT_OFFSET: Vec3 = Vec3::new(12.0, 0.0, 0.0);

pub const POPUP_LIFETIME: f32 = 1.0;
pub const COMPLETION_TEXT_LIFETIME: f32 = 1.5;
pub const BOUNCE_DURATION: f32 = 0.4;

pub const COLOR_BLUEPRINT: Color = Color::srgba(0.1, 0.5, 1.0, 1.0);
pub const COLOR_NORMAL: Color = Color::srgba(1.0, 1.0, 1.0, 1.0);

pub const COLOR_PROGRESS_BG: Color = Color::srgba(0.1, 0.1, 0.1, 0.9);
pub const COLOR_PROGRESS_MATERIAL: Color = Color::srgba(1.0, 0.7, 0.1, 1.0);
pub const COLOR_PROGRESS_BUILD: Color = Color::srgba(0.1, 0.9, 0.3, 1.0);

pub fn calculate_blueprint_state(state: &BlueprintVisualState) -> BlueprintState {
    if state.progress > 0.0 {
        BlueprintState::Building
    } else if materials_complete(state) {
        BlueprintState::ReadyToBuild
    } else {
        let total_delivered: u32 = state.material_counts.iter().map(|(_, d, _)| d).sum();
        let flex_delivered = state
            .flexible_material
            .as_ref()
            .map(|(_, d, _)| *d)
            .unwrap_or(0);
        if total_delivered + flex_delivered > 0 {
            BlueprintState::Preparing
        } else {
            BlueprintState::NeedsMaterials
        }
    }
}

fn materials_complete(state: &BlueprintVisualState) -> bool {
    let fixed_done = state.material_counts.iter().all(|(_, d, r)| d >= r);
    let flex_done = state
        .flexible_material
        .as_ref()
        .map(|(_, d, r)| d >= r)
        .unwrap_or(true);
    fixed_done && flex_done
}

pub fn calculate_blueprint_visual_props(state: &BlueprintVisualState) -> (Color, f32) {
    let total_required: u32 = state.material_counts.iter().map(|(_, _, r)| r).sum::<u32>()
        + state
            .flexible_material
            .as_ref()
            .map(|(_, _, r)| *r)
            .unwrap_or(0);
    let total_delivered: u32 = state.material_counts.iter().map(|(_, d, _)| d).sum::<u32>()
        + state
            .flexible_material
            .as_ref()
            .map(|(_, d, _)| *d)
            .unwrap_or(0);

    let material_ratio = if total_required > 0 {
        (total_delivered as f32 / total_required as f32).min(1.0)
    } else {
        1.0
    };

    let opacity = 0.4 + 0.2 * material_ratio + 0.4 * state.progress.min(1.0);

    let color = if state.progress > 0.0 {
        COLOR_NORMAL
    } else {
        COLOR_BLUEPRINT
    };

    (color, opacity)
}

pub fn attach_blueprint_visual_system(
    mut commands: Commands,
    q_blueprints: Query<Entity, (With<BlueprintVisualState>, Without<BlueprintVisual>)>,
) {
    for entity in q_blueprints.iter() {
        commands.entity(entity).insert(BlueprintVisual::default());
    }
}

pub fn update_blueprint_visual_system(
    mut q_blueprints: Query<(&BlueprintVisualState, &mut BlueprintVisual, &mut Sprite)>,
) {
    for (state, mut visual, mut sprite) in q_blueprints.iter_mut() {
        visual.state = calculate_blueprint_state(state);

        let (color, opacity) = calculate_blueprint_visual_props(state);
        sprite.color = color.with_alpha(opacity);
    }
}

pub fn blueprint_pulse_animation_system(
    time: Res<Time>,
    mut q_blueprints: Query<(&mut BlueprintVisual, &mut Sprite)>,
) {
    for (mut visual, mut sprite) in q_blueprints.iter_mut() {
        if visual.state == BlueprintState::Building {
            if visual.pulse_animation.is_none() {
                visual.pulse_animation = Some(PulseAnimation::default());
            }

            if let Some(ref mut pulse) = visual.pulse_animation {
                let pulse_alpha = update_pulse_animation(&time, pulse);
                sprite.color = sprite.color.with_alpha(pulse_alpha);
            }
        } else {
            visual.pulse_animation = None;
        }
    }
}

pub fn blueprint_scale_animation_system(
    mut q_blueprints: Query<(&BlueprintVisualState, &mut Transform), With<BlueprintVisual>>,
) {
    for (state, mut transform) in q_blueprints.iter_mut() {
        let scale = 0.9 + 0.1 * state.progress.min(1.0);
        transform.scale = Vec3::splat(scale);
    }
}
