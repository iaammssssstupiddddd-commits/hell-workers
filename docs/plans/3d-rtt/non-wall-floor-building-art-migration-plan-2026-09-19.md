# 壁・床以外の建築物のアート移行計画

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `non-wall-floor-building-art-migration-plan-2026-09-19` |
| ステータス | `In Progress` |
| 作成日 | `2026-09-19` |
| 最終更新日 | `2026-09-20` |
| 作成者 | `Codex` |
| 関連提案 | `N/A`（ユーザー依頼による計画） |
| 関連Issue/PR | `N/A` |
| 制作仕様 | [building-art-direction.md](../../building-art-direction.md) |
| 調査基点 | 初稿: `94ccdf23`。自己レビュー: `43ca0cb1`の計画と現行実装（文書変更のみ） |

本書は全10種の移行順、制作物、表示接続、受入条件を所有する。M0の制作・検査toolingと先行無地原本を実装済み。
M1-0のnativeでBridgeと現行川生成の不整合を検出した。ユーザー判断により橋は別途解決し、残る9種を先行する。
runtime表示接続・正式baseline完了・ゲーム内アート受入は未実施。
個別意匠の最終判断はゲーム内の候補画像で行い、本計画の作成をアート受入やreleaseとして扱わない。

自己レビューでは、全種の美術数値を先行2種の試作だけで決める前提を撤回し、共通契約と群別の確定点を分離した。
role別GLB、active/pendingの切替、再利用ghostのanchor復元、移動依頼の行先表示、load後の状態、
基盤自体の性能比較を具体化した。実装済みfixture/toolは§9に記載し、それ以外の新しい型・recipe名は実装予定である。

## 1. 目的

- 解決したい課題: 壁・床以外の建築物で、用途・状態が読める形状と手描き表現を統一し、完成表示から各previewまで整合させる。
- 到達したい状態: モデルが外形・開口・可動部を、テクスチャが手描きの線・材質・色面を担当し、全対象が同じ世界の設備として読める。
- 成功指標: 全10種の完成表示・配置・施工・カタログが整合し、実状態・save/load・撤去に追従する。新しい表示が論理占有・通行・生産・照明の意味を変えない。

## 2. スコープ

### 対象（In Scope）

`BuildingType::ALL`のうちWall / Floorを除く10種。Doorは既存productionの適合監査と不足経路の修正を行う。
建築用asset原本、モデル、テクスチャ、2D画像、preview、asset読み込み・切替・fallback、表示用状態、検証用scenarioを含む。
TankのBucketStorage、Parkingの実物の猫車、休憩者やSpa workerは既存entityを維持し、表示の位置・重なりを確認する。

`2026-09-20`の進行範囲: **Bridge以外の9種を先行**。Bridgeの地形／配置問題と美術導入は別の着手単位へ保留する。
計測専用の川を作る案は不採用。以下の全10種の契約表は再合流時の要件を保持するが、橋の完了を先行9種の依存条件にしない。
先行基盤のruntime接続は新規設備4種＋小物4種、Doorは既存経路の監査。Bridgeのfactory/transform/material/previewは従来経路を維持する。

### 非対象（Out of Scope）

- Wall / Floorの形状・テクスチャ・asset世代の変更。比較画面には現行releaseを固定して置く。
- 材料、建設時間、生産量、回復・発電量、通行、Room、Light Fieldのrule変更。
- 新BuildingType、橋の長さ変更・回転、Doorの新操作や連続開閉animation、移動可能建物の追加。
- Soul / Familiar、資源item、猫車本体のアート再制作。Stockpile / Yard等のゾーンも対象外。
- 全小物の3D化、新しいoutline・照明renderer、過去RtT全段階の再受入。

## 3. 現状とギャップ

寸法は表示画像の余白ではなく、`hw_jobs/src/placement_geometry.rs`の論理占有を示す。
旧アセットマイルストーンの1×1設備表や全GLB候補表を、新規制作の寸法・表示分類の正本にしない。

| 対象 | 占有・経路 | 現状 | 移行で必要な表示 |
| --- | --- | --- | --- |
| Tank | 本体2×2＋別配置のcompanion 2×1、Structural3d | 共通Cuboid、空／途中／満杯の材質交換 | 桶・内壁・独立水面、3水量状態、バケツ置き場と干渉しない接地 |
| MudMixer | 2×2、Structural3d | 共通Cuboid、Idle／Activeの材質交換 | 固定槽・架台＋独立攪拌部、実精製中の回転 |
| RestArea | 2×2、Structural3d | 共通Cuboid、既存Dream粒子 | 布屋根・支柱・入口、空／利用中と既存粒子の整合 |
| SoulSpa | 2×2、Structural3d、完成後も歩行可能 | 建設中から共通Cuboid、専用site/tile経路 | 建設中／稼働可能／実稼働区画。低い4区画と骨の構造 |
| Bridge | 2×5、Structural3d、RiverYMin anchor | 茶色Cuboid、中心高0.09 tile | 橋床・支持部・必要な縁、両岸接続とSoulの通過 |
| Door | 1×1、Structural3d、EW/NS | generation 7の承認済み3状態GLB＋preview | 現行アートの適合監査、カタログを含むpreview統一。必要な差分だけ改修 |
| WheelbarrowParking | 2×2、Foreground2d | 専用PNG＋別entityの猫車 | 木枠・轍・駐車枠。実物の猫車がある／ない状態を妨げない |
| SandPile | 1×1、Foreground2d | 砂item iconと同じPNGを参照 | 建物専用の低い砂山。無限sourceのため残量減少表現を付けない |
| BonePile | 1×1、Foreground2d | 既存PNG、Lampでも流用 | 建物専用の骨山。少数の大きな骨で輪郭を作る |
| OutdoorLamp | 1×1、Foreground2d、歩行可能 | 完成本体はBonePile画像流用、通電で色変更 | 専用の骨支柱・籠、点灯／消灯。既存論理光源と同期 |

### 実装から確認したギャップ

- 設備4種は`building_completion/spawn.rs`で同じ2×2 meshを使い、Bridgeは別の2×5 Cuboidを使う。
- `building3d_cleanup.rs`は`Building3dVisual`に対してowner絶対座標・固定中心高を適用する。可動childへ同markerを付けるとlocal位置を壊す。
- Tank / Mixerの材質syncはroot上のmesh materialを要求する。複数partの状態同期は未実装。
- 通常ghost、Blueprint、pulse child、移動preview、load再構築、既存カタログに画像handleを持つ経路が分散している。
- カタログは生成時に`UiAssets::building_preview()`のhandleをcloneする。Doorも旧`door_closed`を返し、production previewの後着・世代切替を反映する経路が必要。
- SandPileの建物と砂item iconは同じ画像ファイル、BonePileはLampの代用画像として共有される。既存ファイルの上書きでは無関係な表示まで変わる。
- SoulSpaは通常Blueprintを経由せず、Constructingから`Building`と3D visualを持つ。完成eventだけを監視すると段階変化を取りこぼす。
- `docs/building.md`にはTank本体2×1の古い記述がある。M0で実占有2×2、companion 2×1へ同期する。

Doorの既存release・未完closeは[Door計画](production-door-art-plan-2026-09-05.md)と
[preview修正計画](door-preview-alignment-plan-2026-09-11.md)が所有する。本書はそれらの実施済み受入を再計上せず、
新仕様との差と今回変更する経路だけを扱う。旧型枠trackの残件を本計画へ移さない。

## 4. 実装方針（高レベル）

### 4.1 制作と表示分類

モデル・テクスチャ・光の分担は[制作仕様](../../building-art-direction.md)に従う。
新規3D制作はTank / MudMixer / RestArea / SoulSpa / Bridgeの5種（計8 GLB）とし、Doorは既存3GLBを監査する。
小物4種は2D表示を維持し、手描き原図または同じ視点の制作モデルから専用PNGを作る。
小物用の制作モデルを用いても、そのGLBをruntimeへ持ち込む必要はない。

| 対象 | 制作物と初期構成 | 状態の正本・同期方法 |
| --- | --- | --- |
| Tank | 固定桶1 mesh＋水面1 mesh、albedo、代表preview | `StoredItems`とcapacityの既存3分類。空は水面非表示、途中／満杯は所定高さへ。正確な連続水量表示とは主張しない |
| MudMixer | 固定槽・架台1 mesh＋攪拌部1 mesh、albedo、停止姿勢preview | `MudMixerVisualState.is_active`（実`Refining`）。可動部だけVirtual Timeで回転しpauseで停止 |
| RestArea | 固定body 1 mesh、albedo、preview | 既存Dream粒子が実occupantsから発生する経路を維持。粒子の残存・cooldown・pauseがあるため、bodyだけで空／利用中を常時即判別できるとは約束しない。正確な人数は既存UI |
| SoulSpa | 固定body 1 mesh＋共有の稼働部meshを4配置、albedo、必要なemissive、preview | `SoulSpaPhase`、tileのdurable parent、実`TaskWorkers`から建設段階＋4bit稼働mask。建設中は共有施工材質＋既存骨材進捗、稼働部は消灯。`active_slots`は実稼働の代用にしない |
| Bridge | 固定body 1 mesh、albedo、preview | 完成・建設中・撤去は既存lifecycle。長軸と通行域は変更しない |
| Door | 現行3状態mesh、albedo、EW/NS previewを優先再利用 | 既存Door state・topology consumer。カタログ代表画像はEW Closed |
| Parking / SandPile / BonePile | 各専用world PNG＋同じ原本によるpreview | 静的表示。猫車・資源itemの実体と画像を分離 |
| OutdoorLamp | 専用点灯／消灯PNG＋代表preview | `PoweredVisualState`。新画像と旧gray tintを二重適用しない。光源位置・半径・効果は既存契約 |

表のmesh数は制作開始時の部品構成上限であり、実測draw call数ではない。
固定部の結合・共有を優先し、可動部・状態部以外で増やす場合は理由と予算をM0の契約へ記録する。
triangle数、画像解像度、表示高さ、preview canvas・anchorは各群の無地モデルの投影で決め、当該群の着色前に確定する。
壁の240 trianglesやDoorの256px canvasを全設備へ流用しない。

