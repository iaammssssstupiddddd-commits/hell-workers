# 壁の本番アート化計画

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `production-wall-art-plan-2026-08-31` |
| ステータス | `In Progress (M5)` |
| 作成日 | `2026-08-31` |
| 最終更新日 | `2026-09-03` |
| 作成者 | `Codex` |
| 親計画 | [`asset-milestones-2026-03-17.md`](asset-milestones-2026-03-17.md) の `MS-Asset-Pipeline` / `MS-Asset-Build-A` |
| 関連提案 | [`billboard-camera-angle-proposal-2026-03-16.md`](../../proposals/3d-rtt/archived/billboard-camera-angle-proposal-2026-03-16.md)（4形状案の履歴。本計画では孤立／端の意味を満たす6形状へ補完する） |
| 関連Issue/PR | `N/A` |

本書は、現在の茶色い `Cuboid` プレースホルダーを本番壁へ置き換えるための詳細実行計画である。
親計画の壁PoCについて、アート制作、6 GLBから16接続maskへの写像、runtime lifecycle、実機受入、
canonical asset昇格までを一つの閉じた作業列として具体化する。両者が矛盾する場合、壁については
本書を優先する。

## 0. 固定する結論と停止ゲート

| 項目 | 本計画の結論 |
| --- | --- |
| 見た目 | Rough Vector Sketch。黒い石積み、錆びた鉄バンド／トゲ、紫 `#8b008b` の裂け目、暗茶 `#1a0a00` 起点のラフな線 |
| 表示方式 | 現行TopDown 2.5Dの独立 `Structural3d` を維持し、ownerあたりexactly oneの `Mesh3d` を差し替える |
| 形状数 | `isolated` / `end` / `straight` / `corner` / `t_junction` / `cross` の6共有mesh。16個のGLBは作らない |
| 本番壁厚 | 公称構造厚`9.6 wu = 0.30 tile`、接続port半幅`±4.8 wu`。全腕の連続visible cross-sectionも9.6 wu以上、石・鉄・トゲ込み外形は最大`12.8 wu = 0.40 tile` |
| 接続 | Wall、Door、Wall/Door blueprintを接続対象とし、16近傍maskを6形状＋Y軸90度回転へ決定的に写像する |
| material | `TopDownStructuralMaterial` を維持し、active productionは完成／仮設2 handleだけを共有する。堅牢なfallback pairを含む総poolは4、entityごとのcloneは禁止 |
| 裂け目の光 | shared emissive mapによる表面表現だけ。`PointLight`、gameplay照度、Light Field emitterは追加しない |
| normal map | なしから開始し、native A/Bを一度だけ行う。差が採用基準を満たさなければ「なし」で確定する |
| outline / unlit | texture-baked linework＋現行stylized directional shadowを第一候補とする。`base.unlit`比較は同じmaterial型内で行い、global outline rendererは本計画へ混在させない |
| asset障害 | 6 meshとshared textureが全てresidentかつtopology consumerが有効になるまで、既存procedural `Cuboid`＋solid-color material pairをall-or-nothing fallbackとして表示する。透明化、一部だけの切替、読込だけを根拠にした本番化は禁止 |
| 性能上限 | 各GLB 24〜72 triangles、production mesh 6＋fallback mesh 1、active production material 2＋fallback pair 2、production mesh/material組合せ上限12。実draw callとframe値はnative計測で判定 |
| 正本昇格 | stagingで全gateを通した後、ユーザーの明示承認を得てから外部canonical `source/` / `exports/`へ昇格する |

次は独立した停止ゲートであり、検査が通るまで後段の意味を縮小して解釈しない。

1. Blender Flatpak同梱OCIO config `2.5`はruntime OCIO `2.4.2`で読めない。壁校正ではsealed profile 2.1 configを明示し、active config一致と`fallback=false`を証明する。明示なし／不一致ではgeometry検査だけを許し、**色再現性の承認とcanonical昇格は行わない**。
2. 6 GLBまたはshared textureのうち1つでもmissing、failed、構造不正なら、native受入ではproduction wallを合格にしない。runtimeはfallbackを維持する。
3. 自動pixel判定、validator、性能値だけでアート承認にしない。最終候補はユーザーの主観目視を必須とする。
4. baked linework、lit/unlit比較のどちらでも平面的なイラストとして成立しない場合、同系統の数値調整を続けず、wall outline rendererを別提案・別計画として切り出す。

### 0.1 未決事項の閉じ方

| 未決事項 | 初期値 | 閉じるマイルストーン | 後から覆す条件 |
| --- | --- | --- | --- |
| Blender / runtime色変換 | 色承認blocked | M0で同一patchのOCIO gateをpass | OCIO、tonemapping、exposureのいずれかが変わった時だけ再検証 |
| lit / unlit | lit第一候補 | M4のreference条件A/B＋ユーザー承認 | 採用後のP02回帰失敗時はM4へ戻す |
| normal map | なし | M4でtechnical gate後にwinnerへ1回だけA/B | 採用条件を満たさなければなしで確定し、再調整しない |
| baked outline | 採用候補 | M4のstandard / farthest zoom判定 | silhouette不成立なら本計画を止め、別outline提案へ移す |
| Doorとの無隙間seam | 本計画では保証しない | Door本番asset計画へ移管 | Wall側port / mask / centerlineの不具合だけ本計画へ戻す |

M4で比較するのは上表の未決事項だけである。壁厚、6 family、32 wu logical footprint、
9.6 wu portは比較候補へ戻さない。

## 1. 目的

- 解決したい課題: active 3D wallのRough Vector Sketch本番assetへの置換と、制作からruntime受入までの一貫した経路
- 現状の詳細:
  - 完成壁と仮設壁のactive 3D表示は、`TILE_SIZE`立方体と単色materialのプレースホルダーであり、確定済みの建築アート基準を満たさない。
  - 既存の16方向壁textureと接続systemは主に2D `Sprite` を対象としており、active `Building3dVisual` のmesh形状を変更しない。
  - GLB制作、runtime読込、接続更新、load、fallback、実機受入を一つの完了条件で結ぶ計画がない。
- 到達したい状態:
  - 通常ゲームの全Wallが、黒石・錆鉄・紫裂け目を持つ共有GLB wallとして見える。
  - 孤立、端、直線、全corner、全T、crossが、追加・撤去・Door隣接・仮設→完成・save/load後にも正しい形状と向きを保つ。
  - 論理占有、移動阻害、Room境界、遮光、save schema、施工taskは変更せず、presentationだけを置き換える。
  - asset欠落時は壁が消えず、明示的なfallback診断を残す。production受入時はfallback使用数が0である。
- 成功指標:
  - 16 maskのpure resolver testが全件合格し、normal build、Instant Build、blueprint、cancel、deconstruction、rehydrateの実経路testが合格する。
  - wall ownerごとに `Building3dVisual` / `Mesh3d` / materialがexactly one、`MeshTag`は論理root gridを保持する。
  - 全6 meshの接続portが幅9.6 wuの同一profile、各armの局所横断で連続壁体が9.6 wu以上、装飾込み外形が12.8 wu以下で、quarter turn後も同じ契約を満たす。
  - native wall galleryで全16形状、完成／仮設、Door隣接、前後depth、completion bounce、load、撤去更新が実画像とsidecarの両方で合格する。
  - resident production mesh handleは6、active production material handleは2、fallback使用0、各mesh 72 triangles以下、production mesh/material組合せ12以下を満たす。fallbackを含む総poolもmesh 7 / material 4で有限である。
  - M0の`wall-density-v1` Cuboid baselineとM5 production、およびM5内の`force-fallback` controlとproductionを、各run内percentileを先に求めた3 valid runの中央値で比較し、Capture p95 / p99をそれぞれ`+5%`以内にする。completed opaque wallの`N` / `4N`ではM0で凍結したwall main-pass draw-group上限を満たし、provisional transparentはsorted phaseとして別計測する。
  - ユーザーがproduction camera上の見た目を承認し、外部asset manifestとrepo runtime mirrorのSHA-256が一致する。

## 2. スコープ

### 対象（In Scope）

- 壁のreference board、canonical Blender原本、6 GLB、shared albedo / emissive、必要時だけ比較用normal map。
- Bevy 0.19でのGLB primitive直接読込、resident判定、procedural fallback、有限shared material pool。
- Wall / Door / blueprintを含む4方向接続resolverと、3D mesh / rotationおよび既存2D blueprint表示への共有適用。
- 建設開始、仮設、完成、Instant Build、cancel、deconstruction、save/load rehydrate、debug/perf fixtureでのpresentation lifecycle。
- production camera、DPI、RtT quality、Soulとのdepthを含む専用actual-window受入と既存P02回帰。
- asset pipeline、アート基準、建築表示、性能予算、README、親計画、Help影響判断の文書同期。

### 非対象（Out of Scope）

- Wallの占有cell、collision相当の `WorldMap` 障害、Room境界、遮光、耐久、資材、task、save schemaの仕様変更。
- Door、Floor、設備、Soul、Familiarの新規アート制作。
- Doorのframe / jamb、向き契約、暫定leafとの無隙間seam。本計画はWall側の9.6 wu portと接続maskまでを保証し、Door本番化で同portを消費する。
- 16接続形状それぞれのGLB、wall専用LOD system、section view / LOD0、`build_progress` clippingの再有効化。
- global outline post-process、screen-space edge renderer、共有shadow shaderの全面再設計。必要なら別提案にする。
- Indoor Light Fieldのfragment sampling再有効化、紫裂け目によるgameplay light、PointLight、SpotLight。
- canonical昇格、Git commit / pushをユーザー承認なしに実行すること。

## 3. 現状とギャップ

| 領域 | 現状 | 埋めるギャップ |
| --- | --- | --- |
| active wall | `visual_handles.rs`で `Cuboid::new(TILE_SIZE, TILE_SIZE, TILE_SIZE)`、完成は茶色、仮設は半透明amber | 6 production meshとshared texture/materialへ切替 |
| 壁厚 | placeholder meshが32 wu幅で1 cell全体を覆い、見た目の構造厚と論理占有幅が同じ | 各armの局所横断を公称・連続最小9.6 wuへ縮め、12.8 wu装飾外形 / 9.6 wu接続portをasset contract化。論理占有32 wuは維持 |
| 接続 | `hw_visual::wall_connection`にWall / Doorを含む16分岐があるが、`Sprite` imageだけを更新 | pure topology解決を抽出し、3D `Mesh3d` とY回転にも適用 |
| lifecycle | completionはmaterialを交換し、visual transform syncはowner transformで毎回上書き | topology rotationをowner rotation / bounceと一つのpure transformで合成 |
| save/load | rehydrateは通常spawn経路で3D shellを再構築 | load直後に同じ接続resolverでmesh / rotationを再構築するtestと証跡が不足 |
| asset | repo外canonical generation storeは新規環境として空から開始。repo内の旧2D wall画像はruntime reference | stagingから最初のimmutable generationを作り、manifest、validator、recover / rollback、明示promoteを完結 |
| material | `TopDownStructuralMaterial`はPBR/prepass/depth/shadow契約を持つ。Light Field handleはbindするが現fragmentでは意図的にsampleしない | 契約を壊さずalbedo / emissiveを接続し、flat illustration成立方法をnative比較で確定 |
| art | `art-style-criteria.md`は壁デザインを確定、normal / outline詳細はPoC待ち | 比較条件、打切り条件、ユーザー承認artifactを定義 |
| performance | generation 2は見た目を変えない共線分割で216〜240 trianglesを生成していた | 共線分割を除去して24〜72 trianglesへ固定し、6 mesh×2 materialの上限12 handle組合せとactual draw callを計測・記録 |

### 3.1 保持するruntime契約

- 1 logical Wallにつき独立した `Building3dVisual` はexactly oneとし、GLB `SceneRoot`や子mesh treeをspawnしない。
- `MeshTag`はwall topologyの回転後座標ではなく、常に論理ownerのgrid anchorから生成する。
- `Structural3d`、Scene RtT render layer、depth、directional shadow casting、completion bounce、cleanupの既存経路を維持する。
- 完成Wallだけが論理的な遮光対象であり、ProvisionalWallは通光する現行仕様を変えない。
- production meshは1 cell内に収まり、斜め配置や隣cellへの突き出しでgameplay footprintとの見た目をずらさない。
- 見た目の公称壁厚9.6 wuに対し、WorldMapの移動阻害、Room境界、遮光、selectionは従来どおり32×32 wuの1 cellを使う。mesh脇の各11.2 wuは通路ではなく、建築cell内の視覚的余白である。
- 現行Door leafの厚さ5.76 wuはPhase 2 placeholder値であり、Wall厚の根拠にも接続断面の正本にもしない。Wall–Doorはmask / centerline / Wall側portを検証し、leafとの無隙間jambはDoor本番化へ移管する。
- current shaderのIndoor Light Field未sample状態をwall作業のついでに変更しない。

## 4. 実装方針

### 4.1 アートとasset contract

#### 壁厚の算出と固定寸法

このprojectにはmeter換算の正本がないため、一般建築のmm値をworld unitへ仮変換しない。現行の
`TILE_SIZE = 32 wu`、高さ`H = 32 wu`、水平から
`alpha = atan(150 / 90) = 59.036°`のproduction camera、RtT縦補正、Rough Vector Sketchの
texture-baked line、現行zoom範囲からゲーム内の適正厚を算出する。

縦補正後の正射影は、High / DPI 1.0 / camera scale 1を基準に次となる。

```text
screen_x = world_x
screen_y = -world_z + (Z_OFFSET / VIEW_HEIGHT) * world_y
         = -world_z + 0.6 * world_y
```

従ってstraight E-W壁の全投影高は`0.6H + t`であり、公称厚`t = 9.6`では
`19.2 + 9.6 = 28.8 px = 0.90 tile`になる。装飾込み上限`t_max = 12.8`でも
`32 px = 1.00 tile`で止まり、placeholderの`t = 32`が作る`51.2 px = 1.60 tile`の箱状外形へ戻らない。

厚さの下限は遠景の線＋塗りから決める。High、標準zoom、32 px/tileでtextureへ焼く片側2 px線は、
現行`PanCamera`の最大zoom-out factor 5では両側合計`4 / 5 = 0.8 px`になる。内部色を最低1 px残す条件は
`t / 5 - 0.8 >= 1`、すなわち`t >= 9.0 wu`である。偽精度を避けて0.05 tile単位で上へ丸め、
**`t = 0.30 tile = 9.6 wu`を固定値**とする。`0.25 tile = 8.0 wu`では最大zoom-out時の内部色が
`0.8 px`へ落ちる一方、9.6 wuは`1.12 px`を残す。`tile_rtt_px = 14`でも全幅4.2 px、
縮小後の両側線1.75 pxを除いて2.45 pxの内部色を残す。

| geometry項目 | 固定値 | validator契約 |
| --- | ---: | --- |
| 基準高 | `32 wu = 1.00 tile` | raw local Y min / maxを`-16 / +16 wu`、world Yを`0..32 wu`に固定 |
| 公称・連続最小構造厚 | `9.6 wu = 0.30 tile` | 各armの局所横断方向で中心線`±4.8 wu`のbandを含み、visible bodyを9.6 wu未満にしない |
| 装飾込み局所横断外形 | `<= 12.8 wu = 0.40 tile` | 石の出、鉄バンド、トゲを含め各arm中心線から局所横断`±6.4 wu`以内 |
| 接続port | 幅`9.6 wu`の共通profile | 接続軸のcell境界`±16 wu`で6 family共通。装飾は境界前にprofileへ戻す |
| 公称面からcell端まで | 各側`11.2 wu` | logical footprintではなく視覚余白。通行可能にはしない |
| 全local AABB | X/Zは`[-16, 16]`以内、Yはmin / max `-16 / +16` | 90°回転後もcell外へ出ず、ground接地と高さ基準を変えない |

数値validatorの絶対許容差は`0.01 wu`とし、これはfloat export誤差だけに使う。公称厚や外形を
`±0.01 wu`の美術調整範囲として扱わず、reportには測定最小／最大とfixtureとの差を残す。

厚さはarmの**局所横断方向**で測る。corner / T / crossのjunctionで交差する別armの長さを
「厚さ12.8 wu超」と誤判定しない。各connection axisの中心から境界へ向かう座標を`s`とし、
`8 <= |s| <= 16 wu`をport collarとして、visible bodyが中心線`±4.8 wu`を含むこと、全geometryが
局所横断`±6.4 wu`以内であること、`|s| = 16 wu`で共通port profileへ一致することをslice検査する。
中心側`|s| < 8 wu`はactive arm corridorのunionとcell AABBで検査する。`isolated`はX / Z中心断面の
visible body 9.6 wu以上、局所外形12.8 wu以下を別fixtureにする。

`isolated`は腕なし、`end`以降は接続方向だけ同じ9.6 wu portへ届く。石の欠けや左右非対称は、
各sliceのvisible cross-sectionを9.6 wu未満へ細らせない範囲で外側へ作る。深い欠けはalbedo / normalで表現し、
接続境界のprofile、pivot、height silhouetteを全familyで一致させる。
固定screen-space outline、zoom上限、Camera角度、RtT縦補正、wall LODのいずれかを変更する場合だけ
上式から再算定し、単なる好みのA/Bで厚さを漂流させない。

1. `docs/art-style-criteria.md`と`docs/world_lore.md`をreferenceの正本にし、production cameraの水平から約59度、Orthographicで判断する。
2. 石積み、筆跡、暗茶のwobbly line、錆色、紫裂け目の大部分はshared albedo / emissiveへ焼き込み、top silhouetteに影響する鉄バンドとトゲだけをgeometryにする。
3. 裂け目はemissive surfaceとして読ませるが、周辺を照らすlight entityは作らない。紫以外の明るい暖色を増やさない。
4. 6 meshは各1 mesh / 1 primitive、共通UV atlas、material slot増加なしとする。直線辺の共線分割はsilhouette、surface分類、UV補間を変えないため禁止し、target / hard capは24〜72 trianglesとする。
5. authoring sceneでは1 tileを1 Blender unitとし、高さ1.00、公称・連続最小厚0.30、装飾外形上限0.40、接続port半幅0.15で制作する。export専用objectへ32倍scaleを適用して頂点へbakeし、node transformをidentityにする。direct primitive handleはglTF node transformを使わないため、raw runtime Meshのlocal AABBはX/Zを`[-16, 16]`以内、Y min / maxを`-16 / +16`、originを中心に固定する。runtimeのbase scaleは1のまま、通常transformのY=`TILE_SIZE * 0.5`で底面がgroundに接する。
6. shared textureはまず最大1024×1024のalbedo / emissive 1組とする。variant別textureを増やさず、解像不足がnative画像で証明された場合だけ予算を再決定する。

#### 制作物と配置

| 種別 | staging / canonical | repo runtime mirror |
| --- | --- | --- |
| Blender原本 | `staging/blend/wall-production-v1.blend` → 承認後 `generations/<GEN>/source/blender/buildings/wall-production-v1.blend` | 置かない |
| isolated mesh | `staging/exports/models/wall_isolated.glb` → `generations/<GEN>/exports/models/wall_isolated.glb` | `assets/wall_sets/<GEN>/models/wall_isolated.glb` |
| end mesh | `staging/exports/models/wall_end.glb` → `generations/<GEN>/exports/models/wall_end.glb` | `assets/wall_sets/<GEN>/models/wall_end.glb` |
| straight mesh | `staging/exports/models/wall_straight.glb` → `generations/<GEN>/exports/models/wall_straight.glb` | `assets/wall_sets/<GEN>/models/wall_straight.glb` |
| corner mesh | `staging/exports/models/wall_corner.glb` → `generations/<GEN>/exports/models/wall_corner.glb` | `assets/wall_sets/<GEN>/models/wall_corner.glb` |
| T mesh | `staging/exports/models/wall_t_junction.glb` → `generations/<GEN>/exports/models/wall_t_junction.glb` | `assets/wall_sets/<GEN>/models/wall_t_junction.glb` |
| cross mesh | `staging/exports/models/wall_cross.glb` → `generations/<GEN>/exports/models/wall_cross.glb` | `assets/wall_sets/<GEN>/models/wall_cross.glb` |
| shared textures | `staging/exports/textures/buildings/wall/wall_{albedo,emissive}.png` → 同generationの`exports/`配下 | `assets/wall_sets/<GEN>/textures/buildings/wall/` |
| optional normal | 比較中は`staging/exports/textures/buildings/wall/wall_normal.png`。採用時だけ同generationのcoreへ入れる | 比較中は隔離worktreeへcandidate-onlyで配置。採用時だけ`assets/wall_sets/<GEN>/textures/buildings/wall/wall_normal.png`へ同期 |
| authority | stagingの`reports/wall-production-v1.asset-set.json`をM4で最終再封印し、M6で同じbytesを`generations/<GEN>/manifest/`へ配置。immutable receiptとactive pointerが承認を表す | receiptを`assets/wall_sets/<GEN>/authority/promotion-receipt.json`へcopyし、検証済みsubsetをcanonical JSONの`assets/manifests/wall-production-v1.wallset`へ投影する。後者が通常起動の唯一のmutable authority入力 |
| 検査記録 | `staging/reports/`、承認後 `manifests/` / `licenses/` | hashはnative sidecarにも記録 |

`<ASSET_ROOT>` は `HELL_WORKERS_ASSET_ROOT`、未指定時は `~/Sync/hell-workers-assets` とする。
自動処理はcanonical `source/` / `exports/`へ直接書かない。各GLBは `validate-blend`、
`export-staging-glb`、Khronos validator、bounds / mesh / primitive / triangle / UV検査を通す。

6 GLBとshared textureは個別fileの寄せ集めではなく、`wall-production-v1`という1つのasset setとして
固定する。stagingでは`staging/reports/wall-production-v1.asset-set.json`、承認後はimmutableな
`generations/<GEN>/manifest/wall-production-v1.asset-set.json`をpayload正本とし、次をexact schemaで持たせる。

- `schema_version`、`asset_set_id`、単調増加して再利用しない`asset_set_generation`、geometry contract version。
- production coreのexact relative path / role / bytes / SHA-256。常時は6 mesh＋albedo＋emissiveの8 file、`normal_decision=adopted`時だけnormal roleを加えた9 fileを許し、それ以外の未知fileをcoreへ混ぜない。runtime projectionにも採用normalのpath / role / bytes / hashを必須化する。
- optional A/B集合をcoreと別に列挙し、`normal_decision`を`pending | adopted | rejected`のclosed enumにする。M1の`candidate`だけは`pending`を許し、M4最終generationとM5は`pending`を拒否する。
- source `.blend`、Blender / exporter / Khronos version、collection selector、scene / geometry / Khronos reportのhash、生成に使ったM1 tool commit / tool tree hash、検証対象のruntime subject commit。
- provenance、license、`review_status = candidate | art_approved`、art approval artifact locator。M6のrelease承認はpayload manifestを書き換えず、同manifest hashを参照する別のpromotion receiptで表す。

normal集合の正規化規則は次の1通りに固定する。M1 `pending`はcore 8 file（6 GLB＋albedo＋emissive）と
optional normal 1 file、M4 `adopted`はnormalをcoreへ移した9 fileでoptional集合を空、`rejected`は
core 8 fileかつoptional集合を空にする。rejected normalのpath / hashはfinal asset-set manifestへ残さず、
art review evidenceだけが参照する。M5 / M6は常にこのpendingなしfinal coreだけをprovision / promoteする。

