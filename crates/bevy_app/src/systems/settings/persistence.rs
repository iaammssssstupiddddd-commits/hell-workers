use bevy::prelude::*;
use hw_core::GameSettings;
use hw_core::game_state::TimeSpeed;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

pub const DEFAULT_SETTINGS_STORAGE_ROOT: &str = "settings";
const SETTINGS_FILE_NAME: &str = "settings.ron";

/// Runtime-owned directory for settings persistence. Native acceptance and
/// perf inject a fresh root before Startup so they never inspect or alter the
/// player's `settings/settings.ron`.
#[derive(Resource, Debug, Clone, PartialEq, Eq)]
pub struct SettingsStorageRoot(pub PathBuf);

impl SettingsStorageRoot {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self(path.into())
    }

    pub fn as_path(&self) -> &Path {
        &self.0
    }

    pub fn settings_file(&self) -> PathBuf {
        self.0.join(SETTINGS_FILE_NAME)
    }
}

impl Default for SettingsStorageRoot {
    fn default() -> Self {
        Self::new(DEFAULT_SETTINGS_STORAGE_ROOT)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GameSettingsFile {
    pub ui_scale: f32,
    pub camera_pan_speed: f32,
    pub camera_mouse_pan_enabled: bool,
    pub default_time_speed: TimeSpeedFile,
    pub debug_gizmos_enabled: bool,
    pub fps_display_enabled: bool,
    #[serde(default = "default_power_priority_enabled")]
    pub power_priority_enabled: bool,
    #[serde(default = "default_autosave_enabled")]
    pub autosave_enabled: bool,
    #[serde(default = "default_autosave_interval_minutes")]
    pub autosave_interval_minutes: u32,
    #[serde(default = "default_autosave_generations")]
    pub autosave_generations: u8,
}

const fn default_power_priority_enabled() -> bool {
    true
}

const fn default_autosave_enabled() -> bool {
    false
}

const fn default_autosave_interval_minutes() -> u32 {
    10
}

const fn default_autosave_generations() -> u8 {
    3
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum TimeSpeedFile {
    Paused,
    Normal,
    Fast,
    Super,
}

impl From<GameSettings> for GameSettingsFile {
    fn from(settings: GameSettings) -> Self {
        Self {
            ui_scale: settings.ui_scale,
            camera_pan_speed: settings.camera_pan_speed,
            camera_mouse_pan_enabled: settings.camera_mouse_pan_enabled,
            default_time_speed: settings.default_time_speed.into(),
            debug_gizmos_enabled: settings.debug_gizmos_enabled,
            fps_display_enabled: settings.fps_display_enabled,
            power_priority_enabled: settings.power_priority_enabled,
            autosave_enabled: settings.autosave_enabled,
            autosave_interval_minutes: settings.normalized_autosave_interval_minutes(),
            autosave_generations: settings.normalized_autosave_generations(),
        }
    }
}

impl From<GameSettingsFile> for GameSettings {
    fn from(file: GameSettingsFile) -> Self {
        let mut settings = Self {
            ui_scale: file.ui_scale,
            camera_pan_speed: file.camera_pan_speed,
            camera_mouse_pan_enabled: file.camera_mouse_pan_enabled,
            default_time_speed: file.default_time_speed.into(),
            debug_gizmos_enabled: file.debug_gizmos_enabled,
            fps_display_enabled: file.fps_display_enabled,
            power_priority_enabled: file.power_priority_enabled,
            autosave_enabled: file.autosave_enabled,
            autosave_interval_minutes: file.autosave_interval_minutes,
            autosave_generations: file.autosave_generations,
        };
        settings.autosave_interval_minutes = settings.normalized_autosave_interval_minutes();
        settings.autosave_generations = settings.normalized_autosave_generations();
        settings
    }
}

impl From<TimeSpeed> for TimeSpeedFile {
    fn from(speed: TimeSpeed) -> Self {
        match speed {
            TimeSpeed::Paused => Self::Paused,
            TimeSpeed::Normal => Self::Normal,
            TimeSpeed::Fast => Self::Fast,
            TimeSpeed::Super => Self::Super,
        }
    }
}

impl From<TimeSpeedFile> for TimeSpeed {
    fn from(speed: TimeSpeedFile) -> Self {
        match speed {
            TimeSpeedFile::Paused => Self::Paused,
            TimeSpeedFile::Normal => Self::Normal,
            TimeSpeedFile::Fast => Self::Fast,
            TimeSpeedFile::Super => Self::Super,
        }
    }
}

pub fn load_settings_from_disk(root: &SettingsStorageRoot) -> GameSettings {
    let path = root.settings_file();
    match std::fs::read_to_string(&path) {
        Ok(contents) => match ron::from_str::<GameSettingsFile>(&contents) {
            Ok(file) => {
                info!("Loaded settings from {}", path.display());
                file.into()
            }
            Err(err) => {
                warn!("Failed to parse {}: {err}. Using defaults.", path.display());
                GameSettings::default()
            }
        },
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            info!(
                "Settings file not found at {}. Using defaults.",
                path.display()
            );
            GameSettings::default()
        }
        Err(err) => {
            warn!("Failed to read {}: {err}. Using defaults.", path.display());
            GameSettings::default()
        }
    }
}

pub fn save_settings_to_disk(
    root: &SettingsStorageRoot,
    settings: &GameSettings,
) -> Result<(), String> {
    std::fs::create_dir_all(root.as_path()).map_err(|err| err.to_string())?;
    let file: GameSettingsFile = settings.clone().into();
    let contents = ron::ser::to_string_pretty(&file, ron::ser::PrettyConfig::default())
        .map_err(|err| err.to_string())?;
    std::fs::write(root.settings_file(), contents).map_err(|err| err.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn old_settings_file_preserves_existing_values_and_enables_priority() {
        let old = r#"(
            ui_scale: 1.15,
            camera_pan_speed: 750.0,
            camera_mouse_pan_enabled: false,
            default_time_speed: Fast,
            debug_gizmos_enabled: true,
            fps_display_enabled: false,
        )"#;

        let file: GameSettingsFile = ron::from_str(old).expect("old settings must migrate");
        let settings: GameSettings = file.into();

        assert_eq!(settings.ui_scale, 1.15);
        assert_eq!(settings.camera_pan_speed, 750.0);
        assert!(!settings.camera_mouse_pan_enabled);
        assert_eq!(settings.default_time_speed, TimeSpeed::Fast);
        assert!(settings.debug_gizmos_enabled);
        assert!(!settings.fps_display_enabled);
        assert!(settings.power_priority_enabled);
        assert!(!settings.autosave_enabled);
        assert_eq!(settings.autosave_interval_minutes, 10);
        assert_eq!(settings.autosave_generations, 3);
    }

    #[test]
    fn priority_setting_round_trips() {
        let settings = GameSettings {
            power_priority_enabled: false,
            ..default()
        };
        let file: GameSettingsFile = settings.into();
        let body = ron::to_string(&file).unwrap();
        let loaded: GameSettings = ron::from_str::<GameSettingsFile>(&body).unwrap().into();

        assert!(!loaded.power_priority_enabled);
    }

    #[test]
    fn injected_storage_root_keeps_settings_io_out_of_the_working_directory() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock must be after unix epoch")
            .as_nanos();
        let root = SettingsStorageRoot::new(std::env::temp_dir().join(format!(
            "hell-workers-settings-root-{}-{nonce}",
            std::process::id()
        )));
        let settings = GameSettings {
            autosave_enabled: true,
            ..default()
        };

        save_settings_to_disk(&root, &settings).expect("isolated settings write must succeed");

        assert!(root.settings_file().is_file());
        assert!(load_settings_from_disk(&root).autosave_enabled);
        assert_ne!(root.settings_file(), PathBuf::from("settings/settings.ron"));
        let _ = fs::remove_dir_all(root.as_path());
    }
}