#### role単位の制作・export契約

初期方式は **1 mesh role = 1 GLB = 1 node / 1 mesh / 1 primitive**。新規5種は合計8個の固有GLBを作る。
scene全体の読み込みは使わず、既存Doorと同じ`GltfAssetLabel::Primitive { mesh: 0, primitive: 0 }`を解決する。
node transformはidentity、POSITION/NORMAL/UV0を必須とし、skin・morph・GLB animation・埋込textureを使わない。
固定bodyは足元中心、水面・攪拌部・稼働区画はそれぞれのlocal pivotを原点とする。
頂点はexport後のworld unit、part位置もmanifestでworld unitに統一し、runtimeで再度tile倍率を掛けない。
現行`export_glb.py`の`--geometry-scale`は頂点のみを拡縮するため、位置付きobjectをそのまま渡さず、
role別identity objectへ書き出してから変換する。scene reportだけでなく実GLBのbounds・pivot・UV・法線を再検査する。

| kind | mesh role / runtime leaf上限 | 画像roleと代表preview状態 | 無地・輪郭段階で棄却する条件 |
| --- | --- | --- | --- |
| Tank | `body`, `water` / 2 | albedo、world preview、catalog。Empty | 開口・内壁が読めない、水面の途中／満杯が縁に隠れる、companionの作業位置を塞ぐ |
| MudMixer | `body`, `rotor` / 2 | albedo、world preview、catalog。Idle・角度0 | Tankと外形が区別できない、軸が支持されない、一周で槽を貫通する |
| RestArea | `body` / 1 | albedo、world preview、catalog。利用者なし | 屋根・支柱・入口が読めない、既存Dream発生位置との重なりが不自然 |
| SoulSpa | `body`, `slot` / 5（slotを4配置） | albedo、slot発光mask、world preview、catalog。Operational・稼働mask0 | 4区画が分離しない、tileとslotが一致しない、高い縁で歩行者が埋まる |
| Bridge | `body` / 1 | albedo、world preview、catalog。Complete | 両岸とつながらない、橋端・中央でSoulの足元が橋床に埋まる／浮く |
| Parking | なし / 0 | world、catalog。猫車なし | 実物の猫車を画像へ描き込む、空き状態が駐車場所に見えない |
| SandPile / BonePile | なし / 0 | 各world、catalog。静的 | 両者を低い山の輪郭で区別できない、item iconの差替えが必要になる |
| OutdoorLamp | なし / 0 | world off/on、catalog。Off | 点消灯で外形・canvas・anchorが変わる、骨山と区別できない |
| Door | 既存3状態 / 1 | 既存albedo・EW/NS world preview、catalogはEW Closedをcontain表示 | 現行assetの不適合が確認された場合だけ、既存doorsetの手順で改訂 |

`catalog`はworld画像と同じ原本・世代から正方形canvasへcontainした画像roleとする。専用bytesが不要な場合は
同じartifactを参照してよいが、worldのanchorをcatalogの中央配置で上書きしない。
小物の配置・Blueprintはworld（Lampはoff）を参照し、別の完成イラストを描き直さない。
Spaの発光maskはslotだけに使い、bodyの発光や新しい論理光源は追加しない。

Bridgeはactorの高さを変える機能を前提にしない。現行`actor_billboard.rs`はSoulの中心Yを`0.55 tile`に固定し、
橋面追従を行わない。M4の最初に低い橋床で両岸・端・中央の通過を試し、成立しない案は形状へ戻す。
actor height、通行rule、shader変更が必要なら本計画へ暗黙に追加せず、別の判断事項として止める。

#### 群ごとの納品物と確定点

M0では全種の論理shape・role名・試験方式を固定し、M2〜M5の各群では次の順で成果物をそろえる。

1. `tools/blender_ai_workflow/fixtures/building-<kind>-v1.geometry.json`（新規）に、実shape参照、軸、
   body bounds、part pivot/transform、接地・作業/通行域、状態別可動範囲を記録。既存Door fixtureは継承する。
2. 無地／輪郭のゲーム内比較後、同fixtureへ高さ・triangle/part上限・atlas解像度・preview数値を固定。
   Tank/Mixerだけを先行し、未制作のRest/Spa/Bridgeの美術数値を推測で固定しない。
3. 原本、面とatlas領域の対応表、albedo（Spaは発光maskも）、全role GLB/PNG、post-export report、
   preview projection report、状態→部品の対応表をstagingにそろえる。
4. ArtPreviewの対象画像と承認対象のidentityを結び、技術受入・正式candidate・releaseへ進む。
   asset承認・release許可を計画への同意や検査passから推定しない。

2DのfixtureはGLB/body/pivot欄を理由付きN/Aとし、輪郭bounds・world canvas・接地anchorを記録する。
無地の技術候補でもrole欠落は許さず、neutral albedoと同じ無地原本の暫定previewを用意してArtPreviewで確認する。
これらを最終texture/previewや承認済みassetと混同しない。
数値未記入のfixtureは当該群の着色・正式candidate作成を止めるが、他群の制作まで止めない。
仕様変更時はfixtureを改訂し、影響するexport/preview/受入を再実行する。合格後に上限を結果へ合わせない。

### 4.2 asset単位と切り替え

新規9種には共通schema・loaderと型別role一覧を用い、建物種別ごとに独立した世代・readinessを持たせる。
Doorの既存doorsetは維持する。9個のloader複製や全建物一括でしかreleaseできない構造は作らない。

- 各setにkind、generation、原本・exportの識別、mesh/texture/preview role、partのlocal transform・pivot、canvas・anchor、hashを持たせる。
- 同種全instanceは有限のmesh・texture・material poolを共有する。Spaの各区画も共有meshとoff/on材質を使い、ownerごとのmaterial cloneを作らない。
- 同一kindの全必須roleがreadyな同じ世代だけをactiveにし、完成表示とpreviewの解決結果をまとめてpublishする。各consumerが同じrevisionをrender extract前に反映する順序を固定する。
- 初回のasset欠落・不正・世代不一致はそのkindを既存fallbackへする。新候補の失敗と、表示中のactive自体の無効化は区別する（下表）。正常な他kindやDoorは巻き込まない。
- 切替前のセットを旧参照が残る間に破棄しない。切替後は退役世代のpartと強参照を解放し、世代切替の反復でpoolが増えないことを確認する。同世代の非表示／消灯partは保持する。
- 既存のstaging、ArtPreview、承認済みcandidate、releaseのauthorityを新設備にも適用する。未承認assetの通常起動への流入を防ぐ。既存Wall/Doorの検査を緩めない。

runtime projectionは新しい`building asset-set v1`、拡張子`.buildingset`を採用予定とする。
authoring manifestとは分離し、`schema_version / asset_set_id / kind / generation / manifest_sha256 /
authority / approval・receipt identity / artifacts / parts / previews / geometry_contract_sha256`を持つ。
artifactsはrole・相対path・byte長・SHA-256、partsはmesh/material roleとlocal transform、previewsは用途別の
image role・canvas・anchor・代表状態を持つ。unknown field、重複／欠落role、非有限transform、root外path、
hash不一致、当該起動policyで許可されないauthorityを拒否する。通常起動は承認済みreleaseのみ、
専用ArtPreview/candidate起動はexact identityの明示許可を必要とする。role表はkindごとにexactで、未知kindを汎用boxとして受理しない。
artifact pathはkind＋generation＋hashに束縛した不変pathとし、同じpathのbytesをhot reloadで上書きしない。
変更するのは最後のlocatorだけとする。これによりBの読み込みがAの既存Handleの内容を変えない。
`asset_release_manifest.py`、promotion / projection / install / rollbackへのkind dispatchをM1で追加する。
現在のWall/Door専用toolに新拡張子を渡すだけでは動かない。新schemaの追加で既存validatorを緩めない。

| 入力状態 | 同kindの表示結果 | pendingの扱い |
| --- | --- | --- |
| 起動直後、activeなし | 従来fallback一式 | 全roleを検証・読み込み中 |
| 有効なactive AがありBを読み込み中 | Aの完成・preview一式を維持 | Bだけを別poolで準備 |
| Bが不正／失敗、Aは有効 | Aを維持。Bのエラーを記録 | Bの強参照を解放。同じ失敗を毎frame再生成しない |
| Bの全roleとauthorityが有効 | 同じpresentation境界でA→B | 旧参照解放後にAをretire |
| active自身が破損／失効、または明示fallback | fallback一式へ戻す | 他kindは変更しない |

poolはkindごとにactive最大1＋pending最大1。切替完了後のretiredは強参照ゼロとし、AssetServer/GPUの
解放遅延とアプリの参照漏れを区別する。world loadではasset poolを作り直さず、owner/part cacheだけをresetする。

### 4.3 3D rootと部品の所有

新規3Dの5種は、logical ownerとは独立した`Building3dVisual`を1 rootだけ持ち、描画部品をその子へ置く。
rootは表示modeに対応した基準transform・visibility・状態を持ち、meshを持たない。各描画partに別markerを用いる。
Door / Wall / Floorのroot形式は変更しない。

- production rootのXZは既存logical ownerへ対応、接地Yは0。固定body GLBは足元中心、状態partは§4.1のlocal pivotを原点とする。
- fallbackは同じroot entityを使い、root Yを旧中心高（設備0.4 tile、Bridge0.09 tile）、Cuboid childのlocal Yを0とする。これにより中心高をscaleと独立に保ち、完成bounce中も旧transformを再現する。
- ground rootに旧中心高の固定child offsetを足す方式は採らない。root scaleをs、旧中心高をhとすると中心がs×hへ動いてしまうためである。mode切替時はroot原点・part集合・previewを同時に切り替える。
- ownerの移動・回転・完成bounceはrootへ一度だけ適用し、水面高さ・攪拌角は子localへ適用する。描画childへ`Building3dVisual`を付けない。
- 各描画partにScene RtTの`RenderLayers`を明示する。設備は現行の特別anchor tagなしのLight Field samplingを維持する。現行`structural_light_anchor_mesh_tag`が返すWall/Door専用tagを設備へ流用せず、shaderや照明anchor ruleも変更しない。
- legacyのroot material更新をこの5種から分離し、表示用状態→part consumerでmaterial・visibility・transformを決める。
- 新equipment root markerを旧transform/material systemの除外条件へ追加する。ownerの`Changed<Transform>`だけを更新条件にせず、mode/revision変更・新規rootも同じconsumerで適用する。
- 同世代ではTankのwater、Spaのslotを全て固定生成し、空時のvisibility・on/off材質だけを変える。状態変化のたびにspawn/despawnしない。世代の退役と状態の非activeを区別する。
- owner消失、cancel、解体、world replacement、世代交換で不要childを除去する。通常生成・初期配置・Instant Build・loadは同じfactoryを使う。
- 現行diagnosticsのroot数と描画part数を区別する。meshのないrootが存在するだけで表示成功とは判定せず、各leafのresident handle・visibilityを確認する。

