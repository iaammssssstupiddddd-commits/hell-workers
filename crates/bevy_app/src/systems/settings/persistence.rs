use bevy::prelude::*;
use hw_core::GameSettings;
use hw_core::game_state::TimeSpeed;
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
    #[serde(default = "default_power_priority_enabled")]
    pub power_priority_enabled: bool,
}

const fn default_power_priority_enabled() -> bool {
    true
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
            power_priority_enabled: file.power_priority_enabled,
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

#[cfg(test)]
mod tests {
    use super::*;

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
}
