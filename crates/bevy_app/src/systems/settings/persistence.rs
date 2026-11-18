use bevy::prelude::*;
use hw_core::GameSettings;
use hw_core::game_state::TimeSpeed;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use crate::systems::save::atomic_file::{
    AtomicCommitMode, AtomicCommitOutcome, AtomicWriteError, sync_parent_directory,
    write_file_atomic_timed,
};

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
    #[serde(default = "default_notification_duration_seconds")]
    pub notification_duration_seconds: u8,
}

const fn default_notification_duration_seconds() -> u8 {
    4
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
            notification_duration_seconds: settings.normalized_notification_duration_seconds(),
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
            notification_duration_seconds: file.notification_duration_seconds,
        };
        settings.notification_duration_seconds =
            settings.normalized_notification_duration_seconds();
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

#[derive(Default)]
enum SettingsOriginal {
    #[default]
    Missing,
    Readable {
        bytes: Vec<u8>,
        valid: bool,
    },
    Unreadable,
}

#[derive(Default)]
pub(crate) struct SettingsPersistence {
    original: SettingsOriginal,
    baseline: Option<String>,
    pub(super) durability_pending: bool,
    backup: Option<PathBuf>,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum SettingsSaveOutcome {
    Unchanged,
    Saved,
    DurabilityUncertain,
}

impl SettingsPersistence {
    pub(crate) fn load(root: &SettingsStorageRoot) -> (GameSettings, Self) {
        let path = root.settings_file();
        let (settings, original) = match fs::read(&path) {
            Ok(bytes) => {
                let parsed = ron::de::from_bytes::<GameSettingsFile>(&bytes);
                if let Err(error) = &parsed {
                    warn!(
                        "Failed to parse {}: {error}. Using defaults.",
                        path.display()
                    );
                }
                let valid = parsed.is_ok();
                let settings = parsed.map(GameSettings::from).unwrap_or_default();
                (settings, SettingsOriginal::Readable { bytes, valid })
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                (GameSettings::default(), SettingsOriginal::Missing)
            }
            Err(error) => {
                warn!(
                    "Failed to read {}: {error}. Using defaults.",
                    path.display()
                );
                (GameSettings::default(), SettingsOriginal::Unreadable)
            }
        };
        let baseline = encode(&settings).ok();
        (
            settings,
            Self {
                original,
                baseline,
                ..Self::default()
            },
        )
    }

