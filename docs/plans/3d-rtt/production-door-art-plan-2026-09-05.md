# ドアの本番ビジュアル化計画

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `production-door-art-plan-2026-09-05` |
| ステータス | `In Progress` |
| 作成日 | `2026-09-05` |
| 最終更新日 | `2026-09-07` |
| 作成者 | `Codex` |
| 親計画 | [アセット作成マイルストーン](asset-milestones-2026-03-17.md)（Build-BのDoor） |
| 関連計画 | [仮設壁の木製型枠化](provisional-wall-formwork-plan-2026-09-05.md)、[完了済み本番壁計画](archived/production-wall-art-plan-2026-08-31.md) |
| 関連提案 / Issue / PR | `N/A` |

本書は実装中の計画である。M0〜M2の技術候補と実行時接続を実装し、M3の初回ArtPreviewを受けた形状改訂と実機再撮影まで進めている。調査基点は `57a488e1`。
完了済みWall計画から引き継ぐ範囲は、Doorの枠・戸当たり・向き・Wallの9.6 wu portとの継ぎ目である。

## 0. セルフレビューで修正した点

| 指摘 | 具体化した判断 |
| --- | --- |
| 片開きの寸法・枠との干渉・Soulの通路が未定義 | §4.2で両開きへ改め、枠とleafの寸法・蝶番・Open AABBを設定。Soulは別の実画面gateで判定 |
| PNG previewをどう本番アートに合わせるか未決定 | ClosedのEW/NS画像2枚を同じ原本から作り、ghost・blueprint・pulse child・loadへ接続。coreは6 file |
| Door同期がUpdate、共有topologyがPostUpdateで、単にafterを加えても整合しない | §4.4でfinal applyとreaderをPostUpdateへ移す対象を明記 |
| MeshTagの方向を壁線と取り違える恐れ | EWは面法線North=0、NSはWest=3と固定。Open角度を使わない |
| 未承認候補の確認・baseline・joint受入の順序が曖昧 | 型枠計画§4.4の共通C0・封印前の統合freeze・preview・単独M3・J1へ統一。asset viewを専用worktreeへ固定 |
| 「loadと同frame」を要求するとLastでのworld置換と矛盾 | world置換後の最初のpresentation frameから再構成し、pause中も進める |
| 初回ArtPreviewで上枠が太く、Openが板厚だけに見える | 上枠を高さ1.6 wu・奥行7.2 wuへ縮小し、leafと付属金具を68°へ揃えて木製面と中央開口を同時に見せる |
| generation 2でもNS配置のOpenが片側の張り出しに留まる | 両開きの同方向・同角度・左右対称を維持し、共通開度を78°へ深くした上で扉面の骨補強線を投影方向の手掛かりにする |
| generation 3の左右を前後反対側へ振る案は両開きとして不自然 | generation 3は不採用。左右を同じ側へ同じ角度で開く物理契約を明記し、開度差も禁止する |

寸法・予算は制作と検証に使う新契約であり、アート・実機合格を先取りするものではない。

## 1. 目的

- 解決したい課題: ドアの簡易直方体を木と骨の本番アートへ置き換え、壁との接続と開閉・施錠状態を見た目で伝える。
- 到達したい状態: 石壁や木製型枠の間に自然な出入口があり、Openでは通路が見え、Lockedでは施錠が形で分かる。
- 成功指標: 両軸の配置、3状態、通過・自動閉扉・施錠操作、save/load、隣接壁変更後にも一貫した表示が成立する。

推奨意匠は**黒ずんだ厚板の両開き扉＋骨の補強・取っ手／閂＋少量の錆鉄金具**。
現行レシピ `Wood × 1 + Bone × 1` が読み取れる構成にし、本設壁の黒石・錆鉄と調和させる。
骨は小さすぎる飾りにせず、上面からも読める補強や施錠部へ使う。金具は意匠であり材料要求を増やさない。

## 2. スコープ

### 対象（In Scope）

- 固定枠と扉を含む3状態のGLB、共有texture/material、loader・fallback・release経路。
- 隣接Wall / Doorからの表示軸解決、Wall portとのseam、開いた扉の占める範囲。
- 既存の通常建築、Instant Build、選択、Room、照明、save/load、撤去に対する表示整合。
- 設計図・配置previewの方向整合、専用実機scenario、性能・Help・文書更新。

### 非対象（Out of Scope）

- 新しいドア種別、材料・時間・通行コスト・開閉タイマー・施錠rule・配置許可条件の変更。
- プレイヤーの回転／開き勝手操作、保存schemaへの向き追加、スムーズな開閉補間・開閉音。
- Door移設操作の新設（現在の建物移設対象はTank / MudMixer）。
- 論理collisionをmeshの形へ変更すること、遮光やRoom成立をアニメーションで遅らせること。
- 他の建物のGLB化、共有rendererの再設計、PointLightや追加outline pass。

## 3. 現状とギャップ

| 領域 | 調査で確認した現状 | 本計画の変更 |
| --- | --- | --- |
| active表示 | `Structural3d`のCuboid leaf。幅0.82、高さ0.5、厚さ0.18 tile。枠なし | production Wallとの寸法を揃えた固定枠＋木・骨の扉 |
| 状態表示 | Closed / Open / Lockedを茶・緑・赤のshared materialとtransformで区別 | 開口・扉姿勢・閂の形で区別し、主材色を維持 |
| 開き方 | Open時にworld X/Zへ各0.32 tile移動しY軸90°回転するeast hinge fallback | leafだけをローカル蝶番で動かした状態meshを制作。枠は全状態で固定 |
| 向き | 対向するWall/Doorの存在は配置条件に使うが、DoorにWallTopologyStateは付かない | 表示専用の隣接軸resolverを追加 |
| 建築完了 | 新しいTransformをtranslationから生成するためblueprint回転を維持しない | owner回転の保存を前提にせず、同じ隣接情報から表示方向を導く |
| 旧画像 | DoorのPNGは配置・設計図などの参照。active Door本体は3D | PNG差替えだけで本番化を完了扱いしない |

