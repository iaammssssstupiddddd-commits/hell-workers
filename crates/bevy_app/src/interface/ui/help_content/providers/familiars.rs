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
                        "左一覧の行を選ぶと対象へ注目できます。前後の候補を順に巡回することもできます。",
                        "検索欄の入力中はショートカットが抑止されます。",
                    ],
                )
                .with_shortcut(format!(
                    "{} / {}",
                    shortcut(InputAction::ListNext)?,
                    shortcut(InputAction::ListPrevious)?
                )),
                HelpEntry::new(
                    HelpEntryId::new("soul-assignment"),
                    "Soul の所属変更",
                    [
                        "Soul の行を Familiar セクションへドラッグすると所属を変更できます。",
                        "Familiar の使役上限を超える場合は割り当てられません。",
                    ],
                ),
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
                        "通常状態の Familiar を選択中に、Idle と Patrol を切り替えられます。",
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
                        "情報パネルを pin すると、ポインターを別の対象へ動かしても表示対象を維持します。",
                        "unpin すると現在の hover / selection に追従します。",
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
