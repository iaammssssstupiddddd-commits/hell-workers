//! ボトムバー UI

use crate::interface::ui::components::{MenuAction, MenuButton, ModeText};
use bevy::prelude::*;

/// ボトムバーをスポーン
pub fn spawn_bottom_bar(commands: &mut Commands) {
    commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(50.0),
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                bottom: Val::Px(0.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Start,
                padding: UiRect::all(Val::Px(5.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.8)),
        ))
        .with_children(|parent| {
            let buttons = [
                ("Architect", MenuAction::ToggleArchitect),
                ("Zones", MenuAction::ToggleZones),
                ("Orders", MenuAction::ToggleOrders),
            ];

            for (label, action) in buttons {
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
                        BackgroundColor(Color::srgb(0.2, 0.2, 0.2)),
                        MenuButton(action),
                    ))
                    .with_children(|button| {
                        button.spawn((
                            Text::new(label),
                            TextFont {
                                font_size: 18.0,
                                ..default()
                            },
                            TextColor(Color::WHITE),
                        ));
                    });
            }

            // Mode Display
            parent.spawn((
                Text::new("Mode: Normal"),
                TextFont {
                    font_size: 18.0,
                    ..default()
                },
                TextColor(Color::srgb(0.0, 1.0, 1.0)),
                Node {
                    margin: UiRect::left(Val::Px(20.0)),
                    ..default()
                },
                ModeText,
            ));
        });
}
