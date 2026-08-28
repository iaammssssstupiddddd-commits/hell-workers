# ワールド選択とオブジェクトスナップ

通常モードのワールド選択は、カーソルを強制移動しない画面座標ベースのソフトスナップを使います。
Hover、左クリック、右クリックは `interface/selection/hit_test.rs` の共通resolverから同じ候補順位を受け取ります。

## 操作契約

- 左ボタンは押下位置の候補を保持し、5 logical px以内で離した場合だけ選択を確定します。
- 5 pxを超えるドラッグは選択を変更しません。Mouse Drag Panが有効なら、しきい値を超えた移動分からカメラへ適用します。
- 同じ候補列を500 ms以内・4 px以内で再クリックすると、重なった対象を順に選択します。
- 右クリックは非Floor対象のコンテキストメニューを優先します。空地または完成Floor上では、選択中Familiarの移動指示として扱います。
- UI capture、入力欄、modal、selection suppression中のworld操作は受理しません。

## 候補と形状

候補順位はDirect Familiar、Soul、地面上Resource、Building系、Task Area、Snapped各対象、完成Floorの順です。
同順位はshape距離、描画depth、Entity IDで安定化します。Familiarは12 px、Soulは10 px、通常objectは8 px、Doorは12 pxの外周余白を持ちます。

Building、Blueprint、完成Floor、StockpileはTransform中心の円ではなく、`WorldMap`に登録されたowner cellを正本とします。
このため2×2設備、2×5 Bridge、Blueprintの予約済み全セルを同一対象として扱います。通常buildingとFloorが同じcellにある場合はbuildingを先にし、Floorは巡回候補に残します。

Familiar、Soul、地面上Resource、Tree／Rockはcrate-owned spatial indexをbroad phaseに使います。明示Hiddenなactor／resource／obstacle、`StoredIn`または`LoadedIn`のResource、owner不一致のmap targetは候補から除外します。

## 表示と操作結果

Direct Hoverは淡い水色、Snapped Hoverは強い水色、選択中targetは黄色で表示します。Building系はowner footprint全体の矩形へ表示を合わせます。WorldMapが同一Entityの占有セルを移動した場合も次の表示更新で再計算します。

Familiarの地面移動を受理すると、移動先へ短時間の緑色マーカーを表示します。BuildingのMove capabilityは`BuildingType::is_player_movable`が正本で、現在はTankとMud Mixerだけです。hover action、context menu、intent validation、実行処理は同じpredicateを使います。

## 実装境界

- `hw_core::selection`: candidate型、画面矩形距離、順位、表示用resource
- `hw_ui::selection`: gesture intentとtyped placement validation／feedback
- `hw_spatial::selectable_obstacle`: Tree／Rockの差分更新index
- `bevy_app::interface::selection`: WorldMap／index／ECSのcandidate収集、gesture、intent適用
- `hw_visual::selection_indicator`: Hover、footprint selection、Familiar destination marker

プレイヤー向け説明はHelpの`camera-selection` topicに置き、`world-selection`と`world-object-actions`のstable entry IDで管理します。
