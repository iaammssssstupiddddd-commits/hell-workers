use hw_ui::help::{HelpEntry, HelpEntryId, HelpSectionId, HelpTopic, HelpTopicId};

use crate::input_actions::InputAction;

use super::super::{
    HelpCatalogError, HelpContribution,
    manifest::{HelpOwnerId, PlayerFeatureId},
    shortcut,
};

pub(crate) fn save_settings_notifications() -> Result<HelpContribution, HelpCatalogError> {
    Ok(HelpContribution {
        feature: PlayerFeatureId::SaveSettingsNotifications,
        owner: HelpOwnerId::PersistenceAndSettings,
        section_id: HelpSectionId::new("save-settings-notifications"),
        section_title: "保存・設定・通知",
        topic: HelpTopic::new(
            HelpTopicId::new("save-settings-notifications"),
            "保存・設定・通知",
            [
                HelpEntry::new(
                    HelpEntryId::new("save-load"),
                    "保存と読込",
                    [
                        "現在の単一セーブへ保存するか、読込確認を開きます。",
                        "読込は現在の world を置き換えるため、確認ダイアログを経て実行されます。",
                    ],
                )
                .with_shortcut(format!(
                    "{} / {}",
                    shortcut(InputAction::SaveGame)?,
                    shortcut(InputAction::RequestLoadGame)?
                )),
                HelpEntry::new(
                    HelpEntryId::new("settings"),
                    "Settings",
                    [
                        "UI scale、カメラ速度、マウス移動、既定時間速度、デバッグ表示を変更できます。",
                        "設定は変更時に保存され、次回起動でも利用されます。",
                    ],
                ),
                HelpEntry::new(
                    HelpEntryId::new("notifications"),
                    "通知",
                    [
                        "短い結果は toast で表示されます。重要な履歴は通知一覧から後で確認できます。",
                        "同じ失敗が続く場合は、対象・資源・経路・担当範囲を順に確認してください。",
                    ],
                ),
            ],
        ),
    })
}
