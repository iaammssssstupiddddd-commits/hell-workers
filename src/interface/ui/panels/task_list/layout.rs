//! タスクリストパネルのレイアウト構築

use crate::interface::ui::components::{
    RightPanelMode, TaskListBody, TaskListPanel, TaskListTabButton, UiInputBlocker, UiNodeRegistry,
    UiSlot,
};
use crate::interface::ui::theme::UiTheme;
use bevy::prelude::*;
use bevy::ui::{BackgroundGradient, ColorStop, LinearGradient};

pub fn spawn_task_list_panel_ui(
    commands: &mut Commands,
    game_assets: &Res<crate::assets::GameAssets>,
    theme: &UiTheme,
    parent_entity: Entity,
    ui_nodes: &mut UiNodeRegistry,
) {
    let root = commands
        .spawn((
            Node {
                width: Val::Px(theme.sizes.info_panel_width),
                min_width: Val::Px(theme.sizes.info_panel_min_width),
                max_width: Val::Px(theme.sizes.info_panel_max_width),
                max_height: Val::Percent(70.0),
                position_type: PositionType::Absolute,
                right: Val::Px(theme.spacing.panel_margin_x),
                top: Val::Px(theme.spacing.panel_top),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(theme.spacing.panel_padding)),
                border: UiRect::all(Val::Px(theme.sizes.panel_border_width)),
                border_radius: BorderRadius::all(Val::Px(theme.sizes.panel_corner_radius)),
                display: Display::None,
                ..default()
            },
            BackgroundGradient::from(LinearGradient {
                angle: 0.0,
                stops: vec![
                    ColorStop::new(theme.panels.info_panel.top, Val::Percent(0.0)),
                    ColorStop::new(theme.panels.info_panel.bottom, Val::Percent(100.0)),
                ],
                ..default()
            }),
            BorderColor::all(theme.colors.border_default),
            UiInputBlocker,
            TaskListPanel,
            UiSlot::TaskListRoot,
        ))
        .id();
    commands.entity(parent_entity).add_child(root);
    ui_nodes.set_slot(UiSlot::TaskListRoot, root);

    commands.entity(root).with_children(|parent| {
        // タブバー
        spawn_tab_bar(parent, game_assets, theme);

        // ヘッダー
        parent.spawn((
            Text::new("Designations"),
            TextFont {
                font: game_assets.font_ui.clone(),
                font_size: theme.typography.font_size_title,
                weight: FontWeight::BOLD,
                ..default()
            },
            TextColor(theme.colors.panel_accent_info_panel),
            Node {
                margin: UiRect::bottom(Val::Px(6.0)),
                ..default()
            },
        ));

        // スクロール可能なリスト領域
        parent.spawn((
            Node {
                flex_direction: FlexDirection::Column,
                flex_grow: 1.0,
                overflow: Overflow::clip_y(),
                ..default()
            },
            TaskListBody,
        ));
    });
}

fn spawn_tab_bar(
    parent: &mut ChildSpawnerCommands,
    game_assets: &crate::assets::GameAssets,
    theme: &UiTheme,
) {
    parent
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            margin: UiRect::bottom(Val::Px(6.0)),
            column_gap: Val::Px(4.0),
            ..default()
        })
        .with_children(|row| {
            spawn_tab_button(row, game_assets, theme, "Info", RightPanelMode::Info);
            spawn_tab_button(row, game_assets, theme, "Tasks", RightPanelMode::TaskList);
        });
}

fn spawn_tab_button(
    parent: &mut ChildSpawnerCommands,
    game_assets: &crate::assets::GameAssets,
    theme: &UiTheme,
    label: &str,
    mode: RightPanelMode,
) {
    parent
        .spawn((
            Button,
            Node {
                padding: UiRect::axes(Val::Px(10.0), Val::Px(4.0)),
                border: UiRect::bottom(Val::Px(2.0)),
                ..default()
            },
            BackgroundColor(Color::NONE),
            BorderColor::all(Color::NONE),
            TaskListTabButton(mode),
        ))
        .with_children(|btn| {
            btn.spawn((
                Text::new(label),
                TextFont {
                    font: game_assets.font_ui.clone(),
                    font_size: theme.typography.font_size_sm,
                    weight: FontWeight::SEMIBOLD,
                    ..default()
                },
                TextColor(theme.colors.text_secondary_semantic),
            ));
        });
}
