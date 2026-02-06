//! 時間操作 UI

use crate::interface::ui::components::TaskSummaryText;
use crate::interface::ui::components::UiTooltip;
use crate::interface::ui::theme::*;
use crate::systems::time::ClockText;
use bevy::prelude::*;

/// 時間操作UIをスポーン
pub fn spawn_time_control(commands: &mut Commands, game_assets: &Res<crate::assets::GameAssets>) {
    commands
        .spawn((Node {
            position_type: PositionType::Absolute,
            right: Val::Px(PANEL_MARGIN_X),
            top: Val::Px(TIME_CONTROL_TOP),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::End,
            ..default()
        },))
        .with_children(|parent| {
            parent.spawn((
                Text::new("Day 1, 00:00"),
                TextFont {
                    font: game_assets.font_ui.clone(),
                    font_size: FONT_SIZE_CLOCK,
                    ..default()
                },
                TextColor(COLOR_TEXT_PRIMARY),
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
                        (crate::systems::time::TimeSpeed::Paused, "||", "一時停止"),
                        (
                            crate::systems::time::TimeSpeed::Normal,
                            ">",
                            "通常速度 (x1)",
                        ),
                        (crate::systems::time::TimeSpeed::Fast, ">>", "高速 (x2)"),
                        (crate::systems::time::TimeSpeed::Super, ">>>", "超高速 (x4)"),
                    ];

                    for (speed, label, tooltip) in speeds {
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
                                BackgroundColor(COLOR_BUTTON_DEFAULT),
                                crate::systems::time::SpeedButton(speed),
                                UiTooltip(tooltip),
                            ))
                            .with_children(|btn| {
                                btn.spawn((
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
                            font: game_assets.font_ui.clone(),
                            font_size: FONT_SIZE_HEADER,
                            ..default()
                        },
                        TextColor(COLOR_HEADER_TEXT),
                        TaskSummaryText,
                    ));
                });
        });
}
