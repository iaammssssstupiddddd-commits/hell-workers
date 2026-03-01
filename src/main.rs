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
#[cfg(target_os = "linux")]
use std::path::PathBuf;
#[cfg(target_os = "linux")]
use std::os::unix::net::UnixStream;

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
    configure_linux_window_backend();
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

#[cfg(target_os = "linux")]
fn configure_linux_window_backend() {
    let backend = env::var("HW_WINDOW_BACKEND")
        .ok()
        .map(|value| value.to_ascii_lowercase())
        .unwrap_or_else(|| "auto".to_string());

    match backend.as_str() {
        "x11" => force_x11_backend("HW_WINDOW_BACKEND=x11"),
        "wayland" => {}
        "auto" => {
            if should_fallback_to_x11() {
                force_x11_backend("auto fallback (Wayland socket unavailable)");
            }
        }
        _ => {
            eprintln!(
                "Unknown HW_WINDOW_BACKEND={backend}. Supported values: auto, x11, wayland."
            );
            if should_fallback_to_x11() {
                force_x11_backend("auto fallback (Wayland socket unavailable)");
            }
        }
    }
}

#[cfg(not(target_os = "linux"))]
fn configure_linux_window_backend() {}

#[cfg(target_os = "linux")]
fn should_fallback_to_x11() -> bool {
    let has_x11 = env::var("DISPLAY").map(|value| !value.is_empty()).unwrap_or(false);
    if !has_x11 {
        return false;
    }

    // Respect externally-provided Wayland file descriptors.
    if env::var("WAYLAND_SOCKET")
        .map(|value| !value.is_empty())
        .unwrap_or(false)
    {
        return false;
    }

    let Some(wayland_display) = env::var("WAYLAND_DISPLAY")
        .ok()
        .filter(|value| !value.is_empty())
    else {
        return false;
    };

    let Some(socket_path) = resolve_wayland_socket_path(&wayland_display) else {
        return true;
    };

    if !socket_path.exists() {
        return true;
    }

    UnixStream::connect(socket_path).is_err()
}

#[cfg(target_os = "linux")]
fn resolve_wayland_socket_path(wayland_display: &str) -> Option<PathBuf> {
    let display_path = PathBuf::from(wayland_display);
    if display_path.is_absolute() {
        return Some(display_path);
    }

    env::var_os("XDG_RUNTIME_DIR").map(|runtime_dir| PathBuf::from(runtime_dir).join(display_path))
}

#[cfg(target_os = "linux")]
fn force_x11_backend(reason: &str) {
    // SAFETY: this runs at startup on the main thread before Bevy creates worker threads.
    unsafe {
        env::remove_var("WAYLAND_DISPLAY");
        env::remove_var("WAYLAND_SOCKET");
    }
    eprintln!("Using X11 backend ({reason}).");
}

fn select_backends() -> Backends {
    if let Ok(backends) = env::var("WGPU_BACKEND") {
        let parsed = Backends::from_comma_list(&backends);
        if !parsed.is_empty() {
            return parsed;
        }
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
