//! 時間操作 UI

use crate::interface::ui::components::TaskSummaryText;
use crate::systems::time::ClockText;
use bevy::prelude::*;

/// 時間操作UIをスポーン
pub fn spawn_time_control(commands: &mut Commands) {
    commands
        .spawn((Node {
            position_type: PositionType::Absolute,
            right: Val::Px(20.0),
            top: Val::Px(20.0),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::End,
            ..default()
        },))
        .with_children(|parent| {
            parent.spawn((
                Text::new("Day 1, 00:00"),
                TextFont {
                    font_size: 24.0,
                    ..default()
                },
                TextColor(Color::WHITE),
                ClockText,
            ));

            parent
                .spawn(Node {
                    flex_direction: FlexDirection::Row,
                    margin: UiRect::top(Val::Px(5.0)),
                    ..default()
                })
                .with_children(|speed_row| {
                    let speeds = [
                        (crate::systems::time::TimeSpeed::Paused, "||"),
                        (crate::systems::time::TimeSpeed::Normal, ">"),
                        (crate::systems::time::TimeSpeed::Fast, ">>"),
                        (crate::systems::time::TimeSpeed::Super, ">>>"),
                    ];

                    for (speed, label) in speeds {
                        speed_row
                            .spawn((
                                Button,
                                Node {
                                    width: Val::Px(40.0),
                                    height: Val::Px(30.0),
                                    margin: UiRect::left(Val::Px(5.0)),
                                    justify_content: JustifyContent::Center,
                                    align_items: AlignItems::Center,
                                    ..default()
                                },
                                BackgroundColor(Color::srgb(0.2, 0.2, 0.2)),
                                crate::systems::time::SpeedButton(speed),
                            ))
                            .with_children(|btn| {
                                btn.spawn((
                                    Text::new(label),
                                    TextFont {
                                        font_size: 16.0,
                                        ..default()
                                    },
                                    TextColor(Color::WHITE),
                                ));
                            });
                    }
                });

            // Task Summary
            parent
                .spawn(Node {
                    flex_direction: FlexDirection::Column,
                    margin: UiRect::top(Val::Px(10.0)),
                    padding: UiRect::all(Val::Px(5.0)),
                    ..default()
                })
                .with_children(|summary| {
                    summary.spawn((
                        Text::new("Tasks: 0 (0 High)"),
                        TextFont {
                            font_size: 18.0,
                            ..default()
                        },
                        TextColor(Color::srgb(0.8, 0.8, 1.0)),
                        TaskSummaryText,
                    ));
                });
        });
}
