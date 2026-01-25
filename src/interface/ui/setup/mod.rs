//! UIセットアップモジュール
//!
//! UIの初期構造を構築します。

mod bottom_bar;
mod dialogs;
mod entity_list;
mod panels;
mod submenus;
mod time_control;

use bevy::prelude::*;
use crate::interface::ui::components::FpsText;

fn spawn_fps_display(commands: &mut Commands) {
    commands
        .spawn((Node {
            position_type: PositionType::Absolute,
            left: Val::Px(20.0),
            top: Val::Px(20.0),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Start,
            ..default()
        },))
        .with_children(|parent| {
            parent.spawn((
                Text::new("FPS: --"),
                TextFont {
                    font_size: 18.0,
                    ..default()
                },
                TextColor(Color::srgb(0.0, 1.0, 0.0)),
                FpsText,
            ));
        });
}

/// UI全体をセットアップ
pub fn setup_ui(commands: Commands, game_assets: Res<crate::assets::GameAssets>) {
    setup_ui_internal(commands, game_assets);
}

fn setup_ui_internal(mut commands: Commands, game_assets: Res<crate::assets::GameAssets>) {
    bottom_bar::spawn_bottom_bar(&mut commands, &game_assets);
    submenus::spawn_submenus(&mut commands, &game_assets);
    panels::spawn_panels(&mut commands, &game_assets);
    entity_list::spawn_entity_list_panel(&mut commands, &game_assets);
    time_control::spawn_time_control(&mut commands, &game_assets);
    spawn_fps_display(&mut commands);
    dialogs::spawn_dialogs(&mut commands, &game_assets);
}