runtime projectionはcanonical manifest hash、core path / role / bytes / hash、normal採否、review status、
asset-set generationと`authority_mode = isolated_candidate | release_approved`を含み、provenanceの秘密や
外部absolute pathを含めない。isolated candidateはreceipt fieldを持たずlauncherのexact identity opt-inを
必須にし、release modeは同じpayload manifest hashを参照するimmutable promotion receipt path / hashを必須にする。
wire formatはUTF-8 canonical JSON（key順固定、余分な空白なし、末尾LF 1つ）とし、projectorの再実行が
byte-identicalであることをtestする。generic `.json` loaderと衝突させず、M2で`.wallset`だけを読む小さな
`WallAssetSetManifest` asset / loaderを追加し、`serde_json`を通常build dependencyへ移す。Bevy 0.19の
`LoadContext::read_asset_bytes`でcore fileを各1回読み、release modeではgeneration-scoped receiptも読み、
bytes / SHA-256、canonical receipt schema、payload manifest hash / asset-set generation bindingをloader dependencyとして
検証してからauthority候補を生成する。manifestの存在や自己申告hashだけをresident証拠にはしない。
通常起動は`art_approved` payloadとM6の有効なpromotion receiptから投影された`release_authority=approved`
だけを受理し、隔離profileはlauncherが封印したcandidate / art-approved hashとasset-set generationだけを
authorityとして受理する。native sidecarは実際に読んだasset IDとprojection、payload manifest、receiptの
path / hash / bytesを突き合わせ、asset set内の取り違えをfail-closedにする。

promotion receiptもclosed schemaとし、`schema_version`、一意で別payloadへ再利用できない`receipt_id`、
asset-set ID / generation / manifest hash、sealed promotion plan hash、M5 evidence bundle hash、approval artifact
locator / hash / UTC timestamp、previous / new active pointer identity、生成tool commit / tree hashを持たせる。
validatorはmissing field、別generation / manifest、plan hash不一致、承認時preimageと異なるstale pointer、
別plan / payloadへのreceipt ID再利用を拒否する。runtime projectionとrepo copyはcanonical immutable receiptの
同一bytes / hashだけを参照し、自己申告の`approved`文字列だけではauthorityにしない。

#### 1原本から6 GLBを安全にexportする契約

- `wall-production-v1.blend`は6つのnamed collectionを持ち、各collectionにexport対象のrenderable meshを1つだけ置く。
- 現行`export_glb.py`はscene全体をexportするため、そのまま6回呼ばない。M1で後方互換な`--collection <exact-name>` selectorを`validate-blend` / `export-staging-glb`へ追加し、検査とexportの両方を同じ選択集合へ限定する。
- 追加後のCLIは`validate-blend --collection <exact-name> <input.blend> <report.json> [max-triangles]`と`export-staging-glb --collection <exact-name> <input.blend> <output.glb> [max-triangles]`に固定する。`--collection`なしの現行位置引数形式も同じ意味で残す。
- selectorはunknown、空、複数renderable mesh、非identity node transform、未適用scale、2 primitive以上をfail-closedにする。Blender 5.1.1のoperator引数は実行環境のPython introspectionで確認してから実装する。
- runtimeはGLB materialを使わないため、各GLBにshared PNGを重複embedしない。geometry-onlyまたはplaceholder material exportの正確な設定を一次APIで確認し、GLB内embedded image 0を構造gateにする。
- pre-export scene validatorとpost-export GLB validatorを分ける。前者は選択collectionのauthoring state、後者は生成物そのものの1 mesh / 1 primitive、node identity、raw accessor / decoded vertex bounds、UV / tangent、embedded image 0、triangle、port collar / junction / isolated geometryを検査する。scene側の`dimensions`をGLB raw AABBの代理にしない。
- no-selectorの既存workflow smokeを壊さず、selectorごとのunit / integration testと6 output hash reportを追加する。unknown / empty / multi-mesh、non-identity node、2 primitive、embedded image、bounds / port違反をnegative fixtureとして全件failさせる。

#### canonical承認前の隔離runtime検証worktree

M2〜M5はstaging候補をゲームで読む必要があるが、`docs/blender-setup.md`の隔離契約に従い、
承認前候補をprimary worktreeの`assets/`へ書かない。formal native profileが`--repo`配下の
`assets/`をfingerprintする現行契約に合わせ、次の**一つのclean validation worktree**へasset viewを組み立てる。

1. ユーザーが承認したscoped local commitから、repositoryの`target/`配下ではない一時directoryへclean validation worktreeを作る。code subjectの`git status --porcelain`は空を必須とする。
2. checkout済みのtracked WGSLを保持したまま、既知良好なignored runtime asset mirrorをvalidation worktreeの`assets/`へmanifest / SHA-256付きで複製する。primary `assets/`を使う場合もread-only sourceとし、primary側へ逆同期しない。
3. `--dest`はpayload directoryでなくasset rootを受け、manifest modeが`wall_sets/<GEN>/...`と`manifests/...`を決定する。`scripts/sync_external_assets.py --source "$ASSET_ROOT/staging/exports" --dest "$VALIDATION_WORKTREE/assets" --manifest "$ASSET_ROOT/staging/reports/wall-production-v1.asset-set.json" --selection core --dry-run`をreviewしてから、同じ引数から`--dry-run`だけを外してoverlayする。manifest外のstaging file、optional normal、無関係assetをcopyせず、tracked WGSLのhashがsubject commitと一致することを再確認する。
4. 比較用normalは同じmanifestの`--selection optional:normal`で別途candidate-only配置し、core asset setと別hashにする。optional-only実行はcore runtime projection / active pointerを書き換えない。採用まではproduction readiness集合から除外し、raw staging treeの無条件copyや手作業copyを許さない。
5. full asset-view hash、wall asset-set hash、non-wall asset-view hashを別々に固定する。candidate payloadを変更したら`asset_set_generation`と3 hashを更新し、旧generationの画像、performance、sidecarを再利用しない。
6. M2〜M3のasset非依存testはprimaryで行い、実候補を必要とする主観A/B / GPU検証はvalidation worktreeだけで行う。ignored assetの追加は許すが、tracked fileの差分、symlink経由の可変asset view、primary / canonicalへの書込みがあればformal runを開始しない。
7. M4のA/B harnessとfail-closed profileを完成させた後、対象diffをユーザーへ提示してscoped local harness commitの明示承認を得る。A/B採用後はprimaryでart比較toggle / debug material / 不採用normal経路を撤去し、final subjectのscoped local commitについて二度目の明示承認を得る。commit / push / canonical promoteの承認はそれぞれ分離する。
8. M5はfinal commitから作った新しいclean validation worktreeへ、手順4のA/B normal overlayを行わず`--selection core`だけで固定asset viewを再構築する。adoptedならcore 9、rejectedならcore 8であり、final manifest外のnormalが存在すれば開始しない。P02 / wall-art launcherはclean code subjectと3 hashを固定し、M6のユーザー承認まではcanonical assetにもprimary `assets/`にも書かない。

### 4.2 16接続maskから6 meshへの決定的写像

接続順を `(N, S, W, E)` とする。`N/S`はworld gridの`y + 1 / y - 1`、`W/E`は
`x - 1 / x + 1`であり、3Dでは既存の `2D +y = 3D -z` 変換後にY軸回転へ変換する。
GLBのcanonical forwardとquarter turnの符号はM0でfixture画像とunit testに固定し、コード内へ
散在させない。

| `(N,S,W,E)` | family | 向き |
| --- | --- | --- |
| `0000` | isolated | 回転なし。cell中心だけで完結 |
| `1000`, `0100`, `0010`, `0001` | end | canonical Nから接続方向N / S / W / Eへquarter turn |
| `1100` | straight | N-S |
| `0011` | straight | E-W |
| `1010`, `1001`, `0110`, `0101` | corner | NW / NE / SW / SEへquarter turn |
| `1110` | t_junction | Eがopen |
| `1101` | t_junction | Wがopen |
| `1011` | t_junction | Sがopen |
| `0111` | t_junction | Nがopen |
| `1111` | cross | canonical |

- `end`はcell中心から接続側境界までの腕だけを持ち、存在しない反対側へ伸ばさない。`isolated`は接続腕を持たない。これにより16 maskの意味を形状で保持する。
- `end` / `straight` / `corner` / `t_junction` / `cross`の各接続腕はcell境界まで届き、幅9.6 wuの同一port profileになる。連続壁体は9.6 wu未満へ細らせず、装飾は境界前に12.8 wu envelopeからportへ戻し、隣接Wall meshと隙間や重なりを作らない。
- 接続対象は完成／仮設Wall、Wall blueprint、Door、Door blueprintであり、DoorStateのOpen / Closed / Lockedでは接続有無を変えない。
- Door隣接でもWall側portは同じ9.6 wuとするが、現行5.76 wu厚leafとの段差やcell内のjamb gapをWall meshの越境で埋めない。Wall–Doorは接続maskと中心線を本計画で保証し、無隙間のframe / jambはDoor本番assetの契約にする。
- pure resolverは `WallConnectionMask -> WallMeshFamily + QuarterTurns` を返し、3Dと既存2D image選択が同じmask計算を使う。2Dの個別texture名を3D familyへ逆流させない。

### 4.3 Bevy 0.19での読込とfallback

- GLB sceneをspawnせず、`GltfAssetLabel::Primitive { mesh: 0, primitive: 0 }.from_asset(path)`で `Handle<Mesh>` を直接取得する。これによりone wall = one `Mesh3d`、shared material、prepass、`MeshTag`の既存契約を保つ。
- `Building3dHandles`のwall部分を、procedural fallback mesh＋solid-color完成／仮設pair、6 production mesh＋textured完成／仮設pair、asset readinessを表す専用有限poolへ分離する。非wall handleは変更しない。
- assetの不変identityである`asset_set_generation + manifest_hash`と、process-localな表示遷移である`session_id + activation_revision`を分離する。readinessと表示有効化を一つのboolへ潰さず、`Fallback { reason, asset_set_identity, activation_revision }`、`Eligible { asset_set_identity, activation_revision }`、`ProductionActive { asset_set_identity, activation_revision }`相当のaggregate stateで表す。状態discriminantまたはactivation revisionが変わるごとに全Wallを一度だけrefreshし、同一sessionのsteady stateではmesh / material writeを0にする。
- 起動中は6 production meshと採用済みshared texture集合のload stateを一括監視する。normal採用前のproduction core集合は6 mesh＋albedo＋emissiveであり、A/B用normalは別のcandidate-only集合として扱う。normal採用時だけrequired集合へ加える。全件CPU-readyでもM2では`Eligible`までとし、topology resolver / presentation consumerが未導入の通常ゲームへproduction meshを出さない。
- `ProductionActive`へ進めるauthorityは、通常起動では承認済みruntime projection＋promotion receipt、隔離profileでは明示されたcandidate manifest hash / asset-set generationに限定する。staging fileがたまたまignored `assets/`へ存在するだけでは有効化しない。さらに`topology_ready`とrequired asset setの全件readyを同じactivation transitionで満たした時だけ、M3のpresentation applyが既存／同frame spawnを一括置換する。
- missing / failed / unauthorized時は全Wallをfallbackへ揃え、path、load state、manifest hash、reasonを`session_id + activation_revision`ごとに一度だけ診断する。現Cargo featureにはfile watcherがないため、通常buildで「欠落fileを後から置けば自動回復する」とは約束しない。欠落／failedからの回復は同じasset-set identityを読むfresh process restartで検証し、restart自体は新しいasset generationをmintしない。focused testはfresh App再起動とinjectable load-state seamで`Failed -> restart -> Eligible`、`ProductionActive -> synthetic failure -> Fallback`を検証し、player-facing hot reloadや無制限retryは追加しない。
- albedo / emissiveはGLB materialをscene経由で取り込まず、repo catalogのshared `Handle<Image>`から `TopDownStructuralMaterial` baseへ明示接続する。
- 実装時はBevy 0.19のlocal sourceまたはdocsrs-mcpでload-state APIと `GltfAssetLabel` signatureを再確認し、旧版APIを推測で使わない。
- CPU load stateをGPU prepare / actual renderingの証拠にしない。GPU-readyはactual-windowのresident sidecarとclient pixel predicatesで閉じ、missing / late albedo、missing / late emissive、ready切替と同frameのwall spawnをfocused testに含める。

### 4.4 materialと平面イラスト判定

- material型、render layer、prepass、depth、shadow castingは現行 `TopDownStructuralMaterial`を維持する。
- 第一候補は、flat normal、roughness 1、reflectance 0、texture-baked linework、現行stylized directional shadowである。
- lit第一候補は`base.emissive`へ非黒の有限shared multiplierを設定し、`emissive_texture`と乗算して紫裂け目を出す。multiplier、露出、texture color spaceはM0で1つのcandidate値に固定し、entityごとに変えない。
- 比較候補は同じmaterial型の `StandardMaterial::unlit` flagだけを切り替える。現行custom fragmentのunlit分岐はemissiveを加算せずbase colorだけを返すため、比較用albedo自体にも読める紫裂け目を持たせ、unlitを「非emissive control」として評価する。unlitが平面感では勝つが発光要件を満たさない場合、本計画で共有shaderを場当たり的に変更せず別のwall material / shader提案へ送る。
- unlit候補もdepth、wallからterrainへのshadow、Soul前後関係、completion bounceを実機で満たさなければ採用しない。
- normal mapは「なし」と「同一albedoに対応する1枚」を同じcamera / quality / DPIで一度だけ比較する。比較前のtechnical gateとして、6 mesh全ての`Mesh::ATTRIBUTE_TANGENT`、UV0、normal画像の`ImageLoaderSettings::is_srgb = false`、Blender / glTFのOpenGL `+Y`規約に対応する`flip_normal_map_y = false`を検査し、非対称な既知normal patchで照明方向まで確認する。gate不合格を「見た目の差なし」に数えない。石の立体感より3D感やspecular noiseが強まる、またはtechnical gate通過後も差が読めない場合はなしで終了する。
- geometry-only GLBのplaceholder materialにnormal textureがなければ、Bevy 0.19のglTF loaderは欠落tangentを自動生成しない。M1のpost-export reportでtangent有無を記録し、normal candidateで不足する場合だけM4 harnessが6つのresident `Mesh`へ`Mesh::generate_tangents()`を一度だけ適用してからcandidate-readyにする。生成失敗、UV0欠落、wall entityごとの生成は失格とする。normal不採用時はこのcandidate-only生成経路をfinal subjectから撤去する。
- outlineが不足した場合は、texture線幅／色を延々と調整しない。baked lineworkではsilhouetteが成立しないという結果を記録し、global outlineの別提案へ送る。
- 仮設と完成は同一mesh / topology / UVを使い、完成時はmaterial handleだけを交換する。仮設は既存amber / alphaの意味を維持し、紫emissiveを弱めるか無効にする値を有限poolで固定する。

### 4.5 topology lifecycleとtransform ownership

- `hw_visual`はmask、family、quarter turn、bidirectional connector indexとpure resolverを所有し、`bevy_app`はasset pool / readinessとproduction `Mesh3d`適用を所有する。`hw_visual`から`bevy_app`型へ依存させない。
- dirty resourceをdrainするproducerは一つだけにし、transientなlogical-root topology componentまたは同等のimmutable change setを生成する。2D Sprite consumerと3D consumerが別々にdirtyをtakeして更新を奪い合う構成は禁止する。
- 通常frameのreadiness / topology resolve / presentation applyは`PostUpdate`へ置き、`Update`の`GameSystemSet::Interface`内`PlacementFeedbackSet::Commit`を含むwriterが完了した後に、`WallAssetReadinessSet -> WallTopologyResolveSet -> ApplyDeferred -> WallPresentationApplySet`の順で実行する。wall final transform compositionはapplyへ統合し、このchain全体をBevy 0.19の`TransformSystems::Propagate`より前へ明示配置して同frameの`GlobalTransform`までtestする。別scheduleのsetへ見かけ上の`.after(...)`依存を書かない。
- 3D visualへ `Wall3dPresentationState { family, quarter_turns }` 相当のcomponentを持たせ、`Mesh3d`選択と回転を同じstateから導出する。
- owner transform、wall topology quarter turn、completion bounce scaleを一つのpure presentation transformで合成する。topology systemから `Transform`を場当たり的に上書きしない。
- `WallConnectionDirty`を、dirty grid、`entity -> set<grid>`、`grid -> connector contributors / presentation targets`を保持するbidirectional indexへ拡張する。Added / Changed / Removedを旧集合と新集合の両方でself＋4近傍へ展開し、毎frameのworld全走査は行わない。
- indexはpresentation用の派生projectionであり、WorldMap、Building、Door、construction stateに対する第二のgameplay正本にはしない。contributor identityごとのgrid集合と同一gridのref-count / targetを保持し、一時共存をcoalesceできる最小fieldだけに限定する。
- connector snapshotは通常の `BuildingVisualState` / `BlueprintVisualState`だけでなく、normal wall placementの各 `WallTileVisualMirror`＋Transformも収集する。`WorldMap`が複数gridを単一 `WallConstructionSite`へ向けることだけからtile visual targetを推論しない。
- resolverの`is_connector(grid)`はこのcanonical connector indexを読み、`WorldMap::building_entity(grid)`だけを正本にしない。WorldMapは論理occupancyとの不一致を検出するvalidation inputとして使う。
- `WaitingWood`からframing中までの `WallTileBlueprint`は2D blueprint接続には参加するがproduction GLBを持たない。`FramedProvisional`でgridごとのspawned Wallへpresentation ownerを原子的に渡し、同じgridのsite / tile / spawned wall contributorは接続数1としてcoalesceする。
- 次のwriter経路を同じdirty transactionへ接続する。
  - generic Wall / Door blueprintと、複数tile `WallConstructionSite`配下の `WallTileBlueprint`追加、状態／位置変更、cancel。
  - 通常施工の仮設Wall生成、完成material遷移。
  - Instant Build / debug build。
  - Wall / Doorのdeconstructionとdespawn。
  - save/load rehydrateとrollback後のpresentation再構築。
  - native / perf fixtureの直接spawn。
- 仮設→完成は接続maskを変えずmaterialだけを変える。DoorState変更もmaskを変えない。Wall / Door / connector blueprint tileの追加・撤去・grid集合変更だけが隣接形状をdirtyにする。
- visual lookupはframe内に `owner -> visual` mapを一度構築するか、既存cacheを安全に拡張してO(walls × dirty)走査を避ける。cacheを導入する場合はcleanup / reset / rehydrate testを同じマイルストーンに含める。
- aggregate stateのdiscriminantまたは`activation_revision`が変わった時だけ全Wallを一度refreshしてfallback / productionを一括切替し、通常steady stateでは全走査もmesh writeも行わない。asset証拠とartifact照合には不変のasset-set identity、実行時refresh判定にはsession / activation identityを使い回さない。
- world replacement reset hookの所有をcrate境界で分ける。`hw_visual::reset_for_world_replace`はbidirectional index、last-known grid、dirty queue / topology revision、resolved presentation cacheをclearしてfull-rebuild requestを立てるだけにする。`bevy_app`所有の別hook `reset_wall_asset_presentation_for_world_replace`がaggregate modeを`Fallback(WorldReplace)`へ落としてactivation revisionを進める。両hookはreset phase内で完了してから`presentation.shells` rehydrateを開始し、leaf crateからroot-owned stateを参照しない。
- world replacement自体は`Last::SaveLoadApplySet`で`PostUpdate`より後に起きるため、rehydrateがspawnする全Wallはそのframeではfallbackに統一する。parentを持たないroot wall visualのcreation-time bundleへfallback `Mesh3d` / material / composed local `Transform`と、そのlocal値から作った`GlobalTransform`を同時挿入し、伝播済みの`GlobalTransform`既定値を表示しない。次frameの`PostUpdate`でmirrorからindexを一度だけfull rebuildし、全cell resolve完了後にassetが`Eligible`なら一括で`ProductionActive`へ戻す。fallback-only 1 frameは許容するがmixed表示は許容せず、Last frameのworld位置を含めnormal load / rollback / recovery-only / 連続2 resetで固定する。
- spawn / rehydrateにはcreation-time fallback bundleとowner由来`MeshTag`の初期化だけを許可する。spawn後のproduction handle選択と`Mesh3d` / wall material / composed `Transform` / owner由来`MeshTag`のmutationは`WallPresentationApplySet`だけが行う。現行のprovisional material syncはlogical dirty / state producerへ変え、generic transform / tag syncからWallを除外する。`MeshTag`はowner grid / owner rotationから導出し、topology quarter turnを混ぜない。これにより「唯一writer」はcreationとmutationを混同せず検査できる。

## 5. マイルストーン

```text
M0 contract / baseline / blocker確認
  -> M1 staging asset制作・構造gate
       -> M2 runtime load / material / Eligible（fallback維持）
            -> M3 topology / lifecycle統合・ProductionActive化
                 -> M4 art A/B・ユーザー承認
                      -> M5 native回帰・性能gate
                           -> M6 canonical昇格・docs・計画close
```

M0→M1→M2は順番に閉じ、asset / toolingとruntimeを別の編集agentや並行sessionへ分けない。
M4の色承認、M5のproduction合格、M6の昇格はOCIO解消後に限る。

| MS | 入力gate | 凍結する出力 | 変更時に無効化する後段 |
| --- | --- | --- | --- |
| M0 | current code / asset view、OCIO調査環境 | orientation / geometry fixture、color verifier、P02 baseline、`wall-density-v1`計測contract | fixture / color / 計測schema変更でM0 artifactを再採取 |
| M1 | M0 fixture | 6 GLB、texture、`normal=pending` candidate generation、全validator report、承認済みtool commit | geometry / texture / tool変更でM1以降。M4の計画済み最終再封印はM1 validatorとM2 / M3 focused gateを再実行し、それ以外のmanifest差替えはM1以降を無効化 |
| M2 | M1 manifest候補 | finite pool、aggregate readiness、`Eligible` activation。通常表示はfallback | required集合 / material / readiness変更でM2以降 |
| M3 | M2 `Eligible` test pool | 16 mask resolver、index、atomic `ProductionActive` apply、world-replace契約 | resolver / schedule / lifecycle変更でM3以降 |
| M4 | M3 production route、OCIO pass | lit / unlit、normal、outlineの採否、final code commit、final asset-set identity | code / shader / asset / profile変更でM4 artifact以降 |
| M5 | M4 final commit / asset-set identity | final 9 case、P02、Capture、RenderDocのsealed evidence | source / asset / harness fingerprint変更でM5全件 |
| M6 | M5 pass＋ユーザーpromote承認 | immutable canonical / repo generation、active authority、恒久docs、Help decision | 中断時はrecover、問題時は検証済みpointerへrollbackしてM6を再実行 |

codeまたはruntime dataを変更した各マイルストーンでは、完了報告やlocal commitより前に
`hell-workers-review-help-impact` Skillで実際のplayer-visible pathを確認する。後段で見え方や操作を
さらに変えた場合は前回の`No impact`を流用せず、次のcommit前に再実行する。

### M0: baselineと契約を凍結

実装状況（2026-09-01）:

- geometry / density / colorのmachine-readable fixture、CIEDE2000 offline verifier、
  Blender calibration renderer、既知vectorとfail-closed metadataのunit testを実装済み。geometry fixtureは
  `maximum_cell_envelope`、raw vertical bounds、中心pivot、identity node、placement centerを別fieldへ分離し、
  JSON SHA-256へ結合したcanonical上面／側面SVGを追加した。unit testは+Y正回転のN→Wを起点に16 maskを
  全件再導出し、6 familyのarm / mask、SVG metadata、local Y `-16..+16`からworld Y `0..32`への配置を検証する。
