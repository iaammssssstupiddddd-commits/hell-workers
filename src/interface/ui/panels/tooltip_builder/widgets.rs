//! ツールチップ用ウィジェット生成

use crate::assets::GameAssets;
use crate::interface::ui::components::{TooltipBody, TooltipHeader, TooltipProgressBar};
use crate::interface::ui::theme::UiTheme;
use bevy::prelude::*;

use super::text_wrap::{wrap_tooltip_text, TOOLTIP_WRAP_LIMIT_BODY, TOOLTIP_WRAP_LIMIT_ICON_ROW};

pub fn spawn_progress_bar(
    parent: &mut ChildSpawnerCommands,
    label: &str,
    value: f32,
    color: Color,
    game_assets: &GameAssets,
    theme: &UiTheme,
) {
    let clamped = value.clamp(0.0, 1.0);
    parent
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            margin: UiRect::top(Val::Px(4.0)),
            ..default()
        })
        .with_children(|bar_col| {
            bar_col.spawn((
                Text::new(format!("{label}: {:.0}%", clamped * 100.0)),
                TextFont {
                    font: game_assets.font_ui.clone(),
                    font_size: theme.typography.font_size_xs,
                    ..default()
                },
                TextColor(theme.colors.text_secondary_semantic),
                TooltipBody,
            ));

            bar_col
                .spawn((
                    Node {
                        width: Val::Percent(100.0),
                        height: Val::Px(6.0),
                        border_radius: BorderRadius::all(Val::Px(2.0)),
                        margin: UiRect::top(Val::Px(2.0)),
                        ..default()
                    },
                    BackgroundColor(theme.colors.button_default),
                ))
                .with_children(|track| {
                    track.spawn((
                        Node {
                            width: Val::Percent(clamped * 100.0),
                            height: Val::Percent(100.0),
                            border_radius: BorderRadius::all(Val::Px(2.0)),
                            ..default()
                        },
                        BackgroundColor(color),
                        TooltipProgressBar(clamped),
                    ));
                });
        });
}

pub fn spawn_divider(parent: &mut ChildSpawnerCommands, theme: &UiTheme) {
    parent.spawn((
        Node {
            width: Val::Percent(100.0),
            height: Val::Px(1.0),
            margin: UiRect::top(Val::Px(6.0)),
            ..default()
        },
        BackgroundColor(theme.colors.border_default),
    ));
}

pub fn spawn_icon_text_row(
    parent: &mut ChildSpawnerCommands,
    icon: &str,
    text: &str,
    game_assets: &GameAssets,
    theme: &UiTheme,
) {
    parent
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Start,
            column_gap: Val::Px(6.0),
            margin: UiRect::top(Val::Px(4.0)),
            ..default()
        })
        .with_children(|row| {
            row.spawn((
                Text::new(icon),
                TextFont {
                    font: game_assets.font_ui.clone(),
                    font_size: theme.typography.font_size_xs,
                    weight: FontWeight::SEMIBOLD,
                    ..default()
                },
                TextColor(theme.colors.text_accent_semantic),
                TooltipBody,
            ));
            row.spawn(Node {
                flex_direction: FlexDirection::Column,
                flex_grow: 1.0,
                min_width: Val::Px(0.0),
                ..default()
            })
            .with_children(|text_col| {
                for line in wrap_tooltip_text(text, TOOLTIP_WRAP_LIMIT_ICON_ROW) {
                    text_col.spawn((
                        Text::new(line),
                        TextFont {
                            font: game_assets.font_ui.clone(),
                            font_size: theme.typography.font_size_sm,
                            ..default()
                        },
                        TextLayout::new(Justify::Left, LineBreak::WordOrCharacter),
                        TextColor(theme.colors.text_primary_semantic),
                        Node {
                            width: Val::Percent(100.0),
                            ..default()
                        },
                        TooltipBody,
                    ));
                }
            });
        });
}

pub fn spawn_header(
    parent: &mut ChildSpawnerCommands,
    text: &str,
    game_assets: &GameAssets,
    theme: &UiTheme,
) {
    let display_text = wrap_tooltip_text(text, TOOLTIP_WRAP_LIMIT_BODY).join("\n");
    parent.spawn((
        Text::new(display_text),
        TextFont {
            font: game_assets.font_ui.clone(),
            font_size: theme.typography.font_size_md,
            weight: FontWeight::BOLD,
            ..default()
        },
        TextLayout::new(Justify::Left, LineBreak::WordOrCharacter),
        TextColor(theme.colors.text_accent_semantic),
        Node {
            width: Val::Percent(100.0),
            ..default()
        },
        TooltipHeader,
    ));
}

pub fn spawn_body_line(
    parent: &mut ChildSpawnerCommands,
    text: &str,
    game_assets: &GameAssets,
    theme: &UiTheme,
) {
    for line in wrap_tooltip_text(text, TOOLTIP_WRAP_LIMIT_BODY) {
        parent.spawn((
            Text::new(line),
            TextFont {
                font: game_assets.font_ui.clone(),
                font_size: theme.typography.font_size_sm,
                ..default()
            },
            TextLayout::new(Justify::Left, LineBreak::WordOrCharacter),
            TextColor(theme.colors.text_primary_semantic),
            Node {
                width: Val::Percent(100.0),
                ..default()
            },
            TooltipBody,
        ));
    }
}
