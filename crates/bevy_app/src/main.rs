#[cfg(feature = "profiling")]
use bevy::diagnostic::FrameTimeDiagnosticsPlugin;
use bevy::prelude::*;
use bevy::render::RenderPlugin;
use bevy::render::settings::{Backends, RenderCreation, WgpuFeatures, WgpuSettings};
#[cfg(feature = "profiling")]
use bevy::time::{Fixed, TimeUpdateStrategy};
use bevy::window::PresentMode;
use bevy_app::{
    DamnedSoulPlugin, DebugInstantBuild, DebugVisible,
    plugins::{
        input::InputPlugin,
        interface::InterfacePlugin,
        logic::LogicPlugin,
        messages::MessagesPlugin,
        spatial::SpatialPlugin,
        startup::{PerfScenarioConfig, StartupPlugin},
        visual::VisualPlugin,
    },
    systems::{GameSystemSet, save::SavePlugin, settings::SettingsPlugin},
};
use hw_core::game_state::PlayMode;
#[cfg(feature = "profiling")]
use hw_core::simulation_rng::FixedAuditSeed;
use std::env;
#[cfg(target_os = "linux")]
use std::os::unix::net::UnixStream;
#[cfg(target_os = "linux")]
use std::path::PathBuf;

fn main() {
    let perf_config = PerfScenarioConfig::try_from_process().unwrap_or_else(|error| {
        eprintln!("Invalid performance scenario configuration: {error}");
        std::process::exit(2);
    });
    if perf_config.enabled() {
        eprintln!(
            "PERF_SCENARIO: seed={} workload={} size={} souls={} familiars={} render={} clock={} warmup={}s measure={}s fixed_hz={} fixed_warmup_ticks={} fixed_audit_ticks={} virtual_speed=1.0 output_dir={}",
            perf_config.master_seed,
            perf_config.workload.as_str(),
            perf_config.size.as_str(),
            perf_config.soul_count,
            perf_config.familiar_count,
            perf_config.render_mode.as_str(),
            perf_config.clock_mode_as_str(),
            perf_config.warmup_secs,
            perf_config.measure_secs,
            perf_config.fixed_step_hz(),
            perf_config.fixed_warmup_ticks(),
            perf_config.fixed_audit_ticks(),
            perf_config.output_dir.as_deref().map_or_else(
                || "<default>".to_string(),
                |path| path.display().to_string()
            ),
        );
    }
    let (render3d_visible, render_perf_toggles) = perf_config.initial_render_resources();
    let log_filter = if perf_config.enabled() {
        "wgpu=error,bevy_app=warn".to_string()
    } else {
        "wgpu=error,bevy_app=info".to_string()
    };
    configure_linux_window_backend();
    let backends = select_backends();
    let present_mode = select_present_mode();
    let mut app = App::new();
    #[cfg(feature = "profiling")]
    let fixed_step_audit = perf_config.uses_fixed_timesteps();
    #[cfg(feature = "profiling")]
    if fixed_step_audit {
        // Bevy 0.19 guarantees that this advances virtual time by the current
        // fixed timestep and runs FixedMain exactly once per App::update.
        // Normal game systems remain in Update; this audit fixes their
        // Time<Virtual> delta without changing their schedule ownership.
        app.insert_resource(Time::<Fixed>::from_hz(perf_config.fixed_step_hz() as f64));
        app.insert_resource(TimeUpdateStrategy::FixedTimesteps(1));
        app.insert_resource(FixedAuditSeed(perf_config.master_seed));
    }
    app.insert_resource(ClearColor(Color::srgb(0.1, 0.1, 0.1)))
        .insert_resource(perf_config)
        .insert_resource(render3d_visible)
        .insert_resource(render_perf_toggles)
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
                    filter: log_filter,
                    ..default()
                })
                .set(RenderPlugin {
                    render_creation: RenderCreation::Automatic(Box::new(WgpuSettings {
                        backends: Some(backends), // WSL は GL を優先
                        features: WgpuFeatures::CLIP_DISTANCES,
                        ..default()
                    })),
                    ..default()
                }),
        )
        .init_resource::<DebugVisible>()
        .init_resource::<DebugInstantBuild>()
        // PlayMode State
        .init_state::<PlayMode>()
        // Messages
        .add_plugins(MessagesPlugin)
        // Entity plugins
        .add_plugins(DamnedSoulPlugin)
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
        .add_plugins(SettingsPlugin)
        .add_plugins(SavePlugin);

    #[cfg(feature = "profiling")]
    if !fixed_step_audit {
        app.add_plugins(FrameTimeDiagnosticsPlugin::default());
    }

    app.run();
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
            eprintln!("Unknown HW_WINDOW_BACKEND={backend}. Supported values: auto, x11, wayland.");
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
    let has_x11 = env::var("DISPLAY")
        .map(|value| !value.is_empty())
        .unwrap_or(false);
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
