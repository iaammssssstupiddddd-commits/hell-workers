//! ボトムバー UI

use crate::interface::ui::components::{MenuAction, MenuButton, ModeText, UiTooltip};
use crate::interface::ui::theme::*;
use bevy::prelude::*;
use bevy::ui::{BackgroundGradient, ColorStop, LinearGradient};

/// ボトムバーをスポーン
pub fn spawn_bottom_bar(commands: &mut Commands, game_assets: &Res<crate::assets::GameAssets>) {
    commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(BOTTOM_BAR_HEIGHT),
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                bottom: Val::Px(0.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Start,
                padding: UiRect::all(Val::Px(BOTTOM_BAR_PADDING)),
                ..default()
            },
            BackgroundGradient::from(LinearGradient {
                angle: std::f32::consts::FRAC_PI_2, // 上から下
                stops: vec![
                    ColorStop::new(COLOR_PANEL_BOTTOM_TOP, Val::Percent(0.0)),
                    ColorStop::new(COLOR_PANEL_BOTTOM_BOTTOM, Val::Percent(100.0)),
                ],
                ..default()
            }),
        ))
        .with_children(|parent| {
            let buttons = [
                (
                    "Architect",
                    "建築モード切替 (B)",
                    MenuAction::ToggleArchitect,
                ),
                ("Zones", "ゾーンモード切替 (Z)", MenuAction::ToggleZones),
                ("Orders", "命令メニュー切替", MenuAction::ToggleOrders),
            ];

            for (label, tooltip, action) in buttons {
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
                        BackgroundColor(COLOR_BUTTON_DEFAULT),
                        MenuButton(action),
                        UiTooltip(tooltip),
                    ))
                    .with_children(|button| {
                        button.spawn((
                            Text::new(label),
                            TextFont {
                                font: game_assets.font_ui.clone(),
                                font_size: FONT_SIZE_TITLE,
                                ..default()
                            },
                            TextColor(COLOR_TEXT_PRIMARY),
                        ));
                    });
            }

            // Mode Display
            parent.spawn((
                Text::new("Mode: Normal"),
                TextFont {
                    font: game_assets.font_ui.clone(),
                    font_size: FONT_SIZE_TITLE,
                    ..default()
                },
                TextColor(COLOR_TEXT_ACCENT),
                Node {
                    margin: UiRect::left(Val::Px(20.0)),
                    ..default()
                },
                ModeText,
            ));
        });
}
