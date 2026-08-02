use hw_ui::help::{HelpEntry, HelpEntryId, HelpSectionId, HelpTopic, HelpTopicId};

use crate::input_actions::InputAction;

use super::super::{
    HelpCatalogError, HelpContribution,
    manifest::{HelpOwnerId, PlayerFeatureId},
    shortcut,
};

pub(crate) fn camera_and_selection() -> Result<HelpContribution, HelpCatalogError> {
    Ok(HelpContribution {
        feature: PlayerFeatureId::CameraAndSelection,
        owner: HelpOwnerId::InputAndCamera,
        section_id: HelpSectionId::new("basic-controls"),
        section_title: "基本操作",
        topic: HelpTopic::new(
            HelpTopicId::new("camera-selection"),
            "カメラと選択",
            [
                HelpEntry::new(
                    HelpEntryId::new("camera-pan-zoom"),
                    "カメラ移動",
                    [
                        "W / A / S / D で移動し、マウスホイールで拡大・縮小します。",
                        "Settings で Mouse Drag Pan が有効なら、左ドラッグでもカメラを移動できます。",
                        "パン操作では画面を回転させません。表示方向は「表示階層」の切り替えで変更します。",
                    ],
                )
                .with_shortcut("W / A / S / D / Mouse Wheel"),
                HelpEntry::new(
                    HelpEntryId::new("camera-elevation"),
                    "表示階層",
                    ["地表と地下の表示階層を切り替えます。"],
                )
                .with_shortcut(shortcut(InputAction::CycleElevation)?),
                HelpEntry::new(
                    HelpEntryId::new("world-selection"),
                    "選択と右クリック",
                    [
                        "左クリックで対象を選びます。右クリックは選択対象と現在のモードに応じた操作を開きます。",
                        "入力欄を編集中は、ゲーム用ショートカットが抑止されます。",
                    ],
                ),
            ],
        ),
    })
}

pub(crate) fn time_and_help() -> Result<HelpContribution, HelpCatalogError> {
    Ok(HelpContribution {
        feature: PlayerFeatureId::TimeAndHelp,
        owner: HelpOwnerId::InputAndCamera,
        section_id: HelpSectionId::new("basic-controls"),
        section_title: "基本操作",
        topic: HelpTopic::new(
            HelpTopicId::new("time-help"),
            "時間とヘルプ",
            [
                HelpEntry::new(
                    HelpEntryId::new("time-controls"),
                    "時間速度",
                    [
                        "一時停止を切り替えるか、Paused / Normal / Fast / Super を直接選択できます。",
                    ],
                )
                .with_shortcut(format!(
                    "{} / {} / {} / {} / {}",
                    shortcut(InputAction::TogglePause)?,
                    shortcut(InputAction::TimePaused)?,
                    shortcut(InputAction::TimeNormal)?,
                    shortcut(InputAction::TimeFast)?,
                    shortcut(InputAction::TimeSuper)?,
                )),
                HelpEntry::new(
                    HelpEntryId::new("help-pause-behavior"),
                    "ヘルプ中の時間",
                    [
                        "通常時にヘルプを開くと自動で一時停止し、閉じると直前の相対速度で再開します。",
                        "すでに Pause 中なら、ヘルプを閉じても Pause を維持します。",
                    ],
                )
                .with_shortcut(shortcut(InputAction::OpenHelp)?),
            ],
        ),
    })
}
