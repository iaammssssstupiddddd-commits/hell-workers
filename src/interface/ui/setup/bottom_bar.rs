//! ボトムバー UI

use crate::interface::ui::components::{
    MenuAction, MenuButton, UiInputBlocker, UiNodeRegistry, UiSlot, UiTooltip,
};
use crate::interface::ui::theme::UiTheme;
use bevy::prelude::*;
use bevy::ui::RelativeCursorPosition;
use bevy::ui::{BackgroundGradient, ColorStop, LinearGradient};

/// ボトムバーをスポーン
pub fn spawn_bottom_bar(
    commands: &mut Commands,
    game_assets: &Res<crate::assets::GameAssets>,
    theme: &UiTheme,
    parent_entity: Entity,
    ui_nodes: &mut UiNodeRegistry,
) {
    let bottom_bar = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(theme.spacing.bottom_bar_height),
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                bottom: Val::Px(0.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Start,
                padding: UiRect::all(Val::Px(theme.spacing.bottom_bar_padding)),
                border: UiRect::all(Val::Px(theme.sizes.panel_border_width)),
                border_radius: BorderRadius::all(Val::Px(theme.sizes.panel_corner_radius)),
                ..default()
            },
            BackgroundGradient::from(LinearGradient {
                angle: std::f32::consts::FRAC_PI_2,
                stops: vec![
                    ColorStop::new(theme.panels.bottom_bar.top, Val::Percent(0.0)),
                    ColorStop::new(theme.panels.bottom_bar.bottom, Val::Percent(100.0)),
                ],
                ..default()
            }),
            BorderColor::all(theme.colors.panel_accent_control_bar),
            RelativeCursorPosition::default(),
            UiInputBlocker,
        ))
        .id();
    commands.entity(parent_entity).add_child(bottom_bar);

    commands.entity(bottom_bar).with_children(|parent| {
        let buttons = [
            (
                "Architect",
                "建築モード切替 (B)",
                MenuAction::ToggleArchitect,
                Some("B"),
            ),
            (
                "Zones",
                "ゾーンモード切替 (Z)",
                MenuAction::ToggleZones,
                Some("Z"),
            ),
            ("Orders", "命令メニュー切替", MenuAction::ToggleOrders, None),
            (
                "Dream",
                "Dreamメニュー切替 (D)",
                MenuAction::ToggleDream,
                Some("D"),
            ),
        ];

        for (label, tooltip, action, shortcut) in buttons {
            parent
                .spawn((
                    Button,
                    Node {
                        width: Val::Px(100.0),
                        height: Val::Px(40.0),
                        margin: UiRect::right(Val::Px(10.0)),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        ..default()
                    },
                    BackgroundColor(theme.colors.button_default), // Semantic
                    MenuButton(action),
                    match shortcut {
                        Some(shortcut) => UiTooltip::with_shortcut(tooltip, shortcut),
                        None => UiTooltip::new(tooltip),
                    },
                ))
                .with_children(|button| {
                    button.spawn((
                        Text::new(label),
                        TextFont {
                            font: game_assets.font_ui.clone(),
                            font_size: theme.typography.font_size_base, // Semantic
                            weight: FontWeight::SEMIBOLD,               // Variation
                            ..default()
                        },
                        TextColor(theme.colors.text_primary_semantic), // Semantic
                        Underline,
                        UnderlineColor(theme.colors.accent_ember_bright.with_alpha(0.35)),
                    ));
                });
        }

        // Mode Display
        let mode_text = parent
            .spawn((
                Text::new("Mode: Normal"),
                TextFont {
                    font: game_assets.font_ui.clone(),
                    font_size: theme.typography.font_size_md, // Semantic
                    weight: FontWeight::BOLD,
                    ..default()
                },
                TextColor(theme.colors.accent_ember.with_alpha(0.85)),
                Node {
                    margin: UiRect::left(Val::Px(20.0)),
                    ..default()
                },
                UiSlot::ModeText,
            ))
            .id();
        ui_nodes.set_slot(UiSlot::ModeText, mode_text);
    });
}
