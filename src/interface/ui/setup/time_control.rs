//! 時間操作 UI

use crate::interface::ui::components::{
    DreamPoolPulse, MenuAction, MenuButton, SpeedButtonMarker, UiInputBlocker, UiNodeRegistry,
    UiSlot, UiTooltip,
};
use crate::systems::visual::dream::DreamIconAbsorb;
use crate::interface::ui::theme::UiTheme;
use crate::systems::time::{ClockText, TimeSpeed};
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
    // Panel root with semi-transparent background
    let time_control_root = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                right: Val::Px(theme.spacing.panel_margin_x),
                top: Val::Px(theme.sizes.time_control_top),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Stretch,
                padding: UiRect::all(Val::Px(10.0)),
                min_width: Val::Px(180.0),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(6.0)),
                ..default()
            },
            BackgroundColor(theme.colors.time_control_bg),
            BorderColor::all(theme.colors.time_control_border),
            RelativeCursorPosition::default(),
            UiInputBlocker,
        ))
        .id();
    commands.entity(parent_entity).add_child(time_control_root);

    commands.entity(time_control_root).with_children(|panel| {
        // ── Clock row ──
        panel.spawn((
            Text::new("Day 1, 00:00"),
            TextFont {
                font: game_assets.font_ui.clone(),
                font_size: theme.typography.font_size_clock,
                ..default()
            },
            TextColor(theme.colors.text_primary_semantic),
            ClockText,
            Node {
                margin: UiRect::bottom(Val::Px(6.0)),
                ..default()
            },
        ));

        // ── Speed buttons row ──
        panel
            .spawn(Node {
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(4.0),
                margin: UiRect::bottom(Val::Px(8.0)),
                ..default()
            })
            .with_children(|speed_row| {
                let speeds = [
                    (TimeSpeed::Paused, "||", "一時停止", "1"),
                    (TimeSpeed::Normal, ">", "通常速度 (x1)", "2"),
                    (TimeSpeed::Fast, ">>", "高速 (x2)", "3"),
                    (TimeSpeed::Super, ">>>", "超高速 (x4)", "4"),
                ];

                for (speed, label, tooltip, shortcut) in speeds {
                    speed_row
                        .spawn((
                            Button,
                            Node {
                                width: Val::Px(40.0),
                                height: Val::Px(28.0),
                                justify_content: JustifyContent::Center,
                                align_items: AlignItems::Center,
                                border: UiRect::all(Val::Px(1.0)),
                                border_radius: BorderRadius::all(Val::Px(3.0)),
                                ..default()
                            },
                            BackgroundColor(theme.colors.button_default),
                            BorderColor::all(Color::NONE),
                            MenuButton(MenuAction::SetTimeSpeed(speed)),
                            SpeedButtonMarker(speed),
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
                                TextColor(theme.colors.accent_sulfur),
                            ));
                        });
                }
            });

        // ── Separator ──
        panel.spawn((
            Node {
                height: Val::Px(1.0),
                margin: UiRect::bottom(Val::Px(8.0)),
                ..default()
            },
            BackgroundColor(theme.colors.time_control_separator),
        ));

        // ── Task Summary ──
        let task_text_entity = panel
            .spawn((
                Text::new("Tasks: 0 (0 High)"),
                TextFont {
                    font: game_assets.font_ui.clone(),
                    font_size: theme.typography.font_size_status,
                    ..default()
                },
                TextColor(theme.colors.panel_accent_time_control),
                UiSlot::TaskSummaryText,
                Node {
                    margin: UiRect::bottom(Val::Px(4.0)),
                    ..default()
                },
            ))
            .id();
        ui_nodes.set_slot(UiSlot::TaskSummaryText, task_text_entity);

        // ── Dream Pool ──
        panel.spawn((Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            ..default()
        },))
        .with_children(|row| {
            let dream_text_entity = row
                .spawn((
                    Text::new("Dream: 0"),
                    TextFont {
                        font: game_assets.font_ui.clone(),
                        font_size: theme.typography.font_size_status,
                        ..default()
                    },
                    TextColor(theme.colors.accent_soul_bright),
                    UiSlot::DreamPoolText,
                    DreamPoolPulse::default(),
                ))
                .id();
            ui_nodes.set_slot(UiSlot::DreamPoolText, dream_text_entity);

            let icon_entity = row
                .spawn((
                    Node {
                        width: Val::Px(16.0),
                        height: Val::Px(16.0),
                        margin: UiRect::left(Val::Px(6.0)),
                        ..default()
                    },
                    ImageNode::new(game_assets.glow_circle.clone()),
                    BackgroundColor(theme.colors.accent_soul_bright),
                    UiSlot::DreamPoolIcon,
                    DreamIconAbsorb::default(),
                ))
                .id();
            ui_nodes.set_slot(UiSlot::DreamPoolIcon, icon_entity);
        });
    });
}