上記変更はM1でfallbackの見え方を保ったまま成立させる。Bevy 0.19の`ChildOf` cleanup、visibility伝播、
`RenderLayers`、glTF primitive取得は実装時にローカル一次資料／docsrs-mcpで再確認する。

#### 表示更新の順序とreset

新規設備の登録元はrootの`plugins/visual.rs`に限定し、次を一つの順序として固定する。

1. `Update`で既存domain・建設bounce・UI操作を終える。pausedでも既存表示は更新可能にするが、domainのpause条件は変えない。
2. `PostUpdate`でowner/状態のsnapshotと全role readinessを解決し、kind単位のactive descriptorをpublishする。
3. root/part、2D本体、各world preview、catalogへ同じrevisionを適用する。新規entityにも初回適用する。
4. `ApplyDeferred`を通し、`TransformSystems::Propagate`・visibility計算・render extractより前に新childを反映する。
   catalogの画像適用もUIのcontent/layout処理より前へ登録する。Door用順序へcycleを作らないschedule testを追加する。

2〜4の間でgameplayの正本を書き換えない。検証sidecarは伝播・visibility計算の後に採り、全consumerの
`applied_revision`とgeometryを確認する。revisionを進めただけで実体が旧世代のframeは不合格。
owner→root/part対応はindexを使い、毎frame全owner×全visualの二重走査を追加しない。
resetではowner indexとanimation stateを消去し、現行world replacement collectorが全rootを回収する。
永続asset poolは再利用し、旧worldのEntityをpoolへ保持しない。

### 4.4 previewと状態の全経路

画像、描画canvas、anchor、代表状態、active generationを共通の表示記述から取得する。
論理占有は既存shapeを参照し、余白を含むPNGの外形から配置可否を計算しない。

更新対象は、カタログの既存`ImageNode`、配置ghost、Tankの確定済み相方ghost、通常Blueprint、
施工pulse child、Tank/Mixer移動preview、移動確定後の`MovePlantTask`行先シルエット、
SoulSpa専用配置・建設表示、load後のshellである。
世代変更・asset後着はこれら全consumerへ届くrevisionとして公開する。
3Dの同一原本から59°・yaw 0・RtT縦補正を合わせてpreviewを生成する。2Dは同じ原図を用途別canvasへ配置する。
建設中の色、資材カウンタ、progress bar、配置不能理由は既存表示を維持し、装飾へ埋め込まない。

状態の読み取りはpause中にも行う。アニメーション時間だけVirtual Timeへ従い、load時はdurable stateから
再構成する。asset handle・part entity・回転角をsave schemaへ追加しない。
Spaの3D childを論理`SoulSpaTile`の階層へ混ぜず、ConstructingのcancelとOperationalの解体を別経路で確認する。

#### preview descriptorとconsumer一覧

world descriptorは`image / canvas_px / canvas_wu / anchor_px / representative_state / revision`を持つ。
左上基準pixel座標の接地点`(u,v)`とcanvas`(W,H)`から、SpriteのAnchorを`(u/W-0.5, 0.5-v/H)`へ変換する。
camera・world unit倍率・RtT補正は現行投影から取得してreportへ固定し、独自の疑似isometric式を導入しない。
Doorのcenter-origin補正をground-originの設備へコピーしない。全表示状態のboundsをcanvasに収め、
projection reportでは接地点・外形の基準点をrenderer側と照合する。

| consumer | 既存入口 | 適用・維持する内容 |
| --- | --- | --- |
| 配置ghost / Tank locked ghost / Spa配置 | `systems/visual/placement_ghost.rs` | 建物／companionの用途を識別して解決。配置可否tintは既存判定を維持 |
| Blueprint / pulse | `interface/selection/building_place/placement.rs`、`hw_visual/src/blueprint/` | rootと後着pulseに同じimage・size・anchor。色と施工progressは独立 |
| 移動中ghost | `interface/selection/building_move/preview.rs` | Tank/Mixerとcompanionを区別。元建物のworld表示を変えない |
| 移動確定後の行先 | `interface/selection/building_move/finalization.rs` | `MovePlantTask.building`からkindを解決。待機中も後着revision更新、既存0.35 alphaを維持 |
| カタログ | `hw_ui/src/setup/submenus.rs`、rootの`UiAssets` adapter | image entityにkindを持たせ、開いたカードも更新。既存32×32枠へ正方形catalog roleを表示 |
| Spa施工 / load shell | `interface/selection/soul_spa_place/`、`systems/save/rehydrate/` | 既存phaseと保存対象だけから再構成。通常Blueprintと同一視しない |

同じEntityを再利用するため、descriptor適用はimageだけでなくsize・anchor・rect/flipの既定値を全て書き戻す。
位置・scale・tintは既存ownerへ残す。kind変更、世代変更、mode変更、consumer新規生成を更新条件に含める。
BucketStorage等の非移行consumerへ戻るときも旧size・`Anchor::CENTER`を復元する。
Tankのcompanionを新しいTank本体画像へ置換しない。画像の後着により論理footprintや予約範囲を変えない。

#### 状態・停止・load後の期待値

| 対象 | 表示rule | load直後／pause時の期待 |
| --- | --- | --- |
| Tank | count=0でwater非表示、capacity>0かつcount>=capacityでFull、それ以外の非空はPartial | 保存された在庫から再分類。capacity不在/0で非空は現行同様Partial |
| Mixer | `is_active`時のみrotorへVirtual Timeのdeltaを加算 | load直後はIdle/角度0、再割当後のRefiningで回転。pauseは角度保持、通常停止も現在角を保持 |
| RestArea | 既存`RestAreaOccupants`でDream生成。bodyの材質は不変 | durable `RestingIn`からoccupants復元。既存粒子の残存・cooldownを許し、即時の空表示は要求しない |
| Spa | Constructingはmask0。Operationalの各tileの`TaskWorkers`が非空なら対応bitを立てる | phaseは保存対象、workerはruntime派生。load直後mask0、再割当後に点灯。`active_slots`をmaskにしない |
| Lamp | `PoweredVisualState`に対応するoff/on image | paused load直後は初期off。resume後、既存energy再計算結果を表示。表示のためにpause中へ再計算を移さない |
| 移動行先 | 生存する`MovePlantTask`にdescriptorを適用 | taskはdurable対象外。loadで旧行先が残らないことを確認し、移動予約の永続化は追加しない |

Spaのbit順は`building_shape(SoulSpa).ordered_relative_tiles`の`(0,0),(1,0),(0,-1),(1,-1)`とし、
`parent_site`とtileの`grid_pos`から特定する。Query順／Children順／Entity番号をbit位置にしない。
maskは全16通りをtestし、各slotのlocal位置も同じshapeの中心offset `(0.5,-0.5)`から導出する。
これらは`save/schema.rs`のdurable allow-listとruntime除外に従う期待値であり、保存前の見た目の完全再現ではない。

### 4.5 crate ownershipと候補ファイル

| 所有先 | 作業 |
| --- | --- |
| `bevy_app/src/assets/` | 新共通assetset loader、authority、readiness、root所有の画像解決。既存Wall/Doorと必要な検証部だけ共有 |
| `bevy_app/src/plugins/startup/`、`plugins/visual.rs` | pool注入、初期化、state publication→presentation→Transform伝播の登録順 |
| `hw_core/src/visual_mirror/` | root/leaf間で必要な表示専用状態。domain型やasset authorityを流入させない |
| `hw_jobs` / `hw_energy`とroot adapter | 既存正本からTank/Spa等の表示値を投影。Mixer mirror・Rest粒子は再利用し、生産・回復・発電ruleを新visual側へ複製しない |
| `hw_visual/src/` | 建物part、アニメーション、材質・表示同期、preview用handle契約。rootへの逆依存なし |
| `bevy_app/src/systems/jobs/building_completion/spawn.rs`、`systems/visual/building3d_cleanup.rs` | factory、既存fallback移行、owner変換とpart consumerの分離 |
| `systems/logistics/initial_spawn/facilities.rs`、`interface/selection/`、`systems/save/rehydrate/` | 通常以外の生成、移動、専用Spa、Blueprint shell、resetへの接続 |
| `systems/jobs/deconstruction/`、`systems/jobs/soul_spa_construction/` | 既存owner lifecycleから全partを解放、建設段階の投影 |
| `bevy_app/src/assets.rs`、`hw_ui/src/setup/submenus.rs`とUI adapter | カタログのready後／世代変更後の画像更新。ゲームECSの読み取りはroot adapter |
| `tools/blender_ai_workflow/`、asset同期tooling | role別export、群別geometry fixture、preview report、manifestの封印・検査、新schemaのpromotion/install/rollback dispatch |
| native Skillのhelper、`scripts/perf_tool/`、profiling fixture | 新設備用のcurrent-source recipe・独立verifier。旧凍結profileは変更しない |

候補ファイルは変更責務の入口であり、既存の大きなrootファイルへ全処理を追記する指示ではない。
新module名・共有型はM1で責務に沿って決める。新crateは作らない。

## 5. マイルストーン

先行順は **M0 → M1 → M2 → M3 → M4-Door → M5 → M6（9種）**。M4-Bridgeは別件解決後に再合流する。
M2の2設備で制作・動作・preview・releaseまで一巡させ、その確定した方法を後続へ適用する。
各群を受入後に独立導入できるようにし、全10種の制作が終わるまで先行群を未releaseに留めない。