- 現行Flatpakの同梱config 2.5 / runtime 2.4.2を`fallback=true`として検出した後、壁校正専用の
  `wall-calibration-v2.ocio`（profile 2.1、Linear scene reference、exact sRGB transfer）を追加した。
  runtime validation pass、明示config / active config cache ID一致、`fallback=false`をdiagnostic実機で確認した。
  同時にBlender `Camera.ortho_scale`を画像height 96としていたため横長320 px boardの中央1 patchしか視野へ
  入らない経路不成立を検出し、horizontal span 320へ修正した。修正後5 ROI medianはstone `(33,27,27)`、
  rust `(140,74,47)`、dark brown `(26,10,0)`、purple `(139,0,139)`、emissive `(190,0,190)`である。
  dirty diagnosticは正式証跡へ流用せず、clean final subjectからBlender referenceとBevy candidateを再採取する。
- Rustの`wall-density` profiling workloadを実装済み。Small=N=96 / Medium=4N=384、
  completed / provisional、20列・5 cell stride、16 mask、Door blueprint connector、camera scale 5を
  production wall spawn / WorldMap予約経路で構築し、embedded contract hash、phase別layout checksum、
  target / connector全行をfail-closed sidecarへ出す。profiling feature付きtest / Clippyはpass。
- `perf.py`へwall phase / exact matrix / raw sidecar再検証を統合し、専用
  `wall_density_acceptance.py`のCapture laneを実装済み。clean commit、全asset view、source / harness、
  binary、N / 4N×completed / provisional×3 run、p95 / p99中央値・MADをfail-closedで封印する。
  dirty subjectではdirect `kitty`計画をblockすることまでself-test / dry-runで確認済み。
- subject `26dcb5a3`のclean commitから、専用native launcherでN / 4N×completed / provisional×3の
  全12 runを再採取し、独立verifyをpassした。Capture profileは`draw_groups=not-collected`を明記し、
  wall固有RenderDoc gateとは分離する。
- current source専用Bevy actual-window calibration profileをsubject `ad614903`から採取し、
  `target/native-acceptance/wall-art-20260901T055138Z-f93b0b12`の独立verifyがpassした。対象は
  `wall-density-v1`のisolated Wall ordinal 64 / grid `(22, 17)` / mask `0000`、ROIは
  `(416, 518, 96, 96)`で固定UI chromeと非重複である。fallback mesh resident、exact owner、High / DPI 1.0、
  Vulkan / X11、camera scale 5、raw performance sidecarとのlayout checksum一致を封印した。
  current wallはhistorical P02の代用にしない。
- wall専用runtime checkpoint、qrenderdoc extractor、`wall_renderdoc_acceptance.py`を実装し、subject
  `e149918c`からcompleted / provisional各N / 4Nの4 RDCを採取した。各RDCを2回replayした結果は
  byte-identicalで、completed / provisionalとも`D_N=1 / D_4N=1`、rendered instanceはN=96 / 4N=384で
  checkpointed owner全件と一致した。Wall main passは中間color＋depthへ一括描画し、次のfullscreen triangleが
  `hell-workers-rtt-scene`へ合成する実render graphを証拠化したため、最終Scene targetへの直接writeをdraw identityへ
  要求しない。wall固有draw-group gateは完了した。
  残項目の実装でsource / harness closed setが変わる場合は、M0最終baseline commitからCapture 12 runと
  RenderDoc 4 caseを再採取し、途中commitの成功artifactをM5比較baselineへ流用しない。
- `wall-reference-locators-v1`を実装し、登録済みhistorical P02
  `baseline-index.json#/stages/p02`（subject `6ea0bf99` / attempt
  `54d85a63-e237-4501-a0d0-33c1d0a29f3b`）とcurrent fallback壁
  `wall-art-20260901T055138Z-f93b0b12`のauthority / role / 用途外範囲を分離した。offline verifierは
  baseline index、SHA256SUMS、attempt manifest、RenderDoc manifest / checkpoint / extraction / RDC、
  current manifest / observation / PNGの個別hashとidentityを再計算してpassした。historical P02は
  current wall pixels / current source stateへ、current fallback壁はhistorical P02性能 / production art承認へ
  使用できない。現在のP02 semantic validatorで過去artifactを現行matrixとして再解釈せず、登録時のimmutable
  locator整合だけを証明する。
- canonical orientation / boundsの曖昧さを解消した。`local_min/max`は全familyが必ず満たす実AABBではなく
  X/Z `-16..+16 wu`の最大cell envelope、各raw meshのvertical boundsだけをY `-16..+16 wu`へ固定する。
  raw originはcell center / half-height `(0,0,0)`、node T/R/Sはidentity、runtime placement center Yは16 wuとし、
  N=-Z / E=+X、+Yの+90°は上面視counterclockwiseでN→Wと固定した。canonical 6 familyから16 mask mappingを
  数式で再導出するunit testと、同じcontract hashを持つ`wall-production-v1.orientation.svg`がpassした。
- profiling-onlyのBevy 5 patch boardと`wall_color_acceptance.py`を実装した。最初の正式試行
  `wall-color-20260901T080247Z-4a4d4ccc`は実X11 client captureとperformance sidecarまで成立したが、
  highest-orderの専用cameraがBevy 0.19のdefault UI cameraになり、Pause UIが中央3 patchへ重なって
  平均Delta E 2000 `16.092317`でfail-closedになった。このartifactは不採用とし、色値を変えず既存
  MainCameraへ`IsDefaultUiCamera`を明示してUI隔離をstatus contractへ追加した。
- final subject `35f1f6e3` / source fingerprint
  `8b996922b015fe96786d01eddcdc39c467fae6341e65a64967cf5917a509d813`からBlender referenceとBevy
  actual-windowを再採取した。`target/native-acceptance/wall-color-20260901T081824Z-e92a7d9c`は独立verifyで
  passし、stone `(33,27,27)`、rust `(140,74,47)`、dark brown `(26,10,0)`、purple `(139,0,139)`、
  emissive `(190,0,190)`、4 base patchの個別／平均Delta E 2000 `0.0`、emissive luminance lift
  `0.07313`を封印した。manifest SHA-256は
  `79f396735813de9b8c8e9f7e76b6c515ba7416d9f0c71d5efe17e05b246775c4`である。
- 最初の正式Capture試行（subject `a69ea3df`）はfail-closedで棄却した。Smallのraw 3 runは取得できたが、
  仮想時刻を停止するwall-densityへvalidatorがvirtual 30 s / 60 sを誤要求した。Mediumは通常worldの
  初期facilityが固定fixture cell `(82, 57)`を先に予約して起動時に失敗した。M0 contractを凍結する前に、
  durationはreal clockを検証しvirtual 0を要求するよう修正し、wall-densityではterrain以外の通常初期
  resource / facility / regrowth targetを生成しない隔離経路へ変更した。失敗artifactはbaselineへ流用せず、
  修正commitから全12 runを再採取する。
- 修正subject `60328e60`の再試行ではcompleted N / 4N各3 runがpassし、上記2件の再発なしを確認した。
  provisional N / 4N各3 runはrawを完走したが、仮設wallだけがspecializeするprepassでBevy 0.19の
  `PREPASS_FRAGMENT`未定義variantにもfragment entry pointを宣言し、gated
  `prepass_io::FragmentOutput`を参照するshader compile errorを6 runすべてで検出した。このartifactも
  baselineへ流用せず、Bevy 0.19標準`pbr_prepass.wgsl`と同じfragment guardへ修正した次subjectから
  全12 runを再採取する。
- subject `749118d3`ではcompleted N / 4N各3 runが再度passしたが、provisionalは6 runとも起動時に
  depth-only `MAY_DISCARD` pipelineがfragment stageを要求し、guardの`else`側にentry pointがない
  Validation RenderErrorでfail-closedになった。Bevy 0.19標準`pbr_prepass.wgsl`の正本には
  `PREPASS_FRAGMENT`の`else`として返値なしのalpha-discard fragmentがあるため、同じ二分岐を採用し、
  両方でconstruction height discardを維持する。単純guardだけの仮説はこの一回で打ち切り、このartifactも
  baselineへ流用しない。
- subject `26dcb5a3`の正式Captureは
  `target/native-acceptance/wall-density-20260901T042252Z-36b45916`で`valid`となり、独立verifyも
  `capture_runs=12 / status=pass`を返した。source fingerprint
  `026921561a08112d731a30f607a2bbf3eb71f4b8306a45c991e2c18463e8a97a`、harness fingerprint
  `0352f97a9b49cb967fcfbeca330db839ea0194e42293a02c20e8cb4728ae6b93`、asset-view fingerprint
  `ee5acdc8214b61898f662f2fb502f07559309d322fb5e234d37159b63aed7643`を封印した。3 valid runの
  p95 / p99中央値（MAD）は、completed N=`8.683672 (0.034590) / 9.378522 (0.022487)` ms、
  completed 4N=`26.202343 (3.433323) / 40.219051 (1.202868)` ms、provisional
  N=`10.629657 (0.887287) / 13.332263 (1.016646)` ms、provisional
  4N=`10.495838 (1.323651) / 13.341914 (1.645682)` ms。全runのinitial / warm-up / measure-end
  checksumはcase内で一致し、teardown warningは0だった。このartifactはCapture baselineとして使用できるが、
  draw predicateは未検証なのでRenderDoc baselineを兼ねない。
- final subject `35f1f6e3`からCapture 12 runを再採取した。
  `target/native-acceptance/wall-density-20260901T083037Z-e5dced2c`は独立verifyで
  `capture_runs=12 / status=pass`、teardown warning全件0。p95 / p99中央値（MAD）はcompleted
  N=`17.992724 (0.098033) / 18.895598 (0.029325)` ms、4N=`17.869519 (0.024907) /
  18.765569 (0.034324)` ms、provisional N=`17.917287 (0.049389) / 18.933389 (0.157751)` ms、
  4N=`17.971411 (0.016872) / 18.940292 (0.023251)` ms。manifest SHA-256は
  `d52f9ca63344c546a47abf7ebfde2505797a9adb1e22b3799c98edd93647897e`、binary SHA-256は
  `ced21d607e1f43885ebb9de4a964f4e0deb705c5571a457b4817b1c1a410ab3c`であり、M5比較のM0 baselineとする。
- subject `1c136ca5`からhistorical P02 v9を再実行した最初のcaseは、P02時点で存在したlegacy structural
  Door spriteのOpen画像handleを要求してfail-closedになった。Rust側だけ旧画像条件を除去したsubject
  `f2699e9d`ではその検査を越えたが、Pythonのfrozen P02期待表がDoor / Tank / MudMixerのlegacy child Spriteを
  正しく要求して停止した。これはcurrent P08 sourceをhistorical P02 subjectとして採取し直せないことの証拠であり、
  frozen contractを現在値へ書き換えない。Rustの片側緩和は復元し、current wall visual referenceは既存の
  registered P02 artifactと、current source専用のwall calibration profileを別artifactとして扱う。invalid job
  `target/native-acceptance/p02-presentation-20260901T044343Z-344a5e7b`と
  `target/native-acceptance/p02-presentation-20260901T044925Z-ec7236c5`は合格証跡へ流用しない。
- `wall-art-current-calibration-v1`を実装し、High / DPI 1.0 / Vulkan / X11のcurrent fallback Wallを
  `wall-density-v1`のproduction spawnから1枚だけ採取する契約を追加した。game-owned probeはfallback mesh resident、
  exact owner、fixture / contract hash、Camera3dRtt投影ROI、window / quality / camera scaleをnonce / generation付きで
  publishし、launcherが該当X11 clientだけをPNG化してACKする。raw performance sidecarとPNGのlayout checksum、
  clean subject、source / harness / binary / asset-view hashをoffline verifyで再計算する。これはcurrent visual
  referenceであり、historical P02、12-run Capture、RenderDoc draw-group証跡の代用にはしない。
  single-case 10秒warm-up / 10秒measureはPythonの専用selectorとRustの専用flag＋環境変数の二重鍵でのみ
  許可し、正式density laneの30秒 / 60秒契約は緩和しない。対象ROIは固定UI chromeと重ならない領域へ
  完全に収まらなければRust / Pythonの双方でfail-closedにする。
- final subject `35f1f6e3`からRenderDoc 4 caseを再採取した。
  `target/native-acceptance/wall-renderdoc-20260901T084945Z-4d218c02`は独立verifyで`cases=4 / status=pass`。
  completed / provisionalとも`D_N=1 / D_4N=1`で全predicateを満たし、各RDCの2 replayが一致した。
  manifest SHA-256は`59896f585581d852ce2187d19375b1277d27c1e5dd877da360500da486c275bc`、
  RenderDoc binary SHA-256は`4f4eeb4bdee19c06d6cae3839ff855c46442bc3737e09399186e809e4e8a22e6`である。
  color / Capture / RenderDocの3 artifactは同じsource / harness / asset-view fingerprintへ結合され、M0を完了する。

- 変更内容:
  - current `Cuboid` wallをproduction cameraで撮影し、source fingerprint、camera、quality、DPI、adapter、asset viewを記録する。既存P02 visual referenceと、後述の`wall-density-v1`性能baselineを別artifactにする。
  - 6 GLBのcanonical orientation、pivot、raw primitive-local bounds、texture budget、triangle target、16 mask表をfixture test dataとして固定する。公称・連続最小厚9.6 wu、局所横断装飾外形12.8 wu以下、8〜16 wu port collar、境界port共通profileをmachine-readable geometry fixtureへ含める。
  - OCIO差異の解消方法を別環境追加／Flatpak修正／互換configのいずれかで決め、Blenderとruntimeの同一color patchで検証する。Blender側artifactは実際にloadしたconfig path / SHA-256、OCIO runtime version、`fallback=false`の陽性証明を必須にし、4色が偶然近いだけではgateを閉じない。
  - calibration-only Blender renderとBevy actual-window phase、およびoffline color verifierを先に実装する。自動露出を無効化し、view transform、look、tonemapping、exposure、gamma、出力解像度、patch入力値、base / emissive経路を固定する。stone / rust / dark-brown line / purpleの4色は同じunlit base-color経路で比較し、purple emissiveのlit sanity patchは別predicateにしてDelta Eへ混ぜない。
  - rescale / JPEG化していない8-bit sRGB PNGのpatch中央16×16 px medianをD65 CIE Labへ変換して`Delta E 2000`を比較する。4 base patch平均`<= 2.0`かつ各patch`<= 3.0`を色再現gateとし、lit wall本体の美術判断とは分離する。offline verifierは既知CIEDE2000 vectorのunit testを持ち、artifactのPNG / ROI hash、各median、各Delta E、平均、全color metadataを再計算する。環境由来で閾値変更が必要なら候補閲覧前に根拠と新値を記録し、ユーザー承認なしに緩和しない。
  - lit/unlit、normalなし/ありの比較仮説、合格観察、変化がない場合の打切りを記録する。normal比較は6 meshのtangent / UV0、linear image load、`+Y` / `flip_normal_map_y`、既知方向patchをtechnical prerequisiteとして固定する。
  - current wall routeだけを使う`wall-density-v1` fixtureと専用`wall_density_acceptance.py`をproduction asset実装前に作る。測定targetは`N=96` / `4N=384` Wall ownerとし、16 maskをそれぞれ6回／24回ずつ、互いに接続しないspecimenへ配置する。近傍maskはfixture-owned Door blueprint connectorで作り、target外の3D wallを増やさない。座標列、support数、owner数、mask / family分布、fixture checksumをexact sidecarへ固定する。
  - density profile script、Rust fixture module、predicate、measurement contractをprofile固有fingerprintのclosed file setへ追加し、M0 baseline commit以後はbyte変更しない。M4のgallery / color profileは別fileへ追加し、density profileへimportさせない。plan時と各case前後で再hashし、untracked helperやprofile外scriptを合格artifact生成へ使わない。
  - completed opaqueとprovisional transparentを別process / phaseにする。seed `20260901`、High、DPI 1.0、1280×720、Vulkan / X11、`novsync`、30秒warm-up、60秒measure、3 valid runを固定し、各runのp95 / p99を先に計算してから中央値とMADを集約する。invalid runを除外して3本へ見せかけない。
  - completed main passはwall-specific drawをmesh / material IDで抽出し、全6 familyを含む`D_N <= 6`、`D_4N <= 6`、`D_4N = D_N`をgateにする。M0のCuboid controlで4Nがrendererのinstance capacityを越えないことを確認する。provisional sorted phaseは同じbatchingを要求せず、`D_4N <= 4 * D_N + 6`をsuperlinear防止gateにしてcount / slopeを保存する。
  - calibration / density harnessのcheck / clippy / verifyとHelp impact reviewを終え、対象diffを提示してユーザー承認を得たscoped local baseline commitを作る。M5ではこのcommitのclean worktreeへfinalと同一asset viewをprovisionしてbaseline legを再採取し、fixture / measurement contract hashが一致する場合だけ比較する。
- 変更候補:
  - `docs/art-style-criteria.md`
  - `docs/blender-setup.md`
  - `crates/bevy_app/src/plugins/startup/perf_scenario/`
  - `.codex/skills/hell-workers-run-native-acceptance/scripts/wall_density_acceptance.py`（新規・M0後freeze）
  - `.codex/skills/hell-workers-run-native-acceptance/scripts/wall_art_acceptance.py`（新規calibration profile、M4でgallery拡張）
  - `.codex/skills/hell-workers-run-native-acceptance/SKILL.md`
  - `tools/blender_ai_workflow/scripts/render_color_calibration.py`（新規）
  - `tools/blender_ai_workflow/scripts/verify_color_calibration.py`（新規）
  - `tools/blender_ai_workflow/scripts/verify_wall_reference_locators.py`（新規）
  - `tools/blender_ai_workflow/fixtures/wall-reference-locators-v1.json`（新規）
  - `tools/blender_ai_workflow/tests/`
  - `<ASSET_ROOT>/staging/reports/wall-production-v1-baseline.*`
- 完了条件:
  - [x] current wallのP02 visual referenceと`wall-density-v1`のN / 4N、completed / provisional baselineが、承認済みbaseline commitから保存されている。
  - [x] mask表、canonical orientation、bounds / pivotのfixture値に曖昧さがない。
  - [x] 9.6 wu公称・連続最小厚の算出条件、12.8 wu外形上限、9.6 wu共通portと再算定triggerがfixture / art基準に固定され、厚さA/Bを未決事項へ戻していない。
  - [x] OCIO色再現gateがpass、またはgeometry-only継続／色承認blockedが明記されている。
  - [x] OCIO artifactがconfig path / hash、runtime version、`fallback=false`、4 base patch＋emissive sanity patchの入力／経路／PNG／ROI／Delta Eを証明し、offline verifierの既知vector unit testと再検証がpassする。
  - [x] normal technical prerequisite、比較仮説、一度で打ち切る条件がartifact schema / testで固定されている。
  - [x] `wall-density-v1`のN / 4N、配置 / connector / mask checksum、seed、環境、時間、3-run中央値 / MAD、completed / provisional draw predicateがmachine-readable contractで凍結されている。
  - [x] code/runtime batchについてHelp impact decisionが完了し、baseline commit前の対象diffとcommit境界をユーザーが明示承認している。
- 検証:
  - `python3 tools/blender_ai_workflow/scripts/verify_wall_reference_locators.py`
  - `python3 tools/blender_ai_workflow/scripts/verify_color_calibration.py ...`
  - `python3 -m unittest discover -s tools/blender_ai_workflow/tests`
  - `hell-workers-run-native-acceptance` Skillのwall-art `plan`が返すdirect `kitty` launcherだけを実行する。
  - `PYTHONDONTWRITEBYTECODE=1 python3 .codex/skills/hell-workers-run-native-acceptance/scripts/p02_presentation_acceptance.py plan --repo "$PWD" --adapter Intel`
  - 上記も返されたdirect `kitty` launcherだけを実行する。
  - `python3 scripts/dev.py check`
  - `python3 scripts/dev.py cargo -- clippy --workspace --all-targets -- -D warnings`
  - `python3 scripts/dev.py verify`
  - `python3 scripts/dev.py docs --check`
  - `git diff --check`

### M1: production wall assetをstagingで制作

実装状況（2026-09-01）:

- exact collection selectorを`validate-blend` / `export-staging-glb`へ後方互換で追加した。指定時だけ
  unknown / empty / multi-meshを拒否し、Blender実機の1 mesh正例exportと3負例を確認済み。
- `validate_wall_glb.py` / `validate-wall-glb`を追加した。GLB JSON / BIN accessorを直接decodeし、1 node /
  1 mesh / 1 primitive、identity、UV0、tangent有無、embedded image、triangle cap、raw bounds、active armの
  core / port cross-section、junction corridor unionを検証する。純Pythonのisolated / straight正例とnode /
  primitive / image / UV / port負例、およびBlender 5.1.1→Khronos→post-exportの実GLB正例がpassした。
- asset-set manifest v2 templateとfail-closed validatorを追加した。candidate / finalを分離し、6 family、
  core / optionalの3通りのnormal在庫、24個のmesh report、2個のset report、tool identity / version、license、
  provenance、art approval artifact、geometry contract、production role / byte length / actual bytes hashをclosed setで
  検査する。不変identity名は`asset_set_generation`へ統一し、既存generic v1は変更していない。
- `sync_external_assets.py`へcandidate限定manifest allowlist modeを追加した。legacy modeを維持し、core 8 fileまたは
  pending optional normal 1 fileだけを明示asset rootへcopyする。manifest全体のhash検証、dry-run、symlink / path
  escape / tamper / delete併用 / receiptなしfinal拒否を単体testで確認した。
- `promote_asset_set.py`へread-only plan、明示apply、recover、rollbackを追加した。absent / existing preimage、
  exact payload、canonical receipt、単調generation / receipt ID、root外snapshotを検証し、全file / directory fsyncと
  generation / pointer renameの各中断点でactiveが旧または完全な新世代だけになることを故障注入testで確認した。
  rollbackはgenerationを消さずpointerだけを復元し、inert / temporary generationは明示recoverで隔離する。
- workspace init / verifierへgeneric v1を保持したWall v2 generation / authority / quarantine layoutを追加した。
- imagegenでshared albedo / emissive候補を作り、同一条件で1024角sRGBへresampleして外部stagingへ配置した。
- 6 familyを各1 mesh / 216〜240 triangles / UV0 / 1 material、上下2 side bandで作る決定的Blender scene generatorと、production限定の
  in-memory 32倍vertex bake / placeholder material exportを追加した。isolated診断はraw bounds
  `[-4.8,-16,-4.8]..[4.8,16,4.8]`、embedded image 0、Khronos error / warning 0、post-export pass。
- optional normal候補を生成し、shared 3 textureを1024角opaque RGB、emissive purple UV leakage 0、normal linear /
  OpenGL `+Y`、vector length p05 / p50 / p95 `0.877 / 0.969 / 1.023`、positive Z 100%としてpixel gateで検証した。
- texture reportをasset-set manifest v2へhash結合し、report内のnormal decision / inventory / bytes / hashと
  production inventoryを相互検証する。`rejected`ではnormal fileとnormal測定値をreportから除外し、sampling metadataの
  持ち越しも拒否するため、最終manifestが不採用normalを間接参照する経路も閉じた。
- 59.036° Orthographic reference board rendererを追加し、M0と同じOCIO configの陽性証拠付きで6 familyを
  2×3配置に描画した。V原点の不一致を一度修正し、全面発光を解消して紫亀裂をT / Crossの限定bandへ収めた。