根拠はroot `plugins/startup/visual_handles.rs`、`systems/visual/building3d_cleanup.rs`、
`systems/jobs/building_completion/spawn.rs`、`hw_visual/src/wall_connection.rs`、
`hw_world/src/door_systems.rs`、`hw_jobs/src/model.rs`と`docs/building.md`。

## 4. 実装方針

### 4.1 状態別の本番アート

| 状態 | 扉・枠・施錠部 | 視覚上の合格条件 |
| --- | --- | --- |
| Closed | 固定枠に左右2枚の厚板leafが収まり、中央の閂は外れている | 壁と違う出入口と分かり、開いた通路には見えない |
| Open | 枠はClosedと同位置、左右leafと付属金具を各蝶番から同じ側へ同じ78°だけ対称に開く | EWでは中央の地面と両扉面、NSでは枠から張り出す扉面と中央開口が見え、枠が一緒に回らない |
| Locked | Closedと同じleaf姿勢、中央を結ぶ骨の閂が掛かる | 主材を全面赤色にしなくてもClosedと区別できる |

アートはRough Vector Sketch。粗い板目・歪んだ輪郭・暗い太線・骨色の塗り分けを使い、
金属の写実的な光沢は抑える。まずlit `TopDownStructuralMaterial`＋baked linework、共有albedo 1枚、
Opaque、emissiveなし・normalなしで制作する。textureサイズは512×512 RGB（alphaを持つ場合も全画素Opaque）とし、
遠景の施錠表示を微細な絵柄だけに依存させない。
左右leafは上下2本ずつの横向き骨補強を持ち、Closed / Open / Lockedで同じ部材を維持する。
Openでは補強もleafと同じ蝶番から回し、NS投影で扉面の向きを読む形状手掛かりにする。

### 4.2 固定枠と単一visualの両立

初期実装は `door_closed.glb` / `door_open.glb` / `door_locked.glb` の**3共有mesh**。
各GLBに枠と状態別の左右leafを合わせてbakeし、単一node・mesh・primitive・materialにする。
全状態の固定枠は同じ頂点位置で、Openだけ左右leafのローカル蝶番回転をbakeする。

これによりlogical Doorあたりexactly-one `Building3dVisual` / `Mesh3d`、子Sprite 0を保つ。
現在の瞬時の状態切替を維持し、runtimeの状態遷移ではmeshを交換する。whole-visualをOpen用に
平行移動・回転する旧処理はproductionへ適用しない。将来の連続開閉アニメーションは可動leaf分離を
伴う別改修とし、この計画ではSceneRoot・AnimationGraph・複数owner-linked meshを導入しない。

枠高は既存Wallと同じ32 wu、local Y `[-16,16]`・world接地0に揃える。
旧Door用の中心高8 wuをそのまま新meshへ適用しない。root transformはanchor・解決済み軸・ownerの
scale・bounceを合成し、状態によって枠の位置・高さを動かさない。近傍から決めた軸はworld軸である。
owner.rotationを使う場合はそのworld軸をowner-localへ変換してから合成し、最終world軸を方向表へ
一致させる。world軸へowner.rotationをそのまま足す二重回転をしない。

制作は以下の初期寸法から始める。値はexport後のGLB local座標、Xが左右・Yが高さ・Zが前後。
Blenderの1 tile原本を32倍bakeする規約を使い、変換誤差の許容は0.01 wu。

| 部位 | X / Y / Zの範囲・回転 |
| --- | --- |
| 左右jamb | X`[-16,-13.6]` / `[13.6,16]`、Y`[-16,16]`、Z`[-4.8,4.8]` |
| 上枠 | X`[-13.6,13.6]`、Y`[14.4,16]`、Z`[-3.6,3.6]`。高さ1.6 wu・奥行7.2 wuでjambより細くし、閉じた下枠は作らない |
| 閉じた左右leaf | X`[-13.2,-0.2]` / `[0.2,13.2]`、Y`[-15.2,11.2]`、Z`[-5.6,-3.2]` |
| 蝶番 | 左X=-13.2 / 右X=13.2、Z=-3.2の垂直軸。Openは左local -Y78°・右local +Y78°。leafの左右向きを含めると、同じ前後側へ同じ開度で対称に開く |
| Open leaf AABB | 左X`[-13.2,-8.149594]`、右X`[8.149594,13.2]`、両leaf Z`[-3.698988,9.515919]`。Yは閉状態と同じ |
| 接続・隙間 | Wall portはcell境界X=±16・Z±4.8。leaf/jamb間0.4、左右leaf間0.4、leaf上端/上枠間1.6、接地余裕0.8 wu |
| 施錠の形 | 中央の骨閂はLocked meshだけ。Open/Closedの骨・金具はleafの外形内に収め、Openの中央空間へ装飾を追加しない |

開口は27.2×28.8 wu。Open時は中央の地面を見せつつ、左右leafの木製面がEW/NSの両投影に残る共通78°を採用する。中央の平面投影開口は約20.99 wuで、両leafは同じ奥側Z=9.515919まで張り出し、Xは固定枠の内側へ収める。
閉状態の意図した0.4 wu隙間を「Wall接続の穴」と取り違えない。Wall―jamb seamは隙間0を検査する。
基本構成は枠3材・leaf2材で60 triangles。補強、蝶番、Lockedの閂を加え、状態別`204 / 204 / 216` trianglesとして各240以下とする。
部材数を増やす前にtextureで表現できる箇所を選び、全状態の固定枠は実頂点位置の一致を検証する。

**この寸法だけではSoulの通過表示を合格にしない。** 現行billboard幅28.8 wuと
`AlphaMode::Mask(0.5)`に合わせて画像の不透明bboxを実測すると、Normalは19.800 wu、Exhaustedは
20.728 wu、StressBreakdownは24.441 wuだった。Sleep/Wineは左右非対称で、向き反転も影響する。
またNS配置ではbillboardの高さがworld Zへ投影され、横幅だけの比較では通路の余白を判定できない。
M2接続後、M3aのpreviewで実際に移動可能な状態のSoulを両軸で通し、全8状態は静止depthの比較も行う。
Frozen/睡眠のSoulを試験のために通常移動させない。枠による輪郭周縁の自然な遮蔽は許容するが、
OpenでSoul本体や足元の移動が判別不能になる場合は候補不合格。Soulの縮小・経路変更で調整しない。
寸法を修正した場合はgeometry fixture、全3 GLBと2 preview画像、seam/通過画像を一緒に更新する。