### 着手単位・依存関係

| 単位 | 前提 | その単位で閉じる成果物・判断 |
| --- | --- | --- |
| M0-a 現行契約 | 本計画 | 全10種のshape・状態・consumer一覧、対象外画像hash。Tank寸法の文書訂正 |
| M0-b 先行仕様 | M0-a | Tank/Mixerのrole契約・無地案、共通schemaのfield定義、比較fixtureと予算項目・決定根拠 |
| M1-0 基準取得 | M0-b | Bridgeを除く9種の共通profiling fixtureだけを先行導入・検証。baseline sourceを記録し、基盤予算を候補実装前にfreeze |
| M1-a 読み込み | M1-0 | schema validatorとloader、authority、active/pendingのunit test。新assetはまだ通常起動へ公開しない |
| M1-b 表示接続 | M1-a | root/part factory・共通descriptor・全consumer・reset。従来fallbackの実画面同等性 |
| M1-c 導入経路 | M1-b | export/projection/promotion/rollback dispatch、新設備recipe、基盤だけの前後比較。正式asset承認はまだしない |
| M2 Tank/Mixer | M1-c | 2種のゲーム内無地判定→数値fixture固定→描線→全状態→候補受入→許可後release |
| M3 / M4-Door / M5 | M2で制作経路確定 | 各群で同じ順を反復。橋の未解決をDoor・小物の着手条件にしない |
| M4-Bridge（保留） | 別件の地形／配置問題解決、Bridge専用基準 | 橋の制作・lifecycle・性能受入。9種の結果を流用して完了にしない |
| M6 先行混在close | 9種のreleaseとDoor差分処置 | 9種の混在回帰、累積資源・性能、恒久仕様同期。橋の未完・再合流条件を明記 |

各実装単位はcode・test・必要なdocsをそろえて区切る。コードの導入と未承認assetの通常版公開を同じ操作にしない。
M1では全9種の美術制作や専用systemを先作りせず、kind別role表を共通基盤へ接続するところまでに限定する。

### M0: 寸法・状態・制作境界を確定

- 変更内容: 全10種の既存footprint・anchor・作業位置・許可された向き・状態を固定。Tank/Mixerの構造ラフと共通schemaを用意する。美術由来の高さ・画像・mesh数値は各群のゲーム内無地判定後に固定する。
- 変更ファイル: 本計画、`docs/building.md`、`docs/art-style-criteria.md`、`tools/blender_ai_workflow/fixtures/`（新設備契約）。原本は外部staging。
- 完了条件:
  - [x] 表の全10種と`BuildingType::ALL`が一致し、Tank寸法・旧アセット計画の新規制作範囲を文書同期。
  - [ ] 5種の接地方式・part構成、9種のexact role、descriptor fieldと検査方式が確定。未制作群の美術数値は未確定と明記。
  - [ ] 基盤比較のfixture・状態・環境・予算項目と決定根拠が確定。数値はM1-0の基準取得後、候補実装前にfreeze。後続群は各群の候補比較前にfreezeする。
  - [x] Door g7の継承範囲と、既存計画所有の残件を記録。
- 検証: schema/geometryのfocused検査、docs検査。無地の原本previewを本番表示の合格証拠にはしない。

### M1: 共通の読み込み・part・preview経路

- 変更内容: §4.2〜4.5の最小共通実装と新設備用feedback/受入recipeを用意し、Tank/Mixerの技術候補を通す。
- 変更ファイル: §4.5のasset・visual・factory・preview・save入口、authoring/tooling、native helperとprofiling fixture。
- 完了条件:
  - [ ] M1-0→a→b→cの順で閉じ、§4のschema/role契約と退役pool管理、既存Wall/Doorのvalidator回帰が通る。
  - [ ] fallbackが従来と同じ形・位置・状態を保ち、5種のroot/child管理と通常生成・load・cleanupが通る。完成bounceの開始・中間・終了とmode切替中も旧pivotを照合する。
  - [ ] 同一kindのmesh/texture/previewをatomicに切り替え、cold失敗／pending失敗／active失効を区別して検証。
  - [ ] root countとpart count、pool上限、pause中の後着・load、既存カタログ更新を確認。
  - [ ] `building-art`用recipe（新規）のfeedback、art-preview、正式candidateを区別できる。既存Door helperへ設備を偽装しない。
  - [ ] M1前のコードとのfallback比較で基盤の追加コストを確認し、同一binaryのasset A/Bだけで代替しない。
- 検証: loader/state/lifecycle/previewのfocused test、§7共通gate、fallback同等性のactual-window確認。

### M2: Tank・MudMixerの制作と先行導入

- 変更内容: 無地形状→素材の色面→面別UV→線と筆跡の順で2種を制作し、3水量・回転状態と全previewを接続。
- 変更ファイル: 原本・2種assetset、asset catalog、building preview・move経路、Tank/Mixer visual consumer、関連仕様。
- 完了条件:
  - [ ] §4.1の群別納品物と数値fixtureをそろえ、albedoの前に無地の開口・水面高さ・rotor可動域を採否判断。
  - [ ] 通常表示・最遠表示で2種を識別でき、色と模様が形を埋めない。
  - [ ] 空／途中／満杯、精製開始／停止、pause／再開、水・泥itemと可動部の重なりが正しい。
  - [ ] Tank companion、移動成功／拒否／取消、移動確定後の行先、建設・load・解体で足元とownerが一致。再利用ghostのanchor持越しなし。
  - [ ] §7の当該行と当該変更の性能budgetを満たし、アート判断・candidate受入・2種releaseを記録。
- 検証: 状態・移動・資源正本のfocused test、2種gallery・lifecycle、対象性能比較、通常authorityで表示確認。

### M3: RestArea・SoulSpaの制作と導入

- 変更内容: 休憩所の屋根・入口と既存粒子、Spaの建設段階・4区画を制作。Spaの実稼働maskを投影し、Restの粒子正本は既存のまま使う。
- 変更ファイル: 原本・2種assetset、`hw_core/src/visual_mirror/`、rest/energy adapter、Spa専用配置・施工・cancel・save経路、関連仕様。
- 完了条件:
  - [ ] 休憩者の既存非表示・復帰とDream発生位置を保持し、予約数を利用中表示へ含めない。
  - [ ] SpaのConstructing→Operational、4bit全組合せ、空のTaskWorkersは消灯、代表0/1/4区画の実画面と停止中loadでのmask0を確認。
  - [ ] Spaの歩行可能な4tile、骨の搬入、発電出力・配電は既存正本と一致。
  - [ ] M2と並べて画風を確認し、当該受入・Help判断・2種releaseを完了。
- 検証: phase/maskとcancel/deconstructのfocused test、利用開始／終了storyboard、対象性能比較、通常authority確認。

### M4: Bridge制作・Door適合監査

- 変更内容: 2×5固定橋のモデル・texture・previewを制作。Doorはg7を制作仕様へ照合し、旧カタログ画像をproductionへ接続。
- 変更ファイル: Bridge原本・assetset、factory/transform/preview入口、DoorのUI画像解決、関連仕様。不適合がある場合だけdoorset revision。
- 実施順: Door監査を先行。Bridgeの以下2項目は別件解決後まで保留し、Doorの完了条件と分離する。
- 完了条件:
  - [ ] 着色前にBridgeの両岸接続、橋端／中央でのSoul通過、隣接橋を確認し高さを固定。完成bounceも確認し、actor height変更を前提にしない。
  - [ ] Bridgeの通常建設／Instant Build／施工cancel／load／解体後の川の通行復元が既存ruleと一致。
  - [ ] Doorの両軸×3状態と現在の接続・固定枠が新仕様へ適合。適合した既存assetは再制作しない。
  - [ ] Doorの完成・ghost・Blueprint・pulse・カタログを同じ世代に統一。改修scopeに応じた再受入を実施。
- 検証: Bridgeのgeometryとlifecycle、Doorのpreview後着・UI更新。Doorのmeshを変えなければ全density/Memoryを再実行せず、変更経路に絞る。

### M5: 小型Temporary 4種の画像更新

- 変更内容: Parking / SandPile / BonePile / OutdoorLampの専用world画像とpreviewを制作。現在のForeground2d分類を維持。
- 変更ファイル: 外部原図、4種assetset、`asset_catalog.rs`、2D factory/preview/catalog、powered visual consumer。
- 完了条件:
  - [ ] 4種を通常／最遠表示で識別でき、線・色面が導入済み3D建物と調和する。
  - [ ] 砂・骨item iconと猫車本体の画像が意図せず変更されない。Parking画像に猫車を描き込まない。
  - [ ] Lamp専用画像で点灯／消灯が読め、配電変更・供給喪失と同じ表示更新で同期。paused loadはoff→resume時再計算。旧暗色tintとの二重適用なし。
  - [ ] world/ghost/Blueprint/pulse/catalogを同じ原本・世代へ統一。world系の画像・anchorは一致し、catalogは専用contain規約。4種の当該受入・releaseを記録。
- 検証: 画像役割の分離・power状態のfocused test、密集時・壁/Soul付近・明暗場所のactual-window確認。2Dの常時前景を3D depth対応済みと扱わない。

### M6: 全10種の共存確認・文書同期・close

まずBridgeを除く9種で混在・累積比較を閉じる。下記全10種条件のうちBridge分は未完として再合流時へ残し、
9種の先行導入を妨げない。橋の再合流では通常配置成立、専用baseline、橋と先行9種の混在回帰を追加する。

- 変更内容: 通常authorityの同一現場に全10種と現行Wall/Floor/Soulを置き、画風・識別・preview・lifecycleを確認。
- 変更ファイル: `docs/building.md`、`docs/art-style-criteria.md`、`docs/assets_workflow.md`、`docs/blender-setup.md`、`docs/rendering-performance.md`、必要なrest/energy/save仕様・crate README・Help、本計画と索引。
- 完了条件:
  - [ ] 全10種が「新asset導入」または「既存asset適合確認＋必要経路修正」のいずれかで閉じている。
  - [ ] 個別受入から変更がない項目は結果を再利用し、最終混在で新たに生じる問題だけを追加検証。
  - [ ] 保存・再開・失敗復旧、asset不正時fallback、有限pool、対象性能budgetの結果がそろう。
  - [ ] Help impact reviewと§7のfull gate、storage checkを通し、恒久仕様へ移管。本計画をarchiveまたは削除して索引更新。
