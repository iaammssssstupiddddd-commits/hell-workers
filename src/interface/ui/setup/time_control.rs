//! 時間操作 UI

use crate::interface::ui::components::{UiInputBlocker, UiNodeRegistry, UiSlot, UiTooltip};
use crate::interface::ui::theme::UiTheme;
use crate::systems::time::ClockText;
use bevy::prelude::*;
use bevy::ui::RelativeCursorPosition;

/// 時間操作UIをスポーン
pub fn spawn_time_control(
    commands: &mut Commands,
    game_assets: &Res<crate::assets::GameAssets>,
    theme: &UiTheme,
    parent_entity: Entity,
    ui_nodes: &mut UiNodeRegistry,
) {
    let time_control_root = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                right: Val::Px(theme.spacing.panel_margin_x),
                top: Val::Px(theme.sizes.time_control_top),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::End,
                ..default()
            },
            RelativeCursorPosition::default(),
            UiInputBlocker,
        ))
        .id();
    commands.entity(parent_entity).add_child(time_control_root);

    commands.entity(time_control_root).with_children(|parent| {
        parent.spawn((
            Text::new("Day 1, 00:00"),
            TextFont {
                font: game_assets.font_ui.clone(),
                font_size: theme.typography.font_size_clock,
                ..default()
            },
            TextColor(theme.colors.text_primary),
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
                    (
                        crate::systems::time::TimeSpeed::Paused,
                        "||",
                        "一時停止",
                        "1",
                    ),
                    (
                        crate::systems::time::TimeSpeed::Normal,
                        ">",
                        "通常速度 (x1)",
                        "2",
                    ),
                    (
                        crate::systems::time::TimeSpeed::Fast,
                        ">>",
                        "高速 (x2)",
                        "3",
                    ),
                    (
                        crate::systems::time::TimeSpeed::Super,
                        ">>>",
                        "超高速 (x4)",
                        "4",
                    ),
                ];

                for (speed, label, tooltip, shortcut) in speeds {
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
                            BackgroundColor(theme.colors.interactive_default),
                            crate::systems::time::SpeedButton(speed),
                            UiTooltip::with_shortcut(tooltip, shortcut),
                        ))
                        .with_children(|btn| {
                            btn.spawn((
                                Text::new(label),
                                TextFont {
                                    font: game_assets.font_ui.clone(),
                                    font_size: theme.typography.font_size_title,
                                    ..default()
                                },
                                TextColor(theme.colors.text_primary),
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
                let text_entity = summary
                    .spawn((
                        Text::new("Tasks: 0 (0 High)"),
                        TextFont {
                            font: game_assets.font_ui.clone(),
                            font_size: theme.typography.font_size_header,
                            ..default()
                        },
                        TextColor(theme.colors.header_text),
                        UiSlot::TaskSummaryText,
                    ))
                    .id();
                ui_nodes.set_slot(UiSlot::TaskSummaryText, text_entity);
            });
    });
}