### 4.3 接続・向き・開く範囲

| 隣接条件 | 表示軸 |
| --- | --- |
| 東西がWall / Door、南北は対向条件なし | 東西の壁線に沿うClosed leaf |
| 南北がWall / Door、東西は対向条件なし | 上記から90°回転 |
| 両軸が成立 | 東西を優先 |
| どちらも不成立（撤去後・load過渡など） | 東西の決定的fallback |

maskは既存と同じN/S/W/E=`8/4/2/1`。全16maskをtestし、EWは`(mask & 3)==3`を優先、
それ以外でNSの`(mask & 12)==12`なら+Y90°、残りはEWとする。
既存`WallTopologyIndex`のreadonly近傍accessorを追加して使い、Wall側が消費するdirty集合を
Doorが取り出して奪わない。Door owner / blueprint位置の変更とindex revisionで再計算対象を決める。

4近傍のprojectionを共有し、既存Wall resolverの入力規則（必要なblueprintを含む）と揃える。
Door自身へ既存WallTopologyStateが来るとは仮定しない。方向解決のpure部分とroot側のdomain
projectionを分け、`hw_visual`へ新しいdomain直接依存を追加しない。
配置条件の両軸成立は現在合法なので拒否しない。隣接が変われば表示軸を再計算する仕様とし、
同じ世界をloadすれば同じ軸になる。向きの保存や新しいプレイヤー操作は追加しない。

Wall側の接続面は9.6 wu、中心線・cell境界位置を不変とし、Doorのjambをその面へ合わせる。
枠・戸当たり・leaf厚は§4.2の別寸法としてM0でfixture化する。Door―Door、仮設―Door、corner
近傍でも枠の重複や隙間を検査する。石壁の厚みをDoorの旧5.76 wuへ縮めない。

旧片開きfallbackはcell外へ張り出す。新productionは§4.2の両開きでcell内に納める契約へ改める。
geometryがcell内でもSoulのbillboardとの交差はあり得るため、通過表示の判定は独立させる。
selectionは従来のowner cell＋Doorの12 logical px snapを維持し、新しいmesh pickingや占有cellを追加しない。

### 4.4 2D previewと同期順

Closed GLBから約59°正射影でEW/NSの透過PNGを2枚作る。両画像は256×256 px、表示canvasは
64×64 logical px、画像内の地面anchorは(128,192) px＝canvasの(32,48)に固定する。
1 tile=32 logical pxとRtT縦補正を再現したprojectionで余白をrenderし、画像ごとのauto-cropはしない。
Spriteはこの描画anchorとcustom_size=64を使い、logical root Transformと1 cell footprintは変更しない。
PNGの画素をUIで90°回す方式は
視点を回した3D投影と一致しないため採用しない。通常のボタンiconはEWを代表画像とする。

対象は`systems/visual/placement_ghost.rs`、`interface/selection/building_place/placement.rs`、
`systems/save/rehydrate/construction_shells.rs`。建築中に見えるのは`BlueprintPulseOverlayChild`へ
cloneされたSpriteなので、軸の変更はrootと既存pulse childのimage/custom_sizeの両方へ適用する。
既存の色・alpha・点滅周期を書き換えず、通常表示とBuilding pulse表示のどちらでも軸を確認する。
候補未ready時は旧PNGを使い、ghost消失を起こさない。

| schedule | 登録・処理契約 |
| --- | --- |
| Update | 既存Door domain writer、照明再構築・upload、Interfaceからの建築変更を完了。Visual→Interfaceの既存chainへ逆向きafterを加えない |
| PostUpdate、propagation前 | Wall topology解決→ApplyDeferred→Door軸/mesh/material/tag/preview反映の`DoorPresentationSyncSet`→`TransformSystems::Propagate` |
| PostUpdate、propagation後 | 新profileのstate・world位置・ROI観測。nonce/ACK captureはこのcheckpointと対応 |
| Lastでworld置換 | 次のpresentation frameで再構成する。pauseでもtopology/表示/preview再構成を止めない |

`plugins/visual.rs`のUpdate側set設定・Door登録・schedule testと、`plugins/startup/mod.rs`の
indoor fixture検証、behavior observer、profilingだけでconsumerを再実行する登録を一緒に見直す。
final consumerはPostUpdateの唯一の登録とし、state/geometryを観測するreaderも同じscheduleへ移す。
旧fixtureをそのまま合格させるためにUpdateで旧consumerを二重実行しない。過去artifactは過去の
validatorで保持し、現在のfixture/readerはversion付きの新期待値を使う。

MeshTagは壁線ではなく**静止leafの面法線**を持つ。EWならNorth=0、NS（+Y90°）ならWest=3。
owner.rotationがidentityでも解決済み法線を明示的にtag helperへ渡し、Openの各leaf角度は使わない。

### 4.5 runtime・asset authority

- Doorのsemantic stateは既存producerを唯一の正本とし、`DoorPresentationSyncSet`を表示同期の境界として保つ。現在Door同期はUpdate、Wall topology解決はPostUpdateなので、既存indexをそのまま読むだけでは前frameの向きになる。近傍更新→Door軸・state反映→Transform propagation→captureが同frameで成立するよう登録順と検証readerを更新する。Room・遮光・通行判定へpresentationから書き戻さない。
- `MeshTag`はlogical ownerのgridと、解決した静止時のcardinal方向を保持する。Openのleaf先端や開放角度を入力にしない。現行helperはowner.rotationから方向を作るため、派生した表示軸を明示的に渡す経路へ合わせる。selection、preview、bounce、RenderLayers、cleanupを新寸法で確認する。
- Light Fieldのfragment samplingは現状の無効状態を維持する。Door mesh更新を理由に再有効化しない。
- save/load・rollbackは保存済みDoor状態と復元した隣接から表示を再構築する。派生orientation、GLB handle、asset authorityをsaveへ追加しない。
- `door-production-v1`を独立asset setとして新設する。3 GLB＋albedo＋EW/NS preview PNGのcore 6 file、source、report、provenance、review、generation / receiptを管理する。
- 既存Wall専用validatorは6 family・8 fileを要求するため、そのままDoor manifestへ使わない。再利用はhash/path/receipt等の必要な共通処理に限定し、Door用のinventory・geometry・projectionを追加する。
- rootのDoor asset adapterから有限poolを注入する。3 production mesh＋1 production material、fallback最大1 mesh＋3 state materialを予算とし、owner数に比例してmaterialをcloneしない。
- 3 mesh・albedo・2 preview画像が全てresident・identity一致になってから切り替える。不足・破損・未承認では見えるfallbackを維持し、本番受入はfallback 0を要求する。
- 各production meshは240 triangles以下とする。既存Wallの72 triangles契約を変更する値ではない。
- Bevy 0.19の既存primitiveロード・mesh交換・transform更新を参照し、新APIの署名は一次情報で確認する。

