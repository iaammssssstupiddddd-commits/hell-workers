# ワールド選択とオブジェクトスナップ

通常モードのワールド選択は、カーソルを強制移動しない画面座標ベースのソフトスナップを使います。
Hover、左クリック、右クリックは `interface/selection/hit_test.rs` の共通resolverから同じ候補順位を受け取ります。

## 操作契約

- 左ボタンは押下位置の候補を保持し、5 logical px以内で離した場合だけ選択を確定します。
- 5 pxを超えるドラッグは選択を変更しません。Mouse Drag Panが有効なら、しきい値を超えた移動分からカメラへ適用します。
- 同じ候補列を4 px以内で再クリックすると（時間制限なし）、重なった対象を順に選択します。
- 右クリックは非Floor対象のコンテキストメニューを優先します。空地または完成Floor上では、選択中Familiarの移動指示として扱います。
- コンテキストメニューが開いているときのEscは`CloseContextMenu`としてmenuと古いopen要求だけを消去し、Familiarの命令やactive modeを変更しません。Helpなど最前面overlayがある場合は、そのoverlayのEscを優先します。
- UI capture、入力欄、modal、selection suppression中のworld操作は受理しません。

## 候補と形状

候補順位はDirect Familiar、Soul、地面上Resource、Building系、Task Area、Snapped各対象、完成Floorの順です。
同順位はshape距離、描画depth、Entity IDで安定化します。Familiarは12 px、Soulは10 px、通常objectは8 px、Doorは12 pxの外周余白を持ちます。

Building、Blueprint、完成Floor、StockpileはTransform中心の円ではなく、`WorldMap`に登録されたowner cellを正本とします。
このため2×2設備、2×5 Bridge、Blueprintの予約済み全セルを同一対象として扱います。通常buildingとFloorが同じcellにある場合はbuildingを先にし、Floorは巡回候補に残します。

Familiar、Soul、地面上Resource、Tree／Rockはcrate-owned spatial indexをbroad phaseに使います。明示Hiddenなactor／resource／obstacle、`StoredIn`または`LoadedIn`のResource、owner不一致のmap targetは候補から除外します。

## 表示と操作結果

Direct Hoverは淡い水色、Snapped Hoverは強い水色、選択中targetは薄い黄色の塗りと輪郭で表示します。Building系はowner footprint全体の矩形へ表示を合わせます。WorldMapが同一Entityの占有セルを移動した場合も次の表示更新で再計算します。

Familiarの地面移動を受理すると、移動先へ短時間の緑色マーカーを表示します。BuildingのMove capabilityは`BuildingType::is_player_movable`が正本で、現在はTankとMud Mixerだけです。hover action、context menu、intent validation、実行処理は同じpredicateを使います。

設定済みの作業範囲、使い魔の指揮範囲、Site/Yard境界は通常時も常設します。「表示」で担当範囲の凡例・保管タイル・消費設備の給電状態・成立した部屋を切り替えても常設エリアを隠しません。選択した魂の線は実際の確定済み作業先を結び、経路予測ではありません。ツールの一時表示が終わると手動表示へ戻ります。詳細の開閉は選択と独立し、閉じてもカメラ・選択・pinを保持します。

右クリックメニューのアンカーはwindow座標をUiScaleで割り、UIのlogical pxへ変換します。

## 実装境界

- `hw_core::selection`: candidate型、画面矩形距離、順位、表示用resource
- `hw_ui::selection`: gesture intentとtyped placement validation／feedback
- `hw_spatial::selectable_obstacle`: Tree／Rockの差分更新index
- `bevy_app::interface::selection`: WorldMap／index／ECSのcandidate収集、gesture、intent適用
- `hw_visual::selection_indicator`: Hover、footprint selection、Familiar destination marker

プレイヤー向け説明はHelpの`camera-selection` topicに置き、`world-selection`と`world-object-actions`のstable entry IDで管理します。

### 中ボタンの専用pan

Mouse Drag Pan有効時、中ボタンをワールド上で押してドラッグすると配置・移動・範囲編集モードを保ったままカメラを動かす。左/右gestureが先に始まっている場合は中ボタンが割り込まない。中ボタンが先の場合、左/右入力は選択・配置・範囲開始/確定へ送らず、全ボタンreleaseまで抑止する。UI・入力欄・modal・focus消失・world置換で中断し、押したまま再開しない。`UiInputState.world_pointer_claimed`で既存world入力consumerを遮断し、modal開始扱いにはしない。

## 重なり候補一覧（U31）

同じ画面位置（4 logical px以内）で同じ候補順をクリックすると巡回する。500 msの連打期限は撤廃した。複数候補を選んだ地点はworld座標で保持し、画面上部の「候補 n/m — 一覧を開く」から名前付き一覧を開ける。一覧はScrollArea/Scrollbarを使用し、行のreleaseで選択と詳細表示を更新する（camera/pinは不変）。Escは既存ContextMenuの閉鎖ownerで一覧だけを閉じる。

元地点を共通SelectionResolverで再検査し、候補や地点の変更でrevisionを進める。buttonはtarget/revision/epochを持ち、古い候補やworld置換後の操作は拒否する。UI行はrevision変更で新Entityに再生成するため、古いpressが新しい対象へ移らない。TaskArea境界は別のArea開始操作であり候補一覧から除外する。mode変更・foreground capture・loadでは一時候補を破棄する。

右クリックではFloor/TaskAreaを除いた現在の候補から選択済みEntityを優先し、不在なら既定順の最初を使う。Floor/空地だけなら従来のFamiliar地面移動へ進む。stored/loaded/hidden対象は既存resolverの除外規則を共用する。
