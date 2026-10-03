use bevy::app::{AppExit, ScheduleRunnerPlugin};
use bevy::gilrs::GilrsPlugin;
use bevy::prelude::*;
use bevy::render::RenderPlugin;
use bevy::render::settings::{Backends, RenderCreation, WgpuSettings};
use bevy::window::{ExitCondition, PresentMode, WindowResolution};
use bevy::winit::{WinitPlugin, WinitSettings};
use bevy_app::{HellWorkersGamePlugin, plugins::startup::PerfScenarioConfig};
use std::env;
#[cfg(target_os = "linux")]
use std::os::unix::net::UnixStream;
#[cfg(target_os = "linux")]
use std::path::PathBuf;
use std::time::Duration;

fn main() -> AppExit {
    // Offline authoring uses the exact runtime codec without creating an App,
    // opening a window, loading assets, or granting runtime candidate access.
    if env::args().nth(1).as_deref() == Some("--building-asset-codec") {
        use std::io::{Read, Write};
        let mut input = Vec::new();
        let result = std::io::stdin()
            .take(16 * 1024 * 1024)
            .read_to_end(&mut input)
            .map_err(|error| error.to_string())
            .and_then(|_| {
                bevy_app::project_building_asset_json(&input).map_err(|error| error.to_string())
            })
            .and_then(|bytes| {
                std::io::stdout()
                    .write_all(&bytes)
                    .map_err(|error| error.to_string())
            });
        return match result {
            Ok(()) => AppExit::Success,
            Err(error) => {
                eprintln!("Building asset codec: {error}");
                AppExit::error()
            }
        };
    }
    let perf_config = PerfScenarioConfig::try_from_process().unwrap_or_else(|error| {
        eprintln!("Invalid performance scenario configuration: {error}");
        std::process::exit(2);
    });
    let m2_probe = env::var_os("HW_M2_ACCEPTANCE_PROBE");
    if !building_art_entry_allowed(
        cfg!(feature = "profiling"),
        env::var_os("HW_BUILDING_ART_SESSION").is_some(),
        m2_probe.as_deref(),
        perf_config.enabled(),
        perf_config.workload() == bevy_app::plugins::startup::PerfWorkload::BuildingArtStatic,
    ) {
        eprintln!(
            "Building-art admission requires a profiling static fixture or explicit interactive M2 probe."
        );
        return AppExit::error();
    }
    let native_acceptance_plugin =
        bevy_app::systems::save::NativeSaveLoadAcceptancePlugin::try_from_process().unwrap_or_else(
            |error| {
                eprintln!("Invalid native save/load acceptance configuration: {error}");
                std::process::exit(2);
            },
        );
    let native_deconstruction_plugin =
        bevy_app::systems::jobs::NativeDeconstructionAcceptancePlugin::try_from_process()
            .unwrap_or_else(|error| {
                eprintln!("Invalid native deconstruction acceptance configuration: {error}");
                std::process::exit(2);
            });
    let native_notification_plugin =
        bevy_app::interface::ui::notifications::NativeNotificationAcceptancePlugin::try_from_process()
            .unwrap_or_else(|error| {
                eprintln!("Invalid native notification acceptance configuration: {error}");
                std::process::exit(2);
            });
    #[cfg(feature = "profiling")]
    let native_ui_plugin =
        bevy_app::interface::ui::native_acceptance::NativeUiAcceptancePlugin::try_from_process()
            .unwrap_or_else(|error| {
                eprintln!("Invalid UI acceptance configuration: {error}");
                std::process::exit(2);
            });
    #[cfg(feature = "profiling")]
    let native_ui_enabled = native_ui_plugin.is_some();
    #[cfg(not(feature = "profiling"))]
    let native_ui_enabled = false;
    if (native_acceptance_plugin.is_some()
        || native_deconstruction_plugin.is_some()
        || native_notification_plugin.is_some()
        || native_ui_enabled)
        && perf_config.enabled()
    {
        eprintln!("Native acceptance cannot be combined with a performance scenario.");
        std::process::exit(2);
    }
    let native_acceptance_count = [
        native_acceptance_plugin.is_some(),
        native_deconstruction_plugin.is_some(),
        native_notification_plugin.is_some(),
        native_ui_enabled,
        m2_probe.is_some(),
    ]
    .into_iter()
    .filter(|enabled| *enabled)
    .count();
    if native_acceptance_count > 1 {
        eprintln!("Native acceptance profiles cannot run together.");
        std::process::exit(2);
    }
    let use_headless_runner = headless_runner_requested(&perf_config);
    let window_update_settings =
        perf_window_update_settings(perf_config.enabled(), use_headless_runner);
    let window_resolution = perf_window_resolution(&perf_config);
    #[cfg(feature = "profiling")]
    let window_resolution = native_ui_plugin
        .as_ref()
        .map_or(window_resolution, |plugin| plugin.window_resolution());
    let game_plugin = HellWorkersGamePlugin::new(perf_config);
    let log_filter = game_plugin.log_filter().to_string();
    configure_linux_window_backend();
    let backends = select_backends();
    let present_mode = select_present_mode();
    let mut app = App::new();
    if let Err(error) = bevy_app::configure_building_asset_releases(&mut app) {
        eprintln!("Invalid building asset release bindings: {error}");
        return AppExit::error();
    }
    #[cfg(feature = "profiling")]
    if let Err(error) = bevy_app::configure_building_art(&mut app) {
        eprintln!("Invalid building-art session: {error}");
        return AppExit::error();
    }
    let default_plugins = DefaultPlugins
        .set(WindowPlugin {
            primary_window: (!use_headless_runner).then(|| Window {
                title: "Hell Workers".into(),
                resolution: window_resolution,
                present_mode,
                ..default()
            }),
            exit_condition: if use_headless_runner {
                ExitCondition::DontExit
            } else {
                ExitCondition::OnAllClosed
            },
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
                ..default()
            })),
            ..default()
        });
    if use_headless_runner {
        eprintln!("Using headless renderer (HW_WINDOW_BACKEND=headless).");
        app.add_plugins(
            default_plugins
                .disable::<WinitPlugin>()
                .disable::<GilrsPlugin>(),
        )
        .add_plugins(ScheduleRunnerPlugin::run_loop(Duration::ZERO));
    } else {
        app.add_plugins(default_plugins);
    }
    if let Some(settings) = window_update_settings {
        app.insert_resource(settings);
    }
    app.add_plugins(game_plugin);
    if let Some(plugin) = native_acceptance_plugin {
        app.add_plugins(plugin);
    }
    if let Some(plugin) = native_deconstruction_plugin {
        app.add_plugins(plugin);
    }
    if let Some(plugin) = native_notification_plugin {
        app.add_plugins(plugin);
    }
    #[cfg(feature = "profiling")]
    if let Some(plugin) = native_ui_plugin {
        app.add_plugins(plugin);
    }

    app.run()
}

