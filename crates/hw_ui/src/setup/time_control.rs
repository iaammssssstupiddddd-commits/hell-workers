//! Compact map HUD: time on the left, population and current work on the right.
use super::UiAssets;
use crate::components::{
    ClockText, DreamIconAbsorb, DreamPoolPulse, MenuAction, MenuButton, SpeedButtonMarker,
    UiInputBlocker, UiNodeRegistry, UiSlot, UiTooltip,
};
use crate::shell::{PopulationText, WorkspaceAction};
use crate::theme::{UiTheme, font_size_rem};
use bevy::prelude::*;
use bevy::ui::RelativeCursorPosition;
use hw_core::game_state::TimeSpeed;

fn hud_node() -> Node {
    Node {
        position_type: PositionType::Absolute,
        top: Val::Px(12.0),
        height: Val::Px(40.0),
        flex_direction: FlexDirection::Row,
        align_items: AlignItems::Center,
        column_gap: Val::Px(8.0),
        padding: UiRect::all(Val::Px(4.0)),
        border_radius: BorderRadius::all(Val::Px(4.0)),
        ..default()
    }
}

fn hud_button(action: MenuAction, theme: &UiTheme) -> impl Bundle {
    (
        Button,
        Node {
            min_height: Val::Px(32.0),
            padding: UiRect::horizontal(Val::Px(8.0)),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            column_gap: Val::Px(6.0),
            ..default()
        },
        BackgroundColor(theme.colors.button_default),
        MenuButton(action),
    )
}

pub fn spawn_time_control(
    commands: &mut Commands,
    assets: &dyn UiAssets,
    theme: &UiTheme,
    parent: Entity,
    registry: &mut UiNodeRegistry,
) {
    crate::world_view::spawn_world_view_ui(commands, parent, assets, theme);
    let font = TextFont {
        font: assets.font_ui().clone().into(),
        font_size: font_size_rem(14.0),
        ..default()
    };
    let clock = commands
        .spawn((
            Node {
                left: Val::Px(12.0),
                ..hud_node()
            },
            BackgroundColor(theme.colors.bg_surface),
            RelativeCursorPosition::default(),
            UiInputBlocker,
        ))
        .with_children(|row| {
            row.spawn((
                Text::new("Day 1, 00:00"),
                font.clone(),
                TextColor(theme.colors.text_primary_semantic),
                ClockText,
            ));
            for (speed, label, tooltip, shortcut) in [
                (TimeSpeed::Paused, "Ⅱ", "一時停止", "1"),
                (TimeSpeed::Normal, "1×", "通常速度", "2"),
                (TimeSpeed::Fast, "2×", "高速", "3"),
                (TimeSpeed::Super, "4×", "超高速", "4"),
            ] {
                row.spawn((
                    hud_button(MenuAction::SetTimeSpeed(speed), theme),
                    SpeedButtonMarker(speed),
                    UiTooltip::with_shortcut(tooltip, shortcut),
                ))
                .with_children(|button| {
                    button.spawn((
                        Text::new(label),
                        font.clone(),
                        TextColor(theme.colors.text_primary_semantic),
                    ));
                });
            }
        })
        .id();
    commands.entity(parent).add_child(clock);

    let status = commands
        .spawn((
            Node {
                right: Val::Px(12.0),
                ..hud_node()
            },
            BackgroundColor(theme.colors.bg_surface),
            RelativeCursorPosition::default(),
            UiInputBlocker,
        ))
        .with_children(|row| {
            row.spawn((
                hud_button(MenuAction::Workspace(WorkspaceAction::OpenEntities), theme),
                UiTooltip::new("世界全体の人数。クリックで管理を開きます"),
            ))
            .with_children(|button| {
                button.spawn((
                    Text::new("使い魔 0 · 魂 0"),
                    font.clone(),
                    TextColor(theme.colors.text_primary_semantic),
                    PopulationText,
                ));
            });
            row.spawn((
                hud_button(
                    MenuAction::Workspace(WorkspaceAction::OpenBlockedTasks),
                    theme,
                ),
                UiTooltip::new("現在止まっている仕事を確認します。通知の未読数とは別です"),
            ))
            .with_children(|button| {
                let text = button
                    .spawn((
                        Text::new("要対応 0"),
                        font.clone(),
                        TextColor(theme.colors.text_primary_semantic),
                        UiSlot::TaskSummaryText,
                    ))
                    .id();
                registry.set_slot(UiSlot::TaskSummaryText, text);
            });
            row.spawn(hud_button(MenuAction::ToggleDream, theme))
                .with_children(|button| {
                    let text = button
                        .spawn((
                            Text::new("Dream 0"),
                            font.clone(),
                            TextColor(theme.colors.accent_soul_bright),
                            UiSlot::DreamPoolText,
                            DreamPoolPulse::default(),
                        ))
                        .id();
                    registry.set_slot(UiSlot::DreamPoolText, text);
                    let icon = button
                        .spawn((
                            Node {
                                width: Val::Px(16.0),
                                height: Val::Px(16.0),
                                ..default()
                            },
                            ImageNode::new(assets.glow_circle().clone()),
                            BackgroundColor(theme.colors.accent_soul_bright),
                            UiSlot::DreamPoolIcon,
                            DreamIconAbsorb::default(),
                        ))
                        .id();
                    registry.set_slot(UiSlot::DreamPoolIcon, icon);
                });
            row.spawn((
                hud_button(MenuAction::ToggleSystemMenu, theme),
                UiTooltip::new("保存・読込・設定"),
            ))
            .with_children(|button| {
                button.spawn((
                    Text::new("メニュー"),
                    font.clone(),
                    TextColor(theme.colors.text_primary_semantic),
                ));
            });
        })
        .id();
    commands.entity(parent).add_child(status);
}