- 検証: 全10種混在の通常authority storyboard、未検証範囲の明記、§7共通gateとclose確認。

## 6. リスクと対策

| リスク | 影響 | 対策 |
| --- | --- | --- |
| テクスチャで細部を盛りすぎる | 縮小時に黒く潰れる | 無地→色面→描線を段階確認し、最遠比較で情報量を減らす |
| 複数partと旧root前提が衝突 | 二重表示、絶対座標の上書き、不可視 | root/leaf markerを分離し、render layer・state consumer・診断も同時変更 |
| 接地原点と旧Cuboid中心が混ざる | 浮き・沈み・previewずれ | production接地原点とfallback中心原点をmode別に同一factoryで管理。bounce中も照合 |
| 後着assetやUIのhandle clone | 完成とカタログが別世代 | kind単位のactive revisionと全consumer同期、開いたままのUIも検査 |
| 再利用ghostのanchorが残る | 別種へ切替時だけ浮く・ずれる | 全descriptorを復元し、Door→設備→companionを同じEntityで検査 |
| runtime状態を保存済みと誤認 | load直後に幽霊の稼働・点灯 | durableからの復元とresume後の再計算を分離。schemaは拡張しない |
| 同pathのasset上書き | pending失敗時にactiveを維持できない | generation/hash別の不変path。最後のlocatorのみ切替 |
| 資源画像の共有を上書き | 本体以外の表示も変化 | 建物専用roleを新設し、非対象画像のhashを比較 |
| Spaのphaseとworker対応を誤る | 建設中発光・稼働数誤表示 | durable parent＋実workerからmirror構築、ConstructingとOperationalを別検査 |
| 部品・材質数がinstanceごとに増える | 描画負荷・メモリ増加 | 有限pool、固定part上限、N/4Nと世代反復で検証 |
| Doorの既存仕事を再開する | 重複制作・長い受入 | g7を監査し変更面だけ再受入。旧trackのcloseは元計画へ残す |

## 7. 検証計画

### 共通gateとHelp

Rust変更の各batchではrust-analyzer診断、focused test、次を実行する。

```bash
python3 scripts/dev.py check
python3 scripts/dev.py cargo -- clippy --workspace --all-targets -- -D warnings
```

広い実装batchの完了・通常導入前と計画完了時は`python3 scripts/dev.py verify`を必須とする。
全workspace testは同gateの実行結果で確認し、理由なく同じtestを重複実行しない。
`git diff --check`、docs index/link検査、asset生成・loader/helperのfocused testも変更範囲に合わせる。

各production batchは`hell-workers-review-help-impact` Skillで実入力から表示までをレビューする。
水位・稼働・点灯・建設中の見分け方を説明する必要がある場合はprovider・coverage・exact snapshotを更新する。
美術差替えだけで説明が不変の場合も、到達経路と理由をno-impact判断へ残す。計画文書だけの変更はruntimeへ反映されない。

### 固定条件と必須ケース

新規`building-art` profileをM1で用意する。既存P02の凍結fixtureやWall/Door用asset inventoryへ代入しない。
現行production factoryを通る対象に対して、source・asset・binary・harnessのidentity、owner、part role、
実状態、投影位置、画像hashを結び付ける。描画前の画像、fallbackを本制作と誤認した画像を成功にしない。

| ID | 必須ケース | 証拠の分担 |
| --- | --- | --- |
| A1 | geometry、UV、法線、部品原点・pivot、画像role、previewの投影・anchor | export後のGLB/PNG検査＋無地・着色比較 |
| A2 | 3D root exactly one、期待leaf数、2Dは3D rootゼロ、同種有限pool | ECS test＋actual-window sidecarと画像 |
| A3 | asset遅延／不正／欠落／世代不一致／復旧／反復切替 | loader・ECS test。cold/pending/activeの失敗別に§4.2の期待結果を確認 |
| A4 | 通常配置／建設／初期配置／Instant Build、cancel、完成bounce | 存在する経路を種別ごとに列挙し、通常操作storyboardとfocused testで分担 |
| A5 | ghost／Blueprint／pulse／move／確定行先／catalog、ready後とUIを開いたままの更新 | 投影比較・UI同期test＋actual-window。再利用Entityのkind変更、新規consumerにも全descriptor適用 |
| A6 | Tank3状態、Mixer開始停止、Rest空/利用中、Spa建設/0/1/4稼働、Lamp点消灯、Door2軸×3状態 | 全状態ruleのtest。実状態と画像を結ぶstoryboard、動作は複数時点を採取 |
| A7 | Tank/Mixer移動成功・拒否・取消、companion、save/load・rollback・pause中load | §4.4のload期待値をtest＋代表actual-windowで確認。最初のpresentationから有効な状態を出すが、runtime task/powerの保存前状態は復元しない |
| A8 | 通常解体・Spa施工cancel、owner消失、load反復 | 親子・pool・正本resourceのtest。残像・孤児leafゼロを確認 |
| A9 | 壁/床/Soul/運搬item、橋の両岸・隣接橋・通過、Door seamと支持変更 | 実画面。geometryから通行・Room・照明ruleを変更しない |
| A10 | 描画quality/DPI・通常/最遠zoom・明暗場所・全10種混在 | 以下の対象限定matrixと最終通常authority storyboard |

#### 実装testへ落とす最小の具体例

| 対応 | 入力・操作 | 合格条件 |
| --- | --- | --- |
| A2/A3 | A表示中にBのpreviewだけ遅延→B失敗→有効Cを投入 | 遅延・失敗中はA、C eligible後の同じ境界で本体・全previewがC。revision混在frameなし |
| A3/A5 | pause中にasset後着、owner transformは未変更、カタログは開いたまま | root原点・part・全画像が更新される。新規に開いたカードにも同じrevision |
| A4 | 通常生成・初期配置・Instant Build・rehydrateをそれぞれ実行し、同じownerへ同期を再実行 | ownerごとrootが1、追加partの二重生成なし。通常Blueprintと専用Spaは別fixture |
| A5 | 同一ghostでDoor→Tank→BucketStorage、Tank↔Mixer、production→fallback | image・size・anchor・rect/flipが各契約へ復元。tint・位置とcompanionの論理範囲は不変 |
| A5/A7 | 移動確定→worker待機中に世代更新→完了または取消 | 確定行先も更新、完了/取消で消失。load後に旧taskシルエット・予約障害物が残らない |
| A6 | Tank countを0/1/capacityへ、capacity=0で非空も入力 | hidden/Partial/Full、最後はPartial。waterのEntity/mesh handle数は一定 |
| A6 | Mixerを非Refining→Refining→pause→resume→Idle | RefiningのVirtual deltaだけ角度が進む。pause/Idleで角度不変。槽は回転しない |
| A6 | Spaのtile生成順を逆転、空TaskWorkers、予約のみ、全16mask、Constructingを入力 | 座標順のbit対応、空・予約のみ・Constructingは消灯。slot Entity数は常に4 |
| A7 | 稼働状態を保存→paused load→resume | Tank在庫とRestingInは復元、Mixer Idle、Spa mask0、Lamp offから既存再計算へ。旧worldのpart・Entity参照ゼロ |
| A8 | Empty↔Full、mask0↔15、A→B→A、load、Spa cancelを各10反復 | 同世代state切替でpart数不変。世代/owner終了後は旧part・アプリ所有強参照が残らず、共有pool数が収束 |
| A9 | Bridge無地で両岸→橋端→中央→対岸を通過 | 現行actor高さのまま成立。見た目を直すための通行・高さrule変更なし |

全fixtureへ対象kind、存在する生成経路、domain状態の作り方、期待role数、期待descriptorを持たせる。
実装されていない経路は理由付きN/Aとし、架空の全種共通建設経路を用意しない。
側車データだけで画風合格とはせず、逆に画像だけでstate・世代・所有権が正しいとも判定しない。

反復中はHigh/DPI 1の対象群を同じdev cacheで確認する。正式アート候補は対象群を同じgalleryへまとめ、
High/Medium/Low × DPI 1.0/1.5/2.0の9 caseで通常・最遠zoomを採る。部品運動・pause・状態遷移は
High/DPI 1のstoryboardで別確認し、静止画をanimationの証拠にしない。
新規profileでは各行の画面位置・十分な描画面積を固定し、最遠で対象が消えたgalleryを合格にしない。
Doorを改変しない場合は、新規gallery内の混在確認と変更したカタログ経路で足りる。既存9 caseを無条件に再開しない。

### 性能と資源の予算

M0で新profileのN/4N分布、停止・稼働割合、画面内面積、warmup/measurement、比較方式を固定する。
先行fixtureはBridgeを除く9種各4棟のN=36、各16棟の4N=144とする。companion・支持壁床は別集計。
`building-art-static-nine-v2`を旧10種の無効な契約から分離し、現行生成地形と通常validatorで成立を確認する。
静止galleryに加え、Mixer稼働・Spa区画・Rest粒子を含む動作caseを設ける。停止中だけの測定を動作時へ一般化しない。
動作caseの4棟blockはTank=Empty/Partial/Full/Full、Mixer=Idle/Idle/Active/Active、
Rest occupants=0/1/capacity/0、Spa mask=0/1/3/15、Lamp=off/off/on/onを初期案とする。
Nと4Nでこのblockを同じ比率で繰り返し、domain状態を成立させる人数・資源・電力をfixtureへ固定する。
test用のmirror値の直書きだけで稼働性能を測定しない。通常の更新で状態を維持できないfixtureは採用しない。

