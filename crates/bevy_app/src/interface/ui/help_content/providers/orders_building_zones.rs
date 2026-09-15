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
                        "伐採・採掘・運搬・Areaは使い魔が担当します。選択が使い魔でなければ担当範囲のない使い魔を優先して自動選択し、使い魔がいない場合は開始できない理由を通知します。",
                        "担当名はモード表示で確認できます。指定後は対象・適用件数と運搬の受入先不足などを通知履歴で確認できます。Areaは新たに配属した作業件数を表示します。",
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
                        "範囲編集パネルでは、戻す／やり直す対象の使い魔と復元後の寸法、コピー・貼付け先、3つの枠の保存寸法を実行前に確認できます。履歴や保存内容がない操作は理由付きで無効になります。",
                        "Undo／Redoは使い魔全体の履歴です。実行すると表示された担当へ選択が切り替わります。保存枠は寸法を保持し、現在の範囲中心（範囲がなければ目的地）へ適用します。",
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
                        "床・壁をドラッグしている間は、採用・除外タイル数と採用分だけの必要資材を画面下部で確認できます。モード表示の次操作を読み、指定を確定してください。長い案内はボタン列の上で折り返して表示します。",
                        "Floor を Wall と Door で囲むと Room の境界が表示されます。完成した設備を床上に置いても Room は維持されます。",
                        "床を選択すると情報パネルで部屋の成立条件を確認できます。不成立なら境界の開放・扉不足・広さ上限などの理由と確認位置を表示します。",
                        "扉を選択すると開閉・施錠状態が表示されます。右クリックの施錠／解錠で通行を切り替えます。",
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
                        "ドラッグ中は採用・除外セル数、Yardでは追加面積を確認できます。StockpileはYard外を含む範囲全体を拒否し、Yard内の建物・既存Stockpile・歩行不能セルは除外します。",
                        "Yardは既存範囲から拡張し、Site・別のYardとの重なりや変更のない範囲を拒否します。条件が変わった場合は確定せず、再指定を案内します。失敗時やカーソルが画面から外れた場合もドラッグを終了します。",
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
                    [
                        "左パネルを Tasks に切り替えて行を選びます。「現地へ」でカメラ移動、「詳細を固定」で情報パネルの対象を固定できます。行の選択だけではカメラや固定対象を変えません。",
                    ],
                ),
                HelpEntry::new(
                    HelpEntryId::new("task-dashboard-filter-sort"),
                    "絞り込みと並べ替え",
                    [
                        "Type / State / Priority / Workers を開き、選択肢から条件を直接選べます。「全解除」は4つの条件をまとめて解除し、並び順は維持します。0件になった場合も操作できます。",
                        "行を選ぶと、確定している担当の詳細や運搬の関連先へ進めます。関連づけが未確定の場合は候補を推測して表示しません。関連先を開くと情報パネルに固定され、カメラは移動しません。",
                        "Sort と Order では、仕事種別・状態・優先度・担当数の並び順と昇順／降順を変更できます。",
                        "一覧は20件ごとのページです。下部の先頭・前・次・末尾ボタンで全件へ移動し、表示範囲と総件数を確認できます。各ページ内はホイールとスクロールバーで移動します。絞り込みや並べ替えを変えると先頭ページへ戻ります。",
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
