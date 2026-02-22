use crate::constants::{
    DREAM_UI_PULSE_BRIGHTNESS, DREAM_UI_PULSE_DURATION, DREAM_UI_PULSE_TRIGGER_DELTA,
};
use crate::interface::ui::components::{DreamPoolPulse, UiNodeRegistry, UiSlot};
use crate::interface::ui::theme::UiTheme;
use bevy::math::TryStableInterpolate;
use bevy::prelude::*;

#[derive(Component)]
pub struct DreamLossPopupUi {
    pub lifetime: f32,
    pub max_lifetime: f32,
    pub start_y: f32,
}

pub fn update_dream_loss_popup_ui_system(
    mut commands: Commands,
    time: Res<Time>,
    mut q_popups: Query<(Entity, &mut DreamLossPopupUi, &mut Node, &mut TextColor)>,
) {
    let dt = time.delta_secs();
    for (entity, mut popup, mut node, mut text_color) in q_popups.iter_mut() {
        popup.lifetime -= dt;
        if popup.lifetime <= 0.0 {
            commands.entity(entity).try_despawn();
            continue;
        }
        let progress = 1.0 - (popup.lifetime / popup.max_lifetime).clamp(0.0, 1.0);
        node.top = Val::Px(popup.start_y - progress * 25.0);
        let mut color = text_color.0.to_srgba();
        color.alpha = 1.0 - progress;
        text_color.0 = color.into();
    }
}

pub fn update_dream_pool_display_system(
    mut commands: Commands,
    time: Res<Time>,
    assets: Res<crate::assets::GameAssets>,
    dream_pool: Res<crate::entities::damned_soul::DreamPool>,
    theme: Res<UiTheme>,
    ui_nodes: Res<UiNodeRegistry>,
    mut q_text: Query<(&mut Text, &mut TextColor, &mut DreamPoolPulse)>,
) {
    let Some(entity) = ui_nodes.get_slot(UiSlot::DreamPoolText) else {
        return;
    };
    if let Ok((mut text, mut text_color, mut pulse)) = q_text.get_mut(entity) {
        if dream_pool.is_changed() {
            text.0 = format!("Dream: {:.1}", dream_pool.points);
        }

        pulse.timer = (pulse.timer - time.delta_secs()).max(0.0);

        let delta = dream_pool.points - pulse.last_points;
        if delta > 0.0 {
            pulse.pending_gain += delta;
            while pulse.pending_gain >= DREAM_UI_PULSE_TRIGGER_DELTA {
                pulse.pending_gain -= DREAM_UI_PULSE_TRIGGER_DELTA;
                pulse.timer = DREAM_UI_PULSE_DURATION;
            }
        } else if delta < -0.1 {
            // 消費時はアイコンから上に浮かび上がるテキストを発生させる
            if let Some(icon_entity) = ui_nodes.get_slot(UiSlot::DreamPoolIcon) {
                let popup = commands
                    .spawn((
                        Node {
                            position_type: PositionType::Absolute,
                            left: Val::Px(-30.0),
                            top: Val::Px(0.0),
                            ..default()
                        },
                        Text::new(format!("{:.1}", delta)),
                        TextFont {
                            font: assets.font_ui.clone(),
                            font_size: theme.typography.font_size_clock,
                            ..default()
                        },
                        TextColor(theme.colors.task_high_warning),
                        DreamLossPopupUi {
                            lifetime: 1.5,
                            max_lifetime: 1.5,
                            start_y: 0.0,
                        },
                        GlobalZIndex(10050),
                        Name::new("DreamLossPopup"),
                    ))
                    .id();
                commands.entity(icon_entity).add_child(popup);
            }
        }
        pulse.last_points = dream_pool.points;

        let base_color = theme.colors.accent_soul_bright;
        // プラスのパルス（白・発光）
        if pulse.timer > 0.0 {
            let progress = 1.0 - (pulse.timer / DREAM_UI_PULSE_DURATION).clamp(0.0, 1.0);
            let pulse_alpha =
                (progress * std::f32::consts::PI).sin().max(0.0) * DREAM_UI_PULSE_BRIGHTNESS;
            let bright_color = Color::WHITE;
            text_color.0 = base_color
                .try_interpolate_stable(&bright_color, pulse_alpha)
                .unwrap_or(bright_color);
        } else {
            text_color.0 = base_color;
        }
    }
}
