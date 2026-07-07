use bevy::prelude::*;
use hw_core::game_state::TimeSpeed;
use hw_core::GameSettings;
use serde::{Deserialize, Serialize};

const SETTINGS_DIR: &str = "settings";
const SETTINGS_FILE: &str = "settings/settings.ron";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GameSettingsFile {
    pub ui_scale: f32,
    pub camera_pan_speed: f32,
    pub camera_mouse_pan_enabled: bool,
    pub default_time_speed: TimeSpeedFile,
    pub debug_gizmos_enabled: bool,
    pub fps_display_enabled: bool,
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
        }
    }
}

impl From<GameSettingsFile> for GameSettings {
    fn from(file: GameSettingsFile) -> Self {
        Self {
            ui_scale: file.ui_scale,
            camera_pan_speed: file.camera_pan_speed,
            camera_mouse_pan_enabled: file.camera_mouse_pan_enabled,
            default_time_speed: file.default_time_speed.into(),
            debug_gizmos_enabled: file.debug_gizmos_enabled,
            fps_display_enabled: file.fps_display_enabled,
        }
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

pub fn load_settings_from_disk() -> GameSettings {
    match std::fs::read_to_string(SETTINGS_FILE) {
        Ok(contents) => match ron::from_str::<GameSettingsFile>(&contents) {
            Ok(file) => {
                info!("Loaded settings from {SETTINGS_FILE}");
                file.into()
            }
            Err(err) => {
                warn!("Failed to parse {SETTINGS_FILE}: {err}. Using defaults.");
                GameSettings::default()
            }
        },
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            info!("Settings file not found at {SETTINGS_FILE}. Using defaults.");
            GameSettings::default()
        }
        Err(err) => {
            warn!("Failed to read {SETTINGS_FILE}: {err}. Using defaults.");
            GameSettings::default()
        }
    }
}

pub fn save_settings_to_disk(settings: &GameSettings) -> Result<(), String> {
    std::fs::create_dir_all(SETTINGS_DIR).map_err(|err| err.to_string())?;
    let file: GameSettingsFile = settings.clone().into();
    let contents = ron::ser::to_string_pretty(&file, ron::ser::PrettyConfig::default())
        .map_err(|err| err.to_string())?;
    std::fs::write(SETTINGS_FILE, contents).map_err(|err| err.to_string())
}