| 比較 | 時点・対照 | 見落とさないコスト |
| --- | --- | --- |
| 基盤比較 | M1-0のfixtureのみを導入したbaseline vs M1後コードのfallback。同一fixtureで比較できるsource identityを記録 | child化、index、descriptor同期、loader追加。新binary内のmode切替だけでは測れない |
| 群別比較 | M2〜M5。同じbinaryで対象群だけlegacy-control vs candidate、他群は同じ世代固定 | 新mesh・texture・part・animationと状態表示 |
| 累積比較 | M6。M1-0の基準結果 vs 全群導入結果。環境・fixtureが異なる場合は直接比較不可として同条件を用意 | 個別には小さい差の累積。基盤＋全assetの総増分 |

legacy-controlはprofiling専用の明示modeとし、asset欠落を故意に起こして作らない。各測定は別processで開始し、
非対象candidateのGLB/画像をcontrol側で先読みしない。両pool常駐でMemory差が打ち消される比較は無効。
候補側に保持する既存fallback等の必要資源は実運用どおり含める。測定開始時に実resident一覧で差を確認する。

- 差替え前と候補のseed・論理状態・kind数・画面条件・adapter/backendをそろえ、binary/profile/featureとasset identityも記録する。
- frame p95/p99、native peak live bytes、RSS、mesh/material/texture数とbytes、root/part数を比較する。GPU bytesの推計と実測、圧縮PNGサイズと展開画像サイズを区別する。
- 各対照は隣接・順序反転を含む3反復、正式な時間比較は30秒warmup＋60秒measureを基本とする。中央値とMADを併記する。
- 基盤budgetはM1-0、群別budgetは当該群のbaseline取得後・候補比較前に固定。`p95_ms/p99_msの増分、native_peak_bytes、RSS、texture/mesh展開bytes上限、許容ばらつき、設定根拠`をfixtureへ数値で記録する。未記入では性能受入を開始しない。
- 基盤baselineと予算は候補を測る前にレビューする。後続群も予算を結果へ合わせて緩めない。ノイズが許容幅を超える場合は判定不能とし環境を再確認する。壁用+5%/+4 MiBの無条件流用もしない。
- 動作caseは稼働数・worker/occupant数・粒子数を併記する。Dreamは既存乱数を含むためseedだけで同一と主張せず、状態/負荷が異なるrunを除外する基準を事前固定する。fixture都合で通常の乱数ruleを変更しない。
- N→4Nで共有asset数が増えないこと、世代切替・load反復後に退役poolのアプリ所有強参照が残らないことを必須とする。entity/instance bufferの必要な増加は別計上する。
- 描画構造の説明が必要な場合だけRenderDocを追加する。部品数をdraw call実測値と呼ばない。

全10種を完成済み（SpaはOperational）で各k棟置いたとき、建物用3D rootは`6k`、productionの描画mesh entityは`12k`
（新設備のchild `11k`＋Door root `k`）が初期構成の期待値。fallbackでは描画mesh entity `6k`。
先行9種ではそれぞれ`5k`、`11k`（新設備child `10k`＋Door root `k`）、fallback `5k`へ読み替える。
基盤比較・先行累積比較は9種同士で固定し、橋を追加した負荷と直接比較しない。
水面非表示やslot消灯でも割当数は変わらない。4種の2D owner、companion、Soul、粒子、壁床は別集計する。
この期待値はroot/part増殖の検査であり、visible drawや総frame負荷の見積もりではない。

### 実機実行と検証データ管理

正本は[保存管理ワークフロー](../../development-infra/validation-storage-workflow.md)と
[native acceptance Skill](../../../.codex/skills/hell-workers-run-native-acceptance/SKILL.md)。
以下は実装後の手順であり、計画作成時にはjob・worktree・binary copyを作らない。

1. 各開始・再開時にprimaryの保存管理規則を読み、対象candidate/cacheの現在の利用者を確認する。
2. primaryの`python3 scripts/dev.py validation`でretain/plan/executeを管理し、新設備helperが返す直接kitty launcherを使う。
3. 修正中は既存feedback dev buildを再利用。正式subjectをfreezeした後、Capture→Memoryを逐次実行し、sourceやassetを途中編集しない。
4. 実adapter/backend/window、owned client画像、nonce/ACK、state sidecarを独立verifierで照合。headlessはcorrectnessだけに使う。
5. 成功・失敗・中止をsealし、使い終えたjobを整理、finalize/checkを通す。変更した新subjectには新jobを使う。
6. フィードバック中の同じcandidate workspace/Cargo cacheを保持し、最終acceptance/closureまで撤去しない。固定日数・容量・全job archiveを要求しない。

各実行時に次を本計画または後続の群別記録へ追記する。native検証のexact保持pathはprimary台帳にも登録する。

| 項目 | 計画作成時の状態 |
| --- | --- |
| batch / owner / consumers | 未作成。実行時に群名と判断対象を登録 |
| subject / asset view / job root / workspace | 未作成。通常primary以外の作業場なし |
| 開始bytes / 結果 / 最終成果物の正本 | 新規検証出力0。原本は既定外部asset root、採用先はcanonicalとruntime mirror |
| 削除path / 前後bytes / filesystem差 | 削除なし。実行batch終了時に実測記録 |
| 残存path / owner / consumer / next action / release_when | 本計画専用の保持なし。他sessionの既存hold/cacheは変更しない |
| review状態 / 最新提示日時 | 未開始。実装後の提示・修正ごとに更新 |

## 8. ロールバック方針

- kindごとのasset世代を戻せる単位にする。mesh・texture・preview・local transform記述は同じ世代へ戻す。
- 初回release前は既存Cuboid／PNGをfallbackとして保持。後続releaseは直前の承認済み世代とlocator前像を復旧先にする。
- 失敗した候補を消して復旧させず、authority/locatorを正しく戻してactive poolを再解決する。
- 部品・previewが復旧世代へそろい、論理ownerと保存データが維持されることを確認する。
- asset更新で救済できないコード回帰は該当実装batchの差分を確認して修正する。他sessionの変更を破棄しない。

## 9. AI引継ぎメモ（最重要）

### 現在地

- 実装進捗: M0の制作・検査toolingに続き、M1-0の静止fixture・原本検証helperを実装。M0全体、M1〜M6は未完。
- 作業branch: `codex/building-art-migration`、実装基点: `53c8b6fb8f2d17e8cca4458d098060d2d9fb1e42`。
- 全10種・Rust shapeとの照合、9種のrole・consumer・単位契約、非対象4画像hash、Tank寸法訂正、Door g7継承を追加。
- Tank/Mixerのidentity原本、neutral albedo、計4 role GLB、計5状態PNG、projectionとexport後検査を実装。
  `build_building_clay.py build/verify`で既存scene/export/Khronos gateを再利用する。詳細は`docs/blender-setup.md`。
- 次の作業: 9種fixture・camera・Spa初期化の修正はcommit済み。現行描画方式の検証dispatchを修正し、静止実機参照を再取得する。
  通常ゲームの橋／地形ruleは変更しない。稼働fixture・共通runtime schema詳細は残る。
  baseline sourceと基盤budgetをfreezeする前にruntime表示基盤を変えない。
- Rustのprofiling専用経路のみ変更。通常表示・repo assets・canonical・Help本文は未変更。外部原本は未承認stagingのみ。
- 無地の寸法・UV・法線処理・canvasは`clay_draft`。ゲーム内判定、最終atlas、アート承認、releaseではない。

### 次のAIが最初にやること

1. 現在のdirty差分・並行sessionと本計画の作業範囲を分ける。別sessionのCI計画や新しい変更を破棄しない。
2. `building-art-direction.md`と下記参照を読み、M0の論理寸法・状態・role表を固定する。全群の美術数値を先行2種のラフだけで確定しない。
3. asset原本と候補の制作は既定stagingで開始する。モデル／画像制作時は該当Skillを使い、アートを最終判断する前に具体的なゲーム内比較を用意する。

### ブロッカー/注意点

- 正式baselineは新しい計測fixtureを含むclean subjectが必要。ユーザーは検証後のcommitと計測続行を許可済み。
  `c3733fc1`と`9a46751a`で実行したが、後述の配置失敗で正式値は0。commitをM1-0の完了や性能baselineと扱わない。
- 現行mapgenは幅2〜4の川、Bridge validatorは2×5全セルが川であることを要求する。
  fixtureの旧固定川定数に基づく配置がnativeで拒否された。ユーザーは計測専用地形案を却下し、橋以外を先行するよう指示済み。
  橋の修正は別件であり、9種の制作・表示接続・受入を停止する理由にしない。
- 静止参照のnative helperを追加したが、全設備の美術・稼働受入recipeとruntime asset schemaは未実装。
  既存Wall/Door recipeや静止計測を設備の受入済みへ読み替えない。
- 新root形式はroot数だけの既存監査では不十分。visible part、材質、layer、asset readinessまで調べる。
- SpaのConstructing、Tank companion、カタログ後着、砂icon共有が取りこぼしやすい。
- `MovePlantTask`の確定行先を配置ghostと混同しない。loadではtaskが復元されず、Mixer/Spaの稼働やLampの供給結果も保存前とは限らない。
- 技術候補・ArtPreviewの採否とrelease許可は別の判断。承認待ちでも完成済みと記録しない。
- Doorは既存releaseを使う。旧計画の「未導入」などの古い文言だけから再制作を始めない。
- code編集は主担当だけが行う。agentはread-only探索・レビューに限る。

### 参照必須ファイル

- `docs/building-art-direction.md`、`docs/building.md`、`docs/art-style-criteria.md`、`docs/assets_workflow.md`、`docs/blender-setup.md`
- `docs/crate-boundaries.md`、`docs/invariants.md`、`docs/save_load.md`、`docs/soul_energy.md`、`docs/rest_area_system.md`
- `crates/hw_jobs/src/placement_geometry.rs`、`crates/hw_jobs/src/model.rs`
- `crates/bevy_app/src/systems/jobs/building_completion/spawn.rs`、`crates/bevy_app/src/systems/visual/building3d_cleanup.rs`、`crates/bevy_app/src/systems/visual/door_preview.rs`
- `crates/bevy_app/src/plugins/startup/visual_handles.rs`、`crates/bevy_app/src/plugins/startup/asset_catalog.rs`、`crates/bevy_app/src/assets.rs`
- `crates/bevy_app/src/assets/door_asset_set.rs`、`crates/hw_visual/src/visual3d.rs`、`crates/hw_ui/src/setup/submenus.rs`
- `docs/development-infra/validation-storage-workflow.md`、repositoryのnative acceptance / Help impact review Skill

