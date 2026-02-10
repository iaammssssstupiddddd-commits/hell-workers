//! ツールチップのフェード制御

use crate::interface::ui::components::{TooltipProgressBar, TooltipBody, TooltipHeader};
use crate::interface::ui::theme::UiTheme;
use bevy::math::TryStableInterpolate;
use bevy::prelude::*;

pub(crate) fn apply_fade_effects(
    tooltip_bg: &mut BackgroundColor,
    tooltip_border: &mut BorderColor,
    q_tooltip_text: &mut Query<&mut TextColor, Or<(With<TooltipHeader>, With<TooltipBody>)>>,
    q_tooltip_progress: &mut Query<
        (&TooltipProgressBar, &mut BackgroundColor),
        Without<crate::interface::ui::components::HoverTooltip>,
    >,
    fade_alpha: f32,
    theme: &UiTheme,
    fade_t: f32,
) {
    let bg = theme.colors.tooltip_bg.to_srgba();
    let bg_target = Color::srgba(bg.red, bg.green, bg.blue, 0.95 * fade_alpha);
    tooltip_bg.0 = tooltip_bg
        .0
        .try_interpolate_stable(&bg_target, fade_t)
        .unwrap_or(bg_target);

    let border = theme.colors.tooltip_border.to_srgba();
    let border_target = Color::srgba(
        border.red,
        border.green,
        border.blue,
        border.alpha * fade_alpha,
    );
    let border_next = tooltip_border
        .top
        .try_interpolate_stable(&border_target, fade_t)
        .unwrap_or(border_target);
    *tooltip_border = BorderColor::all(border_next);

    for mut text_color in q_tooltip_text.iter_mut() {
        let current = text_color.0.to_srgba();
        let text_target =
            Color::srgba(current.red, current.green, current.blue, fade_alpha);
        text_color.0 = text_color
            .0
            .try_interpolate_stable(&text_target, fade_t)
            .unwrap_or(text_target);
    }

    for (progress, mut color) in q_tooltip_progress.iter_mut() {
        let current = color.0.to_srgba();
        let base_alpha = (0.35 + 0.65 * progress.0).clamp(0.0, 1.0);
        let progress_target = Color::srgba(
            current.red,
            current.green,
            current.blue,
            base_alpha * fade_alpha,
        );
        color.0 = color
            .0
            .try_interpolate_stable(&progress_target, fade_t)
            .unwrap_or(progress_target);
    }
}