- 6 GLBのactual bytes hashとpost-exportのbounds / section / triangle / UV等の構造測定値をbaselineへ封印し、
  独立再生成後のreportまで含めてexact比較するrebuild verifierを追加した。単体fixtureと現staging snapshotの
  capture→verifyがpassし、正式M1 tool commitからの再生成artifact採取は未実施。
- clean Git commit / treeと外部stagingの既知closed setだけからcandidate manifestを組み立て、全validatorがpassして
  から既存manifestをatomic置換するsealerと、provenance / license metadata templateを追加した。dirty repository、
  report欠落、validator不一致は封印前にfail-closedとする。
- clean M1 tool subject `654d8d81` / tree `4d8e913a`から、OCIO陽性でsceneと6 familyを2回独立生成した。
  GLB SHA-256はisolated=`f2562f4e…`、end=`da6da989…`、straight=`a7de77c0…`、corner=`a9c94432…`、
  T=`b830ee9a…`、cross=`d99b92c6…`で2回ともbyte-identical、post-export構造値もexact一致した。
- generation 1 / normal=`pending` candidate manifestをSHA-256
  `d90f05c8837a37d0b2db8da5a0de571890ceeed045f7c4dbfa49aef95eb6b3c5`で封印した。core 8件とoptional normal 1件を
  primary外detached validation worktreeへdry-run後にcopyし、再dry-runは両selectionとも`copied=0`、Git statusはclean。
  primary assets / canonical generation / active pointerは変更していない。
- release receipt対応のrepo sync、正式6 GLB / manifest / reference board / rebuild artifactは未完了。

- 変更内容:
  - 59度Orthographic reference boardを公称厚9.6 wu / 装飾外形最大12.8 wuで作り、黒石、錆鉄、トゲ、紫裂け目、ラフ線の優先順位を一枚で比較できるようにする。
  - 1つのBlender原本内の6 named collectionから6 meshとshared UV / textureを作り、export copyへ32倍scaleをbakeしてstagingへ個別exportする。
  - `validate-blend` / `export-staging-glb`へ同一のexact collection selectorを追加し、既存no-selector contractを維持する。
  - pre-export scene validatorに加え、生成GLBを直接decodeするproject固有post-export validatorを追加する。Khronos validatorと併用し、triangle / raw local bounds / identity node transform / mesh / primitive / UV / tangent有無 / embedded image 0 / missing textureを検査する。各armの8〜16 wu port collarをconnection axisへsliceし、連続最小厚、局所横断最大外形、境界port profileを回転不変で検査する。junctionは別armとのunion、isolatedは中心2軸fixtureで判定する。
  - 既存の単体asset manifest v1を再解釈せず、別schemaのasset-set manifest v2 templateとfail-closed validatorを追加する。source、6 collection→6 GLB、production / optional texture、`normal_decision`、全report / artifact hash、M1 tool commit / tree hash、runtime subject commit、license、art reviewをclosed setとして記録する。M1 candidateだけはnormal / runtime subjectの`pending`と`review_status=candidate`を許すが、M4 final modeとM5は一つでも`pending`なら拒否する。
  - `scripts/sync_external_assets.py`へ後方互換なmanifest allowlist modeと`--selection core | optional:normal`を追加する。manifest modeの`--dest`はasset rootであり、toolがgeneration固有payload pathとprojection pathを導出する。release modeだけは`--receipt <generation-scoped-receipt>`を必須にしてrepo generationへhash検証copyする。wall releaseでは選択集合とmanifestにない`models` / `textures` / `audio`をcopy対象にしない。通常mirrorへのcore同期はtemporary generationを完成・fsync・renameしてから検証済みruntime projectionを`assets/manifests/`へ最後にatomic replaceし、optional-only同期はactive projectionを書き換えない。外部provenanceの秘密や未承認optional fileを含めず、staging全体を無条件に同期しない。
  - canonical向けにmanifest allowlistだけを扱う`promote_asset_set.py`の`plan` / `apply` / `recover` / `rollback` modeを追加する。payloadとgeneration-scoped immutable receiptは同一filesystem上のtemporary directoryへ全件copy・hash検証し、全fileとdirectoryをfsyncする。そのdirectoryを一度だけimmutable `<GEN>`へrenameしてgeneration-store親directoryをfsyncし、最後に**唯一のmutable authority**であるactive pointerをtemporary fileのfsync→atomic rename→pointer親directoryのfsyncで切り替える。固定pathをfile単位で順次上書きしない。killが各fsync / rename段で起きても旧または新の完全なgenerationだけがactiveになり、`recover`はorphan temp / inert generationを検査・隔離する。既定動作はread-only `plan`とし、承認なしの`apply`を手順に入れない。
  - staging snapshotから独立rebuildを1回行い、同じ構造hashで再検査する。
  - current generic manifest v1 / fixed `source`・`exports` routeを変更せず、wall asset-set v2だけがgeneration storeを使うことを`docs/assets_workflow.md` / `docs/blender-setup.md`へM1 tool commit内で追記する。`init-asset-workspace`とtemplate / doctorもwall generation / receipt / active-pointer directoryを冪等にbootstrap・検査し、旧workspaceを壊さないtestを追加する。M6では実際にpromoteしたgeneration / recovery evidenceで同節を最終化する。
  - tooling / pipelineのcheckとHelp impact reviewを終え、対象diffを提示してユーザー承認を得たscoped local M1 tool commitを作る。そのclean commitからscene / GLB / report / candidate manifestを再生成し、tool commit / tree hashが自己申告でなくartifactと一致することを検査する。
- 変更候補:
  - `<ASSET_ROOT>/staging/blend/wall-production-v1.blend`
  - `<ASSET_ROOT>/staging/exports/models/wall_*.glb`
  - `<ASSET_ROOT>/staging/exports/textures/buildings/wall/`
  - `<ASSET_ROOT>/staging/reports/`
  - `tools/blender_ai_workflow/scripts/export_glb.py`
  - `tools/blender_ai_workflow/scripts/validate_scene.py`
  - `tools/blender_ai_workflow/scripts/validate_wall_glb.py`（新規post-export validator）
  - `tools/blender_ai_workflow/scripts/validate_asset_set_manifest.py`（新規）
  - `tools/blender_ai_workflow/scripts/promote_asset_set.py`（新規）
  - `tools/blender_ai_workflow/bin/validate-blend`
  - `tools/blender_ai_workflow/bin/export-staging-glb`
  - `tools/blender_ai_workflow/bin/validate-wall-glb`（新規）
  - `tools/blender_ai_workflow/templates/asset-set-manifest-v2.template.json`（新規。既存v1 templateは維持）
  - `tools/blender_ai_workflow/tests/`
  - `tools/blender_ai_workflow/README.md`
  - `tools/blender_ai_workflow/bin/init-asset-workspace`
  - `tools/blender_ai_workflow/templates/`
  - `scripts/sync_external_assets.py`
  - `scripts/tests/`（manifest allowlist testの既存配置規約に従う）
  - `docs/asset-pipeline-glb.md`（新規）
  - `docs/assets_workflow.md`
  - `docs/blender-setup.md`
- 完了条件:
  - [x] 6 GLB全てが1 mesh / 1 primitive、共通UV、embedded image 0、350 triangles以下である。
  - [x] raw primitive-local bounds / pivot / canonical orientationがM0 fixtureと一致し、Y min / maxは`-16 / +16 wu`、node transformはidentityである。
  - [x] 全接続腕のport collarで公称・連続最小厚が9.6 wu、石・鉄・トゲ込み局所横断外形が12.8 wu以下で、cell境界のport profileが6 mesh間で一致する。junctionの別armを厚さへ誤算入していない。
  - [x] albedo / emissiveが共通で、variantごとのmaterial slotやtextureがない。
  - [x] selectorは6 collectionを別々にexportし、unknown / empty / multi-meshを拒否し、既存no-selector smokeもpassする。
  - [x] asset-set manifest v2 validatorが、6 collection→6 GLB、production core / optional集合、normal decision、全report / license / SHA-256 / review statusをexact検証する。M1 candidate modeだけは明示的`pending`を許し、M4 final / M5 modeは拒否する。candidateは隔離profileだけ、通常productionはpromotion receipt付きauthorityだけにmode分離する。
  - [x] normal集合はpending 8 core＋1 optional、adopted 9 core＋0 optional、rejected 8 core＋0 optionalだけを許し、rejected normalはart evidence以外のfinal manifest / projection / asset viewに残らない。
  - [x] post-export validatorが6正例をpassし、unknown / empty / multi-mesh、node transform、2 primitive、embedded image、bounds / port違反の負例を全件rejectする。
  - [x] manifest allowlist付きsync dry-runの差分がwall asset setだけであり、primary / canonicalや無関係assetを対象にしない。
  - [x] promotion toolとmanifest-aware syncのplan / apply / recover / rollback testが、existing / absent preimage、途中copy失敗、hash差替え、allowlist外file、全fsync / directory rename / pointer replace kill pointでfail-closedになり、canonical / repoのactive pointerが部分generationを指さない。
  - [x] receipt validatorがclosed field setとcanonical bytesを検査し、missing、wrong generation / manifest / plan、stale preimage pointer、別payloadへのreceipt再利用を全件rejectする。
  - [x] generic v1 workspaceは不変のまま、bootstrap / doctorがwall v2 generation / receipt / pointer layoutを冪等に作成・検査し、M1時点の運用docsとtool contractが一致する。
  - [x] manifest / license / SHA-256 / scene・post-export・Khronos validator report / rebuild reportが揃う。
  - [x] staging候補とcandidate-only normalがhash付きでprimary外のclean validation worktreeへprovisionされ、primary / canonicalとは明確に区別されている。
  - [x] 6 mesh全てのUV0とtangent有無をpost-export reportへ記録し、tangent欠落時はM4の一回限り生成pathをtechnical fixtureで検査できる。normalのlinear / `+Y` contractも別reportで検査できる。
  - [x] まだcanonical generation store / active pointerへ昇格していない。
  - [x] tooling / runtime-data候補の実経路についてHelp impact decisionを完了し、scoped M1 tool commitをユーザーが明示承認し、そのcommitから全report / manifestを再生成してからM1完了を報告する。
- 検証:
  - `tools/blender_ai_workflow/bin/validate-blend --collection <exact-name> ...`
  - `tools/blender_ai_workflow/bin/export-staging-glb --collection <exact-name> ...`
  - `tools/blender_ai_workflow/bin/validate-wall-glb ...`
  - `python3 tools/blender_ai_workflow/scripts/validate_asset_set_manifest.py ...`
  - `python3 -m unittest discover -s tools/blender_ai_workflow/tests`
  - `python3 scripts/sync_external_assets.py --source "$ASSET_ROOT/staging/exports" --dest "$VALIDATION_WORKTREE/assets" --manifest "$ASSET_ROOT/staging/reports/wall-production-v1.asset-set.json" --selection core --dry-run`
  - `python3 scripts/dev.py docs --check`

### M2: runtime asset pool、material、Eligible stateを接続

実装状況（2026-09-02）:

- M1 manifestのtool identityは現在HEADではなく、記録されたtool commit objectがrepositoryに存在し、そのcommitの
  treeと`tool_tree`が一致する契約へ修正した。これにより後続runtime commitでM1証跡を誤失効させず、commit / treeの
  組み替えは引き続き拒否する。
- 検証済みcandidate manifestの8 core＋optional normalを、asset-set generation / manifest hash / authorityへ結合した
  canonical JSON `.wallset`へprojectするtoolを追加した。candidateは`isolated_candidate`、normal=`pending`、
  receipt=`null`、review=`candidate`に固定する。release projectorはpendingなしのart-approved final manifestと
  canonical promotion receiptを要求し、core pathを`wall_sets/<GEN>/...`へ写像して`release_approved` projectionへ閉じる。
- Bevy 0.19のcustom `.wallset` asset / loaderを追加した。loaderはcanonical JSON bytes、closed 8／9 core、
  path / role / byte length / SHA-256を検査し、`LoadContext::read_asset_bytes`で全coreのactual bytesをasset-set単位に
  再照合する。releaseではgeneration-scoped receiptも同じAPIで読み、actual bytes / hash、closed canonical schema、
  manifest hash / generation / new-active bindingを検証する。missing / tampered / wrong-generation / wrong-manifest receiptは
  asset loadを`Failed`へ落とす。
- manifestがCPU-loadedになるまでgeneration固有pathのmesh / texture handleを作らない二段階poolへ変更した。
  GLBは`GltfAssetLabel::Primitive { mesh: 0, primitive: 0 }`で6 handleへ直接loadし、SceneRootは作らない。
  fallback 2 materialを維持し、production完成／仮設materialはresolved albedo / emissiveと採用済みnormalだけから作る。
  generation差替え時は旧production materialを除去して有限pairを維持する。M2ではentityへのproduction適用を行わない。
- candidate opt-inは`HW_WALL_CANDIDATE=1`だけでなく、launcherが渡す
  `HW_WALL_CANDIDATE_GENERATION`と`HW_WALL_CANDIDATE_MANIFEST_SHA256`のexact matchを必須にした。
  release authorityは有効receiptをloaderが検査済みの場合だけ通常起動で候補になる。全required handleがreadyの場合だけ
  authority付き`Eligible`へ進み、asset-set identityとprocess-local session / activation revisionを分離する。
- optional normalは`.wallset` loaderがactual bytes / length / SHA-256を検証するがproduction coreからは分離する。
  missing / tampered normalはA/B readinessだけを`Failed`へ落とし、8-file production manifestは`Loaded`を維持する。
  adopted normalは9番目のrelease coreとして必須にする。missing albedo / emissiveはloadを失敗させ、同じgenerationの
  正しいbytesを戻したfresh Appだけで回復し、同process hot reloadを主張しないfocused testを追加した。
- runtime実装commitは`405f6e9c`、readiness負例commitは`546f4007`、sealed real-asset fixture commitは`473d922c`、
  normal mapのlinear load gateは`d194c123`、pool invariant testは`3f2c6f7c`、実system fixtureは`bb266315`、
  topology-gated activation stateは`4ebc23a8`、同frame fallback spawn gateは`116d1ff9`。
  `473d922c`から作成したprimary外clean worktree
  `staging/validation/wall-runtime-473d922c/worktree`へmanifest allowlistで8 core＋candidate normalだけを配置した。
  tracked差分／symlinkは0、candidate projection SHA-256は
  `c13d6134e1a063d36fb4a998612ebd41eb8afe70e6090440c448a75fa92b16b2`。`4ebc23a8`へ進めた同worktree自身から
  Bevyの実GLB / PNG loaderとreadiness systemをheadless実行し、6 primitive、sRGBのalbedo / emissive、linearの
  normal、raw AABB X/Z cell内・Y `-16..16`、350 triangles以下、unauthorized fallback、exact candidate identityでの
  `Eligible`遷移、topology未準備時のfallback、test seamでの`ReadyToApply`、steady-state revision / material handle不変をpassした。
  dirty診断runと旧worktreeは正式結果へ流用していない。

- 変更内容:
  - 6 GLB primitive、shared texture、`WallAssetSetManifest` custom asset / loaderをasset catalogへ追加し、wall専用のfinite handle poolを作る。normal A/B handleは隔離scenario限定のcandidate poolへ分離する。
  - procedural `Cuboid`をfallbackとして保持し、6 mesh＋albedo＋emissive（採用確定後だけnormalを追加）の全CPU-readyとmanifest authorityをaggregateするload-state systemを追加する。このマイルストーンでは`Eligible`までに留め、通常gameplayのentityへproduction handleを適用しない。
  - 完成／仮設のshared `TopDownStructuralMaterial`へalbedo / emissiveを接続する。entity単位のmaterial生成を禁止するtestを追加する。
  - existing wall spawn、cleanup、material promotion、Scene RtT render layer、`MeshTag`のexactly-one契約をfallback上で維持する。spawn / rehydrateのcreation-time fallback / tag初期化だけを例外にし、production `Mesh3d` / materialとspawn後のwall transform / `MeshTag` mutationの唯一writerはM3で追加するpresentation applyへ集約する。
- 変更候補:
  - `crates/bevy_app/src/assets.rs`
  - `crates/bevy_app/src/assets/`またはstartup配下のwall manifest loader（既存module境界に合わせる）
  - `crates/bevy_app/src/plugins/startup/asset_catalog.rs`
  - `crates/bevy_app/src/plugins/startup/visual_handles.rs`
  - `crates/bevy_app/src/systems/jobs/building_completion/spawn.rs`
  - `crates/bevy_app/src/systems/visual/building3d_cleanup.rs`
  - `crates/hw_visual/src/visual3d.rs`
  - `crates/bevy_app/src/plugins/visual.rs`
  - `crates/bevy_app/Cargo.toml`（`.wallset` canonical JSON loader用`serde_json`を通常dependency化）
- 完了条件:
  - [x] 6 primitiveをBevy 0.19 APIで直接ロードし、`SceneRoot`や子meshを生成しない。
  - [x] required assetの全CPU-ready前／failure／unauthorized時は全wallがfallbackで見え、mixed状態やinvisible wallがない。
  - [x] `.wallset` projectorがcanonical JSONをbyte-identicalに再生成し、通常feature集合でloaderがcompileする。loaderは全core file、release modeではimmutable receiptのactual bytes / SHA-256も`LoadContext::read_asset_bytes`で照合し、canonical schemaとmanifest / generation bindingを検査する。非canonical wire、改変・未知・欠落core、missing / tampered / mismatched receiptでは`Eligible`にならない。hash検証はasset-set identityあたり1回で、wall entityごとにI/Oしない。
  - [x] 全CPU-ready＋manifest authority後はaggregate stateだけが`Eligible`へ一度遷移し、M2単独の通常gameplayでは既存／新規wallともfallbackのままである。test専用`topology_ready` seamでだけ、後段のatomic apply条件を検証する。
  - [x] active production materialは完成／仮設2 handle、fallbackを含む総poolは4 handleで有限であり、Indoor Light Field bindingと未sample契約を保持する。
  - [x] missing / late albedo / emissive、adopted normal、release receipt、ready切替と同frame spawn、synthetic asset failure後の一括fallbackがfocused testで合格する。receiptのmissing / tamper / wrong generation / wrong manifestは通常起動をfallbackへ落とす。failed fileの復旧はfresh App restartで検証し、同processの自動hot reloadを主張しない。candidate-only normalのmissing / lateはproduction core readinessを誤ってblockせず、normal A/Bだけをfail-closedにする。
  - [x] loaded primitiveのraw local AABB、identity node前提、world transform後AABBが1 tile / ground接地契約と一致し、asset reportの9.6 / 12.8 wu geometry値とhashが一致する。
  - [x] exactly-one `Building3dVisual` / `Mesh3d` / material / logical `MeshTag` testが合格する。
  - [x] asset-set identityとprocess-local session / activation revisionが混同されず、同一activation revisionのsteady stateではaggregate再遷移、全Wall走査、mesh / material writeが0である。fresh restartは同じasset generationを保持した新sessionとして回復する。
  - [x] M2の実経路についてHelp impact decisionを完了してからマイルストーン完了を報告する。
- 検証:
  - `python3 scripts/dev.py cargo -- test -p bevy_app wall`
  - `python3 scripts/dev.py check`
  - `python3 scripts/dev.py cargo -- clippy --workspace --all-targets -- -D warnings`

### M3: 16 topologyと全lifecycleを統合しproductionを有効化

実装状況（2026-09-02）:

- `WallConnectionMask`のbit順をM0 fixtureと同じ`(N,S,W,E)`へ固定し、16 maskを
  `WallMeshFamily + QuarterTurns`へ写像するpure resolverを`819ca26d`で追加した。既存2D completed / provisionalの
  texture選択も同じmask生成を使用し、従来の16分岐と同値を維持する。
- sealed geometry fixtureの16 mapping / +Y quarter turnをexhaustive table testで固定した。completed Doorがnorth connectorとして
  同じmaskへ寄与する実経路も`4a728b06`で検証した。
- `5f59d8b3`でentityごとのBuilding / Blueprint grid集合とgridごとのcoalesced contributor集合を持つ
  `WallTopologyIndex`を追加した。add / transform move / component removalで旧集合・新集合のself＋4近傍だけをdirtyにし、
  同一gridのBuilding＋Blueprintはconnector 1として解決する。2D completed / provisional consumerは同じresolved maskを使い、
  plain Wall rootへ`WallTopologyState { grid, mask, family, quarter_turn, revision }`を付与する。
- steady frameはdirty target 0、resolution revision不変、component / sprite write 0とした。world replacementではindex / external dirtyを
  `hw_visual`所有hookでclearし、次frameに全Building / Blueprintを1回だけ再構築する。連続2回resetでもrebuild 1回、その次の
  steady frameはrevision不変となるfocused testを追加した。
- `ee1053c2`でtopology producerとasset readinessを`PostUpdate`へ移し、
  `WallAssetReadinessSet -> WallTopologyResolveSet -> ApplyDeferred -> WallPresentationApplySet`を
  `TransformSystems::Propagate`より前へ固定した。deferred topologyが同frameの3D mesh / material / composed transform / owner-derived
  `MeshTag`へ届き、同じframeの`GlobalTransform`まで伝播するfocused testがpassした。
- 3D visualへresolved topology、fallback / production mode、activation revisionを持つ`Wall3dPresentationState`を追加した。
  root所有のowner→visual indexはactivation変更時だけ全ownerをrefreshし、通常はchanged owner / added visualだけを処理する。
  exact asset-set identityとfinite material pairが一致する時だけ6 family meshと+Y quarter turnを適用し、不一致／failureでは全Wallを
  procedural fallbackへ戻す。2つの異なるfamilyを同じactivation revisionで一括fallbackへ戻すtestとsteady frame 0-write testがpassした。
- spawn / rehydrateはfallback bundleとowner tagのcreation-time初期化だけを担当し、Wallをgeneric transform / provisional material writerから
  除外した。`Last` rehydrate frameでも原点表示にならないようfallback local値と同じ`GlobalTransform`を生成時に挿入する。
  `hw_visual`のtopology resetとは別にroot所有hookでactivation / visual owner indexをfallbackへ戻し、連続resetとstale material identityの
  fail-closed testを追加した。
- `0ad6706c`でnormal placementの各`WallTileVisualMirror`をtopology contributorへ接続した。施工tileとspawn済みWallが同一gridに
  共存してもconnectorは1として解決し、tile撤去後はWall、Wall撤去後は空集合へ正しく遷移する。component filterを
  `With<WallTileVisualMirror>`で閉じ、通常Wallの`Changed<Transform>`を施工tile sourceへ誤登録しないtestを追加した。
- 同commitで完成／仮設の切替がmesh family / topology rotation / transform / owner-derived `MeshTag`を維持してshared materialだけを
  交換すること、owner rotation / scaleとtopology quarter turnが合成されること、Door add / removeを実producerが同frameの3D familyへ
  反映することを固定した。rehydrate shellはexactly-one fallback visualを正しいlocal / `GlobalTransform`で生成する。
  multi-tile施工の全phase進行、実save/load / rollback / recovery-onlyとの統合testは後続M3 batchで完了した。
- `54359061`でnormal / Instant Buildの2 tile配置を実`apply_wall_placement`とBuilding visual mirror observerから同じtopology routeへ通し、
  normalは施工tileだけ、Instant Buildは各Wallのexactly-one 3D visualだけを生成することを固定した。framing前の施工tile単独とframing後の
  施工tile＋spawn済みprovisional Wallを実cancellation systemで撤去し、同じframeの`PostUpdate`で隣接Wallがisolatedへ戻る。
  実`building_bounce_animation_system`がowner scaleを更新したframeもtopology quarter turn、local / `GlobalTransform`が一致する。
