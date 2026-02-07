//! UIセットアップモジュール
//!
//! UIの初期構造を構築します。

mod bottom_bar;
mod dialogs;
mod entity_list;
mod panels;
mod submenus;
mod time_control;

use crate::interface::ui::components::UiSlot;
use crate::interface::ui::theme::UiTheme;
use bevy::prelude::*;

fn spawn_fps_display(commands: &mut Commands, theme: &UiTheme) {
    commands
        .spawn((Node {
            position_type: PositionType::Absolute,
            left: Val::Px(theme.sizes.fps_left),
            top: Val::Px(theme.sizes.fps_top),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Start,
            ..default()
        },))
        .with_children(|parent| {
            parent.spawn((
                Text::new("FPS: --"),
                TextFont {
                    font_size: theme.typography.font_size_title,
                    ..default()
                },
                TextColor(theme.colors.text_primary),
                UiSlot::FpsText,
            ));
        });
}

/// UI全体をセットアップ
pub fn setup_ui(commands: Commands, game_assets: Res<crate::assets::GameAssets>, theme: Res<UiTheme>) {
    setup_ui_internal(commands, game_assets, theme);
}

fn setup_ui_internal(mut commands: Commands, game_assets: Res<crate::assets::GameAssets>, theme: Res<UiTheme>) {
    bottom_bar::spawn_bottom_bar(&mut commands, &game_assets, &theme);
    submenus::spawn_submenus(&mut commands, &game_assets, &theme);
    panels::spawn_panels(&mut commands, &game_assets, &theme);
    entity_list::spawn_entity_list_panel(&mut commands, &game_assets, &theme);
    time_control::spawn_time_control(&mut commands, &game_assets, &theme);
    spawn_fps_display(&mut commands, &theme);
    dialogs::spawn_dialogs(&mut commands, &game_assets, &theme);
}