    pub(crate) fn save(
        &mut self,
        root: &SettingsStorageRoot,
        settings: &GameSettings,
    ) -> Result<SettingsSaveOutcome, String> {
        let contents = encode(settings)?;
        let path = root.settings_file();
        if self.baseline.as_ref() == Some(&contents)
            && !matches!(self.original, SettingsOriginal::Missing)
        {
            if self.durability_pending {
                if let SettingsOriginal::Readable { bytes, .. } = &self.original {
                    check_original(&path, bytes).map_err(|error| error.to_string())?;
                }
                if sync_parent_directory(&path).is_err() {
                    return Ok(SettingsSaveOutcome::DurabilityUncertain);
                }
                self.durability_pending = false;
                return Ok(SettingsSaveOutcome::Saved);
            }
            return Ok(SettingsSaveOutcome::Unchanged);
        }

        if matches!(self.original, SettingsOriginal::Unreadable) {
            let bytes = fs::read(&path).map_err(|error| error.to_string())?;
            self.original = SettingsOriginal::Readable {
                bytes,
                valid: false,
            };
        }

        let expected = match &self.original {
            SettingsOriginal::Readable { bytes, valid } => {
                check_original(&path, bytes).map_err(|error| error.to_string())?;
                if !valid
                    && !self.backup.as_ref().is_some_and(|backup| {
                        fs::read(backup).is_ok_and(|original| original == *bytes)
                    })
                {
                    self.backup = Some(backup_original(root, bytes)?);
                }
                Some(bytes.as_slice())
            }
            SettingsOriginal::Missing => None,
            SettingsOriginal::Unreadable => unreachable!("original must be read before saving"),
        };
        let mode = if expected.is_some() {
            AtomicCommitMode::Replace
        } else {
            AtomicCommitMode::NoReplace
        };
        let outcome = write_file_atomic_timed(
            &path,
            mode,
            |file| file.write_all(contents.as_bytes()),
            || expected.map_or(Ok(()), |bytes| check_original(&path, bytes)),
        )
        .map_err(|error| error.to_string())?
        .outcome;

        self.original = SettingsOriginal::Readable {
            bytes: contents.as_bytes().to_vec(),
            valid: true,
        };
        self.baseline = Some(contents);
        self.backup = None;
        self.durability_pending = outcome == AtomicCommitOutcome::CommittedDurabilityUncertain;
        Ok(if self.durability_pending {
            SettingsSaveOutcome::DurabilityUncertain
        } else {
            SettingsSaveOutcome::Saved
        })
    }
}

fn encode(settings: &GameSettings) -> Result<String, String> {
    let file = GameSettingsFile::from(settings.clone());
    ron::ser::to_string_pretty(&file, ron::ser::PrettyConfig::default())
        .map_err(|error| error.to_string())
}

fn check_original(path: &Path, expected: &[u8]) -> io::Result<()> {
    if fs::read(path)? != expected {
        return Err(io::Error::other("settings changed on disk before saving"));
    }
    Ok(())
}

fn backup_original(root: &SettingsStorageRoot, bytes: &[u8]) -> Result<PathBuf, String> {
    for index in 0..128 {
        let path = root.as_path().join(format!(
            "settings.ron.backup.{}.{index}",
            std::process::id()
        ));
        match write_file_atomic_timed(
            &path,
            AtomicCommitMode::NoReplace,
            |file| file.write_all(bytes),
            || Ok(()),
        ) {
            Ok(result) if result.outcome == AtomicCommitOutcome::Committed => return Ok(path),
            Ok(_) => return Err("settings backup durability is uncertain".to_owned()),
            Err(AtomicWriteError::TargetExists) => continue,
            Err(error) => return Err(error.to_string()),
        }
    }
    Err("could not reserve a settings backup name".to_owned())
}

#[cfg(test)]
pub(crate) fn load_settings_from_disk(root: &SettingsStorageRoot) -> GameSettings {
    SettingsPersistence::load(root).0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::systems::save::atomic_file::faults::{Point, on};
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn new_root() -> SettingsStorageRoot {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = SettingsStorageRoot::new(std::env::temp_dir().join(format!(
            "hw-settings-preserve-{}-{nonce}",
            std::process::id(),
        )));
        fs::create_dir_all(root.as_path()).unwrap();
        root
    }

    #[test]
    fn unreadable_settings_are_preserved_until_edited_and_backed_up_test() {
        for original in [b"(broken:".as_slice(), b"bad RON \xff"] {
            verify_corrupt_settings_round_trip(original);
        }
    }

    fn verify_corrupt_settings_round_trip(original: &[u8]) {
        let root = new_root();
        fs::write(root.settings_file(), original).unwrap();
        let (mut settings, mut persistence) = SettingsPersistence::load(&root);
        for _ in 0..2 {
            assert_eq!(
                persistence.save(&root, &settings).unwrap(),
                SettingsSaveOutcome::Unchanged
            );
            assert_eq!(fs::read(root.settings_file()).unwrap(), original);
        }
        let collision = root
            .as_path()
            .join(format!("settings.ron.backup.{}.0", std::process::id()));
        fs::write(&collision, b"previous backup").unwrap();
        settings.ui_scale = 0.85;
        assert_eq!(
            persistence.save(&root, &settings).unwrap(),
            SettingsSaveOutcome::Saved
        );
        let backup = root
            .as_path()
            .join(format!("settings.ron.backup.{}.1", std::process::id()));
        assert_eq!(fs::read(collision).unwrap(), b"previous backup");
        assert_eq!(fs::read(backup).unwrap(), original);
        assert_eq!(SettingsPersistence::load(&root).0.ui_scale, 0.85);
        fs::remove_dir_all(root.as_path()).unwrap();
    }

    #[test]
    fn read_backup_write_and_directory_sync_failures_preserve_the_correct_version() {
        let root = new_root();
        let path = root.settings_file();
        let original = b"(invalid original";
        fs::write(&path, original).unwrap();
        let held = root.as_path().join("original");
        fs::rename(&path, &held).unwrap();
        fs::create_dir(&path).unwrap();
        let (mut settings, mut persistence) = SettingsPersistence::load(&root);
        assert!(matches!(persistence.original, SettingsOriginal::Unreadable));
        settings.ui_scale = 1.20;
        assert!(persistence.save(&root, &settings).is_err());
        assert_eq!(fs::read(&held).unwrap(), original);
        fs::remove_dir(&path).unwrap();
        fs::rename(held, &path).unwrap();

        let backup = |index| {
            root.as_path().join(format!(
                "settings.ron.backup.{}.{index}",
                std::process::id()
            ))
        };
        let result = on(&backup(0), Point::SyncParent, || {
            persistence.save(&root, &settings)
        });
        assert!(result.is_err());
        assert_eq!(fs::read(&path).unwrap(), original);
        assert_eq!(fs::read(backup(0)).unwrap(), original);

        let result = on(&path, Point::Write, || persistence.save(&root, &settings));
        assert!(result.is_err());
        assert_eq!(fs::read(&path).unwrap(), original);
        assert_eq!(fs::read(backup(1)).unwrap(), original);

        let result = on(&path, Point::SyncParent, || {
            persistence.save(&root, &settings)
        });
        assert_eq!(result.unwrap(), SettingsSaveOutcome::DurabilityUncertain);
        assert!(persistence.durability_pending);
        assert_eq!(SettingsPersistence::load(&root).0.ui_scale, 1.20);
        assert_eq!(
            persistence.save(&root, &settings).unwrap(),
            SettingsSaveOutcome::Saved
        );
        assert!(!persistence.durability_pending);
        assert!(!backup(2).exists());
        fs::remove_dir_all(root.as_path()).unwrap();
    }

    #[test]
    fn close_and_quit_do_not_rewrite_corrupt_settings() {
        use crate::systems::settings::feedback::{SettingsSaveFeedback, SettingsSaveState};
        use crate::systems::settings::{load_settings_system, save_settings_on_app_exit_system};
        use hw_ui::components::UiInputState;
        use hw_ui::notifications::{NotificationCenter, UserFacingNotification};

        fn close_settings(
            root: Res<SettingsStorageRoot>,
            settings: Res<GameSettings>,
            mut feedback: SettingsSaveFeedback,
        ) {
            feedback.save_current(&root, &settings);
        }

        for original in [b"(broken:".as_slice(), b"bad RON \xff"] {
            let root = new_root();
            let path = root.settings_file();
            fs::write(&path, original).unwrap();
            let mut app = App::new();
            app.insert_resource(root.clone())
                .init_resource::<Time<Virtual>>()
                .init_resource::<SettingsSaveState>()
                .init_resource::<UiInputState>()
                .init_resource::<NotificationCenter>()
                .add_message::<AppExit>()
                .add_message::<UserFacingNotification>();
            let world = app.world_mut();
            world.run_system_cached(load_settings_system).unwrap();
            world.run_system_cached(close_settings).unwrap();
            world.write_message(AppExit::Success);
            world
                .run_system_cached(save_settings_on_app_exit_system)
                .unwrap();
            assert_eq!(fs::read(&path).unwrap(), original);

            world.resource_mut::<GameSettings>().ui_scale = 1.20;
            on(&path, Point::SyncParent, || {
                world.run_system_cached(close_settings).unwrap()
            });
            assert_eq!(SettingsPersistence::load(&root).0.ui_scale, 1.20);
            let notification = world
                .resource::<Messages<UserFacingNotification>>()
                .iter_current_update_messages()
                .find(|message| message.key.as_str() == "settings-save-durability-uncertain")
                .expect("uncertain commit must offer a retry");
            assert!(matches!(
                notification.action,
                Some(hw_ui::notifications::NotificationAction::RetrySettingsSave { .. })
            ));
            world.run_system_cached(close_settings).unwrap();
            assert_eq!(fs::read_dir(root.as_path()).unwrap().count(), 2);
            fs::remove_dir_all(root.as_path()).unwrap();
        }
    }

    #[test]
    fn notification_duration_round_trips_and_invalid_values_use_default() {
        for (requested, expected) in [(4, 4), (8, 8), (12, 12), (0, 4), (255, 4)] {
            let settings = GameSettings {
                notification_duration_seconds: requested,
                ..default()
            };
            let serialized = ron::to_string(&GameSettingsFile::from(settings)).unwrap();
            let restored: GameSettings = ron::from_str::<GameSettingsFile>(&serialized)
                .unwrap()
                .into();
            assert_eq!(restored.notification_duration_seconds, expected);
        }
    }

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
        assert_eq!(settings.notification_duration_seconds, 4);
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
    fn file_roundtrip_preserves_raw_ui_ranges_and_normalizes_discrete_values() {
        let settings = GameSettings {
            ui_scale: 2.0,
            camera_pan_speed: 1500.0,
            autosave_interval_minutes: 17,
            autosave_generations: 0,
            ..default()
        };
        let body = ron::to_string(&GameSettingsFile::from(settings)).unwrap();
        let loaded: GameSettings = ron::from_str::<GameSettingsFile>(&body).unwrap().into();
        assert_eq!(loaded.ui_scale, 2.0);
        assert_eq!(loaded.camera_pan_speed, 1500.0);
        assert_eq!(loaded.autosave_interval_minutes, 20);
        assert_eq!(loaded.autosave_generations, 1);
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

        SettingsPersistence::default()
            .save(&root, &settings)
            .expect("isolated settings write must succeed");

        assert!(root.settings_file().is_file());
        assert!(load_settings_from_disk(&root).autosave_enabled);
        assert_ne!(root.settings_file(), PathBuf::from("settings/settings.ron"));
        let _ = fs::remove_dir_all(root.as_path());
    }
}