authoring manifest / runtime projectionはDoor専用version 1、runtime locatorは
`manifests/door-production-v1.doorset`とする。core roleは`mesh:closed` / `mesh:open` / `mesh:locked` /
`texture:albedo` / `preview:ew` / `preview:ns`で固定し、release pathは
`door_sets/<GEN>/models/door_<state>.glb`と`door_sets/<GEN>/textures/buildings/door/`配下の3 PNG。
新`door_asset_set.rs`、Door用validator/projector/sealerを実装し、旧Wallのlit/normal比較を必須にする
sealerへ架空の証拠を渡さない。reviewは`double_leaf_approved`、`normal=not_used_by_design`と
具体的なpreview PNG/hashへのユーザー判断を保持する。

承認前preview・正式candidateの区別、sourceとassetを封印する順序は
[型枠計画§4.4](provisional-wall-formwork-plan-2026-09-05.md)を正本とする。
通常起動はrelease receiptを必要とし、未承認previewを通常authorityへ混ぜない。
新asset set初回releaseの復帰先は「旧Door generation」ではなく既存のprocedural fallbackである。
初回promotionのpreimageとしてpointerが存在しなかった状態も記録する。

### 4.6 編集責務と着手単位

| 単位 | 主な対象（新規名は実装予定） | 終了時に確認する契約 |
| --- | --- | --- |
| D0: geometry / baseline | 新Door geometry・gallery/density fixtureとprofile、共通C0 | 両開き寸法、全16mask、現在のDoor表示によるbefore、previewと正式証拠の区別 |
| D1: 制作・asset | 新`door_asset_set.rs`、Door用authoring/projector/sealer、外部staging | core 6、3状態同じ枠、candidate/release/fallback、旧Wall互換 |
| D2: 軸・preview | `hw_visual/src/wall_connection.rs`のreadonly accessor、Door pure resolver、root ghost/placement/rehydrate adapter | 成立mask共有、root＋pulse child画像、world軸と法線tag |
| D3: 同期・cleanup | root `building3d_cleanup.rs`、`plugins/visual.rs` / `plugins/startup/mod.rs`、load resetと関連tests | PostUpdateの唯一のconsumer、reader順、world置換後の再構成 |
| D4: 受入・close | 新Door profile/sidecar、§7のdocs | semantic操作・geometry・pixelの一致、J1、Help判断、capsule |

## 5. マイルストーン

### M0: 共通C0・寸法・baseline基盤

- 変更内容: §4の両開き寸法・方向表・法線tag・PNG anchorをfixture化し、共通C0のpreviewと§7のbefore用profileを整備する。
- 対象: `tools/blender_ai_workflow/fixtures/`の新Door geometry契約、本計画、仮設壁計画との接続仕様。
- 完了条件:
  - [ ] 既存Door state / placement / Room / light / saveの正本と描画consumerを確認。
  - [x] 固定枠＋左右leafの3状態mesh、core 6、pose/fallback、PNG表示、§7の予算をcontractに落とす。
  - [ ] M0のinstrumentationだけを加えたclean before subjectで現在のDoorを採取し、支持Wallのbytes・状態分布とharnessを封印。
- 検証: 新profileのself-test→現行Doorのfresh画像・state・性能。新geometryの合格値を旧placeholderに要求しない。

### M1: 本番ドアの制作とmanifest

- 変更内容: stagingで1つの原本から3状態GLB、shared albedo、EW/NSの2 preview PNG、固定枠・開き範囲のreportを制作する。
- 対象: 外部asset rootの`staging/`、`tools/blender_ai_workflow/`、必要なallowlist同期tool。
- 完了条件:
  - [x] 木・骨、Openの通路、Lockedの閂を3状態meshへ反映し、ArtPreview判断に渡せる候補を制作。
  - [x] scene / Khronos / GLB実bytes・triangle・UV・state間の枠位置一致・Open envelopeを検証。
  - [ ] 石壁両軸との継ぎ目を確認。型枠候補ができ次第同じboardへ追加。
  - [ ] core 6 file、hash、source/provenanceとpreview/candidate/release authorityを揃える（technical candidateとArtPreview projectionは完了。正式candidate・release・初回pointer復旧はM3/M4）。
- 検証: Door用Python validatorのfocused tests。OCIO正常性を確認したreference render。

### M2: 向き・3状態・lifecycleの接続

- 変更内容: root Door adapter、表示軸resolver、state mesh交換、preview、fallback、loadを実装する。
- 対象: §4.6のD1〜D3。既存のplacement / pulse child / rehydrate / profiling observerを一緒に更新する。
- 完了条件:
  - [x] Closed/Open/Locked×両軸でproduction rootの枠位置を状態非依存にし、解決軸だけを合成。
  - [ ] 通常建築・Instant Build・Wall置換・隣接追加削除が同じ表示へ収束。Lastでのload/rollbackは次の最初のpresentation frameで再構成。
  - [x] 近傍解決→Door asset readiness→mesh/軸/tag/preview→Transform propagationをPostUpdateへ一本化し、profiling readerも同境界後へ移動。
  - [x] exactly-one visual、logical MeshTag、全ready前fallback、cleanup、production 3 mesh / 1 materialの有限poolを維持。
  - [x] 設計図root・pulse child・placement ghostを完成後と同じ軸resolver・asset identityへ接続。候補無効時は旧PNGへ復帰。
