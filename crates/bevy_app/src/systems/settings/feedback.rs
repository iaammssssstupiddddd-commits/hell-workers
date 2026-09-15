use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use hw_core::GameSettings;
use hw_ui::notifications::{
    NotificationAction, NotificationCenter, NotificationRetention, NotificationSeverity,
    UserFacingNotification,
};

use super::persistence::{SettingsStorageRoot, save_settings_to_disk};

#[derive(Resource, Default)]
pub(crate) struct SettingsSaveState {
    next_attempt: u64,
    pending_retry: Option<u64>,
}

#[derive(SystemParam)]
pub(crate) struct SettingsSaveFeedback<'w> {
    state: ResMut<'w, SettingsSaveState>,
    input: Res<'w, hw_ui::components::UiInputState>,
    center: ResMut<'w, NotificationCenter>,
    notifications: MessageWriter<'w, UserFacingNotification>,
}

impl SettingsSaveFeedback<'_> {
    pub(crate) fn can_retry(&self, attempt: u64) -> bool {
        !self.input.world_input_captured && self.state.pending_retry == Some(attempt)
    }

    pub(crate) fn save_current(&mut self, root: &SettingsStorageRoot, settings: &GameSettings) {
        let was_retry = self.state.pending_retry.take().is_some();
        self.state.next_attempt = self.state.next_attempt.wrapping_add(1).max(1);
        self.center.retain_settings_retry_attempt(None);
        match save_settings_to_disk(root, settings) {
            Ok(()) if was_retry => {
                self.notifications.write(UserFacingNotification::new(
                    "settings-save-restored",
                    NotificationSeverity::Success,
                    "現在の設定を保存しました",
                    "次回起動時にもこの設定を利用します。",
                    NotificationRetention::Important,
                ));
            }
            Ok(()) => {}
            Err(error) => {
                warn!("Failed to save settings: {error}");
                let attempt = self.state.next_attempt;
                self.state.pending_retry = Some(attempt);
                self.notifications.write(UserFacingNotification::new(
                    "settings-save-failed", NotificationSeverity::Error,
                    "設定を保存できませんでした",
                    "今回の変更は反映済みですが、次回起動用の設定は未保存です。保存先の空き容量・書込権限を確認し、通知履歴から現在の設定を再保存してください。",
                    NotificationRetention::Important,
                ).with_action(NotificationAction::RetrySettingsSave { attempt }));
            }
        }
    }
}

pub(crate) fn update_settings_save_status(
    state: Res<SettingsSaveState>,
    mut labels: Query<&mut Text, With<hw_ui::components::SettingsSaveStatusText>>,
) {
    let label = if state.pending_retry.is_some() {
        "変更は反映済み・次回用は未保存です。通知履歴、またはこの画面を閉じて再保存できます。"
    } else {
        "変更は画面を閉じると保存されます。"
    };
    for mut text in &mut labels {
        if text.0 != label {
            text.0 = label.to_string();
        }
    }
}

/// Also removes actions from notifications queued earlier in the same frame.
pub(crate) fn sync_settings_retry_actions(
    state: Res<SettingsSaveState>,
    mut center: ResMut<NotificationCenter>,
) {
    center.retain_settings_retry_attempt(state.pending_retry);
}
