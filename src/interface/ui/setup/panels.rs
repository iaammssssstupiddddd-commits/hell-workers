//! パネル UI (Hover Tooltip)
//!
//! InfoPanelは選択変更時に動的にspawn/despawnされるため、
//! ここではHoverTooltipのみをStartup時にspawnする。

use crate::interface::ui::components::{HoverTooltip, UiSlot};
use crate::interface::ui::theme::UiTheme;
use bevy::prelude::*;

/// パネルをスポーン
pub fn spawn_panels(commands: &mut Commands, game_assets: &Res<crate::assets::GameAssets>, theme: &UiTheme) {
    spawn_hover_tooltip(commands, game_assets, theme);
}

fn spawn_hover_tooltip(commands: &mut Commands, game_assets: &Res<crate::assets::GameAssets>, theme: &UiTheme) {
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                display: Display::None,
                flex_direction: FlexDirection::Column,
                border: UiRect::all(Val::Px(1.0)),
                padding: UiRect::all(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(theme.colors.tooltip_bg),
            BorderColor::all(theme.colors.tooltip_border),
            HoverTooltip,
            ZIndex(100),
        ))
        .with_children(|tooltip| {
            tooltip.spawn((
                Text::new(""),
                TextFont {
                    font: game_assets.font_ui.clone(),
                    font_size: theme.typography.font_size_small,
                    ..default()
                },
                TextColor(theme.colors.text_primary),
                UiSlot::HoverTooltipText,
            ));
        });
}