- 検証: 状態/軸resolver、実経路lifecycle、asset失敗系のfocused test、rust-analyzer診断、check、Clippy。

### M3: アート判断・正式candidateの単独受入

- 変更内容: 共通手順のM3aでアートpreviewと両軸Soul通過を提示・判断・封印し、M3bでpreview無効の正式candidateを受け入れる。
- 前提: 最終承認preview・封印を行うM3aの前に両M2のコードを同じclean subjectへ統合・freezeする。Doorの制作・探索previewは先行可能で、相手のM3完了やreleaseを待たない。単独M3とJ1は同じcommitでも異なるasset viewなので、共通手順§4.4の専用worktreeを使う。
- 対象: root `plugins/startup/perf_scenario/`、native acceptance helper、必要な`perf_tool`のvalidator。
- 完了条件:
  - [ ] §7の操作・画像・状態・resource gateが合格し、最終アート候補を比較できる。
  - [ ] Door単独は既存本設壁で受入を完了し、相手のM3完了を待たない。両単独M3後の共通J1へ渡す。
  - [ ] 既存P02の旧色material predicateやhistorical mirror列を流用して合格にしていない。
- 検証: `hell-workers-run-native-acceptance` Skillの専用scenarioとdirect `kitty` launcher、独立artifact verify。

### J1: 型枠との共通受入

- [型枠計画J1](provisional-wall-formwork-plan-2026-09-05.md)を両track共通で1回実行し、同じsubject・両asset hash・job IDを参照する。
- 両軸seam、Door連続、支持変更、tile完成、pause中loadで枠・leaf・previewが一致したことを確認する。
- J1の前提は両単独M3の完了。DoorのM4はJ1を待つが、Doorの単独M3は待たない。

### M4: release・文書同期・close

- 変更内容: 最終候補画像、寸法、全状態、検証結果、manifest / receipt / 同期差分を揃え、canonicalとruntime mirrorへreleaseする。
- 対象: `docs/building.md`、`docs/art-style-criteria.md`、`docs/assets_workflow.md`、`docs/rendering-performance.md`、必要な選択・saveの説明、親計画とHelp。
- 完了条件:
  - [ ] session内の既存承認を確認し、不足するアート／releaseの最終判断だけを完成候補で求める。
  - [ ] 共通J1がvalidで、両candidateの変更後の組合せを指す。
  - [ ] candidate opt-inのない通常releaseで3状態と両軸が成立。
  - [ ] Help影響レビュー、全品質gate、artifact封印、worktree整理後に計画をarchive／削除して索引更新。

## 6. リスクと対策

| リスク | 影響 | 対策 |
| --- | --- | --- |
| 枠込みmeshへ旧Open transformを適用 | 枠まで回転・移動する | 状態はleafだけbakeしたmeshで表し、root姿勢は状態非依存 |
| 高さ16 wu用の中心を流用 | 新枠が地中へ埋まる | asset anchorと32 wu高さをresolverと一緒に検証 |
| 支持壁変更時の軸反転 | 外見が切り替わる | 決定表を仕様化し、両軸成立・片側撤去・loadを同じ隣接条件でtest |
| Open leafが枠・隣接物へ刺さる | 本番品質を満たさない | 両開きのcell内AABBを実bytesで検証し、密集配置とSoul depthを別に評価 |
| Lockedを色だけで表す | 遠景や色の見え方によって識別不能 | 閂・錠の形と既存の状態UIを合わせて確認 |
| 旧のexact material/transform検証を流用 | 描画成功を誤判定 | 現行subject用のmesh identity・state・枠位置・pixel観測を新profileへ実装 |

## 7. 検証計画

| 層 | ケース・合格条件 |
| --- | --- |
| geometry | 3状態×両軸で枠/jamb/anchor不変、Wall port接続、Open envelope、全GLB単一primitive、有限material |
| 操作・domain | Soul進入前待機→Open→通過→自動Closed、プレイヤー施錠/解除、Locked通行不可。論理stateと表示が同期 |
| Room・照明 | DoorはRoom境界として既存契約を維持。Openは通光、Closed/Lockedは遮光。表示状態からruleを書き換えていない |
| lifecycle | 通常建築、Instant Build、blueprint cancel、支持壁追加削除、Door撤去、paused load、rollback、asset遅延/破損、再読込で重複・消失なし |
| 画面 | 本設Wall―Door―Wall、型枠混在、Door連続、corner隣接、狭い通路、Soulの前後depth、選択/hover/previewのずれなし |
| quality | High/Medium/Low × DPI 1/1.5/2、標準zoom・最大zoom-out。通常zoomで木・骨と3状態、遠景で開口と施錠の手掛かりを確認 |
| 性能 | N=32 / 4N=128のDoor、同じ支持壁数・状態分布のstatic fixtureでbefore/afterを比較。30/60秒×各3 runs、p95/p99中央値+5%以内 |
| resource | Capture→Memoryを逐次実施。3 state mesh×1 materialの有限pool、state切替/反復loadでmaterial・visual残存数が増えない |

全quality/DPIのgalleryに両軸と3状態を含め、source・asset・実adapter/backendとphase nonce/ACKを
window画像へ結ぶ。操作経路はギャラリーへの直接state設定だけで代替せず、productionの
Door producerとUIを通す。素の緑/赤pixelだけを探す旧predicateは使わない。

性能用の新Door fixtureをM0で固定し、support Wallの差をDoorの性能差として数えない。
仮設壁trackと同時変更する場合も、比較時のWall asset / state分布を揃える。
実draw groupingを主張する場合だけ別RenderDoc計測を行い、Mesh3d数から推定しない。
headlessと`visual_test`は補助検証であり、actual-windowの代替にしない。

### 7.1 ケースと観測点

新profile名は`door-art-v1`（画像・操作）と`door-density-v1`（性能）とする。画像は
`door-art-v1-quality`と`door-art-v1-behavior`へ分離して`plan / status / verify / self-test`を実装済み。
性能profileは未実装であり、M0で同じcommand surfaceを揃える。baselineの旧Doorは旧material/pose、新Doorは
新mesh/枠位置と、同じprofile内でsubject role別の期待値を持たせる。