- `729db30d`でnormal / Instant Buildの配置systemを実`PlacementFeedbackSet::Commit`へ置き、同じframeの`PostUpdate`で隣接Wallの
  topologyが更新されることを固定した。実deconstruction finalizerがWallを撤去し`WallConnectionDirty`を発行したframeも、隣接Wallは
  同じ`PostUpdate`でisolatedへ戻る。Door add / remove、Blueprint add / move / remove、施工cancelの既存focused testと合わせ、
  Update writerからtopology producerまでの主要lifecycleを同frame契約へ接続した。
- `d0e23996`で2 tile siteを実`wall_framed_tile_spawn_system -> wall_construction_phase_transition_system ->
  wall_construction_completion_system` chainへ通した。Framing完了時は各tileにprovisional Wallとexactly-one 3D visualが生成され、
  相互のend topologyを保ったままCoatingへ進む。Completionではsite / tileだけが消え、同じWall rootが完成material対象、
  completion bounce、同一topologyを維持する。
- `122e9d47`で実DynamicWorld transaction、実rehydrate shell、root / leaf reset hook、production activation chainを一つのfixtureへ
  統合した。normal load、rollbackの連続2 reset、recovery-onlyの全経路で、transaction直後は全Wallがfallback-onlyかつowner由来の
  local / `GlobalTransform`と正しいworld位置を持つ。次の`PostUpdate`でexact asset identityと再構築済みtopologyを満たした全Wallが
  production-onlyへ一括復帰し、presentation topologyとownerのmask / family / rotationが一致する。その次のsteady frameは
  resolution revision不変で、旧Wall / visual Entityは残らない。
- clean validation worktree `staging/validation/wall-runtime-473d922c/worktree`のsealed generation 1を現行geometry
  contract SHA-256 `0c13f5a2084dd60e6e4634d3f6b5ddaee33b6e90273c2cc4876eab0b76f6c563`で再検証した。
  6 GLBはmanifest hashと一致し、全armのcore / `8.01, 12.0, 15.99 wu` port collar断面がsolid、port幅
  `-4.8..+4.8 wu`、Y `-16..+16 wu`で共通、junctionはactive corridor union、isolatedは中心2軸でpassした。
  全vertexは12.8 wu ornament corridor / 32 wu cell envelope内、各mesh 216〜240 triangles、UV0ありである。
  現行Bevy実loader ignored gateとsealed geometry fixtureの16 mask / quarter-turn exhaustive testもpassし、M3を完了した。

- 変更内容:
  - mask計算と `WallMeshFamily + QuarterTurns` resolverをpure functionへ抽出し、16件のexhaustive table testを追加する。
  - 3D `Mesh3d` / topology state / transformを同期し、既存2D blueprint image selectionも同じmask sourceを使う。
  - bidirectional `entity -> set<grid>` / `grid -> contributors + targets` indexを追加し、add / move / remove / cancel / rehydrateで旧集合・新集合のself＋4近傍だけを更新する。
  - normal construction、Instant Build、deconstruction、save/load、fixtureの全production routeをresolverへ接続する。
  - `Eligible + topology_ready`を一つのactivation transitionとして`ProductionActive`へ進める。spawn / rehydrateのcreation-time fallback bundle / tag初期化を除き、production handleとspawn後の`Mesh3d` / wall material / composed transform / owner-derived `MeshTag` mutationは`WallPresentationApplySet`だけが書く。failure / resetでは同じapplyが全wallをfallbackへ戻す。
  - `hw_visual`のworld-replace hookへindex / last-known grid / dirty topology revision / resolved cacheのclearとfull-rebuild request、`bevy_app`の別hookへglobal fallback化 / activation revision更新を接続する。両reset完了後に`Last` rehydrateし、次frameに一回だけfull rebuild / production復帰する。
- 変更候補:
  - `crates/hw_visual/src/wall_connection.rs`
  - `crates/hw_visual/src/visual3d.rs`
  - `crates/hw_core/src/visual_mirror/construction.rs`（tile grid mirrorが必要な場合）
  - `crates/bevy_app/src/systems/visual/building3d_cleanup.rs`
  - `crates/bevy_app/src/systems/jobs/wall_construction/`
  - `crates/bevy_app/src/systems/jobs/deconstruction/`
  - `crates/bevy_app/src/systems/save/rehydrate/`
  - `crates/bevy_app/src/plugins/startup/perf_scenario/`
- 完了条件:
  - [x] 16 mask、Door接続、canonical rotationがtable-driven testで全件合格する。
  - [x] 全family / quarter turnのport collarで中心bandと接続portの半幅4.8 wu以上、局所横断の装飾半幅6.4 wu以下が不変で、Wall–Wall境界にgap / overlapがない。junctionはactive arm unionとして別判定する。
  - [x] `Update`内のwall / door / blueprint追加・撤去・cancelが、同frameの後続`PostUpdate`で対象と4近傍だけを更新する。
  - [x] 複数tile siteの配置→framing→`FramedProvisional`→完成と、framing前／後cancelで、全gridのconnectorとtile visual targetが欠落・重複しない。
  - [x] site / tile / spawned wallが同一gridに一時共存しても接続数1へcoalesceされ、GLBはspawned Wallだけにexactly oneである。
  - [x] 仮設→完成はmaterialだけが変わり、mesh family / rotation / `MeshTag`は不変である。
  - [x] topology producerだけがdirtyをdrainし、2D / 3D consumerが同じresolved stateを同じ`PostUpdate`で観測する。`PlacementFeedbackSet::Commit`由来の配置も1 frame遅延しない。
  - [x] `WallAssetReadinessSet -> WallTopologyResolveSet -> ApplyDeferred -> WallPresentationApplySet`が`TransformSystems::Propagate`より前に実行され、final transform / tag compositionもapply内で完了する。owner移動 / owner回転ではlocal / `GlobalTransform`と`MeshTag`が同frame更新され、completion bounceやtransform syncでtopology回転が消えず、topology quarter turnだけではowner-derived tagが変化しない。
  - [x] save/load後にexactly-one visualと同一mask / family / rotationが復元される。
  - [x] normal load、rollback、recovery-only、連続2回resetの全world replacement testで、`Last` rehydrate frameは全wall fallbackかつlocal / `GlobalTransform`が正しいworld位置、次frame rebuild後は全wall production、mixed frame 0、旧`Entity`参照0、index rebuild 1回、重複contributor 0である。
  - [x] asset `Eligible`からの初回有効化、同frame spawn、synthetic failure、restart後回復の各activation revisionで既存／新規wallが同じmodeへ一括収束する。creation-time fallback / tag初期化以外にpresentation apply外の`Mesh3d` / wall material / wall transform / `MeshTag` mutationがなく、既存generic writerがWallを除外している。
  - [x] steady stateのtopology再計算とmesh writeが0件である。
  - [x] M3のplayer-visible routeについてHelp impact decisionを完了し、必要なHelp更新を同じbatchへ含めてから完了を報告する。
- 検証:
  - `python3 scripts/dev.py cargo -- test -p hw_visual wall_connection`
  - `python3 scripts/dev.py cargo -- test -p bevy_app wall`
  - `python3 scripts/dev.py cargo -- test -p bevy_app rehydrate`
  - `python3 scripts/dev.py check`
  - `python3 scripts/dev.py cargo -- clippy --workspace --all-targets -- -D warnings`

### M4: art候補をnative A/Bし、ユーザー承認を得る

実装状況（2026-09-02）:

- M3で再検証したsealed generation 1のgeometry gateをM4の停止条件にも採用し、6 GLBのmanifest hash、
  9.6 wu共通port、12.8 wu corridor union、UV0、triangle capがpassしている。
- sealed OCIO陽性artifact `wall-color-20260901T081824Z-e92a7d9c`を現行offline verifierで再検証し、
  生成reportは保存済みreportとbyte-identical、SHA-256
  `470a6e06ec0c61c916aa7fe9ecabe34da66437626aa45ae339cff4296613b77c`、4 base patchの平均／各
  Delta E 2000は`0.0`、emissive sanityもpassした。
- candidate normal textureはsealed texture reportでlinear sampling / OpenGL `+Y`、vector length / positive Zをpassするが、
  6 GLB全てに`TANGENT` attributeがないためnormal A/Bのtechnical gateはfailと判定した。「差なし」には分類せず、
  generation 1のpending manifestは変更しない。M4 finalの新generationでは`normal_decision=rejected`として封印する。
- lit / unlit A/B用gallery / launcherを実装した。M3 production routeのN=96 fixtureをそのまま使い、16 maskを
  各6件、6 mesh、production 96 / fallback 0、manifest identity、layout checksum、material lighting modeを
  game-owned statusとoffline verifierの双方でexact検証する。`unlit`はprofiling + isolated candidate +
  actual-window + exact comparison値の全条件が揃う場合だけ有効で、通常／release経路には入らない。
- M0 current fallback profileのschemaと挙動は維持し、candidate比較だけを別profile identityで追加した。
  未知comparison、actual-window未併用、plan後のcandidate generation / manifest hash driftはfail-closedにする。
- ユーザーは2026-09-02にscoped harness実装と、そのcommit後のclean candidate worktreeでのnative lit / unlit
  capture開始を明示承認した。check / clippy / verify / Help reviewを完了してharness commitを作るまで、候補assetを
  primaryへprovisionせずnative phaseを開始しない。
- harness commit `2cd86551`のclean worktreeから開始した最初のlit jobは、profiling buildとcandidate activationを
  通過した後、aggregate gallery residency predicateでfail-closedとなった。画像は採用せず、複合predicateを推測で
  調整しないためproduction / fallback / mesh / materialのactual countをfailure reasonへ追加して再診断する。
- 診断commit `6456b173`の再試行で`production=96/96, fallback=0, meshes=1/6, materials=1/1`を取得し、asset / material
  ではなくfixture connectorのvisual mirror初期値がproduction topology ingressへ届いていないことを確定した。
  durable Door `Blueprint`からproduction `blueprint_visual_state()`を生成して同時挿入し、同じ経路で16 maskを解決する。
  2件のinvalid jobと画像は比較証拠へ流用しない。
- connector修正commit `e792b710`では、candidate coreだけのasset viewに通常起動用texture/fontとmanifest pendingの
  optional normalが不足していた2 runを警告ゼロgateでfail-closedとした。primaryとbyte-identicalな通常runtime assetを
  非上書きで補い、optional normalはmanifest allowlist syncをdry-run後に実行した。最終candidate authorityはgeneration 1 / manifest
  `d90f05c8…`のまま、完全asset-view fingerprintは`d6720d3b…`となった。
- 完全asset viewのlit `wall-art-20260902T063714Z-f47631e1`とunlit
  `wall-art-20260902T063830Z-4ad652e2`は96 production / fallback 0 / 6 mesh / 16 mask各6件とoffline verifyをpassしたが、
  captureはpause UIに覆われ、lit側だけprofiling panelも表示され、camera scaleもstandard 1.0でなく5.0だったためart比較には
  採用しない。比較profileだけsubject中心・scale 1.0・top-level UI root 0 visibleをstatusとoffline verifierで固定して再撮影する。
- view修正commit `ee4defe2`のfocused lit `wall-art-20260902T065809Z-a5ff5829`はstandard zoom / UI抑制と全gallery
  stateをpassしたが、topologyを作るDoor Blueprintの施工sprite / task progressが壁を覆ったためart比較へ採用しない。
  connectorの論理Blueprint / visual mirrorは維持したまま比較時だけ192 visual subtreeをHiddenにし、0 visibleをsidecarで要求する。
- connector visual抑制commit `3070fa44`から最終lit `wall-art-20260902T072458Z-7debdcc8`とunlit
  `wall-art-20260902T074751Z-e126b294`を取得し、両方ともclient captureとoffline verifyをpassした。共通条件は
  asset-view `d6720d3b…`、High / DPI 1 / standard zoom 1.0、96 production / fallback 0、6 mesh、16 mask各6件、
  UI root 0 visible、connector visual 192 hidden / 0 visibleである。画像SHA-256はlit `b5c8e49a…`、unlit
  `b34ffb09…`、全画面normalized MAE `0.00242271`、96 px subject ROI MAE `0.00300869`、litのROI平均は
  unlitより`0.767216`高い。directional shadingと紫emissive要件を保持するlitを推奨した。
  完全な設定・locator・hashはstaging report `wall-production-v1-art-review.json`へ保存した。
- ユーザーは2026-09-02にlit画像`wall-art-20260902T072458Z-7debdcc8/current-wall.png`を採用した。
  art review artifactを`decision=lit_approved`へ更新し、選定画像SHA-256 `b5c8e49a…`、承認UTC、normalの
  tangent不在によるrejectを封印した。artifact SHA-256は`477f4176…`である。
- 採用後cleanupでは通常／release／isolated final候補をlit固定とし、`HW_WALL_ART_COMPARISON`、unlit debug material、
  pending optional normalのruntime load / readiness / allowlist経路を撤去した。final candidate galleryはplayer-facing toggleを
  持たず、`art_approved`・normal判定済みmanifestとexact generation/hash opt-inだけを受理する。
- cleanup commit `df5c4dc5`をruntime subjectとして、pendingなしfinal generation 2を
  `normal_decision=rejected` / `review_status=art_approved`で封印した。asset-set manifest SHA-256は
  `04555ddea56feee93337ff3a3abbaec9019be06007c62866dac3888de424a3d6`、isolated projection SHA-256は
  `8370c278c5c0b4749007cc1a1b5146f2388c98364eba42350a3c5c1bd8818075`、coreは6 GLB＋albedo＋emissiveの8 fileで
  optional normalは0である。追補commit `f787be1a`ではignored実asset loader testのgeneration固定値をmanifest実値へ変更し、
  generation 2の6 primitive、runtime bounds、lit material準備、Eligible→ReadyToApplyを`1 passed`で再検証した。
- final fixed-lit actual-window job
  `target/native-acceptance/wall-art-20260902T143128Z-a98b763e`はsubject `f787be1a`、Intel Arc / Vulkan / X11、
  High / DPI 1.0でstatus / offline verifyともpassした。candidate generation 2、lit、production 96 / fallback 0、
  distinct mesh 6、16 mask各6、connector visual 192 hidden / 0 visibleをsidecarで確認し、client PNG SHA-256は
  `fae394ed2afaf6210a285209588dfd41f4bfbe83dbc42013cfe262794a11f4ba`である。

- 変更内容:
  - M3のproduction spawn / WorldMap / presentation routeを使う専用wall-art gallery scenarioを `bevy_app`へ追加する。`visual_test`の独自meshを使わない。
  - source / asset fingerprint、phase ACK、client capture、offline再検証を持つwall-art fail-closed profileをA/B前に完成させ、High / Medium / Low、DPI、zoom、lifecycle phaseをparameter化する。M0でfreezeした`wall_density_acceptance.py`とRust density fixtureは変更せず、別の`wall_art_acceptance.py`へgallery phaseとart predicateを追加する。
  - profile / gallery / comparison toggleを含むharness diffのcheck / clippy / verifyとHelp impact reviewを通した後、ユーザー承認を得てscoped local harness commitを作り、そのcommitのclean validation worktreeだけへstaging候補をprovisionする。
  - A/Bを直積にせず、次の順次funnelで行う。
    1. OCIO proofを固定calibration条件で一度だけ閉じる。OCIOまたはgeometry gate不合格は全art比較をblockする。optional normalだけのtechnical gate不合格はlit / unlit比較をblockせず、`normal_decision=rejected`として手順3だけをskipする。
    2. normalなし、High、DPI 1.0、standard zoomの同じgalleryでlit対unlitだけを比較し、winnerを1つにする。unlitは現shader上emissive / normalを評価できないcontrolであることを観察記録へ明記する。
    3. lit採用かつnormal technical gate合格時だけwinnerの同一条件でnormalなし対ありを1回比較する。normal gate不合格ならnormalをrejectしてlit候補を維持する。unlit採用時はnormal A/Bを省略し、紫emissive要件を満たせないため別wall shader提案へblockする。
    4. 勝者だけを`tile_rtt_px = 14`、High最大zoom、Medium / Low最遠景、全16 mask / lifecycle galleryへ展開する。High / Medium / Low × DPI 3段のfull matrixはM5のfinal candidateだけに実行する。
  - normal候補は隔離validation worktreeから`ImageLoaderSettings::is_srgb = false`で読み、tangent / UV / known-direction sidecar predicate不合格なら画像比較を開始しない。
  - 一時toggleはcandidate IDとartifactを固定するためだけに使い、採用判断後に削除する。
  - 石積みの読解性、平面イラスト感、wobbly line、錆鉄とトゲのsilhouette、紫裂け目、tile境界、Soulとのdepth、directional shadowを観察する。
  - 厚さは9.6 wu固定で、standard、`tile_rtt_px = 14`境界、最大zoom-out 5、各qualityの全quarter turnを撮る。14 px/tileでは内部色、最大zoom-outではHighの内部色1 px以上を数値検査する。Medium / Lowの最遠景はfinal client composite後のanti-aliased silhouetteに穴、断線、frame間flickerがないことを定性的に検査し、物理1 pxの内部色を要求しない。全投影高とcell脇が通路に見えないことも確認し、8 / 9.6の通常A/Bは再開しない。
  - ユーザーが1候補を承認し、outline / unlit / normalの最終値を `art-style-criteria.md`へ反映する。
  - 採用後はprimaryでart comparison toggle / debug material / 不採用normal経路を撤去し、final profileを再度check / clippy / verifyする。M5の性能比較に必要なprofile-only controlはplayer-facing toggleと分離して保持する。Help impact reviewを再実行し、最終diffを提示して二度目の明示承認を得たscoped local commitからM5用clean worktreeを作る。
  - final commit確定後、M1 candidateをそのまま書き換えず新しい`asset_set_generation`として再封印する。`normal_decision`を`adopted | rejected`へ確定し、採用A/B入力と同じpayload hash、M1 tool commit、M4 runtime subject commit、art approval artifact hash、`review_status=art_approved`を記録する。candidate runtime projectionを再生成し、manifest validator / post-export report再照合 / core allowlist sync / M2 loader-readiness / M3 activation-world-replace focused testを新generationで再実行する。payload差が選定artifactと一致しない、または`pending`が残る場合はA/Bへ戻り、M5へ進めない。
- 完了条件:
  - [x] OCIO gateがconfig path / hash、runtime version、`fallback=false`を陽性証明し、Blender referenceとBevy client capture内の4 base-color patchが固定ROI / exposure / sRGB→Lab変換で`Delta E 2000`平均`<= 2.0`、各patch`<= 3.0`である。emissive sanity patchも別predicateでpassする。
  - [x] normal technical gateがcandidate-only hash、linear load、UV0、`+Y`方向predicateと全6 meshのtangent有無を証明した。全mesh tangent不在を明示的technical failureとしてnormalをrejectし、画像A/Bや「差なし」判定へ進めていない。
  - [x] candidateごとの画像、hash、設定、観察結果が保存され、同じ仮説を無目的に再調整していない。
  - [x] lit / unlitを先に一軸比較し、normal technical gate不合格後はnormal画像比較を行っていない。中間candidateへ9 case / full lifecycleを掛けておらず、交絡した比較artifactを採用根拠にしていない。
  - [x] ユーザーがproduction cameraで最終候補を承認している。
  - [x] texture-baked lineworkが承認され、別outline提案を不要と判断している。
  - [ ] 9.6 wu straight E-W wallがstandardで全投影高0.90 tile、装飾最大でも1.00 tile以内である。`tile_rtt_px = 14`では内部色、最大zoom-out 5のHighでは内部色1 px以上、Medium / Lowではfinal compositeの無穴・無断線・無flicker silhouetteを全回転で満たす。
  - [x] 比較用normal / toggle / debug materialを採用結果に従い撤去している。
  - [x] M4 final asset generationが`normal_decision`、M1 tool commit、M4 runtime subject commit、art approval hashをpendingなしで封印し、選定時payloadと一致する。新generationのmanifest / projection、allowlist、M2 readiness、M3 activation / world-replace focused gateを再実行している。
  - [x] A/B harness commitと採用後final commitの前にそれぞれHelp impact decisionを完了し、各対象diff、およびcommitはするがpush / canonical promoteはしない境界を提示してユーザーの明示承認を得ている。
- 変更候補:
  - `crates/bevy_app/src/plugins/startup/perf_scenario/`
  - `.codex/skills/hell-workers-run-native-acceptance/scripts/wall_art_acceptance.py`（M0 calibration profileへgallery / art phaseを追加）
  - `.codex/skills/hell-workers-run-native-acceptance/SKILL.md`（profile手順を更新）
  - `<ASSET_ROOT>/staging/reports/wall-production-v1-art-review.*`
- 検証:
  - `hell-workers-run-native-acceptance` Skillのno-prompt actual-window launcherを使用する。
  - client windowだけをcaptureし、headless、`visual_test`、desktop全体captureを代用しない。

### M5: 専用actual-window、P02回帰、性能を閉じる

実装状況（2026-09-02）:

- final fixed-lit単一caseは上記M4最終jobでpassした。一方、現行`wall_art_acceptance.py`は1 case固定で、
  このM5が要求するHigh / Medium / Low × DPI 1.0 / 1.5 / 2.0の9 case実行を未実装だったため、
  `--candidate --matrix`を追加した。各caseを別process、nonce / ACK、X11 client PNG、raw performance bundleへ分離し、
  job statusとoffline verifierはexact 9 case / screenshot hash / quality / scale factorを再計算する。
- current subject `f787be1a`でhistorical P02 v9を試行したjob
  `target/native-acceptance/p02-presentation-20260902T150153Z-1caa5981`は、最初のHigh / DPI 1 / visible caseで
  既知のlegacy Door child Sprite契約を検出してfail-closedとなった。semantic state / WorldMap owner / passabilityは一致し、
  P08 active Structural3dは子Sprite 0である。M0で既に打ち切った同じ仮説なので、frozen P02 validatorの緩和やmirror復元は行わず、
  invalid artifactをM5証拠へ流用しない。current-source回帰はWall専用9 case、historical P02は登録済みimmutable locatorの
  offline整合検査で閉じる。
- 9 case harnessを追加したため、先に得た単一case jobはM4の固定lit / asset identity証拠として保持するが、M5 matrix合格には
  流用しない。harness commitと新source / harness fingerprintを確定してからclean validation worktreeを更新し、9 caseを新規実行する。
- harness commit `add2e162`の初回matrix job
  `target/native-acceptance/wall-art-20260902T152521Z-8d678312`はHigh / DPI 1.0を完了後、High / DPI 1.5の入口で
  `perf.py`の既存single-case固定条件によりfail-closedとなった。描画結果によるfailureではない。このartifactを無効のまま保持し、
  formal densityとsingle-caseのHigh / DPI 1.0契約を緩和せず、Wall helperだけが渡す内部`--wall-art-matrix` authorizationで
  3品質×3 DPIを許可するよう分離した。新commit / fingerprintで全9 caseを最初から再実行する。
- Python入口を分離したcommit `50ec1c43`の再実行job
  `target/native-acceptance/wall-art-20260902T153105Z-6723db42`もHigh / DPI 1.0完了後にHigh / DPI 1.5でinvalidとなったが、
  この時点ではRust profiling configが同じHigh / DPI 1.0固定条件を保持していたためsidecar生成前に終了した。そこで
  launcher環境、`perf.py`から渡すbinary flag、Rust configを`HW_WALL_ART_MATRIX=1` / `--perf-wall-art-matrix`で対にし、
  profiling feature内でも専用matrixだけがexact 9組を受理する。通常起動、formal density、single-case条件は不変とする。
