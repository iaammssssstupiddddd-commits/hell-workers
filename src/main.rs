mod assets;
mod constants;
mod entities;
mod events;
pub mod game_state;
pub mod interface;
pub mod plugins;
pub mod relationships;
pub mod systems;
pub mod world;

use bevy::prelude::*;
use bevy::render::RenderPlugin;
use bevy::render::settings::{Backends, RenderCreation, WgpuSettings};
use bevy::ui_widgets::popover::PopoverPlugin;
use bevy::window::PresentMode;
use std::env;

use game_state::PlayMode;

use crate::entities::damned_soul::DamnedSoulPlugin;
use crate::plugins::{
    InputPlugin, InterfacePlugin, LogicPlugin, MessagesPlugin, SpatialPlugin, StartupPlugin,
    VisualPlugin,
};
use crate::systems::GameSystemSet;
use crate::systems::familiar_ai::FamiliarAiPlugin;

/// ゲーム内のデバッグ情報の表示状態（独自実装用）
#[derive(Resource, Default)]
pub struct DebugVisible(pub bool);

fn main() {
    let backends = select_backends();
    let present_mode = select_present_mode();
    App::new()
        .insert_resource(ClearColor(Color::srgb(0.1, 0.1, 0.1)))
        .add_plugins(
            DefaultPlugins
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "Hell Workers".into(),
                        resolution: (1280, 720).into(),
                        present_mode,
                        ..default()
                    }),
                    ..default()
                })
                .set(bevy::log::LogPlugin {
                    level: bevy::log::Level::INFO,
                    filter: "wgpu=error,bevy_app=info".to_string(),
                    ..default()
                })
                .set(RenderPlugin {
                    render_creation: RenderCreation::Automatic(WgpuSettings {
                        backends: Some(backends), // WSL は GL を優先
                        ..default()
                    }),
                    ..default()
                }),
        )
        .add_plugins(PopoverPlugin)
        .init_resource::<DebugVisible>()
        // PlayMode State
        .init_state::<PlayMode>()
        // Messages
        .add_plugins(MessagesPlugin)
        // Entity plugins
        .add_plugins(DamnedSoulPlugin)
        .add_plugins(FamiliarAiPlugin)
        // Configure system sets
        .configure_sets(
            Update,
            (
                GameSystemSet::Input,
                GameSystemSet::Spatial.run_if(|time: Res<Time<Virtual>>| !time.is_paused()),
                GameSystemSet::Logic.run_if(|time: Res<Time<Virtual>>| !time.is_paused()),
                GameSystemSet::Actor.run_if(|time: Res<Time<Virtual>>| !time.is_paused()),
                GameSystemSet::Visual,
                GameSystemSet::Interface,
            )
                .chain(),
        )
        // Game plugins
        .add_plugins(StartupPlugin)
        .add_plugins(InputPlugin)
        .add_plugins(SpatialPlugin)
        .add_plugins(LogicPlugin)
        .add_plugins(VisualPlugin)
        .add_plugins(InterfacePlugin)
        .run();
}

fn select_backends() -> Backends {
    if env::var("WGPU_BACKEND").is_ok() {
        return Backends::PRIMARY;
    }
    Backends::VULKAN
}

fn select_present_mode() -> PresentMode {
    match env::var("HW_PRESENT_MODE") {
        Ok(mode) => match mode.to_ascii_lowercase().as_str() {
            "auto_no_vsync" | "novsync" | "off" => PresentMode::AutoNoVsync,
            "fifo" | "vsync" | "on" => PresentMode::Fifo,
            "auto_vsync" | "auto" => PresentMode::AutoVsync,
            "mailbox" => PresentMode::Mailbox,
            "immediate" => PresentMode::Immediate,
            _ => PresentMode::AutoVsync,
        },
        Err(_) => PresentMode::AutoVsync,
    }
}
