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
                        "下部の「作業を指示」から作業を選び、対象をクリックまたは範囲ドラッグします。",
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
                        "「作業を指示」の「建物を解体」を選び、完成した建物をクリックします。カーソル表示で対象にできるかと、指定できない理由を確認できます。",
                        "指示は専用タスクとして「管理」の「仕事」に追加されます。優先度の変更と確認付きキャンセルができ、解体を許可した Familiar が担当します。",
                        "カーソルには固定回収量も表示されます。Wall・Door・Tank・WheelbarrowParking は Wood×1、MudMixer・RestArea は Wood×2、Floor・OutdoorLamp は Bone×1、BonePile は Bone×5、SoulSpa は Bone×6、Bridge は Rock×3、SandPile は回収なしです。",
                        "設備内の資材や利用中の対象を安全に退避できない間は、建物を残したまま理由付きで待機します。Wall、Door、Floor、Bridge、Operational Soul Spa、Outdoor Lamp も対象になり、通路・部屋・電力網は撤去後に再計算されます。指定結果と解体結果は通知にも表示されます。",
                    ],
                )
                .with_shortcut(shortcut(InputAction::CancelActiveMode)?),
                HelpEntry::new(
                    HelpEntryId::new("area-edit"),
                    "使い魔の作業範囲を指定・変更する",
                    [
                        "使い魔を左クリックし、詳細上部の「作業範囲を指定／変更」から開始します。管理一覧で使い魔を選んだ後も同じ入口を使えます。固定した詳細では、現在の選択ではなく表示中の使い魔が対象になります。「作業を指示」の範囲指定、使い魔の右クリック、既存範囲の枠の左クリックからも開始できます。",
                        "地面をドラッグして範囲を指定し、離すと適用します。範囲内のドラッグで移動、辺・角でサイズ変更ができます。「編集を終了」は適用済みの範囲を保持します。時間の一時停止中も編集できますが、別の前景画面を開いている間は操作できません。",
                        "小型の範囲編集パネルには対象名と現在の寸法を表示します。「詳細操作を開く」で copy / paste、undo / redo、3つのサイズ保存枠を表示します。各ボタンにカーソルを合わせると、対象・寸法や実行できない理由を確認できます。詳細は対象切替と編集終了で閉じます。",
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
                    "建てる",
                    [
                        "「建てる」で全12種類の画像・名前・用途を一覧します。カテゴリで絞り込むこともできます。種類を選ぶとカタログを閉じ、地図上の配置へ進みます。床と壁は範囲を指定して施工予定を作ります。",
                        "必要資源が届くと、担当可能な Soul が工程を進めます。",
                        "床・壁をドラッグしている間は、採用・除外タイル数と採用分だけの必要資材を画面下部で確認できます。モード表示の次操作を読み、指定を確定してください。長い案内はボタン列の上で折り返して表示します。",
                        "床を壁とドアで囲むと部屋が成立し、「表示」の「成立した部屋」で境界を確認できます。完成した設備を床上に置いても部屋は維持されます。",
                        "床を選択すると情報パネルで部屋の成立条件を確認できます。不成立なら境界の開放・扉不足・広さ上限などの理由と確認位置を表示します。",
                        "扉を選択すると開閉・施錠状態が表示されます。右クリックの施錠／解錠で通行を切り替えます。",
                        "Outdoor Lamp は通行できますが建物としてタイルを占有するため、同じ場所へ別の建物を重ねて配置できません。",
                    ],
                )
                .with_shortcut(shortcut(InputAction::ToggleArchitect)?),
                HelpEntry::new(
                    HelpEntryId::new("zones-workflow"),
                    "範囲を設定する",
                    [
                        "「範囲を設定」の「保管場所」で保管範囲を作成し、「Yardを拡張」で既存範囲を広げます。「保管範囲を削除」は保管場所の解除に使います。",
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
                        "Dream の「植樹」を選び、植える範囲を指定します。必要な Dream と成立条件を確認してください。",
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
                        "「管理」を開き「仕事」に切り替えて行を選びます。行の優先度・取消などの操作は一覧内で行います。「現地へ」でカメラ移動、「詳細を固定」で共通枠を詳細に切り替えます。行の選択だけではカメラや固定対象を変えません。",
                    ],
                ),
                HelpEntry::new(
                    HelpEntryId::new("task-dashboard-filter-sort"),
                    "絞り込みと並べ替え",
                    [
                        "上部の「要対応」は現在の停止対象の件数です。新鮮な診断で停止が確定した仕事だけを数え、同じ施工元が確認できる仕事はまとめます。「判定中」は別の件数で、通知の未読数とも異なります。",
                        "「要対応」を押すと以前の絞り込みを解除して停止中の仕事を開き、先頭ページを表示します。並び順は維持します。対象をまとめたHUD件数と、個々の仕事の明細行数は異なる場合があります。",
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