### 最終確認ログ

- `2026-09-19`: `python3 scripts/dev.py docs --write`で索引を生成し、両索引を確認。`docs --check`、`git diff --check`、`python3 scripts/dev.py validation check`は成功。今回の計画用job・build・native出力は作成していない。
- `python3 scripts/check_help_impact.py`は成功。ただし既存commitのHelp更新を検出した結果であり、今後の実装に必要な実際のプレイヤー経路に基づくHelp判断の代替にはしない。
- read-onlyレビューを反映し、既存fallbackの中心原点とproductionの接地原点を区別。建築bounce中の高さを維持するため、表示mode切替時にroot原点・part構成・previewを同時更新する計画へ修正。
- `dev.py check` / Clippy / workspace test / `dev.py verify`: 未実行（今回は計画文書のみ）。実装完了の証拠なし。
- native / performance / art acceptance: 未実行。既存Door releaseの成果と今回の受入を区別する。

### 自己レビューの修正記録（2026-09-20）

| 指摘 | 修正先・決定 |
| --- | --- |
| 全種の美術寸法をM0で固定すると未制作群の判断を先取りする | §4.1 / §5。論理契約はM0、美術数値は各群の無地判定後・着色前 |
| GLBのファイル単位とtransform単位が未定義 | §4.1。role別8 GLB、identity node、world unitとpivotを分離 |
| 部品に既存Wall/Door用の照明anchorを導入する根拠がない | §4.3。設備の既存samplingを保持 |
| 新候補失敗時まで正常なactiveを消してしまう | §4.2。不変artifact path、active/pending分離、失敗種別別の状態表 |
| 表示のatomic切替が実行順と結ばれていない | §4.3。PostUpdate適用とdeferred/伝播境界、旧system除外 |
| 再利用ghost・確定行先・非正方カタログの経路漏れ | §4.4。全descriptor復元、consumer表、独立catalog role |
| loadでruntime稼働も保存前へ戻るように読める | §4.4 / A7。durableと派生状態を区別し、pause条件を維持 |
| Restの常時利用表示、Bridgeの橋面追従を暗に要求 | §4.1。既存粒子/UIの範囲、actor高さ不変のclay判定 |
| 同一binary A/Bだけでは共通基盤の負荷を測れない | §5 / §7。fixture先行baseline、基盤・群別・累積の3比較 |

このレビューは計画上の矛盾・未定義の解消であり、上記実装の動作証明ではない。
`2026-09-20`に`dev.py docs --write` / `docs --check`、本計画・索引を対象にした`git diff --check`、
`dev.py validation check`、`scripts/check_help_impact.py`を再実行し成功。Help gateは既存commitの判断を確認したもので、
今回の文書修正や将来の実装に対する新しいプレイヤー経路の受入ではない。
本作業の変更は本計画と索引の該当項目。並行CI変更・全workspaceの実装品質は今回の検証対象外で、
build・実機・性能検証用の出力や専用workspaceは作成していない。

### M0制作toolingの実装記録（2026-09-20）

- `building-art-v1.contract.json`、保護画像hash、Tank/Mixerのgeometry draftを追加。
- `check_building_art_contract.py`は全10種のshape定数だけでなく、実際のkind割当・中心・anchorと全preview入口を照合する。
- 部品原点を保存してから、Blender確認だけに状態translationとcameraを適用する。export時の二重scaleを防ぐ。
- Tank開口は実際の閉じた容器mesh。Mixerの360° sweepを幾何testで確認し、槽・架台は回転させない。
- 無地目視でTankの満水時の隙間を検出し、内径を上下一定、水面との半径差を0.05 wuへ修正。
  Partial/Fullとも隙間が0より大きく0.1 wu以内であるtestを追加し、貫通と浮いた円盤状の水面を防ぐ。
- 新しい13 testに、4 role GLB roundtrip、法線/UV/shape、不正node・skin・animation・morph・画像・index、
  hash変更・偽authority・画像差替え・上書き・path escapeの拒否を含める。
- 技術検査はBlender 5.1.1で4 GLBのscene/Khronos error・warning各0、projection誤差0.01px以内。
  GLBのUV未使用infoは外部材質用のplaceholder exportとして確認。neutral素材は完成textureではない。
- Help review: **No impact**。変更はstaging専用制作toolとfixture/docのみ。通常入力→建築factory→既存描画consumer、
  runtime locator・画像・保存・Help providerへ新しい経路は接続しておらず、操作・成立条件・表示意味は不変。
- 実機Capture / native Memory / 性能budget / アート承認は未実施。Blender画像をそれらのpassへ読み替えない。
- 最新制作原本・出力は外部stagingで次のM1/M2の制作consumerが使う。native job、binary copy、専用worktreeは未作成。
- 検証: `dev.py ci check --base 53c8b6fb8f2d17e8cca4458d098060d2d9fb1e42 --mode auto`の
  contracts/toolingがpass（scripts 241件、Blender tooling 164件、perf self-test、文書・Help・storage gate）。
  追加script/testのRuffもpass。Rust変更なしのためRust build/Clippy/nativeは今回の選択対象外。
- 制作物のowner=`building-art-migration`、consumer=`Tank/Mixer M1/M2 clay integration`。
  次の作業はruntime接続後のゲーム内無地比較。採否確定後、採用原本へ引継ぎ、未採用stagingは整理する。
  現在はreview-activeで、制作原本を日数・中間passで消さない。

外部asset root `/home/satotakumi/Sync/hell-workers-assets` 配下の最新制作物（allocated bytes実測）:

| 名前 | `staging/blend/<名前>.blend` | `staging/exports/building-clay/<名前>/` | `staging/reports/<名前>.json` | `staging/reports/building-clay/<名前>/` |
| --- | ---: | ---: | ---: | ---: |
| `building-tank-clay-20260920-v4` | 98,304 | 102,400 | 8,192 | 20,480 |
| `building-mud-mixer-clay-20260920-v4` | 98,304 | 77,824 | 8,192 | 20,480 |

計434,176 bytes。途中試作v1〜v3の同名系列だけを、後継版の独立検査後にごみ箱へ移動した（復元可能）。
canonical・runtime asset・他sessionの原本／検証cacheは変更していない。trash移動のためdisk解放量は主張しない。
本件のゲーム受入jobは0で、既存primary開発cacheの保持者も変更していない。

### M1-0静止fixtureの実装記録（2026-09-20）

- [静止参照仕様](../../building-art-static-reference.md)を追加。各4棟／16棟、実Soul 15／60体、支持壁・companionを固定。
  川の位置は旧定数を使っており、現行mapgenでの合法性は成立しなかった（下記native結果）。
- `building-art-static`はprofiling限定。建設完了・relationship・発電／表示mirrorは既存systemを使い、通常表示基盤は変更しない。
- 原本検査は独立layout、実owner・状態・resident/visible handle、承認Door g7、実adapter/window、Capture/Memory分離を要求する。
- Rust focused 4 test、Python 9 testは成功。全体検証とnative実測は別gateで、両方の結果を得るまで完了扱いしない。
- Help review: **No impact**。明示profiling入力からのみ到達する計測経路。通常の建築操作・前提・描画・保存・Help providerは不変。
- 静止参照は稼働性能の証拠ではない。Mixer回転・Dream粒子・継続生産、基盤budgetは未確定でM1-0を完了扱いしない。
- `c3733fc1`でfixtureをcommit。変更別contracts/tooling/rust gate（通常/profiling workspace test、計測feature check、Clippy）、`dev.py check`は成功。
  profiling専用Clippyも成功。rust-analyzerはstartup入口のerror/warning 0。profiling専用moduleはdefault featureの解析対象外で、compiler/testで検証した。
- 最初のnative batch `building-art-static-20260920-v1`は、停止中に完了ポップアップが残る経路を発見したためCapture build中に中止。
  ゲーム計測は0回、基準値の採用なし。中止jobの8,192 allocated bytesを削除し、cacheは保持、seal/finalize/storage checkを完了。
  準備時だけ完了演出を除き、その後の再出現を拒否する修正を`9a46751a`でcommitした。
- `9a46751a`の検証: `dev.py check`、通常とprofilingのClippy（警告0）、
  `dev.py ci check --base c3733fc10c3337c619d5fcfab7d377173cb91937 --mode auto`のcontracts/tooling/rustが成功。
  通常/profiling workspace test、Memory/Tracy/RenderDoc feature check、scripts 250件、Blender tooling 164件を含む。
  検証source fingerprintは`c64a4d36c8c808cacb70f2e5a2348fc6ce338201223f65d381a7a195c83eb448`。
  変更別gateを使い、`dev.py verify`自体は実行していない。

### 静止nativeの停止結果と保持（2026-09-20）

- batch `building-art-static-20260920-v2`、subject `9a46751af0a92671c22718dcbc7fa6f0700ca550`。
  Capture build成功後、smallの3試行すべてが`Bridge/0`・grid `(8,65)`の`NotRiverTile`で準備完了前に終了した。
  source fingerprint: `f07bfc6bda8c71a7a3f70e8482235f65481e8f3357376143ae3720d3250353c4`、
  asset view: `95a4526096f2857275fc783013bf8cbe7bf38687a3e9bb05f004c195a8abc624`。
- `generate_world_layout`はseed依存の幅2〜4の川を生成し、旧`RIVER_Y_MIN/MAX`はその地形を表さない。
  Bridgeの2×5全セルRiver条件とfixture配置が不整合。単体layout検査だけでは実地形との成立を確認できていなかった。
- X11 window生成・Intel Arc/Vulkan adapterログは診断情報だけ。window/readiness原本を得ておらずrenderer受入ではない。
  frame-time有効測定0、medium CaptureとMemory未実行、baseline・budget未確定。新表示基盤は変更しない。
- 当時の提案だった計測専用の幅5の川はユーザー判断で不採用。橋は別途解決し、9種を先行する。
  通常ゲームの地形／橋／配置ruleは変更しない。旧jobを再開せず9種契約の新subject/jobで計測する。