// This only selects the execution route. configure_building_art still validates
// the full session, and m2_probe restricts interactive admission to feedback
// ArtPreview Tank/MudMixer identities before any load request is registered.
fn building_art_entry_allowed(
    profiling: bool,
    session: bool,
    probe: Option<&std::ffi::OsStr>,
    perf_enabled: bool,
    static_workload: bool,
) -> bool {
    if let Some(probe) = probe {
        return profiling && session && probe == "1" && !perf_enabled;
    }
    !session || (profiling && perf_enabled && static_workload)
}

fn perf_window_update_settings(perf_enabled: bool, headless: bool) -> Option<WinitSettings> {
    // Bevy's default unfocused game loop is limited to 60 Hz independently of VSync.
    // Benchmark both presentations without focus-dependent pacing; leave normal play alone.
    (perf_enabled && !headless).then(WinitSettings::continuous)
}

fn perf_window_resolution(perf_config: &PerfScenarioConfig) -> WindowResolution {
    let (width, height) = perf_config.requested_window_size().unwrap_or((1280, 720));
    let resolution = WindowResolution::new(width, height);
    match perf_config.requested_window_scale_factor() {
        Some(scale_factor) => resolution.with_scale_factor_override(scale_factor),
        None => resolution,
    }
}

fn headless_runner_requested(perf_config: &PerfScenarioConfig) -> bool {
    let requested =
        env::var("HW_WINDOW_BACKEND").is_ok_and(|value| value.eq_ignore_ascii_case("headless"));
    if requested && !perf_config.enabled() {
        eprintln!("HW_WINDOW_BACKEND=headless is reserved for --perf-scenario profiling runs.");
        std::process::exit(2);
    }
    requested
}

#[cfg(target_os = "linux")]
fn configure_linux_window_backend() {
    let backend = env::var("HW_WINDOW_BACKEND")
        .ok()
        .map(|value| value.to_ascii_lowercase())
        .unwrap_or_else(|| "auto".to_string());

    match backend.as_str() {
        "x11" => force_x11_backend("HW_WINDOW_BACKEND=x11"),
        "wayland" | "headless" => {}
        "auto" => {
            if should_fallback_to_x11() {
                force_x11_backend("auto fallback (Wayland socket unavailable)");
            }
        }
        _ => {
            eprintln!(
                "Unknown HW_WINDOW_BACKEND={backend}. Supported values: auto, x11, wayland, headless."
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

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::winit::UpdateMode;

    #[test]
    fn building_art_entry_separates_interactive_and_static_evidence() {
        let probe = Some(std::ffi::OsStr::new("1"));
        assert!(building_art_entry_allowed(true, true, probe, false, false));
        assert!(building_art_entry_allowed(true, true, None, true, true));
        assert!(building_art_entry_allowed(false, false, None, false, false));
        for (profiling, session, value, perf, static_workload) in [
            (false, true, probe, false, false),
            (true, false, probe, false, false),
            (true, true, Some(std::ffi::OsStr::new("0")), false, false),
            (true, true, probe, true, true),
            (true, true, probe, true, false),
            (true, true, None, false, false),
            (true, true, None, true, false),
            (false, true, None, true, true),
        ] {
            assert!(!building_art_entry_allowed(
                profiling,
                session,
                value,
                perf,
                static_workload
            ));
        }
    }

    #[test]
    fn perf_window_updates_are_continuous_with_or_without_focus() {
        let settings = perf_window_update_settings(true, false).unwrap();
        for focused in [true, false] {
            assert!(matches!(
                settings.update_mode(focused),
                UpdateMode::Continuous
            ));
        }
    }

    #[test]
    fn normal_play_and_headless_keep_their_existing_update_settings() {
        for (perf_enabled, headless) in [(false, false), (false, true), (true, true)] {
            assert!(perf_window_update_settings(perf_enabled, headless).is_none());
        }
    }
}