| ID | setup / 操作 | 観測する結果 |
| --- | --- | --- |
| D-G01 | 3状態×EW/NS、本設Wallとの接続、all8 Soul静止depth、左右facing | 全状態のframe頂点/world位置一致、state mesh、seam、標準/最大zoom-outの識別性 |
| D-G02 | mask0〜15、支持の追加/撤去、ownerの非identity回転・scale | 決定表の最終world軸、EW North / NS West tag、二重回転なし |
| D-L01 | 通常placement、既存Wall→Door blueprint、pulse中の支持変更、途中cancel/load、通常完成・Instant Build | ghost/root/pulse childの画像・anchorと完成軸が一致、owner占有は既存ruleのまま |
| D-L02 | Soul進入→Open→通過→自動Closed、UIからLocked/解除（pause含む） | 実producerと`UiIntent::ToggleDoorLock`経路、semantic/WorldMap/presentationが同期。Lockedのみ通行禁止 |
| D-L03 | 両軸・全stateをsaveしpause中load、失敗load rollback、load10回 | rehydrate直後は可視fallback、次のPostUpdateでtopology/assetsが揃えばproductionへ復帰。意図的な1 frame待機なし、worldごとにexactly-one、ready/settle後のcount一定 |
| D-A01 | GLB1件欠落、PNG1件欠落、hash不一致、未承認、世代不一致、初回pointerなしへ復帰 | fallbackで消失なし、正式受入はfail、3Dとpreviewの部分的な世代混在なし |

sidecarはphase・frame・epoch・owner grid・semantic state・mask・world軸・法線tag・表示mode・
state mesh role/hash・固定枠のworld bbox・preview role/anchor・active/resident/retained数を持つ。
各PNGは同じphaseのnonce/ACKと紐付ける。state設定だけのgalleryは画像比較に使い、操作成立の
証明にはD-L02を使う。枠の遮蔽と扉姿勢はpixel/ROIでも検査し、sidecarの自己申告だけで合格にしない。

### 7.2 計測matrixと合格値

[型枠計画§7.2](provisional-wall-formwork-plan-2026-09-05.md)の
共通比較規則を使う。Doorのfixtureはseed `20260906`、8列のspecimen、5×5 cellの間隔で並べる。
ordinal iの中心はgrid `(12+5*(i%8), 12+5*(i/8))`（整数除算）。Nは先頭32、4Nは128。
性能windowは1280×720、camera scale 5、High/DPI 1・Vulkan/X11・novsyncを共通とする。
偶数iはEW・奇数iはNS、各Doorの対向2 cellに同じ完成Wallを置く（支持Wall数64 / 256）。
状態は`i%3`の順にClosed/Open/Lockedとし、Nは11/11/10、4Nは43/43/42。

静的性能fixtureは全setup・asset ready後もVirtual Timeをpauseして状態分布を固定し、Real Timeで
30/60秒を計測する。これは定常表示の性能比較であり、開閉中の性能を主張しない。実開閉とloadは
D-L02/D-L03で別に検証する。before/afterで同じfreeze契約を使い、開始/終了の状態分布を照合する。

- galleryは9 quality/DPI process、各processで標準/最大zoom-outの2 checkpoint以上。lifecycleはHigh/DPI 1の1 storyboard process。
- CaptureはN / 4N × before/after × 3＝12 runs、各caseのp95/p99中央値+5%以内、MADを併記。
- Memoryは4N × before/after × 3＝6 runs。max RSS中央値+5%以内、peak live bytes中央値+4 MiB以内、accounting error 0。
- load10回の各ready/settle後でowner/visual/保持asset数をsidecarへ記録し、単一のMemory peakから漏れなしと推測しない。
- runtimeのみのmesh/material poolはproduction3 / 1＋fallback最大1 / 3。画像はalbedo1＋preview2。reload中は最大2世代、settle後は旧世代参照0を確認。
- dedicated helperの直列launcherとpreflightを使用し、RenderDoc・過去RtT formal matrixを標準caseへ追加しない。

実装中は`python3 scripts/dev.py check`と
`python3 scripts/dev.py cargo -- clippy --workspace --all-targets -- -D warnings`。
完了時は`python3 scripts/dev.py verify`、`git diff --check`。
Help impact review Skillで建築／ドアの実説明を読み、色表示への言及や施錠の見分け方を更新する。
文言変更不要ならその実経路を理由にNo impactを記録し、計画段階で判定を先取りしない。

## 8. ロールバック方針

- Door asset setはWall releaseと独立したgeneration / pointerで管理する。初回releaseはpointerなし＋既存procedural fallbackへの復帰、それ以降は旧generationへの復帰を検証する。
- 開閉semantic・保存schemaは変えないため、表示assetの差戻しにsave変換を伴わせない。
- Rust差戻しは履歴・全差分・並行sessionを確認し、対象変更だけをrevertする。asset世代の上書きや広範囲なgit復元をしない。

## 9. AI引継ぎメモ

### 現在地

- 進捗: 実装 `92%`。M0のgeometry/全16mask、M1のtechnical candidate、M2の3D/2D adapterとPostUpdate境界、M3aを完了。承認済みgeneration 5 bytesをclean subject `49c43e0f`のgeneration 6 authoring finalへ封印し、preview無効のisolated candidateでEW/NS 6状態の実機visual legと、7 behavior case×3 runsの候補紐付き受入まで通過した。9 quality/DPI processで各2 zoom checkpointをACK保持する正式runnerも実装済み。
- 次の作業: `door-art-v1-quality`をclean candidate worktreeで採取し、Door density Capture/Memoryを続けてDoor単独M3bを閉じる。その後に共通J1、releaseを行う。behavior受入は再採取せず、正式jobと保持中の失敗jobを比較証跡に使う。
- 仮設壁M0とport契約を先に共有。Doorの制作・既存石壁との接続は型枠release待ちにしない。joint受入は双方のruntime接続後。
- 3 mesh方式は現在の瞬時状態切替に合わせる判断。スムーズな開閉を追加する場合はこの選択を再検討する。
- セルフレビューで片開きから両開きへ変更し、初回ArtPreview後に完全90°から68°、NS識別性の再指摘後に同方向・同角度の78°へ改訂した。Open leaf AABBはfixture値であり、全Soul状態・両軸の実画面通過を承認済みとしない。
- 本番asset実体は外部stagingでgeneration 5（manifest SHA256 `63a428e0d7f2e8bda20bac871cc27fabcfd1edb13f00edd5d79765bc5b218638`）として封印済みであり、通常起動では無効。Door専用sealer/projector/provisionerとgallery sidecarを使い、Wall専用manifestへ混ぜない。

