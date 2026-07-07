//! 一時停止メニュー（Save / Load）

use super::UiAssets;
use crate::components::{MenuAction, MenuButton, PauseMenu, UiInputBlocker};
use crate::theme::UiTheme;
use bevy::prelude::*;
use bevy::ui::RelativeCursorPosition;

fn spawn_menu_button(
    parent: &mut ChildSpawnerCommands,
    game_assets: &dyn UiAssets,
    theme: &UiTheme,
    label: &str,
    action: MenuAction,
) {
    parent
        .spawn((
            Button,
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(36.0),
                margin: UiRect::bottom(Val::Px(8.0)),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(theme.colors.button_default),
            BorderColor::all(theme.colors.dialog_border),
            MenuButton(action),
        ))
        .with_children(|button| {
            button.spawn((
                Text::new(label),
                TextFont {
                    font: game_assets.font_ui().clone().into(),
                    font_size: FontSize::Px(theme.typography.font_size_base),
                    ..default()
                },
                TextColor(theme.colors.text_primary_semantic),
            ));
        });
}

/// 一時停止中に表示する Save / Load メニューをスポーンする。
pub fn spawn_pause_menu(
    commands: &mut Commands,
    game_assets: &dyn UiAssets,
    theme: &UiTheme,
    parent_entity: Entity,
) {
    let pause_menu = commands
        .spawn((
            Node {
                display: Display::None,
                width: Val::Px(260.0),
                position_type: PositionType::Absolute,
                left: Val::Percent(50.0),
                top: Val::Percent(45.0),
                margin: UiRect::left(Val::Px(-130.0)),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(16.0)),
                border: UiRect::all(Val::Px(2.0)),
                border_radius: BorderRadius::all(Val::Px(6.0)),
                row_gap: Val::Px(4.0),
                ..default()
            },
            BackgroundColor(theme.colors.dialog_bg),
            BorderColor::all(theme.colors.dialog_border),
            Interaction::default(),
            RelativeCursorPosition::default(),
            UiInputBlocker,
            PauseMenu,
            ZIndex(35),
        ))
        .id();
    commands.entity(parent_entity).add_child(pause_menu);

    commands.entity(pause_menu).with_children(|parent| {
        parent.spawn((
            Text::new("Paused"),
            TextFont {
                font: game_assets.font_ui().clone().into(),
                font_size: FontSize::Px(theme.typography.font_size_xl),
                ..default()
            },
            TextColor(theme.colors.text_accent),
            Node {
                margin: UiRect::bottom(Val::Px(12.0)),
                align_self: AlignSelf::Center,
                ..default()
            },
        ));

        spawn_menu_button(
            parent,
            game_assets,
            theme,
            "Save Game",
            MenuAction::SaveGame,
        );
        spawn_menu_button(
            parent,
            game_assets,
            theme,
            "Load Game",
            MenuAction::RequestLoadGame,
        );
        spawn_menu_button(
            parent,
            game_assets,
            theme,
            "Settings",
            MenuAction::ToggleSettings,
        );
    });
}
