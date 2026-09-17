use hw_ui::help::{HelpEntry, HelpEntryId, HelpSectionId, HelpTopic, HelpTopicId};

use crate::input_actions::InputAction;

use super::super::{
    HelpCatalogError, HelpContribution,
    manifest::{HelpOwnerId, PlayerFeatureId},
    shortcut,
};

pub(crate) fn entity_list_and_squads() -> Result<HelpContribution, HelpCatalogError> {
    Ok(HelpContribution {
        feature: PlayerFeatureId::EntityListAndSquads,
        owner: HelpOwnerId::FamiliarManagement,
        section_id: HelpSectionId::new("familiars-workers"),
        section_title: "Familiar と作業員",
        topic: HelpTopic::new(
            HelpTopicId::new("entity-list-squads"),
            "一覧と所属",
            [
                HelpEntry::new(
                    HelpEntryId::new("entity-list-selection"),
                    "一覧から選択",
                    [
                        "「管理」または上部の人数から右の一覧を開きます。行を選ぶと同じ枠で詳細を表示し、「戻る」で一覧へ復帰します。「現地へ」だけがカメラを移動し、選択や固定中の詳細は変えません。",
                        "検索欄の入力中はショートカットが抑止されます。",
                        "Soul名の部分一致で、折りたたんだ配下・未所属Soulも検索できます。一致するSoulのグループは一時的に開き、検索を消すと元の開閉状態へ戻ります。",
                        "一覧本文は使い魔・配下・未所属をまとめてスクロールできます。候補の巡回は一覧を開いている間だけ有効で、表示順で末尾と先頭をつなぎます。",
                        "Soul行の作業アイコンには作業名を併記します。発電・解体・移設・精製・骨回収も個別の名前で確認できます。",
                        "「使い魔・魂」と「仕事」を切り替えられます。最小化すると本文と検索欄を隠し、展開すると選んだタブへ戻ります。検索・絞り込み・一覧の読書位置は、詳細への移動や開閉後も保持します。",
                        "使い魔の「所属 n/m」は現在の所属人数と使役上限です。±は上限を変更し、即時に魂を雇用する操作ではありません。「配下を監督」は使い魔の状態で、全員が作業中という意味ではありません。",
                        "「仕事あり」は移動・準備を含め仕事が割り当てられた魂、「休息中」は仕事がなく休息・睡眠中の魂です。集会内の睡眠はこの休息数に含みません。検索や折り畳み前の全所属から数えます。",
                    ],
                )
                .with_shortcut(format!(
                    "{} / {} / 戻る: {}",
                    shortcut(InputAction::ListNext)?,
                    shortcut(InputAction::ListPrevious)?,
                    shortcut(InputAction::WorkspaceBack)?
                )),
                HelpEntry::new(
                    HelpEntryId::new("soul-assignment"),
                    "Soul の所属変更",
                    [
                        "Soul の行を Familiar セクションへドラッグすると所属を変更できます。",
                        "長押しの待ち時間はゲーム速度に依存しません。ドラッグ中に一覧本文の上端・下端へ寄せるとスクロールします。",
                        "Familiar の使役上限を超える場合は割り当てられません。",
                    ],
                ),
                HelpEntry::new(
                    HelpEntryId::new("familiar-operation-policy"),
                    "Familiar の運用ポリシー",
                    [
                        "Familiar のコンテキストメニューから Open Operation を選ぶと、疲労閾値、最大使役 Soul 数、作業種別ごとの許可と Low / Normal / High の優先度を設定できます。",
                        "Disable all は新しい作業の割り当てだけを止めます。すでに実行中の作業と休息などの自己維持は継続します。",
                        "方針だけで新しい割り当てが止まった仕事は「管理」の「仕事」に Blocked: Disabled by familiar policy と表示されます。担当が確定していれば「担当の詳細」から情報パネルへ進み、新規割当を停止している作業種別を確認できます。",
                        "命令が Idle でも、1体以上所属していれば既存メンバーを監視しながら最大使役 Soul 数まで追加募集を続けます。",
                        "設定は Familiar ごとに保存されます。最大数を現在の使役数より下げると、超過した Soul は所属と作業から解放されます。",
                    ],
                )
                .with_shortcut(shortcut(InputAction::CloseOperationDialog)?),
            ],
        ),
    })
}

pub(crate) fn familiar_commands() -> Result<HelpContribution, HelpCatalogError> {
    Ok(HelpContribution {
        feature: PlayerFeatureId::FamiliarCommands,
        owner: HelpOwnerId::FamiliarManagement,
        section_id: HelpSectionId::new("familiars-workers"),
        section_title: "Familiar と作業員",
        topic: HelpTopic::new(
            HelpTopicId::new("familiar-commands"),
            "Familiar の命令",
            [
                HelpEntry::new(
                    HelpEntryId::new("familiar-designations"),
                    "作業指定",
                    [
                        "Familiar を選択中に Chop / Mine / Haul を選び、対象範囲をクリックまたはドラッグします。",
                        "Cancel は既存の指定を範囲で取り消します。",
                    ],
                )
                .with_shortcut(format!(
                    "{} / {} / {} / {}",
                    shortcut(InputAction::FamiliarChop)?,
                    shortcut(InputAction::FamiliarMine)?,
                    shortcut(InputAction::FamiliarHaul)?,
                    shortcut(InputAction::FamiliarCancelDesignation)?,
                )),
                HelpEntry::new(
                    HelpEntryId::new("familiar-idle-patrol"),
                    "Idle / Patrol",
                    [
                        "通常状態の Familiar を選択中にショートカット、または右クリックメニューの「待機 / 巡回」で Idle と Patrol を切り替えます。作業範囲がない場合は Idle になります。時間操作のショートカットは選択対象によって変わりません。",
                    ],
                )
                .with_shortcut(shortcut(InputAction::ToggleFamiliarIdlePatrol)?),
            ],
        ),
    })
}

pub(crate) fn info_panel() -> Result<HelpContribution, HelpCatalogError> {
    Ok(HelpContribution {
        feature: PlayerFeatureId::InfoPanel,
        owner: HelpOwnerId::FamiliarManagement,
        section_id: HelpSectionId::new("familiars-workers"),
        section_title: "Familiar と作業員",
        topic: HelpTopic::new(
            HelpTopicId::new("info-panel"),
            "情報パネル",
            [
                HelpEntry::new(
                    HelpEntryId::new("info-panel-pin"),
                    "表示を固定する",
                    [
                        "詳細を固定すると、別の対象を選んでも固定中のカードを優先します。「現在選択: 対象名 → 詳細へ」で別対象の一時詳細を開き、「戻る」で固定カードへ戻れます。",
                        "「選択を表示」で固定を解除すると、現在の選択を表示します。固定中の対象名とこのボタンは、本文をスクロールしても残ります。",
                        "別の対象の詳細を開くと本文は先頭へ戻り、同じ対象の情報更新では読んでいる位置を保ちます。",
                        "「閉じる」は枠を隠すだけで、固定解除ではありません。管理も同じ枠で開き、固定対象と選択を保持します。ワールドを読み込むと対象と戻り先を破棄します。",
                    ],
                ),
                HelpEntry::new(
                    HelpEntryId::new("soul-rename"),
                    "Soul の名前変更",
                    [
                        "Soul の情報パネルから名前を編集できます。Enter で確定、Esc でキャンセルします。",
                    ],
                )
                .with_shortcut("Enter / Esc"),
            ],
        ),
    })
}
