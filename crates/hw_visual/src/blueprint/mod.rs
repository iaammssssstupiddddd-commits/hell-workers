//! 建築ビジュアルシステム

mod components;
mod effects;
mod material_display;
mod progress_bar;
mod worker_indicator;

use bevy::prelude::*;

use crate::animations::{PulseAnimation, update_pulse_animation};
use hw_jobs::Blueprint;

pub use components::*;
pub use effects::*;
pub use material_display::*;
pub use progress_bar::*;
pub use worker_indicator::*;

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

pub fn calculate_blueprint_state(bp: &Blueprint) -> BlueprintState {
    if bp.progress > 0.0 {
        BlueprintState::Building
    } else if bp.materials_complete() {
        BlueprintState::ReadyToBuild
    } else {
        let total_delivered: u32 = bp.delivered_materials.values().sum();
        if total_delivered > 0 {
            BlueprintState::Preparing
        } else {
            BlueprintState::NeedsMaterials
        }
    }
}

pub fn calculate_blueprint_visual_props(bp: &Blueprint) -> (Color, f32) {
    let total_required: u32 = bp.required_materials.values().sum();
    let total_delivered: u32 = bp.delivered_materials.values().sum();

    let material_ratio = if total_required > 0 {
        (total_delivered as f32 / total_required as f32).min(1.0)
    } else {
        1.0
    };

    let opacity = 0.4 + 0.2 * material_ratio + 0.4 * bp.progress.min(1.0);

    let color = if bp.progress > 0.0 {
        COLOR_NORMAL
    } else {
        COLOR_BLUEPRINT
    };

    (color, opacity)
}

pub fn attach_blueprint_visual_system(
    mut commands: Commands,
    q_blueprints: Query<Entity, (With<Blueprint>, Without<BlueprintVisual>)>,
) {
    for entity in q_blueprints.iter() {
        commands.entity(entity).insert(BlueprintVisual::default());
    }
}

pub fn update_blueprint_visual_system(
    mut q_blueprints: Query<(&Blueprint, &mut BlueprintVisual, &mut Sprite)>,
) {
    for (bp, mut visual, mut sprite) in q_blueprints.iter_mut() {
        visual.state = calculate_blueprint_state(bp);

        let (color, opacity) = calculate_blueprint_visual_props(bp);
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
    mut q_blueprints: Query<(&Blueprint, &mut Transform), With<BlueprintVisual>>,
) {
    for (bp, mut transform) in q_blueprints.iter_mut() {
        let scale = 0.9 + 0.1 * bp.progress.min(1.0);
        transform.scale = Vec3::splat(scale);
    }
}
