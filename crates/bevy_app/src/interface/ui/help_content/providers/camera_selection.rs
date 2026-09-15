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
                        "Settings で Mouse Drag Pan が有効なら、通常モードの左ドラッグ、または中ボタンドラッグでカメラを移動できます。",
                        "中ボタンドラッグは配置・移動・範囲編集モードを維持します。左／右ボタンの操作中には開始せず、中ボタンで移動中の左／右操作は配置や選択に使いません。UI・入力欄・ダイアログへ入ると移動を終了します。",
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
                        "同じ場所の左クリックで重なった候補を順に選べます。連続クリックの時間制限はありません。「候補 n/m — 一覧を開く」から名前を確認して直接選ぶこともできます。別の地点を選ぶと候補を更新し、移動・消滅した対象や非表示の対象は選べません。建物とBlueprintは占有範囲全体が対象です。",
                        "入力欄を編集中は、ゲーム用ショートカットが抑止されます。",
                    ],
                ),
                HelpEntry::new(
                    HelpEntryId::new("world-object-actions"),
                    "右クリック操作",
                    [
                        "対象上の右クリックは、同じ地点に残っている選択済みの候補を優先してコンテキストメニューを開きます。該当する候補がなければ手前の対象を使います。使い魔では確認、Task Area編集、Operationを選べます。",
                        "使い魔を選択して空き地を右クリックすると移動先を更新し、短時間マーカーを表示します。対象上では移動よりコンテキストメニューを優先します。",
                        "Doorはメニューから施錠・解錠できます。TankとMud MixerはMoveを選べますが、移動できない建物にはMoveを表示しません。",
                        "コンテキストメニューの閉じる操作は、使い魔の命令や目的地を変更しません。別のダイアログが前面にある場合は、先にそのダイアログを閉じます。",
                    ],
                ).with_shortcut(shortcut(InputAction::CloseContextMenu)?),
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
                        "時間停止だけではメニューを開きません。停止中も選択・カメラ・担当範囲の編集と履歴・新規の伐採/採掘指定・既存の保管範囲1件の方針・完成ドアの施錠を操作できます。変更はその場で適用し、再開時には再送しません。",
                        "停止中は運搬指定・建築/移設・Zone作成/解除・植林・Soul配属・作業取消・方針の範囲編集を利用できません。対象ボタンは薄く表示し、Tooltipで停止中の制限を案内します。AIと移動・作業進行は停止したままです。",
                        "Menuボタン、または他の画面や操作を閉じる役割がない終了キーでシステムメニューを開きます。メニューが止めた時間だけ、閉じると元の速度で再開します。元から停止していた場合は停止を維持します。メニュー上のHelp/Settingsを閉じても時間は再開しません。",
                    ],
                )
                .with_shortcut(format!(
                    "{} / {} / {} / {} / {} / メニュー: {}",
                    shortcut(InputAction::TogglePause)?,
                    shortcut(InputAction::TimePaused)?,
                    shortcut(InputAction::TimeNormal)?,
                    shortcut(InputAction::TimeFast)?,
                    shortcut(InputAction::TimeSuper)?,
                    shortcut(InputAction::ToggleSystemMenu)?,
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
