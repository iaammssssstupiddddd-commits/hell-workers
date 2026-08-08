use hw_ui::help::{HelpEntry, HelpEntryId, HelpSectionId, HelpTopic, HelpTopicId};

use crate::input_actions::InputAction;

use super::super::{
    HelpCatalogError, HelpContribution,
    manifest::{HelpOwnerId, PlayerFeatureId},
    shortcut,
};

pub(crate) fn orders_and_areas() -> Result<HelpContribution, HelpCatalogError> {
    Ok(HelpContribution {
        feature: PlayerFeatureId::OrdersAndAreas,
        owner: HelpOwnerId::OrdersAndBuilding,
        section_id: HelpSectionId::new("orders-building-zones"),
        section_title: "タスク・建築・ゾーン・Dream",
        topic: HelpTopic::new(
            HelpTopicId::new("orders-areas"),
            "Orders と範囲編集",
            [
                HelpEntry::new(
                    HelpEntryId::new("orders-designation"),
                    "タスクを指定する",
                    [
                        "下部の Orders から作業を選び、対象をクリックまたは範囲ドラッグします。",
                        "未確定の操作または開いているメニューは、その時点の入力文脈に応じて解除できます。",
                    ],
                )
                .with_shortcut(shortcut(InputAction::CancelActiveMode)?),
                HelpEntry::new(
                    HelpEntryId::new("building-deconstruction"),
                    "完成した建物を解体する",
                    [
                        "Orders の Deconstruct を選び、完成した建物をクリックします。カーソル表示で対象にできるかと、指定できない理由を確認できます。",
                        "指示は専用タスクとして Tasks に追加されます。優先度の変更と確認付きキャンセルができ、解体を許可した Familiar が担当します。",
                        "カーソルには固定回収量も表示されます。Wall・Door・Tank・WheelbarrowParking は Wood×1、MudMixer・RestArea は Wood×2、Floor・OutdoorLamp は Bone×1、BonePile は Bone×5、SoulSpa は Bone×6、Bridge は Rock×3、SandPile は回収なしです。",
                        "設備内の資材や利用中の対象を安全に退避できない間は、建物を残したまま理由付きで待機します。Wall、Door、Floor、Bridge、Operational Soul Spa、Outdoor Lamp も対象になり、通路・部屋・電力網は撤去後に再計算されます。指定結果と解体結果は通知にも表示されます。",
                    ],
                )
                .with_shortcut(shortcut(InputAction::CancelActiveMode)?),
                HelpEntry::new(
                    HelpEntryId::new("area-edit"),
                    "Task Area を編集する",
                    [
                        "範囲編集では copy / paste、undo / redo、3つの preset 保存・読込を利用できます。",
                        "3つの preset は保存用と読込用のショートカットから使い分けます。",
                    ],
                )
                .with_shortcut(format!(
                    "{} / {} / {} / {} / {} / {} / {} / {} / {} / {}",
                    shortcut(InputAction::AreaCopy)?,
                    shortcut(InputAction::AreaPaste)?,
                    shortcut(InputAction::AreaUndo)?,
                    shortcut(InputAction::AreaRedo)?,
                    shortcut(InputAction::AreaSavePreset1)?,
                    shortcut(InputAction::AreaSavePreset2)?,
                    shortcut(InputAction::AreaSavePreset3)?,
                    shortcut(InputAction::AreaLoadPreset1)?,
                    shortcut(InputAction::AreaLoadPreset2)?,
                    shortcut(InputAction::AreaLoadPreset3)?,
                )),
            ],
        ),
    })
}

pub(crate) fn building_zones_dream() -> Result<HelpContribution, HelpCatalogError> {
    Ok(HelpContribution {
        feature: PlayerFeatureId::BuildingZonesDream,
        owner: HelpOwnerId::OrdersAndBuilding,
        section_id: HelpSectionId::new("orders-building-zones"),
        section_title: "タスク・建築・ゾーン・Dream",
        topic: HelpTopic::new(
            HelpTopicId::new("building-zones-dream"),
            "建築・ゾーン・Dream",
            [
                HelpEntry::new(
                    HelpEntryId::new("architect-building"),
                    "Architect で建築",
                    [
                        "建物を選び、world 上で配置します。Floor と Wall は範囲を指定して施工予定を作ります。",
                        "必要資源が届くと、担当可能な Soul が工程を進めます。",
                        "Floor を Wall と Door で囲むと Room の境界が表示されます。完成した設備を床上に置いても Room は維持されます。",
                        "Outdoor Lamp は通行できますが建物としてタイルを占有するため、同じ場所へ別の建物を重ねて配置できません。",
                    ],
                )
                .with_shortcut(shortcut(InputAction::ToggleArchitect)?),
                HelpEntry::new(
                    HelpEntryId::new("zones-workflow"),
                    "Zones で保管範囲を作る",
                    [
                        "Stockpile は新しい保管範囲を作成でき、Yard は既存範囲を拡張できます。Remove は Stockpile の削除に使います。",
                        "Stockpile の対象資源、目標量、優先度、持出可否は情報パネルから変更できます。",
                    ],
                )
                .with_shortcut(shortcut(InputAction::ToggleZones)?),
                HelpEntry::new(
                    HelpEntryId::new("dream-planting"),
                    "Dream で植樹",
                    [
                        "Dream の Plant Trees を選び、植える範囲を指定します。必要な Dream と成立条件を確認してください。",
                    ],
                ),
            ],
        ),
    })
}

pub(crate) fn task_dashboard() -> Result<HelpContribution, HelpCatalogError> {
    Ok(HelpContribution {
        feature: PlayerFeatureId::TaskDashboard,
        owner: HelpOwnerId::OrdersAndBuilding,
        section_id: HelpSectionId::new("orders-building-zones"),
        section_title: "タスク・建築・ゾーン・Dream",
        topic: HelpTopic::new(
            HelpTopicId::new("task-dashboard"),
            "タスク一覧",
            [
                HelpEntry::new(
                    HelpEntryId::new("task-dashboard-focus"),
                    "仕事の場所を確認する",
                    ["左パネルを Tasks に切り替え、行を選ぶと該当する仕事へフォーカスできます。"],
                ),
                HelpEntry::new(
                    HelpEntryId::new("task-dashboard-filter-sort"),
                    "絞り込みと並べ替え",
                    [
                        "Type / State / Priority / Workers の条件を順に切り替えて、表示する仕事を絞り込めます。",
                        "Sort と Order では、仕事種別・状態・優先度・担当数の並び順と昇順／降順を変更できます。",
                    ],
                ),
                HelpEntry::new(
                    HelpEntryId::new("task-dashboard-actions"),
                    "優先度変更とキャンセル",
                    [
                        "変更可能なタスクだけ優先度を調整できます。Deconstruct を含むキャンセルは確認を経て実行され、状態が変わって受理できない場合は理由が通知されます。",
                    ],
                ),
            ],
        ),
    })
}