- v2は`invalid`でseal、使用中processと固有成果物がないことを確認してjobを削除、finalize/storage check成功。
  削除rootはprimaryの`target/native-acceptance/`配下の次の2つ（復元不可、成果・失敗理由は本書へ集約）:

  - `building-art-static-20260920T021601Z-197f72c4`: 8,192→0 allocated bytes（v1中止）。
  - `building-art-static-20260920T022937Z-8b2a0067`: 90,112→0 allocated bytes（v2無効）。

  合計98,304 bytesを撤去。v2削除前後のfilesystem availableは661,915,824,128→661,915,222,016 bytes。
  共有filesystemの差は他の書込み・圧縮等を含むため、job削除による空き容量増加とは主張しない。
- 保持: `/home/satotakumi/projects/hell-workers`の同一checkout/Cargo target。専用worktree・binary copyは作成していない。
  hold=`building-art-static-reference-review`、owner=`building-art-migration`、consumer=`building-art-static-reference-v1`。
  次は9種fixture修正・差分ビルド・再計測。accept/abandonと修正・レビュー完了まで保持する。
  primary台帳の実測値は297,874,731,008 allocated bytes（共有primary全体であり新規専用消費量ではない）。
  通常開発cacheの寿命と他sessionのholdは変更しない。

### 9種先行への切替（2026-09-20）

- `building-art-static-nine-v2` / native profile `building-art-static-nine-reference-v2`へ改訂。
  対象36/144棟、3D root `5k`、foreground `4k`、対象の共有mesh 2種。9種の座標・状態・camera・Soul数は維持。
  Bridgeの混入と旧10種contractを拒否する。通常地形・橋のvalidator・runtime表示・assetは未変更。
- 実`generate_world_layout(20260920)`をWorldMapへ反映し、N/4Nとも通常配置validator・支持物・companion検査が通るtestを追加。
  Rust focused 5件、Python focused 10件が成功。旧10種の失敗を新契約の結果に読み替えない。
- Help review: **No impact**。`profiling` feature＋明示`BuildingArtStatic`入力からだけ到達するsetup/inspectと独立検証の対象変更。
  通常プレイヤーの建設条件・地形・描画・保存・Help本文のconsumerは変更していない。
- rust-analyzerはprofiling専用fileにerror/warning 0、default featureで対象外の`unlinked-file` hintあり。
  profilingの検証は実compiler・focused test・Clippyで行う。変更別全体gateとnativeの結果は完了時に追記する。
- 同じprimary checkout/targetを修正・計測に再利用し、新しい専用worktreeやbinary copyは作らない。
  保持owner/consumer/release_whenは上記review holdを継続し、次の作業を9種基準取得へ更新した。
- `8a2d0649d53a0f804733285f3c277f466efc0316`で9種切替をcommit。`dev.py check`、通常/profiling Clippy警告0、
  変更別contracts/tooling/rustが成功（base `18e24cef3132b16d009ab01133e89795b553f9e5`、
  source `5437f6c837e018ae17d07d6a764bd69facedd3ec174318bcf0c2ab9b914bb79c`）。
  scripts 251件、Blender tooling 164件、通常/profiling workspace testと計測feature checkを含む。
- native batch `building-art-static-nine-20260920-v3`は配置検査を通過したが、smallの3試行とも準備完了前にcamera不一致で失敗。
  `PanCamera.zoom_factor`を初期化せずTransformだけ設定していたため、Bevy 0.19の操作処理が倍率を戻していた。
  正式frame-time/Memoryは0。controllerとTransformの一度限りの初期化、および実PanCameraPluginを通す回帰testを追加。
  修正前の倍率resetを再現し、修正後の3 frame維持と後続変更を修復しないことを確認、Rust focused 6件成功。
  Helpは引き続きNo impact。変更は明示profiling fixtureのカメラ初期化だけで、通常入力・倍率範囲・描画・地形は不変。
  失敗をsealし、不要job `target/native-acceptance/building-art-static-20260920T031838Z-107ba24e`を削除（復元不可、90,112→0 allocated bytes）。
  finalize/storage check成功。filesystem availableは661,738,901,504→662,169,714,688 bytes（同時のbuild等を含む共有差）。
  同じcacheを使って新subjectで再検証する。橋の条件・地形は変更しない。
- camera修正を`a25ee8c577b669d3c225d55f0036f2d6ef9e0b3f`でcommit。
  `dev.py check`、通常/profiling Clippy警告0、変更別contracts/tooling/rustが成功
  （base `8a2d0649d53a0f804733285f3c277f466efc0316`、source `a10d49ac20986dbb9be06ef9dc1be8a83864d1a997f7f9c4c914def352a82640`）。
- native v4はcameraの即時エラーが再発せず、small初回が準備未完了のまま300秒timeout（exit 124）。
  2回目をowned helperへのSIGINTで打ち切り、helper/runner/gameの終了を確認、interruptedとしてseal。
  sidecar・window/summary・完了markerがなく、正式Capture/Memoryは引き続き0。値を採用しない。
- コード経路からSpaの初期化漏れを確認した。factoryのdefault phaseはConstructingで、搬入量のみ完了値へ変更していた。
  通常deliveryは新規Bone消費が0ならphaseを遷移させず、停止fixtureではそのLogic処理自体も走らない。
  完成済み参照として初期phaseも一度だけOperationalへ設定し、通常tile activationを通す回帰testを追加。
  通常ゲームの建設・配送や毎frameの状態修復は変更しない。native再試行前に同修正を検証・commitする。
  修正後のfocused Rust 7件、`dev.py check`、通常/profiling Clippy警告0が成功。
  対象fileのrust-analyzer診断は0件。ただし専用featureの保証はprofiling compiler/testで行う。
  HelpはNo impact（明示profiling fixtureの完成済み初期状態だけ）。修正後のnative再計測は未実施。
- v4の不要job `target/native-acceptance/building-art-static-20260920T034737Z-874abd24`を削除
  （復元不可、49,152→0 allocated bytes）。finalize/storage check成功。
  filesystem availableは661,976,092,672→661,975,867,392 bytes（同時buildを含む共有差、削除量ではない）。
  primary checkout/Cargo cacheのreview holdは継続し、専用worktree・binary copyは作っていない。

### 静止基準の再開（2026-09-20）

- Spa初期化修正は`43d4471398421be3cde8c7803e0d1dec43e2291d`でcommit済み。
  変更別contracts/tooling/rust成功（base `a25ee8c577b669d3c225d55f0036f2d6ef9e0b3f`、
  tested source `21d0b4ce47a385d5c2e08822cb397449f1361d1f6c8a5df6fdd595f7f2137a0e`）。
- native v5は同commit/source `824863e1dfc3bd1ec53fd03a925111c79488f07f32cee96c4ebae141e41abed3`で実行。
  small初回は準備完了→30秒warmup→60秒measureを完走。9種sidecarとIntel Arc (MTL) / Mesa 26.1.8 /
  Vulkan / X11 / 1280×720 / DPI1 / high / immediateのwindow原本を得た。
- ただし共通validatorの旧Soul proxy/mask/shadow各15体という期待に対し、現行ActorBillboard方式では全て0となり不合格。
  既存の明示ActorBillboard検査オプションを使った元runのread-only診断では全検査が通ったため、修正対象をdispatchへ限定。
  `building-art-static`だけを現行方式へ振り分け、旧方式の条件は維持する。旧proxy混入の拒否と旧workloadの期待を回帰test。
  Rust・通常描画・asset・地形・橋は変更しない。Help: No impact（計測データ検証だけ）。
- small 2回目でowned helperへSIGINTを送り、helper/runner/game終了確認後にinterruptedとしてseal。
  正式baselineは未取得、medium/Memory未実行。v5の結果は新subjectの成功へ流用しない。
  診断完了後、不要job `target/native-acceptance/building-art-static-20260920T043139Z-6180a892`を削除（復元不可）。
  204,800→0 allocated bytes、filesystem availableは661,578,760,192→661,579,190,272 bytes（共有差）。
  finalize/storage check成功。primary checkout/Cargo cacheは次の再計測に継続使用する。

### Definition of Done

- [ ] M0〜M6の完了条件と全10種の処置が確定。
- [ ] 恒久仕様・Help判断・asset正本・release/復旧記録を同期。
- [ ] rust-analyzer、`dev.py check`、Clippy警告0、workspace testを含む`dev.py verify`が成功。
- [ ] 必須actual-window・対象性能budgetを満たし、未検証範囲を正確に記録。
- [ ] 全batchの結果確定と不要job / binary copyの整理、storage checkを報告前に完了。
- [ ] 最終closeで本計画consumerは0。残る共有資源は別の具体的consumer・owner・bytes・終了条件へ引継ぎ。
- [ ] 採用成果と最終結果を恒久的な正本へ移し、本計画をarchiveまたは削除、両索引を再生成。

## 10. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| `2026-09-19` | `Codex` | 制作仕様を前提に全10種の移行、9種の新asset経路、Door監査、段階導入・全表示consumer・受入・保存管理を計画。実装未着手 |
| `2026-09-20` | `Codex` | 実装照合の自己レビュー。role別export、群別freeze、active/pending、表示順、preview全経路、load期待値、試験入出力、基盤比較を具体化。code・assetは未変更 |
| `2026-09-20` | `Codex` | M0のshape/role/保護画像契約、先行2種の無地原本生成・GLB/PNG検査・13 testを実装。stagingのみ。runtimeとM1-0以降は未着手 |
| `2026-09-20` | `Codex` | M1-0用の静止fixture、独立sidecar検証、Capture→Memory helperを追加。通常表示とassetは不変、稼働fixture・budget・実測は未完 |
| `2026-09-20` | `Codex` | 検証・commit後のnativeでBridgeと現行川生成の不整合を検出。正式値なし、無効job整理完了、計測専用地形の方針確認待ち |
| `2026-09-20` | `Codex` | ユーザー判断で計測専用川を不採用とし、Bridgeを別件へ分離。残る9種の基準・導入・混在closeを先行、Doorを橋の依存から分離 |
