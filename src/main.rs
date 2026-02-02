mod assets;
mod constants;
mod entities;
mod events;
mod game_state;
mod interface;
mod plugins;
mod relationships;
mod systems;
mod world;

use bevy::prelude::*;
use bevy::render::RenderPlugin;
use bevy::render::settings::{Backends, RenderCreation, WgpuSettings};
use bevy::diagnostic::FrameTimeDiagnosticsPlugin;
use std::env;

use game_state::{
    PlayMode, log_enter_building_mode, log_enter_task_mode, log_enter_zone_mode,
    log_exit_building_mode, log_exit_task_mode, log_exit_zone_mode,
};

use crate::entities::damned_soul::DamnedSoul;
use crate::entities::damned_soul::DamnedSoulPlugin;
use crate::entities::familiar::FamiliarSpawnEvent;
use crate::plugins::{
    InputPlugin, InterfacePlugin, LogicPlugin, SpatialPlugin, StartupPlugin, VisualPlugin,
};
use crate::systems::GameSystemSet;
use crate::systems::familiar_ai::FamiliarAiPlugin;
use crate::systems::jobs::Designation;
use crate::systems::logistics::ResourceItem;
use crate::systems::soul_ai::gathering::GatheringSpot;

/// ゲーム内のデバッグ情報の表示状態（独自実装用）
#[derive(Resource, Default)]
pub struct DebugVisible(pub bool);

#[derive(Resource, Default)]
struct FrameSpikeLogger {
    last_logged_time: f32,
}

fn main() {
    let backends = select_backends();
    App::new()
        .insert_resource(ClearColor(Color::srgb(0.1, 0.1, 0.1)))
        .add_plugins(
            DefaultPlugins
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "Hell Workers".into(),
                        resolution: (1280, 720).into(),
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
        .init_resource::<DebugVisible>()
        .init_resource::<FrameSpikeLogger>()
        // PlayMode State
        .init_state::<PlayMode>()
        .add_systems(OnEnter(PlayMode::BuildingPlace), log_enter_building_mode)
        .add_systems(OnExit(PlayMode::BuildingPlace), log_exit_building_mode)
        .add_systems(OnEnter(PlayMode::ZonePlace), log_enter_zone_mode)
        .add_systems(OnExit(PlayMode::ZonePlace), log_exit_zone_mode)
        .add_systems(OnEnter(PlayMode::TaskDesignation), log_enter_task_mode)
        .add_systems(OnExit(PlayMode::TaskDesignation), log_exit_task_mode)
        // Events
        .add_message::<FamiliarSpawnEvent>()
        .add_message::<crate::events::FamiliarOperationMaxSoulChangedEvent>()
        .add_message::<crate::events::FamiliarAiStateChangedEvent>()
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
        // Diagnostics plugins
        .add_plugins(FrameTimeDiagnosticsPlugin::default())
        .add_systems(Update, frame_spike_logger_system)
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
    if is_wsl() {
        Backends::GL
    } else {
        Backends::PRIMARY
    }
}

fn is_wsl() -> bool {
    env::var("WSL_DISTRO_NAME").is_ok() || env::var("WSL_INTEROP").is_ok()
}

fn frame_spike_logger_system(
    time: Res<Time>,
    mut logger: ResMut<FrameSpikeLogger>,
    q_souls: Query<Entity, With<DamnedSoul>>,
    q_spots: Query<Entity, With<GatheringSpot>>,
    q_designations: Query<Entity, With<Designation>>,
    q_resources: Query<Entity, With<ResourceItem>>,
) {
    let dt = time.delta_secs();
    if dt < 0.2 {
        return;
    }
    let now = time.elapsed_secs();
    if now - logger.last_logged_time < 1.0 {
        return;
    }
    logger.last_logged_time = now;
    info!(
        "PERF_SPIKE: dt={:.3}s souls={} spots={} designations={} resources={}",
        dt,
        q_souls.iter().count(),
        q_spots.iter().count(),
        q_designations.iter().count(),
        q_resources.iter().count()
    );
}