### 参照必須ファイル

- `docs/building.md`、`docs/art-style-criteria.md`、`docs/assets_workflow.md`、`docs/blender-setup.md`、`docs/room_detection.md`、`docs/world-selection.md`。
- `crates/bevy_app/src/systems/visual/building3d_cleanup.rs`、`crates/bevy_app/src/plugins/startup/visual_handles.rs`。
- `crates/bevy_app/src/systems/jobs/building_completion/spawn.rs`、`crates/hw_world/src/door_systems.rs`、`crates/hw_visual/src/wall_connection.rs`。
- `.codex/skills/hell-workers-run-native-acceptance/SKILL.md`、`.codex/skills/hell-workers-review-help-impact/SKILL.md`。

### 最終確認ログ

- 計画作成: `2026-09-05`。Rust / runtime asset変更なし。
- セルフレビュー: `2026-09-06`。§0の指摘を計画へ反映。実装・アセット制作・native起動は行っていない。
- 文書gate: `python3 scripts/dev.py docs --check` / `git diff --check` はpass。
- Help gate: Door形状候補は通常起動から隔離されたArtPreview用で、入力・建築条件・Doorの意味状態・UI/Help文言・操作手順を変えないため`No impact`。commit `32e4f2f4`へ理由付きtrailerを記録し、`python3 scripts/dev.py verify`をpass。
- 初回ArtPreview: production 6 / fallback 0をIntel Arc・Vulkan・X11・1280×720で確認。上枠の太さとOpen識別性のユーザー指摘により未承認。
- 形状改訂: GLB/Khronos/texture gateと状態別Blender review renderを通過。commit `32e4f2f4`、candidate generation 2（manifest SHA256 `6a6af6064052fb1c1715b9e36fb9e8a10eff4e60b3569279490c4fad40519375`）をclean validation worktreeへ固定した。
- 変更後ArtPreview: Intel Arc・Vulkan・X11・1280×720でproduction 6 / fallback 0を確認。ゲーム所有statusはACK後17秒を越えて`ready`を維持し、画像・sidecar・binaryのhashを `target/native-acceptance/door-art-preview-32e4f2f4-v2-r3/manifest.json` に記録した。性能値は採取・主張しない。
- generation 2レビュー: EW Openは改善したが、NS Openは同じ前後側へ重なって見えるとのユーザー指摘により未承認。
- generation 3レビュー: NS投影を分離するため左右leafを前後反対側へ振り分けたが、通常の両開きではないとのユーザー指摘により不採用。左右の開度差も採用しない。
- generation 4レビュー: 左右を同じ側へ同じ78°で対称に戻したが、実機NS投影では片側の扉面が枠と同化したため未承認。角度だけの調整を打ち切る。
- generation 5改訂: 左右同方向・同角度・対称を維持し、各leafの上下2本の骨補強を扉面方向の形状手掛かりとして追加する。
- generation 5 ArtPreview: Intel Arc・Vulkan・X11・1280×720でproduction 6 / fallback 0を確認。ACKは`ready`から0.53秒で完了し、17秒後も`ready`を維持した。画像・sidecar・binaryは `target/native-acceptance/door-art-preview-aaed33a7-v5/manifest.json`（SHA256 `926c009171fc63da9d41a8e86c94acd1ba742a3aaa1be168a54fd91a6fc898a0`）へ記録し、性能値は採取・主張しない。
- generation 5承認: ユーザー回答「OKです」をcandidate manifest SHA256 `63a428e0d7f2e8bda20bac871cc27fabcfd1edb13f00edd5d79765bc5b218638`、capture SHA256 `62ce9380e5e4e083f3fea125965d1da44785ee2b07ce70edf6d7a3e750db23e2`、review crop SHA256 `f2ab99b9d4d233103f0294b3124f5c9b73977b360e847cc96ecb29094698a6b2`へ結び、approval artifact SHA256 `7db1849ff53121623016cc65d269a603cd8f98a0ee1ea7f515edf01e8221755f`として封印した。
- generation 6正式candidate: authoring final SHA256 `4f2b795f596fbd30c84a16588a873f889c158a5f87b9d6357d7233c13de38448`、runtime locator SHA256 `6c4752242d207a66c628ea0891731f5512cc813542fc176a7fdc2fe3b2d24883`。Intel Arc・Vulkan・X11・1280×720で`authority=IsolatedCandidate`、production 6 / fallback 0を確認し、visual leg manifest SHA256 `7bca23820055836169e29f9931bfe09df134a839ae35d67ad803514ee84cc4d9`へ封印した。最初の試行は共通asset不足でACK timeoutとなり失敗jobを保持し、共通assetを追加して候補6 fileとlocatorのhash不変を再確認後に再試行した。
- behavior受入基盤: `2026-09-07`に`door-art-v1-behavior`を追加。既存P08の7 behavior case×3 runsを変えず、専用opt-in時のみ`IsolatedCandidate` identity、semantic state対応のproduction mesh、共有material、production 1 / fallback 0を各processでfail-closed検証する。case別statusは共通nonceとprocess IDへ結び、既存P08 data schema外へ保存するためhistorical artifact契約は不変。runner self-testとprofiling feature compileはpass。初回jobはP08のheadless/repeat契約に対してX11/repeat 1を要求したためgame起動前にfail-closed停止し、artifactを保持。固定P08契約へ合わせた正式native採取は次項。
- behavior正式native受入: clean validation worktreeのsubject `6d5ea2f9`、generation 6 candidate、source fingerprint `2af90c49929f2c5c44fb63e7eb089c07c70eb7496e9cc7f780499aa005ca711b`、harness fingerprint `27ae31a90d2aed7334b4633df8fb7ecff2468554e3521d3afee1f87483e914d0`で`door-behavior-20260906T183549Z-0895f5ad`を採取。Door state、通常load、preflight reject、rollback、recovery-only、recovery-failed、duplicate-resetを各3 runs、計21 runsでpassした。通常・復旧成功系は観測semanticとproduction mesh/materialの一致およびproduction 1 / fallback 0、recovery-failedは破壊前Closed candidateと終端fail-darkのproduction 0 / fallback 0を検証した。job manifest SHA256 `209af7220b17ca3b6a47b7e08b861f2f5eeaa3d9eeb54f48b38aecebbcf69ae4`、session manifest `610d9c4a029a859188a73bba507b19bbd9aa5c705bef4f37366f0a823cef2762`、matrix `4e35402fb5ef468c2baedb82e54c65cf396cad2c31f87279263b05b4cde0c4a9`、report `1a0fb40b639b56013f085b3f178db2298e929e954f4e8d1eb4b81ed0a9ed53fb`。独立`verify`もpassし、残存game process 0を確認した。
- quality/DPI受入基盤: `door-art-v1-quality`を追加。High/Medium/Low × DPI 1.0/1.5/2.0の9 processを逐次実行し、各processで標準zoomと最大zoom-outをgeneration 1/2のnonce/ACKへ固定する。18枚のX11 client PNGをcandidate/source/harness/binary/asset fingerprints、実Vulkan adapter、quality/DPI、EW/NS×Closed/Open/Lockedのproduction 6 / fallback 0、投影ROIと状態差pixel観測へ結ぶ。正式native採取前であり、この項だけではquality gateを合格扱いしない。
- 失敗job保持: `door-behavior-20260906T170630Z-952adc7b`（P08 backend/repeat契約違反）、`door-behavior-20260906T173316Z-6c17f870`（recovery-failedを通常rebind扱い）、`door-behavior-20260906T175052Z-e55cf022`（到達不能なbaseline observer）、`door-behavior-20260906T181055Z-87886a8f`・`door-behavior-20260906T182844Z-6dc51b2b`・`door-behavior-20260906T183035Z-8f8d34b8`・`door-behavior-20260906T183230Z-1cc267d4`（集約器の固定role／log policy誤り）を成功jobと同じworktreeに保持する。成功jobのcapsule化まではworktreeを削除しない。
- ブロッカー: ArtPreview承認、正式candidate visual leg、候補紐付きbehavior matrixは解消。quality/DPI、Door density Capture/Memory、Wall単独M3との共通J1が未完であり、releaseは行わない。