- runtime contract commit `926a3530`のfresh matrix job
  `target/native-acceptance/wall-art-20260902T153723Z-b52a734e`はIntel Arc / Mesa 26.1.6 / Vulkan / X11で
  High / Medium / Low × DPI 1.0 / 1.5 / 2.0の全9 caseを完了し、status `valid`、独立offline verify `pass`となった。
  subject `926a3530`、source `1d9f3c68…`、harness `c364a4ad…`、asset view `ebeb81af…`、binary
  `52e6f525…`で、全caseが1280×720、lit、generation 2、production 96 / fallback 0、distinct mesh 6 / material 1、
  connector visual 192 hidden / 0 visibleを満たす。全9 client PNGを目視し、品質／DPI固有のWall欠落、断線、黒抜け、
  fallback混在がないことを確認した。
- `tools/blender_ai_workflow/scripts/verify_wall_reference_locators.py`を再実行し、registered historical P02
  subject `6ea0bf99` / attempt `54d85a63-e237-4501-a0d0-33c1d0a29f3b` / Capture SHA-256 `38561b2a…`と、
  分離済みcurrent fallback Wall locatorを改変なしでoffline `pass`した。currentのRender3d visibleは9 case sidecar、
  completion bounce / depth-preserving transform / exactly-oneは既存focused testと全体verifyでpassしている。
- M5正式Capture用に、凍結済み`wall_density_acceptance.py` / `wall_density_fixture.rs` / `wall-density-v1.json`を
  変更しない追加profile `wall_production_performance_acceptance.py`と
  `wall_density_presentation.rs`を実装した。binary flag / launcher環境の二重鍵で`production`と
  `fallback-control`を分離し、候補assetのload・activation・全owner一括収束後だけ30秒warm-upを開始する。
  各runの追加sidecarは初期／終了の同一性、production 6 mesh / 2 material、fallback 1 mesh / 2 material、
  7 / 4有限pool、350 triangle上限、family / rotation分布、candidate identityを検査する。
- subject `5537f621`の初回実機job
  `target/native-acceptance/wall-production-performance-20260902T164026Z-bf161556`は全24 runを完走した。
  completedはN / 4Nのp95 / p99が全件passし、provisional 4N p95も`-0.136%`だったが、provisional N p95だけが
  fallback `11.328262 ms`に対してproduction `12.283340 ms`（`+8.431%`）となり、`+5%` gateでinvalidになった。
  provisional Nのp50も3 runすべてproductionが約0.23〜0.31 ms遅いため単一外れ値とは扱わない。completed productionは
  同じalbedo / emissiveを使いながらN p95 `-8.827%`であり、transparent provisionalだけに残る追加sampleを切り分ける。
  完成壁のemissiveは維持し、建築途中だけemissive textureを外してalbedo＋shared light field＋Blendを維持する仮説を一度だけ
  再計測する。改善しなければこの仮説を打ち切り、閾値を緩和しない。
- 最適化subject `991392b8`のfresh job
  `target/native-acceptance/wall-production-performance-20260902T173804Z-3f6bc903`は全24 runと4比較を完走し、
  独立verifyも`status=pass`となった。completedはN / 4Nのp95が`+0.451% / +0.438%`、p99が
  `+0.910% / +0.540%`、provisionalはp95が`+2.031% / +0.987%`、p99が`+1.134% / +0.369%`で、
  全8行が`+5%`以内である。source `f5c24842…`、harness `7da8d338…`、asset view `ebeb81af…`、binary
  `200234f5…`、manifest SHA-256 `1d0ba117fcc755f303d29c170a7389777dcedc9b678f51f91dd4fb70badf3de1`を封印した。
  初回invalidは削除・合格扱いせず、建築途中emissive sample仮説の失敗証拠として保持する。同一final binary内の
  fallback-control比較はこれでpassした。
- M0最終subject `35f1f6e3`と現行の`wall_density_fixture.rs`には、後続commit `e792b710`で入った
  Door connector mirror初期化22行の差がある。この既知差を無視してbyte-identicalと主張しない。baseline比較は
  `35f1f6e3`へこのfixture-only修正だけを載せたclean派生subjectを作り、finalとdensity profile / fixture / contractを
  byte-identicalにした上で再採取する。元のM0 artifactは履歴基準として保持するが、この比較の直接baselineには流用しない。
- M0 `35f1f6e3`へ上記22行だけを適用したclean派生subject `0241d3b9`を作成し、density helper / fixture / contractを
  final側とbyte-identicalにした。final generation 2と同じasset view `ebeb81af…`を配置したactual-window job
  `wall-density-20260902T190707Z-b8e81f70`は12 / 12 run valid、当時の凍結verifierによる独立verifyもpassした。
  M0 p95 / p99中央値はcompleted N `11.895858 / 16.078749 ms`、4N `16.063391 / 17.250262 ms`、
  provisional N `11.051377 / 16.423798 ms`、4N `16.173516 / 17.376066 ms`である。
- 通常の`perf.py compare`は2 clean worktreeの正しい`BEVY_ASSET_ROOT`絶対path差を拒否したため、既存sessionや
  requested environmentを改変せず、offline `wall-cross-subject-performance-v1`を追加した。両job自身の凍結verifier、
  byte-identicalな3 density file、同一asset view / actual adapter / matrixと、asset root以外の環境完全一致を先に検証する。
  `wall-m5-cross-subject-0241d3b9-vs-991392b8`は4 CSVの独立verifyがpassし、completed N / 4Nのp95
  `-22.313% / +0.852%`、p99 `-37.737% / +0.816%`、provisionalのp95
  `-13.160% / +0.924%`、p99 `-37.360% / +0.681%`で全8値が`+5%`以内だった。
- 続くfinal RenderDoc準備で、M0 helperがfallback Cuboidの単一index数だけをdraw identityに使い、candidate authorityも
  有効化しない経路不成立を確認した。productionをfallbackとして採取せず、runtime checkpoint schema 2へactive mesh
  index数集合、index数別instance合計、active mesh数、presentation / candidate identityを追加した。extractorは同じ
  color+depth passの分布と全owner総数をexact検査し、launcherは`--candidate`とbinary / environment二重鍵を結ぶ。
  このprofiling-only source変更でもsource fingerprintは更新されるため、`991392b8`の9 case / 24 run / cross比較は
  履歴passとして保持し、新clean subjectからM5の3 legを再採取してから最終checkを閉じる。
- clean final subject `ef5f2b8b`、source `e0e888e8…`、harness `2e6b234c…`、asset view `ebeb81af…`の
  fresh matrix job `wall-art-20260902T210312Z-2ab3b1c9`は、Intel Arc / Mesa 26.1.6 / Vulkan / X11で
  High / Medium / Low × DPI 1.0 / 1.5 / 2.0の9 / 9 caseを完了し、独立offline verifyもpassした。全PNGを目視し、
  production 96 / fallback 0、distinct mesh 6 / material 1、connector visual 192 hidden / 0 visibleと、品質低下時の
  silhouette連続を確認した。fixture / focused testと合わせ、16 mask、lifecycle、9.6 / 12.8 wu geometry契約をfresh subjectへ
  再接続した。
- 同subjectのRenderDoc job `wall-renderdoc-20260902T200533Z-c3f9b66d`はcompleted Nの2 replayを完了し、
  active mesh 6、owner 96、draw group 6、index別instance `648:30 / 720:66`のexact一致を得た。しかしcompleted 4Nの
  1回目replayがGPU fence待ちのまま600秒timeoutとなった。build cache済み・GPU process解放後のfresh再試行
  `wall-renderdoc-20260902T204607Z-30b0d6c5`も同じcase / replayで600秒timeoutとなったため、一時的な初回build負荷仮説は
  打ち切った。kernel logにGPU hang / resetはなく、両artifactをinvalidのまま保持する。4 case predicateは未合格である。
- 同subjectのfresh performance job `wall-production-performance-20260902T213112Z-837efcb8`は24 / 24 runをvalid採取したが、
  completed N p99がfallback `9.905611 ms`対production `10.506062 ms`（`+6.062%`）で`+5%` gateを超えた。
  他のcompleted 3指標とdiagnostic上のprovisional 4指標はgate内だったが、manifest生成前にfail-closedとなった。
  load average低下後の1回限りのfresh再測定 `wall-production-performance-20260902T221218Z-96e4b21b`も24 / 24 runをvalid採取し、
  completed N p95がfallback `9.171164 ms`対production `14.011493 ms`（`+52.778%`）で再失敗した。production Nの
  run 2 / 3にはそれぞれ約8.3 / 9.9秒の連続tail-latency区間があり、p50は`+3.957%`、completed 4N p95 / p99は
  `-0.078% / +0.468%`だった。単一外れ値仮説は打ち切り、閾値変更やartifact合格扱いを行わない。valid production
  manifestがないためM0 cross-subject再封印も開始しない。
- hardware起因として再試行する方針を撤回し、generation 2の実装を再監査した。scene generatorは各輪郭の直線辺を
  3〜10分割していたが、分割点はsilhouette、atlas surface分類、線形UV補間のいずれも変えず、全て共線である。
  そのため各mesh 216〜240 trianglesのうち大部分は表示へ寄与しない。M4を再開し、この分割だけを除去して
  24〜72 trianglesへ下げた新generationを作る。材質、texture、9.6 / 12.8 wu geometry、fixture、性能閾値は同時に
  変更しない。新generationでtail-latencyとRenderDoc 4Nが改善しなければ、この仮説は一度で打ち切る。
- 実装修正commit `5152778d`からgeneration 3 candidateを再生成し、isolated / end / straight / corner / t_junction / crossを
  それぞれ`24 / 24 / 24 / 36 / 48 / 72` trianglesへ削減した。6 GLBのscene gate、Khronos validator、post-export geometry、
  texture、manifest、二重生成のdeterministic rebuildは全件passし、candidate manifest SHA-256は
  `8b41a825daf84a15e331cc9e3839702151f3d88ba13c528c4aad24c98862f073`である。既存generation 2 finalとcanonical runtimeは
  変更していない。
- cleanup後のfinal helperはart-approved manifestだけを受理し、旧M4 helperはasset generationを`1`へ固定していたため、
  承認前generation 3を再レビューできなかった。旧M4 subjectへ72-triangle契約と、正のcandidate generationをplan時の
  exact manifest hashへ結合するだけの隔離検証branch `2fbb713f`を作成した。初回job
  `wall-art-20260903T010924Z-4a9f75c9`はgeneration 3の96 production / fallback 0 / 6 mesh / 16 maskを描画できたが、
  Wall以外の通常runtime asset不足をログgateが検出してinvalidとした。画像やstatusを合格証拠へ流用しない。
- 完全runtime asset viewを非上書きで補完したfresh lit job `wall-art-20260903T013625Z-b954997d`はIntel Arc / Mesa 26.1.6 /
  Vulkan / X11でstatus `valid`、独立offline verify `pass`となった。generation 3、manifest `8b41a825…`、asset-view
  `4e8cd152…`、production 96 / fallback 0、distinct mesh 6 / material 1、16 mask各6件、connector visual 192 hidden /
  0 visibleを満たし、PNG SHA-256は`3989015e…`である。generation 2の採用画像に対するnormalized MAEは全画面
  `0.00293811`、中央96 px ROI `0.01901325`で、目視上のsilhouette、lit shading、紫emissiveを維持する。manifest世代が
  変わったため旧承認は流用せず、このfresh PNGへの明示承認後にのみpendingなしfinal generationを再封印する。
- ユーザーは2026-09-03に上記generation 3のPNG SHA-256 `3989015e…`を「OKです」と明示承認した。
  art review artifactをgeneration 3 / manifest `8b41a825…` / subject `2fbb713f` / screenshotへ再結合し、承認UTC
  `2026-09-03T12:26:51Z`、artifact SHA-256 `2892f0a8…`を記録した。この承認はart選定であり、canonical
  promote承認には流用しない。
- 承認artifactを含むpendingなしfinal generation 4を`normal_decision=rejected` /
  `review_status=art_approved`で再封印した。final manifest SHA-256は`7ecdfbb0…`、coreは6 GLB＋albedo＋
  emissiveの8 file、isolated projection SHA-256は`76f7d448…`である。canonical generation 2とprimary runtime
  assetは変更していない。
- current subject `48f790bf`のclean validation worktreeへgeneration 4の8 coreだけをprovisionし、fresh matrix job
  `wall-art-20260903T122914Z-4dd4d822`をIntel Arc / Vulkan / X11で実行した。High / Medium / Low × DPI
  1.0 / 1.5 / 2.0の9 / 9 caseがstatus `valid`、独立offline verify `pass`となり、全caseでproduction 96 /
  fallback 0、distinct mesh 6 / material 1、16 mask各6件、connector visual 192 hidden / 0 visibleを満たした。
  9枚を目視し、品質／DPI固有のWall欠落、fallback混在、黒抜け、輪郭断線がないことを確認した。
- 同一subject / generation 4のperformance job `wall-production-performance-20260903T131432Z-78414216`は24 / 24 runを
  valid採取したが、最初のcompleted p95比較でfail-closedとなった。completed 4Nはp95 / p99が
  `-36.922% / -21.823%`、provisional Nは`-67.614% / -65.788%`、4Nは`-64.760% / -62.194%`と改善したが、
  completed Nだけがp95 `+190.461%`、p99 `+213.020%`で`+5%`gateを超えた。manifestは生成されず、独立
  verifyもmissing manifestを検出してinvalidである。閾値を変更せず、「共線分割だけがtail-latencyの原因」
  という仮説はこの1回で打ち切った。presentation sidecarはinitial / final同一で、revision、mesh、material、
  transformの定常再適用は否定できた。一方、helperは対照側12 runを全て先に、production側12 runを
  全て後に固定実行し、run内の連続tail区間がcompleted Nの一方だけへ偏った。invalid artifactを合格扱いせず、
  実draw経路をRenderDocで確認した後、計測順序とcompleted material経路を切り分ける。
- 上記の固定順を計測実装の欠陥として修正し、formal profileを`wall-production-performance-v2`へ更新した。
  v2は各`phase × size × run`でfallback-control / productionを隣接pairにし、pairの先行modeを交互にする。
  24 runの予定順と実完了順は`capture-order.json`へ記録し、独立verifyで完全一致を要求する。これにより
  grouped v1のinvalid値をruntime回帰の根拠へ流用せず、同一時間帯の比較でcompleted emissive経路を再評価する。

- 変更内容:
  - M4で完成・commit済みのwall-art gallery / fail-closed profileを変更せず、final commitのclean validation worktreeで実行する。code / launcher / predicate修正が必要になった時点でartifactを無効化してM4へ戻り、final commit承認からやり直す。
  - 性能用にはM0 baseline commitとM4 final commitから別々のclean worktreeを作り、同じfinal asset viewをexact copyする。full / wall-set / non-wallの3 hash、freeze済みdensity profile / Rust fixture / measurement contract hash、adapter / backend / windowを一致させる。baseline binaryは追加wall fileを参照しなくてもasset view自体はfinalと同じにする。
  - gallery 1枚に、孤立、4端、2直線、4corner、4T、cross、Door隣接を配置し、完成列と仮設列を比較できるようにする。
  - phaseを追加／撤去、仮設→完成bounce、Soul front/back、save/load rehydrate、standard / farthest player zoomに分け、launcher ACKをasset-set identityとprocess-local session / activation revisionの両方へ結んだclient-window captureを作る。
  - High / Medium / Low × DPI 1.0 / 1.5 / 2.0のGPU-visible 9 caseをcurrent-source専用profileで検証する。historical P02の
    18 case matrixはP02当時のsource / legacy mirror inventoryにのみ再実行可能であり、登録済みimmutable artifactをoffline検証する。
  - sidecarでsource fingerprint、3 asset hash、manifest authority / asset-set generation、session / activation revision、6 GLB / texture hash、load state、phase別fallback count、mask / family / rotation、owner / visual exact count、active / total mesh / material IDs、triangles、distinct mesh/material組合せ、geometry report hashと9.6 / 12.8 wu契約を検証する。steady productionはfallback 0、world-replace transitionだけは全owner fallbackの1 frameを許し、mixed countは全phase 0とする。
  - M0 baseline commit対final production、およびfinal commit内のprofile-only `force-fallback` control対productionを、`wall-density-v1`のcompleted / provisional各N / 4Nで実行する。各runのp95 / p99を先に求め、3 valid runの中央値で両比較とも`<= +5%`を要求し、MADを併記する。
    M0側は`35f1f6e3`のproduct treeへ`e792b710`のfixture-only connector mirror修正だけを載せたclean派生subjectとし、
    product差分を混ぜずに現行fixture bytesへ揃える。
  - bounded RenderDoc captureではcompleted opaqueのwall main-passを抽出し、`D_N <= 6`、`D_4N <= 6`、`D_4N = D_N`を要求する。provisional transparentは別sorted phaseで`D_4N <= 4 * D_N + 6`を要求し、count / normalized slopeを別artifactへ記録する。
- 変更候補:
  - `crates/bevy_app/src/plugins/startup/perf_scenario/`
  - `docs/rendering-performance.md`
- 完了条件:
  - [x] 9 caseのclient-window PNGとsidecarがfresh source / asset fingerprintに対してfail-closedでpassする。
  - [ ] 全16 mask、Door接続、完成／仮設、追加／撤去、completion、depth、loadが画像とstateの両方でpassする。
  - [ ] 全family / rotationのWall–Wall portが9.6 wuの同一profileで連続し、各armの局所横断で連続壁体が9.6 wu未満へ細らず、装飾が12.8 wu envelopeとcell AABBを越えない。最遠zoom-outではHighの内部色1 px以上、Medium / Lowのfinal composite silhouette連続を満たす。
  - [x] registered historical P02 artifactのimmutable locator / hashがoffline再検証でpassし、current-sourceのwall depth、
    completion bounce、Render3d visible、exactly-oneはWall専用profile / focused testでpassする。P08 sourceをP02 selectorへ偽装しない。
  - [ ] production resident mesh 6、active production material 2、finite total pool mesh 7 / material 4、steady phaseのfallback active 0、world-replace transitionのfallback-only 1 frame、全phase mixed 0、各mesh 72 triangles以下、distinct production mesh/material組合せ12以下である。
  - [x] M0 baseline commitとfinal commitのclean worktreeが同一のfinal asset viewとbyte-identicalなdensity profile / fixture / contractを使い、N=96 / 4N=384、seed、warm-up / measure、3-run集約のいずれにもdriftがない。
  - [ ] baseline対production、final `force-fallback`対productionのcompleted / provisional Capture p95 / p99中央値がそれぞれ`+5%`以内で、全run valid、MAD併記である。
  - [ ] RenderDoc上のcompleted wall main-passが`D_N <= 6`、`D_4N <= 6`、`D_4N = D_N`、provisional sorted phaseが`D_4N <= 4 * D_N + 6`を満たす。
  - [ ] native実行中のcode / profile / predicate変更が0で、source fingerprintはM4 final commitと一致する。修正が発生したrunを合格artifactへ流用していない。
  - [x] manifestはM4のpendingなしfinal generationと一致し、M1 candidate generationやA/B用optional集合のartifactをfinal証拠へ混ぜていない。validation worktreeはadopted core 9またはrejected core 8だけを含む。
  - [ ] compare開始前に空の`<sealed-artifacts>` directoryを作り、4つの固有CSVと各SHA-256を保存している。既定`comparison.csv`へ上書きしていない。
  - [x] adapter / backend / window backendがartifactに記録され、headless結果をrenderer証拠にしていない。
- 検証:
  - 専用profileの `plan` が返すdirect `kitty` launcherだけを実行し、15〜30秒間隔でstatusをpollする。
  - `PYTHONDONTWRITEBYTECODE=1 python3 .codex/skills/hell-workers-run-native-acceptance/scripts/wall_art_acceptance.py plan --repo "$VALIDATION_WORKTREE" --adapter Intel --candidate --matrix`
  - `PYTHONDONTWRITEBYTECODE=1 python3 .codex/skills/hell-workers-run-native-acceptance/scripts/wall_production_performance_acceptance.py plan --repo "$VALIDATION_WORKTREE" --adapter Intel`
  - `PYTHONDONTWRITEBYTECODE=1 python3 .codex/skills/hell-workers-run-native-acceptance/scripts/wall_cross_subject_performance_acceptance.py seal --baseline-job "$M0_JOB" --production-job "$FINAL_JOB" --output "$CROSS_SUBJECT_ROOT"`
  - `PYTHONDONTWRITEBYTECODE=1 python3 .codex/skills/hell-workers-run-native-acceptance/scripts/wall_renderdoc_acceptance.py plan --repo "$VALIDATION_WORKTREE" --adapter Intel --candidate`
  - `python3 scripts/perf.py compare --baseline <final-force-fallback-session> --candidate <final-production-session> --metric p95 --max-regression-pct 5 --min-runs 3 --output <sealed-artifacts>/force-fallback-vs-production-p95.csv`
  - `python3 scripts/perf.py compare --baseline <final-force-fallback-session> --candidate <final-production-session> --metric p99 --max-regression-pct 5 --min-runs 3 --output <sealed-artifacts>/force-fallback-vs-production-p99.csv`
  - `python3 scripts/dev.py verify`
  - `python3 scripts/dev.py cargo -- clippy --workspace --all-targets -- -D warnings`

### M6: canonical昇格、文書同期、計画close

- 変更内容:
  - stagingの承認対象、asset-set manifest、license、validator、native artifact、recovery結果をユーザーへ提示する。承認直前にcandidate hashを再計算し、M5のasset-set identityと一致しなければM4へ戻る。M5で検証したpayload manifest bytesは変更せず、release承認は別のpromotion receiptへ記録する。
  - canonical generation storeとprimary runtime mirrorについて、現在のactive pointer / receiptと参照先generationのpreimage（既存hashまたはabsent）を記録し、repositoryの`target/`外にimmutable promotion snapshotを作る。snapshot自体のmanifest / SHA-256とpointer rollback dry-runを検証してからpromote承認を求める。
  - 明示承認後だけ、allowlistにある`.blend`、6 GLB、採用texture、payload manifest / licenseとgeneration-scoped immutable promotion receiptを同一filesystem上のtemporary generation directoryへcopyする。全file / directoryをfsyncし、hash検証後にimmutable `<GEN>`へ一回renameしてgeneration-store親directoryもfsyncする。その後にだけ、唯一のmutable authority pointerをtemporary fileのfsync→atomic rename→pointer親directoryのfsyncで切り替える。kill / I/O failure時は旧pointerを維持し、すでに切替済みなら完全な新generationを維持する。固定pathのfile単位上書きや「例外が捕捉できた場合だけrestore」に依存しない。
  - canonical promote後にsource / destination / exact file listを再提示し、runtime mirror同期の別の明示承認を得る。generation-aware `scripts/sync_external_assets.py --dry-run`と実同期を同じallowlistで行い、repo側もtemporary generationのfile / directory fsync→generation rename→generation-store親fsyncを終えてから、唯一のmutable runtime authorityである`assets/manifests/wall-production-v1.wallset`をtemporary fileのfsync→atomic rename→manifest親directoryのfsyncで最後に切り替える。`--delete-missing`とunrelated asset copyを使わず、canonicalとrepoの各切替段にkillされてもactive pointerが常に完全な旧／新generationを指すことを`recover`で検査する。
  - 実同期後のprimary repositoryから新processのclean startupとsave/loadを専用wall profileでactual-window再実行し、canonical / repo mirror / runtime sidecarのhash一致、promotion receiptに承認されたasset-set identity、resident production asset、fallback 0を確認する。このpost-promote runより前にM6を完了扱いしない。
  - runtime再検証後に、アート基準、asset pipeline、building表示、性能、READMEの旧「2D pixel art」記述、親計画、workstation M4を実態へ同期する。
  - functionality / runtime dataの最終状態に対し `hell-workers-review-help-impact` Skillを再実行し、実際のplayer-visible pathから `Update required` / `No impact`を記録する。
  - 完了後は本計画を削除またはarchiveし、恒久仕様だけを正本docsへ残す。
