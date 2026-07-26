use hw_ui::help::{HelpEntry, HelpEntryId, HelpSectionId, HelpTopic, HelpTopicId};

use super::super::{
    HelpContribution,
    manifest::{HelpOwnerId, PlayerFeatureId},
};

pub(crate) fn soul_energy() -> Result<HelpContribution, super::super::HelpCatalogError> {
    Ok(HelpContribution {
        feature: PlayerFeatureId::SoulEnergy,
        owner: HelpOwnerId::SoulEnergy,
        section_id: HelpSectionId::new("familiars-workers"),
        section_title: "Familiar と作業員",
        topic: HelpTopic::new(
            HelpTopicId::new("soul-energy"),
            "Soul Energy",
            [
                HelpEntry::new(
                    HelpEntryId::new("soul-energy-status"),
                    "Yard の電力網を確認する",
                    [
                        "電力は Yard ごとのリアルタイムな発電量と需要で決まり、蓄電はしません。",
                        "Outdoor Lamp を選ぶと Demand と Grid の発電量 / 消費量を確認できます。発電が需要を下回ると BLACKOUT になり、接続中の設備が停止します。",
                    ],
                ),
                HelpEntry::new(
                    HelpEntryId::new("soul-energy-recovery"),
                    "Soul Spa で発電する",
                    [
                        "Yard 内に Soul Spa を建てて Bone を搬入すると、Operational になった発電枠へ Soul が入れます。",
                        "発電中の Soul は Dream を消費します。Lamp を増やしたら、発電量、Soul の Dream、Yard への接続を一緒に確認してください。",
                    ],
                ),
            ],
        ),
    })
}