### Definition of Done

- [ ] M0〜M4と共通J1完了、3状態・両軸・Wallと型枠のseam・操作とloadが成立。
- [ ] Help判断と恒久文書更新が完了、check / Clippy / verify成功。
- [ ] 通常releaseでの実機受入と最終候補の承認記録がある。
- [ ] job manifest / 比較CSV / 承認PNGを小さなcapsuleへ封印し、hashをclose文書へ記録。
- [ ] 本trackで作ったvalidation worktreeとbranchを削除し、`git worktree list`、削除前後の`du -sh`、回収容量を記録。他trackと共用なら両trackのcloseまで所有を明記する。

## 10. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| `2026-09-05` | `Codex` | 木・骨の意匠、固定枠＋3状態mesh、向きと開き範囲、runtime・実機・releaseの計画を作成 |
| `2026-09-06` | `Codex` | セルフレビュー。両開き寸法、Soul alpha幅と通過gate、2軸PNG/pulse、法線tagとPostUpdate ordering、core 6、共通C0/J1・数値受入を具体化 |
| `2026-09-06` | `Codex` | M0〜M2実装。3状態GLB・共有albedo・EW/NS preview、Door専用asset authority、全16mask resolver、2D/3D同期、PostUpdate境界、6状態ArtPreview galleryを追加 |
| `2026-09-06` | `Codex` | 初回ArtPreview指摘を反映。上枠を高さ1.6 wu・奥行7.2 wuへ薄型化し、Open leafと付属金具を68°の蝶番姿勢へ変更 |
| `2026-09-06` | `Codex` | generation 2を実機再撮影。EWで両扉面と中央開口、NSで閉扉輪郭から張り出す扉面を確認し、ユーザー承認待ちへ移行 |
| `2026-09-06` | `Codex` | generation 2のNS識別性指摘を反映。左右leafの同方向重なりを、前後反対側へ振り分ける68°開扉へ変更 |
| `2026-09-06` | `Codex` | generation 3の不自然な開き方を不採用。左右同方向・同角度・対称を明文化し、共通78°でNSの張り出しと開口を両立するgeneration 4へ改訂 |
| `2026-09-06` | `Codex` | generation 4実機セルフレビューで角度だけではNS識別性が不足と判定。開き方を変えず、扉面と連動する横向き骨補強を加えるgeneration 5へ移行 |
| `2026-09-06` | `Codex` | generation 5を実機撮影。両開きの同方向・同角度・対称を維持し、EW/NS 6状態のproduction表示と骨補強を確認してユーザー承認待ちへ移行 |
| `2026-09-06` | `Codex` | ユーザーがgeneration 5 ArtPreviewを「OKです」で承認。候補・実機job・capture/crop・status/ACK hashをapproval artifactへ封印し、正式candidate用のfail-closed projectionを追加 |
| `2026-09-06` | `Codex` | 承認済みbytesをgeneration 6 finalへ封印。isolated candidateの実機6状態でproduction 6 / fallback 0を確認し、visual legのみ合格。Soul/lifecycle/性能を未採取のためM3b全体は継続 |
| `2026-09-07` | `Codex` | generation 6 candidateへ既存P08の7 behavior case×3 runsを結合。成功・復旧・fail-darkを候補identity、production mesh/material、nonce付き21 statusへ固定し、正式native jobと独立verifyをpass |
| `2026-09-07` | `Codex` | `door-art-v1-quality`を実装。9 quality/DPI processの各々で標準／最大zoom-outを同一processの2段階nonce/ACKへ結び、18 client PNGと状態別投影ROIをfail-closed検証する受入基盤を追加 |
