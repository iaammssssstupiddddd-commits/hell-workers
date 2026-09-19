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
                        "カメラは固定のトップダウン表示です。「表示」は地図上の補助情報を切り替え、視点や倍率は変更しません。",
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
                        "選択対象は輪郭で示します。設定済みの作業範囲、使い魔の指揮範囲、Site・Yardの境界は通常時も表示します。「表示」から担当範囲の凡例・保管場所・給電状態・成立した部屋を選べます。「通常表示」に戻しても設定済みのエリアは消えません。",
                        "担当の四角は作業範囲、円は指揮が届く範囲です。保管表示の青枠は実際の保管タイル、給電表示の白丸は給電中、橙の×は未給電の消費設備です。部屋表示は成立済みの室内境界を示します。",
                        "選択した魂からの線は、確定済みの作業対象を結びます。移動経路の予測ではありません。範囲編集で一時的に表示が変わる場合も、操作終了後は手動で選んだ表示へ戻ります。",
                    ],
                ),
                HelpEntry::new(
                    HelpEntryId::new("world-object-actions"),
                    "右クリック操作",
                    [
                        "対象上の右クリックは、同じ地点に残っている選択済みの候補を優先してコンテキストメニューを開きます。該当する候補がなければ手前の対象を使います。使い魔では確認、Task Area編集、Operationを選べます。",
                        "使い魔を選択して空き地を右クリックすると移動先を更新し、短時間マーカーを表示します。対象上では移動よりコンテキストメニューを優先します。",
                        "Doorはメニューから施錠・解錠できます。TankとMud MixerはMoveを選べますが、移動できない建物にはMoveを表示しません。",
                        "移設先を利用できなくなった場合は移設を取り消し、建物と付属の保管場所を元の位置に残します。",
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
                        "左上で一時停止を切り替えるか、Ⅱ / 1× / 2× / 4× を直接選択できます。",
                        "時間停止だけではメニューを開きません。停止中も選択・カメラ・担当範囲の編集と履歴・新規の伐採/採掘指定・既存の保管範囲1件の方針・完成ドアの施錠を操作できます。変更はその場で適用し、再開時には再送しません。",
                        "停止中は運搬指定・建築/移設・Zone作成/解除・植林・Soul配属・作業取消・方針の範囲編集を利用できません。対象ボタンは薄く表示し、Tooltipで停止中の制限を案内します。AIと移動・作業進行は停止したままです。",
                        "右上の「メニュー」から保存・読込・設定を開けます。終了キーは入力欄、前面の画面、コンテキストメニュー、未確定の操作、開いた操作メニュー、右の共通枠の順に戻り、何も開いていなければシステムメニューを開きます。",
                        "メニューが止めた時間だけ、閉じると元の速度で再開します。元から停止していた場合は停止を維持します。メニュー上のHelp/Settingsを閉じても時間は再開しません。",
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
