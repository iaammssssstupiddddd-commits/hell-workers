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
                        "Outdoor Lamp を選ぶと、接続、発電・供給済み需要・予備力または不足量、設備件数、供給状態と遮断順を確認できます。供給不足時は High、Normal、Low の順で給電し、同じ優先度では位置順に遮断します。",
                        "情報パネルの Priority ボタンで設備の優先度を変更できます。Settings の Power priority allocation を無効にすると、需要超過時は接続中の全設備が停止する従来方式へ戻ります。",
                    ],
                ),
                HelpEntry::new(
                    HelpEntryId::new("soul-energy-recovery"),
                    "Soul Spa で発電する",
                    [
                        "Yard 内に Soul Spa を建てて選択すると Bone の搬入進捗を確認できます。建設中は情報パネルまたは搬入タスクからキャンセルでき、実際に搬入済みの Bone だけをすべて返します。一時停止中は再開してからキャンセルしてください。Operational になると出力と電力網が表示され、発電枠へ Soul が入れます。",
                        "情報パネルの Active slots は 0〜4 で変更できます。稼働数より小さくしても作業中の Soul は追い出さず、終了後の新規割り当てだけを設定枠まで止めます。",
                        "発電中の Soul は Dream を消費します。Lamp を増やしたら、発電量、Soul の Dream、Yard への接続を一緒に確認してください。",
                    ],
                ),
            ],
        ),
    })
}
