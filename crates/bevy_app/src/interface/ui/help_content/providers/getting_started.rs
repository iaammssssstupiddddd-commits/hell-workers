use hw_ui::help::{HelpEntry, HelpEntryId, HelpSectionId, HelpTopic, HelpTopicId};

use super::super::{
    HelpContribution,
    manifest::{HelpOwnerId, PlayerFeatureId},
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
                    ],
                ),
                HelpEntry::new(
                    HelpEntryId::new("getting-started-first-steps"),
                    "最初に確認する場所",
                    [
                        "左の一覧で Familiar と未所属 Soul を確認し、下の Orders・Architect・Zones から仕事を作ります。",
                        "対象を選ぶと右の情報パネルに状態と操作が表示されます。",
                    ],
                ),
            ],
        ),
    })
}
