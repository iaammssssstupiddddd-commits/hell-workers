//! UIセットアップモジュール
//!
//! UIの初期構造を構築します。

mod bottom_bar;
mod dialogs;
mod panels;
mod submenus;
mod time_control;

use bevy::prelude::*;

/// UI全体をセットアップ
pub fn setup_ui(commands: Commands) {
    setup_ui_internal(commands);
}

fn setup_ui_internal(mut commands: Commands) {
    bottom_bar::spawn_bottom_bar(&mut commands);
    submenus::spawn_submenus(&mut commands);
    panels::spawn_panels(&mut commands);
    time_control::spawn_time_control(&mut commands);
    dialogs::spawn_dialogs(&mut commands);
}
