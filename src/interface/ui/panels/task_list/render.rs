//! タスクリストの UI 再構築

use crate::interface::ui::components::TaskListItem;
use crate::interface::ui::theme::UiTheme;
use bevy::prelude::*;

use super::presenter;
use super::view_model::TaskEntry;
use crate::systems::jobs::WorkType;

/// タスクリストボディの子ノードを再構築
pub fn rebuild_task_list_ui(
    parent: &mut ChildSpawnerCommands,
    snapshot: &[(WorkType, Vec<TaskEntry>)],
    game_assets: &crate::assets::GameAssets,
    theme: &UiTheme,
) {
    if snapshot.is_empty() {
        parent.spawn((
            Text::new("No designations"),
            TextFont {
                font: game_assets.font_ui.clone(),
                font_size: theme.typography.font_size_small,
                ..default()
            },
            TextColor(theme.colors.empty_text),
        ));
        return;
    }

    for (work_type, entries) in snapshot {
        let (header_icon, header_color) = presenter::get_work_type_icon(work_type, game_assets, theme);

        parent
            .spawn(Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                margin: UiRect {
                    top: Val::Px(4.0),
                    bottom: Val::Px(2.0),
                    ..default()
                },
                padding: UiRect::horizontal(Val::Px(6.0)),
                column_gap: Val::Px(4.0),
                ..default()
            })
            .with_children(|row| {
                row.spawn((
                    ImageNode {
                        image: header_icon,
                        color: header_color,
                        ..default()
                    },
                    Node {
                        width: Val::Px(theme.sizes.icon_size),
                        height: Val::Px(theme.sizes.icon_size),
                        ..default()
                    },
                ));
                row.spawn((
                    Text::new(format!("{} ({})", presenter::work_type_label(work_type), entries.len())),
                    TextFont {
                        font: game_assets.font_ui.clone(),
                        font_size: theme.typography.font_size_xs,
                        weight: FontWeight::SEMIBOLD,
                        ..default()
                    },
                    TextColor(theme.colors.text_secondary_semantic),
                ));
            });

        for entry in entries {
            let (item_icon, item_color) = presenter::get_work_type_icon(work_type, game_assets, theme);
            let desc_color = if entry.priority >= 5 {
                theme.colors.accent_ember
            } else {
                theme.colors.text_primary
            };

            parent
                .spawn((
                    Button,
                    Node {
                        width: Val::Percent(100.0),
                        height: Val::Px(theme.sizes.soul_item_height),
                        flex_shrink: 0.0,
                        padding: UiRect::axes(Val::Px(6.0), Val::Px(2.0)),
                        flex_direction: FlexDirection::Row,
                        align_items: AlignItems::Center,
                        column_gap: Val::Px(4.0),
                        border: UiRect::left(Val::Px(0.0)),
                        ..default()
                    },
                    BorderColor::all(Color::NONE),
                    BackgroundColor(theme.colors.list_item_default),
                    TaskListItem(entry.entity),
                ))
                .with_children(|btn| {
                    btn.spawn((
                        ImageNode {
                            image: item_icon,
                            color: item_color,
                            ..default()
                        },
                        Node {
                            width: Val::Px(theme.sizes.icon_size),
                            height: Val::Px(theme.sizes.icon_size),
                            ..default()
                        },
                    ));
                    btn.spawn((
                        Text::new(&entry.description),
                        TextFont {
                            font: game_assets.font_ui.clone(),
                            font_size: theme.typography.font_size_item,
                            ..default()
                        },
                        TextColor(desc_color),
                        Node {
                            flex_grow: 1.0,
                            ..default()
                        },
                    ));
                    if entry.worker_count > 0 {
                        btn.spawn((
                            Text::new(format!("\u{00d7}{}", entry.worker_count)),
                            TextFont {
                                font: game_assets.font_ui.clone(),
                                font_size: theme.typography.font_size_small,
                                ..default()
                            },
                            TextColor(theme.colors.text_secondary),
                        ));
                    }
                });
        }
    }
}
