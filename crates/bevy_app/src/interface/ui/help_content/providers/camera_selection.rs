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
                        "カメラは固定のトップダウン表示で、表示階層や視点を切り替える操作はありません。",
                    ],
                )
                .with_shortcut("W / A / S / D / Mouse Wheel"),
                HelpEntry::new(
                    HelpEntryId::new("world-selection"),
                    "選択とスナップ",
                    [
                        "対象へ近づけると選択候補が先に表示されます。水色の強い表示は、カーソルの少し外側から対象へスナップしている状態です。",
                        "左ボタンを離したときに候補を選びます。5 pxを超える左ドラッグはカメラ移動になり、選択を変更しません。",
                        "同じ場所を続けて左クリックすると、資源と建物、建物と床など、重なった候補を順に選べます。建物とBlueprintは中心ではなく占有範囲全体が対象です。",
                        "入力欄を編集中は、ゲーム用ショートカットが抑止されます。",
                    ],
                ),
                HelpEntry::new(
                    HelpEntryId::new("world-object-actions"),
                    "右クリック操作",
                    [
                        "対象上の右クリックは、その対象を選んでコンテキストメニューを開きます。使い魔では確認、Task Area編集、Operationを選べます。",
                        "使い魔を選択して空き地を右クリックすると移動先を更新し、短時間マーカーを表示します。対象上では移動よりコンテキストメニューを優先します。",
                        "Doorはメニューから施錠・解錠できます。TankとMud MixerはMoveを選べますが、移動できない建物にはMoveを表示しません。",
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
