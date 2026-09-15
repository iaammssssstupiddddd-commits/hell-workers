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
                        "保存操作とMenu内のSave Gameはセーブカタログを開き、手動スロット1〜3から保存先を選びます。",
                        "読込操作とMenu内のLoad Gameはロードカタログを開き、手動スロット・オートセーブ世代・既存のlegacy defaultを一覧します。",
                        "空の手動スロットは確認なしで保存し、既存スロットは上書き確認後にだけ置換します。読込は確認後に現在の world を置き換えます。",
                        "一覧には手動・自動・旧形式の区分、状態、保存日時を表示します。日時は UTC（協定世界時）です。ホイールまたは右端のスクロールバーで末尾まで比較でき、確認画面にも対象の日時を表示します。",
                        "確認画面で対象と置換の説明を読み、下部の Confirm で確定します。Back は元の一覧へ戻ります。同じスロット行を続けて押しても確定しません。",
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
                        "UI倍率・カメラ速度・自動保存間隔・保持数は、適用済みの値と単位を表示します。UI倍率とカメラ速度は即時、起動時のゲーム速度は次回起動時に反映されます。",
                        "画面を閉じると次回起動用の保存を試みます。失敗時も今回の変更は反映済みです。通知履歴の「現在の設定を再保存」、または設定画面を閉じる操作で、現在の値を再保存できます。",
                        "通知の表示時間は4・8・12秒から選べます。変更は新しい通知から反映され、ゲーム速度や停止状態に影響されません。",
                        "項目が収まらない場合は本文をホイールまたは右のスクロールバーで移動します。Close は画面下部に残ります。",
                    ],
                ),
                HelpEntry::new(
                    HelpEntryId::new("notifications"),
                    "通知",
                    [
                        "短い結果は toast で表示されます。重要な履歴は通知一覧から後で確認でき、最後に届いてからの経過時間も表示します。再保存などの操作ボタンは履歴だけに表示します。",
                        "履歴は最新の通知から表示します。ホイールまたは右のスクロールバーで過去の通知へ移動でき、追記があっても読んでいる位置を保ちます。見出しと閉じるボタンはスクロールしません。",
                        "ボタンや配置失敗のTooltipは、ゲーム停止中にも同じ実時間の待ち時間で表示されます。",
                        "同じ失敗が続く場合は、対象・資源・経路・担当範囲を順に確認してください。",
                    ],
                ),
            ],
        ),
    })
}
