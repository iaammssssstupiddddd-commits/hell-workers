use crate::input_actions::InputAction;
use hw_ui::help::{HelpEntry, HelpEntryId, HelpSectionId, HelpTopic, HelpTopicId};

use super::super::{
    HelpContribution,
    manifest::{HelpOwnerId, PlayerFeatureId},
    shortcut,
};

pub(crate) fn getting_started() -> Result<HelpContribution, super::super::HelpCatalogError> {
    Ok(HelpContribution {
        feature: PlayerFeatureId::GettingStarted,
        owner: HelpOwnerId::RootOnboarding,
        section_id: HelpSectionId::new("getting-started"),
        section_title: "はじめに",
        topic: HelpTopic::new(
            HelpTopicId::new("getting-started"),
            "基本の仕事ループ",
            [
                HelpEntry::new(
                    HelpEntryId::new("getting-started-work-loop"),
                    "Familiar が仕事を管理します",
                    [
                        "Familiar を選び、担当範囲と命令を決めると、範囲内の Soul が必要な仕事へ自動で割り当てられます。",
                        "資源・建築予定・作業範囲を整え、一覧と通知で詰まりを確認するのが基本です。",
                        "ヘルプ下部の「操作ガイドを開始」から任意の案内を開始できます。使い魔選択→担当範囲→本人の伐採/採掘指定→Soulの作業開始と完了を、実際のゲーム状態から確認します。既存の担当範囲・本人の指定も利用できます。",
                        "使い魔・範囲・対象が失われると必要な手順へ戻ります。待機中はTasksの停止理由、Soulの状態、通路、仕事設定を確認してください。ガイドの閉じる/スキップで終了し、読込でも進捗を破棄します。Helpからいつでもやり直せます。",
                        "長いガイドは本文をスクロールして読めます。下部の終了ボタンは本文をスクロールしても残ります。",
                    ],
                ),
                HelpEntry::new(
                    HelpEntryId::new("getting-started-first-steps"),
                    "最初に確認する場所",
                    [
                        "ボタンは押してから同じボタンの上で離すと実行します。押している途中で外へ動かしたり、対象や画面が変わったりすると実行を取り消します。",
                        "ダイアログ内ではフォーカス枠を移動してボタンを決定できます。隠れた項目・無効な項目を飛ばし、末尾から先頭へ戻ります。確認画面では戻る操作を先に選び、閉じると利用可能な呼出元へ戻ります。入力欄では文字編集を優先します。",
                        "左の一覧で Familiar と未所属 Soul を確認し、下の Orders・Architect・Zones から仕事を作ります。",
                        "対象を選ぶと右の情報パネルに状態と操作が表示されます。",
                        "ヘルプは項目ごとの読書位置を覚えます。別の項目から戻ったときや閉じて開き直したときも続きから読めます。ワールドの読込ではこの履歴をリセットします。",
                        "ヘルプの検索欄は見出しと本文を検索します。空白で区切った語はすべて一致する項目に絞り込み、結果を選ぶと該当見出しへ移動します。検索欄の Esc は検索を消して編集を終えます。",
                        "情報パネルの「詳しく（ヘルプ）」から、Soul・保管範囲・Soul Spa・電力の説明を開けます。その他の対象では情報パネルの使い方を表示します。別のダイアログが前景にある間は開きません。",
                    ],
                ).with_shortcut(format!("次の項目: {} / 前の項目: {} / ボタン決定: {}",
                    shortcut(InputAction::ModalFocusNext)?, shortcut(InputAction::ModalFocusPrevious)?,
                    shortcut(InputAction::ModalActivate)?)),
            ],
        ),
    })
}