- 変更候補:
  - `docs/art-style-criteria.md`
  - `docs/asset-pipeline-glb.md`
  - `docs/building.md`
  - `docs/rendering-performance.md`
  - `docs/assets_workflow.md`
  - `docs/plans/3d-rtt/asset-milestones-2026-03-17.md`
  - `docs/plans/development-workstation-blender-migration-plan-2026-07-29.md`
  - `README.md`
  - Help manifest / provider / coverage / approval snapshot（Help impactが`Update required`の場合だけ）
- 完了条件:
  - [ ] ユーザーのcanonical promote明示承認が記録されている。
  - [ ] canonical / repo mirrorのactive pointer preimageまたはabsent状態、参照generation、immutable snapshot、snapshot manifest / hash、rollback dry-runがpromote前に検証されている。
  - [ ] canonical generation payload / promotion receipt / repo runtime generation / runtime projectionのhashとasset-set identityが一致する。
  - [ ] allowlist外のcanonical / repo asset差分が0で、file fsync / generation-directory fsync / generation rename / generation-store親fsync / pointer file fsync / pointer rename / pointer親fsync各段のprocess-kill test後もactive pointerは完全な旧または新generationだけを指す。`recover`がorphanを検出し、rollbackはpointerを検証済みpreimageへ同じdurable atomic手順で戻す。
  - [ ] actual sync後のprimary clean startupとloadを専用actual-window profileで再実行し、promotion-receipt-approved asset-set generation、production asset resident、fallback 0、runtime hash一致を再確認している。
  - [ ] Help impact decisionと必要なHelp / docs更新が完了している。
  - [ ] `docs --write`後の2 indexをreviewし、全workspace gateがgreenである。
  - [ ] 本計画の一時情報を恒久docsへ移し、plan lifecycleを閉じている。
- 検証:
  - `python3 tools/blender_ai_workflow/scripts/promote_asset_set.py plan --manifest "$ASSET_ROOT/staging/reports/wall-production-v1.asset-set.json" --asset-root "$ASSET_ROOT" --snapshot <outside-target-snapshot>`
  - 上記plan artifactとユーザー承認後だけ、同じtoolの`apply --plan <sealed-plan>`。中断後は`recover --plan <sealed-plan>`、rollback承認後は`rollback --plan <sealed-plan>`を実行してactive pointer / generation hashを再検証
  - `python3 scripts/sync_external_assets.py --source "$ASSET_ROOT/generations/<GEN>/exports" --dest "$PWD/assets" --manifest "$ASSET_ROOT/generations/<GEN>/manifest/wall-production-v1.asset-set.json" --receipt "$ASSET_ROOT/generations/<GEN>/authority/promotion-receipt.json" --selection core --dry-run`
  - 上記dry-runと同じ引数から`--dry-run`だけを外した実同期（別途ユーザー承認後）
  - `hell-workers-run-native-acceptance` Skillのpost-promote wall-art `plan`が返すdirect `kitty` launcher
  - `python3 scripts/dev.py docs --write`
  - `python3 scripts/dev.py docs --check`
  - `python3 scripts/dev.py verify`
  - `python3 scripts/dev.py cargo -- clippy --workspace --all-targets -- -D warnings`
  - `git diff --check`

## 6. 受入マトリクス

| 対象 | 自動test | actual-window | 合格条件 |
| --- | --- | --- | --- |
| geometry / 厚さ | port collar slice / junction union / isolated / AABB validator | standard、14 px/tile、最大zoom-out 5の全回転 | 局所横断の公称・連続最小9.6 wu、装飾外形12.8 wu以下、共通port。straight standard外形0.90 tile／最大1.00 tile。14 px/tileで内部色、最遠Highで内部色1 px以上、最遠Medium / Lowでfinal silhouette連続 |
| 16 topology | pure table 16件 | gallery全形状 | family / quarter turnと見た目が一致 |
| Door接続 | DoorState 3種、blueprint含む | Wall-Door列 | stateに関係なくmask接続し撤去で近傍更新。Wall側portは9.6 wu、placeholder leafとの無隙間jambは要求しない |
| lifecycle | add / cancel / deconstruct / completion | phase capture | stale mesh、重複visual、回転消失なし。同frame `GlobalTransform`まで一致 |
| rehydrate | save candidate / presentation test | load前後capture | `Last` frameはfallback-onlyでlocal / `GlobalTransform`が正しいworld位置、次frameはproduction-only。同じmask / family / rotation / exact countでmixed 0 |
| material | finite handle ID test | 完成／仮設列 | geometry不変、materialだけ遷移 |
| depth / shadow | MeshTag / render layer invariant | Soul front/back、地面shadow | P02契約を維持 |
| asset failure | missing / one-failed / late-ready / unauthorized / restart test | failure smoke | 全wall fallback、消失／mixedなし。通常buildのfailed回復はrestart-only |
| asset production | manifest authority / hash / load state / bounds / tri | 9 case | promotion receipt承認済みまたはprofile明示candidate asset-set identityの6 mesh＋採用textureがGPU描画済み、fallback 0、72 tri以下 |
| performance | `wall-density-v1` checksum、steady-state writes、handle / pair count | 3-run Capture / RenderDoc | topology write 0、production mesh 6、active material 2、total pool 7 / 4、pair 12以下、baseline / control比p95 / p99中央値`<= +5%`。completedは`D_N,D_4N<=6`かつ同数、transparentは`D_4N<=4D_N+6` |
| art / color | candidate metadata、OCIO / CIEDE2000 verifier | OCIO-valid client PNG | config path / hashと`fallback=false`を証明し、順次A/Bの勝者をRough Vector Sketchとしてユーザー承認 |

## 7. リスクと対策

| リスク | 影響 | 対策 |
| --- | --- | --- |
| OCIO fallbackの色を正しいと誤認 | runtimeとの色差を本番化 | config path / hash、runtime version、`fallback=false`の陽性証明とoffline CIEDE2000再検証が揃うまで色承認とpromoteをblock |
| GLB `SceneRoot`で複数childをspawn | exactly-one、cleanup、draw call、MeshTagが破綻 | primitive `Handle<Mesh>`を直接読み、1 mesh / 1 primitiveをvalidatorとtestで固定 |
| transform syncがtopology回転を上書き | 数frame後やbounce後に向きが戻る | owner / topology / bounceをpure helperで一括合成し、順序testを追加 |
| final transformがBevy propagationより遅い | 画像と`GlobalTransform`が1 frame古い | chainを`TransformSystems::Propagate`より前へ置き、same-frame local / global testを固定 |
| 撤去後の旧gridを取得できない | 隣接壁がstale形状のまま | entity→last-known grid indexを保持し、Removedを旧位置からdirty化 |
| world replacement後も旧indexが残る | 再利用Entityへのalias、重複connector | `hw_visual` resetでindex / topology revision / cache、`bevy_app` resetでactivation stateを分担してclearし、`Last` fallback spawnと次frameの一度だけの再構築・一括復帰を固定 |
| asset readyだけでM3前にproduction化 | 全wallが誤ったfamily / rotationになる | M2は`Eligible`まで、manifest authority＋`topology_ready`をM3のatomic activation条件にする |
| 6 meshまたはshared Imageの一部だけload | wallごとにstyle混在、壁消失 | asset-set identityとactivation revisionを分けたall-or-nothing readiness、procedural fallback、native fallback 0 gate。failed回復は同じasset generationのrestart-only |
| promote中にprocess kill / 電源断 | 固定pathが旧新混在しmanifestと不一致 | immutable generationとreceiptをfsyncし、directory rename後にstore親もfsyncしてから、唯一のactive pointerをfile fsync→rename→親fsyncで切替。全kill pointのrecover / rollback testを必須化 |
| ignored staging assetが通常起動へ紛れ込む | 未承認generationを本番表示 | 通常起動はart-approved payload＋approved promotion receiptのprojectionだけ、candidateは隔離profileのexact identity opt-inだけを許可 |
| validation assetがprimaryやtracked shaderを汚す | formal subjectとasset証拠が一致しない | clean worktree内のignored assetだけへmanifest allowlistでoverlayし、tracked hash / git clean / 3 asset hashを毎run確認 |
| 鉄トゲや石目をgeometry化しすぎる | triangle / silhouette noise / batch増 | silhouette-critical部だけgeometry、detailはshared texture、72 tri hard cap |
| 壁厚が細すぎて遠景で輪郭だけになる | 石積みの塗りと接続が消える | Highの最大zoom-out 5で内部色1 px以上から9.6 wuを固定し、14 px/tile / Lowをactual-window受入へ含める |
| 壁厚や装飾が太すぎて箱へ戻る | Rough Vector Sketchの平面感と隣tileの読解性が低下 | 公称9.6 wu、装飾12.8 wuをvalidatorで上限化し、straight E-Wの横断silhouetteだけを対象に縦補正後の全投影高を通常0.90 tile / 最大1.00 tile以内にする |
| 細いvisual脇が歩ける空間に見える | 32 wu cell全体を塞ぐgameplayと見た目が食い違う | 11.2 wuは建築cell内余白と明記し、placement / selection maskとgalleryでblocked cellを同時表示する |
| 暫定Door leafへ壁厚を合わせる | 5.76 wu placeholder値が本番asset契約へ固定され、向き／jamb不足を隠す | Wall portは9.6 wuを正本にし、Door seamは別asset scope。Wall–Door受入はmask / centerlineまでに限定 |
| emissive裂け目がgameplay lightに見える | 室内照明ルールと混同 | surface emissive限定、PointLight / emitterを禁止、Help gameplay説明は変更しない |
| unlit化でshadow / depthが退行 | actorと壁の前後関係や地面shadowが崩れる | 同じmaterial型でA/Bし、P02 actual-window回帰を採用gateにする |
| normal / outline調整を惰性で反復 | 原因不明のまま工数増大 | tangent / linear / `+Y` technical gate後、仮説ごとに一度のA/B。変化なし／基準未達なら打切り、outlineは別計画 |
| lit / unlit / normal / qualityを直積比較 | 交絡して採否理由が不明、artifactが肥大 | lit/unlit→lit winnerのnormal→final winnerのfull matrixという順次funnelに固定 |
| performance fixtureがbaseline後に変わる | `+5%`比較が無意味 | `wall-density-v1` contract hashをM0で凍結し、変更時はbaselineから再採取。3-run中央値 / MADとdraw predicateをexact化 |
| external binaryがGit差分に現れない | fresh cloneで欠落 | canonical manifest、runtime asset view hash、native residency、fallbackをfail-closed検査 |
| 旧2D textureとactive 3Dを混同 | 見えないassetだけを修正 | production `Building3dVisual`経路を受入fixtureで直接証明 |

## 8. ロールバック方針

- コード側:
  - production pool選択を無効にし、保持したprocedural `Cuboid` fallbackへ全wallを一括で戻せるようにする。
  - topology resolver自体は2D blueprint correctnessにも使うため、asset rollbackと分離する。resolver不具合時は最後の承認済みmask tableへ戻す。
- asset側:
  - canonical promote前はstagingと一時validation worktreeを隔離したまま廃棄または再制作でき、primary `assets/`とcanonicalへ影響しない。一時worktreeの削除前にartifact / manifestの保存先を検証する。
  - promote直前にcanonical / repo mirrorのactive pointerと参照generationのpreimageまたはabsent状態をimmutable snapshotへ保存する。中断時は`recover`でactive pointerが完全な旧／新generationを指すことを検証し、orphan temporary generationを隔離する。
  - promote後の問題はpayloadをfile単位で戻さず、snapshot内の検証済みpreimage pointerへatomic rollbackする。repo mirrorも対応する完全なgenerationを検証してからpointerだけを戻し、旧generationはpost-promote受入完了まで削除しない。
  - broad削除、`--delete-missing`、未確認の上書きを行わない。
- release判断:
  - missing、load failure、GPU不具合が出たbuildはfallbackを使ってplayableに保つが、production壁完成とは報告しない。
  - rollback後もlogical Wall、save、Room、遮光データは変更しないため、save migrationは不要とする。

## 9. AI引継ぎメモ

### 現在地

- 進捗: `M0〜M3完了、M4未着手`
- 完了済み:
  - current active wall経路、2D connection system、material / transform / MeshTag、save rehydrate、external asset workflowを棚卸し済み。
  - 6 mesh / 16 mask、finite pool、fallback、native受入の実装境界を本書で固定済み。
  - production wallの局所横断における公称・連続最小厚9.6 wu、装飾外形上限12.8 wu、共通接続portを投影式と遠景pixel下限から固定済み。
  - M0のBlender geometry / color fixture、offline verifier、Rust `wall-density-v1` fixture、Capture profileを実装済み。
  - 最初のnative Captureをfail-closedで棄却し、duration clock誤判定と通常初期world混入を修正済み。
  - subject `26dcb5a3`からcompleted / provisional各N / 4Nの全12 runを採取し、専用verifyでpass。3-run中央値 / MADと3 fingerprintを封印済み。
  - historical P02再実行とcurrent source calibrationの契約差を確定し、current fallback専用actual-window profileを実装済み。
  - subject `ad614903`のcurrent fallback actual-windowを採取し、isolated WallのUI非重複ROI、fallback resident、
    exact owner、client PNG、raw sidecar、全fingerprintを独立verifyで封印済み。
  - subject `e149918c`のwall RenderDoc 4ケースを採取し、全8 replay一致、completed / provisionalとも
    `D_N=1 / D_4N=1`、rendered instance 96 / 384でdraw predicateを封印済み。
  - M1 generation 1の6 GLB＋shared texture＋candidate normalをproduction manifestへ封印し、primary外のclean
    validation worktreeへallowlist配置済み。canonical / primary runtime asset mirrorは未変更。
  - M2のcandidate / release `.wallset` projector、promotion receipt検証、二段階finite pool、exact candidate identity gate、
    optional normal分離、fresh-process recovery testを`405f6e9c`〜`546f4007`で実装済み。
  - subject `4ebc23a8`のclean validation worktreeからBevy 0.19実loader / readiness systemをheadless実行し、
    6 primitive、sRGBのalbedo / emissive、linearのnormal、candidate projection `c13d6134...`、raw bounds / triangle、
    unauthorized fallback、exact identityのEligible、topology gateのReadyToApply、steady-state revision / material handle不変をpass済み。
  - `116d1ff9`で`ReadyToApply`と同frameの新規WallもM3 apply前はfallback bundleをexactly oneで維持するtestをpassし、
    batchごとのHelp impact decisionを完了してM2を閉じた。
  - M3のcanonical `(N,S,W,E)` maskと6 family / quarter turn resolverを`819ca26d`で実装し、completed Door connectorの
    実経路を`4a728b06`で固定済み。後続M3 batchで3D mesh適用と全lifecycle統合まで完了した。
  - `5f59d8b3`でbidirectional connector index、resolved `WallTopologyState`、bounded old/new dirty、Building＋Blueprint
    coalesce、steady 0-write、world-replace full rebuildを実装済み。2D consumerは同じresolved maskへ移行した。
  - registered historical P02とcurrent fallback actual-windowを`wall-reference-locators-v1`で分離し、
    index / ledger / referenced artifactのidentityとhashをoffline verifierで封印済み。
  - canonical orientation / bounds / pivot / placementをgeometry JSONとcontract-hash付きSVGへ固定し、
    +Y quarter turnから16 maskを再導出するunit testをpass済み。
  - profiling-onlyのBevy 5 patch actual-window phaseと専用fail-closed launcherを実装済み。通常current-wall
    profileとは二重鍵で排他にし、dedicated final Camera2d、単一X11 clientのexact 320×96 crop、nonce / ACK、
    MainCameraへの明示UI隔離、Blender reference / OCIO proof / candidate metadata / offline Delta E再検証を
    同じartifactへ封印する。
  - final subject `35f1f6e3`から色校正、Capture 12 run、RenderDoc 4 caseを再採取し、全artifactの独立verifyをpass。
    M0のsource / harness / asset viewを同じfingerprintへ凍結済み。
- 未完了:
  - M4のlit / unlitとoptional normalの順次native A/B、全16 mask / lifecycle gallery、ユーザーの主観目視承認。
  - production assetはcanonical / primary `assets/`へまだ書き込んでおらず、通常gameplayのWallはfallbackのままである。

### 次のAIが最初にやること

1. M4のlit / unlit一軸A/B用gallery / fail-closed launcherのcheck / clippy / verify / Help reviewを終え、
   scoped commitを作る。承認済みのため、そのclean candidate worktreeでnative lit / unlit captureを開始する。
   normalはtechnical reject済みなので比較せず、canonical / primary assetへはまだ書き込まない。2枚のclient
   captureとexact sidecarをユーザーへ提示し、production cameraでのwinner承認を待つ。

### ブロッカー/注意点

- Blender同梱OCIO config `2.5` / runtime `2.4.2` mismatchは残るためdefault configで色承認しない。
  壁5 patchはsealed profile 2.1 config＋`--require-ocio-positive`だけを正式経路とする。
- `source/` / `exports/`が空なのは新規authoring baselineとして正常で、repo GLBを偽のBlender原本へ逆変換しない。
- parent planの100 triangle / active `build_progress` / Light Field目視条件は現runtimeとずれている。壁の実行基準は本書の72 triangle cap、現行施工mask / material transition、Light Field未sample維持とする。
- native受入は `hell-workers-run-native-acceptance` Skillのdirect `kitty` launcherとfail-closed artifact監視を使う。GUI権限をユーザーへ繰り返し依頼しない。
- functionality、code、runtime dataを変更したら完了報告前に必ず `hell-workers-review-help-impact` Skillを使う。gate passだけでreview済みにしない。
- agentを使う場合はread-only explore / code-reviewに限定し、file editはmain agentが `apply_patch`で行う。

### 参照必須ファイル

- `docs/art-style-criteria.md`
- `docs/world_lore.md`
- `docs/assets_workflow.md`
- `docs/blender-setup.md`
- `docs/rendering-performance.md`
- `docs/indoor_lighting.md`
- `docs/plans/3d-rtt/asset-milestones-2026-03-17.md`
- `docs/plans/development-workstation-blender-migration-plan-2026-07-29.md`
- `crates/hw_visual/src/wall_connection.rs`
- `crates/hw_visual/src/visual3d.rs`
- `crates/bevy_app/src/plugins/startup/visual_handles.rs`
- `crates/bevy_app/src/systems/jobs/building_completion/spawn.rs`
- `crates/bevy_app/src/systems/visual/building3d_cleanup.rs`

### 最終確認ログ

- 計画ブラッシュアップ後 `python3 scripts/dev.py docs --write`（plans / proposals両index review）: `pass (2026-09-01)`
- 計画ブラッシュアップ後 `python3 scripts/dev.py docs --check`: `pass (2026-09-01)`
- 計画ブラッシュアップ後 `git diff --check`: `pass (2026-09-01)`
- M0初期tooling `PYTHONDONTWRITEBYTECODE=1 python3 -m unittest discover -s tools/blender_ai_workflow/tests -v`: `pass (12 tests, 2026-09-01)`
- M0初期tooling `ruff check tools/blender_ai_workflow/scripts tools/blender_ai_workflow/tests`: `pass (2026-09-01)`
- M0 Rust fixture `python3 scripts/dev.py cargo -- test -p bevy_app@0.1.0 --features profiling wall_density -- --nocapture`: `pass (5 tests, 2026-09-01)`
- M0 Rust fixture `python3 scripts/dev.py cargo -- clippy -p bevy_app@0.1.0 --all-targets --features profiling -- -D warnings`: `pass (2026-09-01)`
- M0 Capture harness `PYTHONDONTWRITEBYTECODE=1 python3 scripts/perf.py self-test`: `pass (2026-09-01)`
- M0 Capture harness `PYTHONDONTWRITEBYTECODE=1 python3 .codex/skills/hell-workers-run-native-acceptance/scripts/native_acceptance.py self-test`: `pass (2026-09-01)`
- M0 Capture harness `PYTHONDONTWRITEBYTECODE=1 python3 .codex/skills/hell-workers-run-native-acceptance/scripts/wall_density_acceptance.py self-test`: `pass (2026-09-01)`
- M0 Capture harness exact matrix dry-run: `pass`。未コミット差分でnative `plan`が`blocked`になることも確認（2026-09-01）
- M0最初のnative Capture（subject `a69ea3df`）: `fail-closed / artifact invalid`。Small 3 runのreal 30 s / 60 s rawは取得したがvalidatorが停止中virtual clockを誤判定し、Medium 3 runは通常初期facilityと固定cell `(82, 57)`が衝突。合格baselineへ不使用（2026-09-01）
- M0 native修正後 `PYTHONDONTWRITEBYTECODE=1 python3 scripts/perf.py self-test`: `pass`。wall-density / indoor-lightはreal duration＋virtual 0、通常workloadはvirtual durationを検証（2026-09-01）
- M0 native修正後 focused Rust config test: `pass (1 test, 2026-09-01)`。enabled wall-densityだけが通常初期resource / facility / regrowth targetを省略する隔離worldを要求。
- M0 native再試行（subject `60328e60`）: completed N / 4Nの各3 runは`pass`。provisional N / 4Nの各3 runは
  既存structural prepassの`PREPASS_FRAGMENT` guard欠落による同一shader compile errorで`fail-closed`。
  Bevy 0.19標準shaderを一次情報として修正し、このartifactも合格baselineへ不使用（2026-09-01）。
- M0 native再々試行（subject `749118d3`）: completed N / 4Nの各3 runは`pass`。provisional 6 runは
  depth-only `MAY_DISCARD`用の返値なしfragment不足によるValidation RenderErrorで`fail-closed`。
  Bevy 0.19標準prepassの`#else` entry pointまで一致させ、artifactは合格baselineへ不使用（2026-09-01）。
- M0 native正式Capture（subject `26dcb5a3`）:
  `target/native-acceptance/wall-density-20260901T042252Z-36b45916`は全12 run valid、teardown warning 0、
  launcherの独立verifyも`capture_runs=12 / status=pass`。p95 / p99中央値（MAD）はcompleted
  N=`8.683672 (0.034590) / 9.378522 (0.022487)` ms、4N=`26.202343 (3.433323) / 40.219051 (1.202868)` ms、
  provisional N=`10.629657 (0.887287) / 13.332263 (1.016646)` ms、4N=`10.495838 (1.323651) / 13.341914 (1.645682)` ms。
  `draw_groups=not-collected`のためCapture baselineとしてのみ採用（2026-09-01）。
- M0 P02 reference初回試行（subject `1c136ca5`）:
  `target/native-acceptance/p02-presentation-20260901T044343Z-344a5e7b`は最初のHigh / DPI 1 / Render3d visibleで
  invalid。P02時点のlegacy Door child SpriteのOpen画像handleを要求し、semantic Door state / WorldMap owner /
  passabilityは一致（2026-09-01）。
