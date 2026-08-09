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
                        "F5 と Pause の Save Game はセーブカタログを開き、手動スロット 1〜3 から保存先を選びます。",
                        "F9 と Pause の Load Game はロードカタログを開き、手動スロット・オートセーブ世代・既存の legacy default を一覧します。",
                        "空の手動スロットは確認なしで保存し、既存スロットは上書き確認後にだけ置換します。読込は確認後に現在の world を置き換えます。",
                        "読込後は建築進捗と Familiar 設定を復元し、進行中の担当・運搬を安全な待機状態へ戻して再割り当てします。猫車の積載資材は車両付近へ荷下ろしされます。",
                        "破損・非対応形式・別 seed の候補はロードできません。適用途中で失敗した場合は直前の world の復旧を試み、復旧不能時は専用の再ロードだけが許可されます。",
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
                        "UI scale、カメラ速度、マウス移動、既定時間速度、電力の優先配電、オートセーブ（有効・間隔・世代数）、デバッグ表示を変更できます。",
                        "オートセーブは初期状態ではオフです。有効化すると操作中の実時間間隔（5/10/20/30 分）と世代数（1〜5）で自動保存します。",
                        "設定画面を閉じると保存され、次回起動でも利用されます。",
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