- M0 historical P02仮説確認（subject `f2699e9d`）:
  `target/native-acceptance/p02-presentation-20260901T044925Z-ec7236c5`は同caseのproduction phase PNG 10枚を
  採取した後、Python frozen期待表がDoor / Tank / MudMixerのlegacy child Sprite不足を検出してinvalid。
  current P08 sourceへP02値を上書きする仮説を打ち切り、Rust validatorの片側緩和を復元。registered historical
  P02 artifactとcurrent wall calibrationを分離する（2026-09-01）。
- M0 current Wall calibration tooling:
  Rust probe focused test `wall_actual_window` 2件、`wall_art_acceptance.py self-test`、`perf.py self-test`、
  `native_acceptance.py self-test`がpass（2026-09-01）。
- M0 current Wall calibration初回試行（subject `e53195ed`）:
  `target/native-acceptance/wall-art-20260901T051811Z-94e51f8b`は、正式12-run laneの引数validatorへ
  single-case条件を渡してcapture開始前にinvalid。専用selectorを追加し、正式laneの条件は維持した（2026-09-01）。
- M0 current Wall calibration第2試行（subject `26bcdb00`）:
  `target/native-acceptance/wall-art-20260901T053432Z-8d0e2f43`は、Rust側が正式30秒 / 60秒だけを
  許可して起動時にinvalid。専用Rust flagと`HW_WALL_ART_ACTUAL_WINDOW=1`の二重鍵へ10秒 / 10秒契約を
  結び、片側だけではfail-closedにした（2026-09-01）。
- M0 current Wall calibration第3試行（subject `5816b028`）:
  `target/native-acceptance/wall-art-20260901T053708Z-6c99466c`は自動verifyでpassしたが、原寸目視で
  ordinal 0のROIが下部UI barと重なることを確認したため正式referenceへ不採用。colored pixel判定を
  UIが代替できないよう対象とcapture regionを固定した（2026-09-01）。
- M0 current Wall calibration正式採取（subject `ad614903`）:
  `target/native-acceptance/wall-art-20260901T055138Z-f93b0b12`は独立verifyで
  `screenshots=1 / status=pass`。source fingerprint
  `08a71968ca2582bcfd7d7aef575f256e6f8702286108cf292badbbad4940f678`、harness fingerprint
  `c333eeb53db6343253de28abeefbb05d98f47ba51653dcde092bb9439017c2d9`、asset-view fingerprint
  `ee5acdc8214b61898f662f2fb502f07559309d322fb5e234d37159b63aed7643`、PNG SHA-256
  `d69049a2b406acce7ab11537e9bef9acec63231c65e0fb016e40fdf7f30b8cc4`を封印。isolated Wall
  ordinal 64 / grid `(22, 17)` / mask `0000`、ROI `(416, 518, 96, 96)`はfixed UI chromeと非重複、
  raw performance validationも理由0でpass（2026-09-01）。
- M0 Wall RenderDoc初回（subject `e0f9e8a8`）:
  `target/native-acceptance/wall-renderdoc-20260901T064623Z-2f98da48`はcompleted Nをcaptureし2 replayしたが、
  Wall main passが最終Scene targetへ直接writeするという未検証前提で候補0となりinvalid。failure sidecarに条件別の
  bounded draw inventoryがなかったため合格証跡へ不使用（2026-09-01）。
- M0 Wall RenderDoc診断run（subject `842860a1`）:
  `target/native-acceptance/wall-renderdoc-20260901T065924Z-18959771`で、Wallは`pass-0003 / event 298 /
  indexed 36 / instances 96 / fragment+depth`の中間color targetへ一括描画され、`pass-0004 / event 368`の
  fullscreen triangleが`hell-workers-rtt-scene`へ合成することを確定。最終target直接write仮説を一回で打ち切り、
  このdiagnostic artifactも合格証跡へ不使用（2026-09-01）。
- M0 Wall RenderDoc正式採取（subject `e149918c`）:
  `target/native-acceptance/wall-renderdoc-20260901T070200Z-e09c0760`は独立verifyで`cases=4 / status=pass`。
  completed / provisionalとも`D_N=1 / D_4N=1`で全predicateをpassし、rendered instanceも各96 / 384で
  checkpointed owner全件と一致。4 RDCは各2 replayがbyte-identicalで、manifest SHA-256は
  `cf590e7e2e6e9d9eadc47292bd21ca1c53b59274ae3941d7006cfe2984786969`、source fingerprint
  `b8dba6fb34c7a1566f4a3adc3ed599c76d5f10d8470efcf3da8dfb67902ce26f`、harness fingerprint
  `538e7b6958d68876cb68a6e6de9ecdca5b5b3e1a3ec5f29d7d64b3e8062e1d31`、binary SHA-256
  `0f3c83b698f27a70072c517f6f263d00b96736b30de6a9150bef61261afac1c8`を封印（2026-09-01）。
- M0 5 patch色校正初回（subject `b142820f`）:
  `target/native-acceptance/wall-color-20260901T080247Z-4a4d4ccc`は実client captureまで完走したが、
  Bevy 0.19のdefault UI camera選択によりPause UIが中央3 patchへ混入し、平均Delta E 2000
  `16.092317`でinvalid。色値の調整を行わずMainCameraへUI targetを固定し、このartifactは不採用（2026-09-01）。
- M0 final subject色校正（subject `35f1f6e3`）:
  `target/native-acceptance/wall-color-20260901T081824Z-e92a7d9c`は独立verifyで`status=pass`。
  4 base patchの個別／平均Delta E 2000は全て`0.0`、emissive luminance liftは`0.07313`、
  OCIO validation / active config cache ID / `fallback=false`を封印（2026-09-01）。
- M0 final subject Capture（subject `35f1f6e3`）:
  `target/native-acceptance/wall-density-20260901T083037Z-e5dced2c`は全12 run valid、teardown warning 0、
  独立verifyで`capture_runs=12 / status=pass`。3-run中央値 / MADと全fixture checksumを同じsource / harness /
  asset-view fingerprintへ封印（2026-09-01）。
- M0 final subject RenderDoc（subject `35f1f6e3`）:
  `target/native-acceptance/wall-renderdoc-20260901T084945Z-4d218c02`は独立verifyで`cases=4 / status=pass`。
  completed / provisionalとも`D_N=1 / D_4N=1`、全8 replay一致でM0 draw predicateを完了（2026-09-01）。
- 実装時 `python3 scripts/dev.py check`: `pass (2026-09-01)`
- 実装時 `python3 scripts/dev.py cargo -- clippy --workspace --all-targets -- -D warnings`: `pass (2026-09-01)`
- M3 atomic presentation `python3 scripts/dev.py cargo -- test -p bevy_app@0.1.0 wall_presentation --lib`:
  `pass (8 tests, 2026-09-02)`。family / material / quarter turn、2-owner一括fallback、steady 0-write、
  deferred topologyから同frame `GlobalTransform`、identity mismatch、連続reset、material-only completion、owner transform合成、
  real Door add / remove producer、completion bounceから同frame`GlobalTransform`までを検証。
- M3 lifecycle focused test:
  `python3 scripts/dev.py cargo -- test -p bevy_app@0.1.0 wall_construction --lib`は`pass (5 tests, 2026-09-02)`、
  `normal_and_instant_placement_enter_the_same_topology_route`は`pass (1 test, 2026-09-02)`。normal / Instant Build、
  framing前後cancelから同frame topology更新、2 tileのframing / coating / completion、completion promotion / bounce開始を検証。
- M3 commit / teardown timing focused test:
  `normal_and_instant_placement_enter_the_same_topology_route`を`PlacementFeedbackSet::Commit`内のproducerへ強化し、
  `wall_commit_removes_the_connector_in_the_same_post_update`とともに各`pass (1 test, 2026-09-02)`。配置／解体の同frame topology更新を検証。
- M3 world replacement focused test:
  `python3 scripts/dev.py cargo -- test -p bevy_app@0.1.0 systems::save::transaction::tests --lib`は`pass (16 tests, 2026-09-02)`、
  `python3 scripts/dev.py cargo -- test -p bevy_app@0.1.0 rehydrate --lib`は`pass (85 tests, 2026-09-02)`。normal / rollback /
  recovery-only、fallback-only Last相当frame、次frame production-only、旧Entity除去、single rebuild / steady 0-writeを検証。
- M3 topology / fallback spawn focused test:
  `python3 scripts/dev.py cargo -- test -p hw_visual wall_connection --lib`は`pass (8 tests, 2026-09-02)`、
  `fallback_wall_spawn_has_exactly_one_visual_mesh_material_and_tag`と
  `rehydrated_wall_starts_in_visible_fallback_at_its_world_position`は各`pass (1 test, 2026-09-02)`。
- M3 sealed candidate geometry gate: validation worktreeの6 GLBを`validate_wall_glb`で直接decodeし全familyが`pass`。
  manifest SHA-256一致、port collar `-4.8..+4.8 wu`、Y `-16..+16 wu`、corridor union / isolated中心断面、
  216〜240 triangles、UV0を確認した。`all_sixteen_masks_match_the_sealed_geometry_fixture`と
  `HW_WALL_ASSET_TEST_ROOT=<isolated-assets> ... isolated_candidate_loads_six_real_primitives_with_valid_runtime_bounds -- --ignored --exact`
  も各`pass (1 test, 2026-09-02)`。
- M4 OCIO / normal preflight: `verify_color_calibration.py`でsealed Blender / Bevy pairを再検証し、保存済み
  `verification.json`とbyte-identical SHA-256 `470a6e06...`、mean Delta E 2000 `0.0`でpass。
  `wall-production-v1.textures.json`はnormalのlinear / `+Y`をpassする一方、同じsealed 6 GLBのdirect decodeは
  全件`tangent_present=false`を確定したためnormal A/Bをtechnical reject（2026-09-02）。
- M3 atomic presentation実装後 `python3 scripts/dev.py check`と
  `python3 scripts/dev.py cargo -- clippy --workspace --all-targets -- -D warnings`: `pass (2026-09-02)`。
- M3 atomic presentation batchのHelp実経路判断: `No impact`。壁のmesh / material / rotation選択だけを変更し、
  操作、workflow、成立条件、label、tooltip、shortcut、setting、notification、gameplay ruleは不変。
  `scripts/check_help_impact.py`とtooling unit test 24件がpassし、commit `ee1053c2`へexact trailerを記録（2026-09-02）。
- M3 normal construction topology / lifecycle test batchのHelp実経路判断: `No impact`。施工tileの既存visual topology参加と
  presentation orderingの検証を追加したが、操作、workflow、成立条件、label、tooltip、shortcut、setting、notification、gameplay ruleは不変。
  `scripts/check_help_impact.py`とtooling unit test 24件がpassし、commit `0ad6706c`へexact trailerを記録（2026-09-02）。
- M3 placement / cancel / bounce lifecycle test batchのHelp実経路判断: `No impact`。既存のnormal / Instant Build配置、framing前後cancel、
  completion bounceをpresentation契約へ接続する検証だけを追加し、操作、workflow、成立条件、label、tooltip、shortcut、setting、notification、
  gameplay ruleは不変。`scripts/check_help_impact.py`とtooling unit test 24件がpassし、commit `54359061`へexact trailerを記録（2026-09-02）。
- M3 placement commit / deconstruction timing test batchのHelp実経路判断: `No impact`。既存systemの同frame visual topology更新を固定しただけで、
  操作、workflow、成立条件、label、tooltip、shortcut、setting、notification、gameplay ruleは不変。
  `scripts/check_help_impact.py`とtooling unit test 24件がpassし、commit `729db30d`へexact trailerを記録（2026-09-02）。
- M3 multi-tile phase integration test batchのHelp実経路判断: `No impact`。既存の2 tile建設phaseとvisual ownershipを検証しただけで、
  操作、workflow、成立条件、label、tooltip、shortcut、setting、notification、gameplay ruleは不変。
  `scripts/check_help_impact.py`とtooling unit test 24件がpassし、commit `d0e23996`へexact trailerを記録（2026-09-02）。
- M3 world replacement integration test batchのHelp実経路判断: `No impact`。test-only rehydrate accessと既存transactionの検証だけで、
  load / recovery操作、workflow、成立条件、label、tooltip、shortcut、setting、notification、gameplay ruleは不変。
  `scripts/check_help_impact.py`とtooling unit test 24件がpassし、commit `122e9d47`へexact trailerを記録（2026-09-02）。
- M3全体 `python3 scripts/dev.py verify`: 初回は既存tracked `scripts/check_crate_dependencies.py`と
  `scripts/perf_tool/wall_renderdoc_extract.py`がshebang付き`100644`であるrepository hygiene違反を検出した。
  blobを変更せずGit modeだけを`100755`へ修正したcommit `365bcfb9`後、profiling testがWallを旧generic
  transform writerへ接続していた回帰を検出。実runtimeと同じ`WallPresentationApplySet`経路へtestを移し、
  Clippyの固定collectionも修正したcommit `7d51a374`後に全quality gateがpass（2026-09-02）。
- M3 verify回帰修正batchのHelp実経路判断: `No impact`。Wall bounceのtest-only scheduleを既存presentation ownerへ
  合わせ、固定test collectionを`Vec`から配列へ変えただけで、操作、workflow、成立条件、label、tooltip、shortcut、
  setting、notification、runtime data、gameplay ruleは不変。`scripts/check_help_impact.py`とtooling unit test 24件がpassし、
  commit `7d51a374`へexact trailerを記録（2026-09-02）。
- M0初期batchのHelp実経路判断: `No impact`。開発用fixture / calibration tooling / test / docsだけで、
  通常ゲームの入力、表示、建築成立条件、runtime data、プレイヤー向け文言は不変（`HELL_WORKERS_DIFF_BASE=HEAD`でgate pass）。
- 初期M0 toolingはユーザー指示により`06826fd2`へ中間commit済み。ただしCapture harnessを含む凍結済みbaseline commitではない。
- M4 final generation 2実asset loader:
  `assets::wall_asset_set::tests::isolated_candidate_loads_six_real_primitives_with_valid_runtime_bounds`は
  `1 passed / 622 filtered`。試験のgeneration 1固定をmanifest実値へ修正したcommitは`f787be1a`（2026-09-02）。
- M4 final fixed-lit actual-window:
  `target/native-acceptance/wall-art-20260902T143128Z-a98b763e`はstatus `valid`、独立verify `pass`、
  screenshot `fae394ed…`、Intel Arc / Mesa 26.1.6 / Vulkan / X11、generation 2、production 96 / fallback 0（2026-09-02）。
- M5 historical P02 current-source誤用確認:
  `target/native-acceptance/p02-presentation-20260902T150153Z-1caa5981`は既知のlegacy Door mirror条件で最初のcaseを
  fail-closed。M0の同仮説を再調整せず、current-source 9 case Wall matrixとregistered historical locatorへ分離（2026-09-02）。
- M5 Wall matrix初回:
  `target/native-acceptance/wall-art-20260902T152521Z-8d678312`はHigh / DPI 1.0後、既存single-case CLI契約が
  High / DPI 1.5を拒否してinvalid。描画値調整へ進まず、専用authorization flagへ入口を分離（2026-09-02）。
- M5 Wall matrix再実行:
  `target/native-acceptance/wall-art-20260902T153105Z-6723db42`はRust config側の同固定条件を検出してinvalid。
  launcher / Python / Rustの専用authorizationを対にし、次commit / fingerprintで再実行する（2026-09-02）。
- M5 Wall matrix確定:
  `target/native-acceptance/wall-art-20260902T153723Z-b52a734e`は9/9 case、status / offline verifyともpass。
  全caseでlit generation 2、production 96 / fallback 0、6 mesh / 1 materialを確認し、全PNG目視完了（2026-09-02）。
- M5 historical P02 locator:
  registered subject `6ea0bf99` / attempt `54d85a63-e237-4501-a0d0-33c1d0a29f3b`のindex、ledger、formal、
  RenderDoc payloadをcurrent P08へ読み替えずoffline verify `pass`（2026-09-02）。
- 未解決エラー: なし。M5の性能比較、RenderDoc再採取、phase / loadを含む拡張回帰は未実行であり、完了条件として残る。

### Definition of Done

- [ ] M0〜M6の全完了条件を満たす。
- [ ] 全16 topologyと全production lifecycleがunit / integration / actual-windowで合格する。
- [ ] 6 production mesh、active production material 2、finite total pool mesh 7 / material 4、steady fallback 0、world-replace fallback-only 1 frame、mixed 0、72 triangles / mesh以下、mesh/material組合せ12以下を証拠化する。
- [ ] 全16 maskを6 mesh＋該当quarter turnへ写像した全caseで、各armの局所横断における公称・連続最小厚9.6 wu、装飾外形12.8 wu以下、境界port同一profileを自動検査し、standard / 14 px per tile / 最大zoom-out 5の品質別基準で目視合格する。
- [ ] `wall-density-v1`のN=96 / 4N=384、3 valid run中央値でbaseline / control比Capture p95 / p99 `<= +5%`、completed `D_N,D_4N<=6`かつ同数、provisional `D_4N<=4D_N+6`を証拠化する。
- [ ] OCIO config path / hash、runtime version、`fallback=false`、offline CIEDE2000再検証が有効なclient captureでユーザーが本番アートを承認する。
- [ ] canonical / repoのimmutable generation、payload manifest、promotion receipt / active projection、preimage snapshotが一致し、全kill-point recovery testとpost-promote primary actual-window profileがpassする。
- [ ] `hell-workers-review-help-impact`の実経路判断と影響docs更新が完了する。
- [x] `python3 scripts/dev.py check`が成功する。
- [ ] rust-analyzer workspace diagnosticsが0件である。
- [x] `python3 scripts/dev.py cargo -- clippy --workspace --all-targets -- -D warnings`が成功する。
- [x] `python3 scripts/dev.py verify`が成功する。
- [x] 専用wall-art 9 case native profileとregistered historical P02 artifactのoffline検証がfail-closedで成功する。
- [ ] plan lifecycleを閉じ、恒久仕様へ引継ぎ済みである。

## 10. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| `2026-08-31` | `Codex` | active 3D wall、16接続、asset workflow、OCIO blocker、native受入を統合した初版を作成 |
| `2026-08-31` | `Codex` | 32 wu placeholderから本番公称・連続最小厚9.6 wuを算出し、12.8 wu装飾外形、共通port、品質別遠景／Door／占有の検証契約を追加 |
| `2026-09-01` | `Codex` | readinessとtopology activationを分離し、OCIO陽性証明、post-export / manifest gate、clean worktree、順次A/B、再現可能なN / 4N性能lane、world-replaceを具体化。M1→M4のasset再封印、receipt検証、immutable generation＋単一pointerのdurable promoteまで閉じた |
| `2026-09-01` | `Codex` | M0を開始。geometry / density / color fixture、Blender calibration renderer、offline CIEDE2000 verifierとunit testを追加し、現行OCIO fallbackをfail-closedで検出する初期toolingを実装 |
| `2026-09-01` | `Codex` | M0 Capture laneを実装。`perf.py`へwall phaseとraw sidecar再検証を追加し、専用native profileでclean subject / asset view / 12 run / median・MADを封印。draw-groupは未採取として分離 |
| `2026-09-01` | `Codex` | current fallback専用actual-windowを実装・採取。formal density laneとsingle-case条件を二重鍵で分離し、isolated WallのUI非重複ROI、X11 client PNG、fallback residency、raw sidecar、fingerprintを封印 |
| `2026-09-01` | `Codex` | wall専用RenderDoc checkpoint / extractor / native profileを実装・採取。中間color＋depth main passを実RDCで同定し、completed / provisionalのN / 4Nを各1 draw、全owner instance、2 replay一致で封印 |
| `2026-09-01` | `Codex` | final subject `35f1f6e3`でBevy 5 patch UI隔離を実証し、OCIO陽性Blender referenceとのDelta E 0、Capture 12 run、RenderDoc 4 caseを同一fingerprintから再採取してM0を完了 |
| `2026-09-02` | `Codex` | M3のPostUpdate topology / ApplyDeferred / presentation / transform propagation順、owner-indexed atomic production/fallback apply、creation-time fallback GlobalTransform、root / leaf world-reset分担を実装。steady 0-writeを閉じ、multi-tile / save-load統合を残した |
| `2026-09-02` | `Codex` | normal施工tileをtopology indexへ接続し、同一grid coalesce、material-only completion、owner / topology transform合成、Door同frame更新、rehydrate fallback位置をfocused testで固定した |
| `2026-09-02` | `Codex` | normal / Instant Build配置、framing前後cancel、completion bounceを実systemからPostUpdate topology / presentation / GlobalTransformまで通すfocused integration testを追加した |
| `2026-09-02` | `Codex` | PlacementFeedbackSet commitとdeconstruction finalizerを同frame PostUpdate topologyへ接続する実system testを追加し、主要Update lifecycleのdirty timingを閉じた |
| `2026-09-02` | `Codex` | 2 tile siteをframing spawn、coating transition、completionまで実system chainで通し、exactly-one visualと相互topologyが完成後も維持されることを固定した |
| `2026-09-02` | `Codex` | normal / rollback / recovery-onlyの実world replacementをrehydrate fallbackから次frame production一括復帰まで検証し、旧Entity 0・single rebuild・steady 0-writeを固定した |
| `2026-09-02` | `Codex` | shebang付き検証script 2件のGit modeを100755へ修正し、P02 Wall bounce testをWall専用presentation owner経路へ移行。check、0-warning Clippy、全体verifyをpassした |
| `2026-09-02` | `Codex` | clean validation worktreeのsealed 6 GLBをmanifest hash付きで再decodeし、9.6 wu共通port、12.8 wu corridor union、216〜240 triangle、16 mask回転、Bevy実loaderをpassしてM3を完了 |
| `2026-09-02` | `Codex` | M4 preflightでsealed OCIO artifactをbyte-identical再検証。normal textureのlinear/+Yはpassしたが全6 GLBのtangent不在をtechnical failureとしてnormal A/Bをrejectし、lit/unlit比較だけを次段に残した |
| `2026-09-02` | `Codex` | M4 lit/unlit比較用に既存N=96 production fixtureをgallery化し、16 mask×6、6 mesh、fallback 0、candidate identity、material modeをexact sidecarへ追加。profiling/candidate/actual-window限定toggleとfail-closed launcherを実装し、ユーザーからharness commit後のnative開始承認を得た |
| `2026-09-02` | `Codex` | ユーザーが最終lit画像を採用。art approval artifactを確定し、比較unlit materialとpending optional normal経路を撤去、lit固定のart-approved candidate projection / sealing / gallery契約へ更新した |
| `2026-09-02` | `Codex` | final generation 2を封印し、実asset loaderと固定lit actual-window単一caseをpass。current P08 subjectでhistorical P02を再実行できない既知境界を再確認し、current-source M5用の9 case Wall matrixを追加した |
| `2026-09-02` | `Codex` | Wall matrix初回はHigh / DPI 1.0完了後にperf入口のsingle-case固定条件でfail-closed。formal / single-case契約を維持したまま、Wall native helper専用authorizationで9 caseを許可するよう分離した |
| `2026-09-02` | `Codex` | 再実行でRust profiling configにも同じ固定条件があると確認。専用matrixのlauncher環境・Python引数・binary引数を対にし、通常／formal／single-caseを広げず9組だけを許可した |
| `2026-09-02` | `Codex` | subject `926a3530`でcurrent-source Wall matrixを再実行し、High / Medium / Low × DPI 1.0 / 1.5 / 2.0の9/9 case、sidecar、client PNG、offline verifyをpassした |
| `2026-09-02` | `Codex` | registered historical P02 locatorを再検証し、subject `6ea0bf99`のindex / SHA ledger / formal / RenderDoc payloadをcurrent P08へ読み替えずoffline passした |
