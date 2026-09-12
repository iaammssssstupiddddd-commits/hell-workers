# 本設壁の側面UV・上面断面修正

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `wall-surface-uv-plan-2026-09-12` |
| ステータス | `In Progress` |
| 作成日 / 最終更新日 | `2026-09-12` |
| 作成者 | Codex |
| 関連提案 / Issue/PR | N/A |

## 1. 目的

ユーザー報告の「側面が引き伸ばされ、断面に外壁模様が貼られる」を解消する。
側面は面内の水平距離と高さで等密度に展開し、上面は同系色の不規則な内部模様で
外壁ではなく断面と読める表現を検討する。確認画像の厚み比較はゲームと同じ投影条件を必要とする。

## 2. スコープ

- 本設6 familyのUV生成、専用回帰検査、隔離Blender原本・GLB・確認画像。
- 型枠7 file、Door、形状・厚さ・高さ・接続・material数は変更しない。
- 現在のWall generation 10を上書きしない。新しいアートの承認と本番昇格は別段階。
- 承認済み追加範囲: 本設albedo/emissiveの石面と断面textureを整理する。
  2領域外の全画素は保持する。「元texture画像不変」はv2、断面領域以外不変はv3までの実績。

## 3. 現状とギャップ

`create_wall_production_scene.atlas_uv`のstoneはXYだけを参照しZを無視する。
側面UVが線に潰れ、上面だけが2次元の石積みを表示する。rust/purpleもX固定面ではUが潰れる。
既存post-export gateはUVの型・有限値しか調べず、この退化を拒否しない。

## 4. 実装方針

- 外壁は輪郭辺の接線方向をU、高さをVとする。長短の辺を同じ画像幅へ正規化しない。
- 石面の上下は同じ高さ座標を使い、中段で模様をリセットしない。
- 上下面はatlasの専用低コントラスト内部石材領域へ平面投影する。
- 断面の色は側面の目地を除く石の代表色へ合わせる。Blender確認材質の不要な反射を除き、
  runtimeのroughness=1 / reflectance=0に対応させる。照明の違いは別に明記する。
- v4では下半分のrust/purple装飾面選択を廃止し、全側面を連続した石面に統一する。
- 専用UV profile検査を追加し、旧世代・型枠の検証契約へ遡及適用しない。
- 通常のゲーム表示/APIは変更しない。GLBのUV0を既存lit materialが消費する経路を維持する。
  perf専用ArtPreviewの設定入口は、本設・型枠双方を明示選択できるよう既存phase判定へ整合する。

## 5. マイルストーン

### M1: 原因固定・実装修正

- [x] active GLBで側面UV退化を確認する。全152側面triangle中134が線状UV。
- [x] 純粋UV関数、Blender生成器、出力GLB用検査と回帰testを追加する。

### M2: 技術・見た目検証

- [x] 隔離rootで6 GLBをscene/Khronos/geometry/UV gateへ通す。
- [x] 陽性OCIOの同条件Blender前後画像を確認する（実機証拠とは呼ばない）。
- [x] Help影響レビュー、仕様書同期、check / Clippy / verify。
- [x] 追加指示「断面も色味を合わせる」: v2原本・6 GLB・色確認画像と全体gateを再検証。
- [x] 追加依頼の断面texture案と厚み差の調査を記録する。
- [x] ゲームの縦補正・基準grid・両軸を備えた寸法比較画像へ更新する。
- [x] 断面専用の粒子・微細亀裂を実装し、標準zoomと最大zoom-out相当の静止Blender画像を確認する。
- [x] nativeで標準／最大zoom-outの静止画像と対象assetの描画成立を確認する（M3証拠参照）。
- [ ] 時間方向のちらつきを確認する。今回の単一frame画像では未検証。

### M3: 実機・本番反映

- [x] コミットの明示承認を得る（候補をコミットし実機確認へ進める質問への「OKです」）。
- [x] clean subjectを固定し、候補を検証用asset viewへ封印する。
- [x] 新しい本設UVを扱う承認前preview経路を用意し、nativeスキルのdirect kitty launcherで観察する。
- [x] v4標準／最遠画像への「OKです。進めてください」を新アート承認として受領する。
- [x] 承認記録を両native job・PNG・exact 15 coreへ束縛し、改変／取り違え拒否testを追加する。
- [x] 本設更新専用final封印を実装する。新しい本設v2 sourceを同梱するauthoring v3とし、
  旧型枠7 fileの不変検査、独立release validator、provenance・再export証拠を必須にする。
- [x] clean subjectでfinal封印し、正式candidateの標準倍率品質matrix（9条件）を検証する。
- [ ] 正式candidateの性能Capture／Memory、時間方向の確認を行う。
- [x] 非focus経路を観測し、perf実windowだけ連続更新へ固定する修正と、
  通常playの省電力設定・headless runnerを維持する回帰testを追加する。
- [x] 更新後のclean subjectで再採取して60Hz制限の解消を確認する。
  旧subjectの性能値を合格証拠へ流用しない。
- [ ] 本番昇格の明示承認とreceipt・復旧前像を揃え、通常authorityで反映後確認する。

## 6. リスクと対策

### v4: ゲーム等倍で読める面への整理（追加実装）

ユーザーが実機画像で報告した「黒い筋は格子にすら見えない」を制約とする。
拡大で解釈できることを合格条件にせず、細い補強帯・留め具案は撤回する。
2026-09-12の「進めてください」は以下の修正実装を対象とし、本番昇格は保留する。

- [x] 上下段をstoneに統一。rust/purpleの任意edge番号による割当を削除する。
- [x] 3段程度の大きな石面と弱い目地、低コントラストで粗い内部石材を画像生成する。
  新隔離root `target/wall-readable-RDcceXFi/`で保管し、既存候補と本番を上書きしない。
- [x] profile v4とstone/coreのpack領域を束縛し、領域外不変・両領域emission=0を検査する。
- [x] 6 GLBの形状・法線・厚み不変、側面装飾UV拒否、上下連続、全体gateを検証する。
- [x] 等倍／最遠相当Blender比較を作る。
- [x] v4修正コミットとゲーム内撮影の明示承認を得る（質問への「どうぞ」）。本番昇格は含まない。
- [x] 新clean subjectでv4標準／最遠native ArtPreviewを撮影・独立再検証する。
  過去のv3 passは再利用せず、既存validation worktreeとCapture buildを再利用して新jobを採取した。

UVの線への退化、面ごとの密度差、上面への目地混入は専用検査で拒否する。
旧型枠sealerは本設8 file不変を要求するため、その承認を新しい本設meshへ流用しない。

## 7. 検証計画

全6 familyで側面のUV面積が正、面内2軸の密度が等しいこと、上下面が内部色領域内であることを確認。
GLBの輪郭・port・厚さ・triangle数は旧geometry gateで別に確認する。
`python3 scripts/dev.py check`、`python3 scripts/dev.py verify`、
`python3 scripts/dev.py cargo -- clippy --workspace --all-targets -- -D warnings`を実行する。
native未実施のBlender画像をゲーム内の完成証拠へ格上げしない。新規性能baselineの登録は対象外。
本番昇格前のcandidate性能Capture／Memoryは既存の正式受入profileで別に検証する。

## 8. ロールバック方針

承認前はcanonical/runtimeに触れない。昇格時はgeneration 10のpointerとruntime locatorを
別々に保存し、immutable世代を残す。既存Wall/Door共通validation worktreeはこの小修正だけで削除しない。

## 9. AI引継ぎメモ

- 開始subject: `c58d2aed`、開始時worktree clean。
- 現在地: M1完了。v3の断面texture・投影条件を実装し、6 GLBとBlender比較を検証済み。
  壁修正を`23d6cd68`へコミット済み。`87f0e682`の隔離候補で標準／最遠native ArtPreviewがpass。
  v3の等倍可読性指摘を受けたv4整理候補は`8d93bf11`へコミットし、標準／最遠native ArtPreview pass。
  ユーザーの新アート承認を記録し、`edebfc90`でgeneration 13をfinal封印済み。
  同subject・同候補で正式標準9条件matrixを再試行し、9/9 valid＋独立verify pass。
  性能Captureは同条件2回とも60Hz paced判定で停止。非focus時の更新制限を避ける
  perf専用連続更新と回帰testを`552d9c91`で実装。新subjectでは22回すべて非pacedだが、
  fallback / provisional / smallの反復間p50比1.628が上限1.25を超え、正式jobはinvalid。
  新たな性能改善／合格は主張しない。ばらつきの原因調査と安定条件での再採取が必要。
  Memory・時間方向の確認・新本番昇格は未実施。
  承認前previewをformal authorityへ手作業で変更しない。
- 参照必須: `docs/blender-setup.md`、`docs/assets_workflow.md`、native/Help/docsスキル。
- 初回検証ログ: workflow Python 128/128 pass（新規7 testを含む）。6 GLBのscene/Khronosは
  errors=0 / warnings=0、geometry＋新surface UV gate pass。旧GLBとの全triangleの頂点位置・法線を
  頂点分割順に依存しない比較で照合し、6/6不変を確認した。
- 2026-09-12: `python3 scripts/dev.py verify`は`All quality gates passed`、
  `python3 scripts/dev.py check`および明示的なworkspace Clippy `-D warnings`もpass。
  初回M2時点ではRust変更・native Capture / Memoryなし。後続M3の結果は下記へ分離して記録する。
- Definition of Done: M1〜M3、仕様同期、全gate合格、承認証拠のcapsule保存と所有workspaceの棚卸し。

### M2初回の隔離成果物（色修正前の履歴）

primaryの`target/wall-surface-uv-rP92eW0A/`（6.0 MiB）にだけ生成した。
`staging/`が修正候補、`before/staging/`がgeneration 10の本設原本コピーによる比較画像。
元画像を編集せず、同じalbedo / emissiveを両方へコピーした。新しいvalidation worktreeは未作成。

| artifact | SHA-256 |
| --- | --- |
| `staging/blend/wall-production-v1.blend` | `9e50627049fb0668f93516260f95a0c36ca4a33d451db2892ad306b6c1981251` |
| 修正後 `staging/renders/wall-production-v1-reference-board.png` | `fb5dcf8e5df249030fb5d660c7835825c28f2c148542d947a35c27e4b885a9fc` |
| 修正前 `before/staging/renders/wall-production-v1-reference-board.png` | `62f0918a403b3ff25afe55f32708fbd8cef666e71ba1f61cfbbfd804d1651246` |
| 修正後reference report | `277ff1e2708a48aa8807d1fa8d68eaf58ba4b0325e36f7c27c248488c68babe3` |
| `wall_isolated.glb` | `bdcd5d5df970d9af6e397b6f6d35eea8000b0fdc781890d8b58c29aaacbd8ab5` |
| `wall_end.glb` | `540fe475868fe3ac5f89199aa7c085bac87008727eced4c64393e15605d49c1c` |
| `wall_straight.glb` | `8426f1d7523cff0c6ce41253bd0148acbe33f15577332af0d0f8fc31dce95058` |
| `wall_corner.glb` | `4cbd1031ebf39fcdbb4e76e158142dd717a7640205712e7ab650dcd1dc29767c` |
| `wall_t_junction.glb` | `7196c2dbd0114911f29a20d4928bb20481d6e159f4498144b07fafb9f5513d07` |
| `wall_cross.glb` | `914d0d733bfe3c96c8e162ca3067bc5515fb9545496f257a2a2abbb73ac8b92c` |

本番Wall locatorは`0b012912bf5bffc6009f082789ebaddf413a6ad186607ce39f5139ed1020fdf9`、
Door locatorは`21075f6b5c6e4412ccc17632ecc3b44bcb619bb901a06062fa0905c0be303002`で不変。
上面が使うatlas内部色領域のfilter余白を含む21×21 sampleはalbedo各channelの標準偏差
`2.931 / 2.317 / 1.931`、emissive最大値は`1/255`で、目地や紫発光領域ではない。
Blender boardのOCIOは`fallback=false / validation_status=pass`。制作sceneの初回保存時は
既知の同梱OCIO fallbackを出したが、画像比較と全exportは固定configを明示して別に検証した。
これらは未承認の技術候補であり、native / art approval / releaseの証拠ではない。

### 断面の色合わせ（v2履歴）

ユーザー追加指示: 「断面も色味を合わせてください」。旧画像と原本は保持し、
`target/wall-core-color-7P2rJugP/`へ別出力した。UV profileは`wall-face-uv-v2`。
旧core（image x330.5〜346.5 / y45.5〜61.5）から、側面の石の代表RGB `(34,30,28)`へ近い
小領域（image x590.5〜606.5 / y398.5〜414.5）へ変更した。
filter余白を含む21×21 patch平均は`(33.93,30.09,28.25)`、最大channel標準偏差は1.65。

UVだけを変えた比較を`uv-only/`へ保存し、側面の固定ROIが不変で断面の元色だけ変わることを確認。
しかし金属反射の明るい紫が残ったためUVの値調整は繰り返さず、runtime実装と比較した。
`make_topdown_structural_material`はroughness=1 / reflectance=0、Blenderはroughness=.82 /
metallic=.12 / default specularだった。確認材質をroughness=1 / metallic=0 / specular=0へ合わせた。
この変更はBlender用で、placeholder exportを読むゲームのshared materialを変更しない。
UV-onlyとmatteの比較は同じカメラ・照明・textureで行い、isolated上面の固定ROI medianは
`(59,45,56) → (42,30,34)`。鏡面反射を除いた後も、紫rimが当たる面には拡散光の色づきが残る。

色そのものの確認には白色rim＋中性色ambientの`--neutral-light`画像を別出力し、通常の紫rim
referenceと混同しない。カメラ・形状・texture・UV・材質は共通。これはBlender内の素材確認であり、
Bevyとの最終画素一致やnative acceptanceは主張しない。

| 最新artifact（`staging/`配下） | SHA-256 |
| --- | --- |
| `blend/wall-production-v1.blend` | `05256cda9094f329be9a505ebc65814d3805a3b5291034d999d2163c5b53527e` |
| `renders/wall-production-v1-reference-board.png` | `05fcf48334b47accde4b7bcf31acbfa6a10bb13a7c6446c75db5572dd5d005ca` |
| `renders/wall-production-v1-reference-board-neutral.png` | `ba0d89d9b5bec0e8f3339ca70e2e05b40901ec8220c218579ef7aef2fe05897b` |
| `exports/models/wall_isolated.glb` | `22c8e91ae3333650ee62d89598cb606a05e5e3490b4f48887c7f53e64cb613d9` |
| `exports/models/wall_end.glb` | `5c73a1e95c885b6ebea0abcc8204a81d2f96cd0598505f81505a58ee8751b1ef` |
| `exports/models/wall_straight.glb` | `f221e149d9003177fae10fb04b44a4ccb1cc90dc91455eb4691fbf446a11f325` |
| `exports/models/wall_corner.glb` | `6a1dedf6e0c5d2a4180ef238d84f0a3b5abacf60111802e29019b9f1274459c8` |
| `exports/models/wall_t_junction.glb` | `54524b3f9358a567a4a06f9dddc3422fd02d0c34a081f268828b033baacc6a19` |
| `exports/models/wall_cross.glb` | `00064715577aeb0620dda3d4f16cdd525ffd6ad741f0438cc1e78dcce9c3488c` |

6 GLBのscene/Khronos/geometry/UV-v2 gateはpass。旧warm coreを拒否する回帰caseと
確認用材質の反射設定testを追加。新旧6 GLBを比較し、形状・法線・側面UVがすべて不変と確認した。
色確認モード追加時の固定ファイル名source契約testが一度failしたため、既定名を明示的に維持する
分岐へ整理し、白色照明の別名・引数転送・実材質記録の契約もtestへ追加した。
その後のworkflow Pythonは129/129 pass、`dev.py verify`は`All quality gates passed`。
`dev.py check`、明示workspace Clippy `-D warnings`も再度pass。本番locatorと元textureは変更していない。

### 追加検討時の記録: 断面の可読性と厚みの比較条件

ユーザー依頼: 「色味だけでなく、断面だとわかるようにテクスチャも検討してください。
また、厚みが実装と違うように見えるので確認してください」。このターンは診断・検討に留め、
原本・コード・runtimeを変更していない。以下の案を実装済みやアート承認済みとは扱わない。

#### 厚みの確認結果

- runtime locatorはWall generation 10 / `release_approved`、全15 coreの実bytesのhashが一致。
  Door generation 7の6 coreも一致。起動中のユーザーのgame processまでは観測していない。
- 本番6 GLBとv2候補6 GLBをgeometry gateで再検証し、全件pass。
  straightはlocal XYZ=`9.6 × 32 × 32`、isolatedは`9.6 × 32 × 9.6`。
  corner/T/crossは全体AABBの幅ではなく各armの断面・port検査で厚さ9.6を確認する。
- 1 tile=32、厚さ=0.30 tile、高さ=1 tile。authoringの.30にexport scale=32を一度だけbakeする。
  `building_presentation_transform`はownerのscaleを一様に適用し、壁だけ厚みを変える倍率はない。
  接続による追加変換はY軸quarter turn。完成bounce終了後のowner scale=1を比較条件とする。
- 見た目の比率には実際に相違がある。ゲームは`rtt_composite::composite_logical_size`で
  `hypot(150,90)/150 = 1.16619038`の縦補正を適用する。WallのBlender boardにはその補正がない。
  角度59.036°だけを揃えても最終投影は一致しない。横倍率を揃えた場合、boardの縦寸法は
  ゲームの約85.75%となり、縦置き壁は幅に対して短く、相対的に太く見え得る。
- 例: 横置きWallの上面厚は、1 wu=1 logical px換算でboardが約8.232、ゲームが9.6。
  高さの画面寄与はboard約16.464、ゲーム19.2。これは画像内の実zoom/DPI値ではなく
  共通横倍率での投影比較である。既存1200×800の拡大boardはゲーム標準zoomの証拠ではない。
- fallbackは`visual_handles.rs`の32×32×32 Cuboidで、productionの9.6厚とは別物。
  ローカルasset整合性だけで、ユーザーが見た時点のprocessがfallbackでなかったとは断定しない。

#### 断面textureの推奨案

今の16×16 texelの石内部を拡大する方法は、色合わせには使えても断面を示す情報が足りない。
外壁の石積み模様を戻す案と、粒子のない単色だけの案は採用しない。

1. 側面に合わせた暗い石色を地に、疎らな角ばった細粒と少数の短い、不規則な亀裂を置く。
   平行な目地・整ったレンガ列・全面を横断する大きな割れを避け、内部が露出した面として読む。
2. 外周に細い暗い境目と局所的な欠けを添え、側面の仕上げと内部を分ける。太い枠や明るい縁取りで
   壁が厚く見える補正はしない。粒子・境目は色差を抑え、材質の色を変えない。
3. 接続portでは切れ目や外周線を作らず、隣tileと内部模様の密度・向きを揃える。
   各tileに閉じた四角い縁を描くと継ぎ目が復活するため、露出境界と接続面を区別する。
4. 既存atlasの未使用領域を実UVで確認して断面用に割り当てる案を優先し、既存側面領域を保持する。
   共有materialやgeometryの厚さは増やさない。emissiveの断面領域は黒を基本とし、光で断面を強調しない。
5. 標準zoomで9.6 logical px幅に納まる粒子の密度で比較する。拡大画像だけで合格させず、
   最大zoom-outでちらつくノイズや潰れた太枠にならないことを確認する。

#### 実装時の順序

最初に確認用投影をゲームの縦補正へ揃え、32×32基準grid、厚9.6／高32の寸法表示、EW/NSを
同一倍率で並べる。Door previewの`door_preview_projection.py`は既に同じ補正を扱う参考経路。
実際のcamera projected reference pointsで検証し、目視合わせでmesh寸法を変えない。
その比較条件で断面専用texture案を載せ、raw albedo・白色照明・従来照明を区別して評価する。
最終的なgame画像は別途native acceptanceが必要。今回の調査をその代用とはしない。

### v3: 断面textureとゲーム投影の実装

ユーザーの「いいと思います。進めてください」を受けて追加案を実装した。
隔離rootは`target/wall-core-texture-NVprLDwY/`。本番locator、Wall generation 10とDoor generation 7は不変。

- 画像生成で暗い石内部の粒子・不規則な短い亀裂を作成し、専用256px tileへ縮小・pack。
  細い境界は外壁模様との切り替わりで表現し、人工的な外周線／欠けgeometryは追加しない。
  tile単位の縁取りを避け、3連結標本に偽の目地や太い枠がないことを画像で確認した。
- `pack_wall_core_atlas.py`はImageMagickによる機械的配置のみを担当し、粒子を手続き的に描画しない。
  image座標`(702,750,260,260)`以外はalbedo/emissiveの全画素不変。側面UV rectangleとも非交差。
  断面平均RGB=`36.77/33.29/30.85`（側面代表色34/30/28）、標準偏差=`9.28/8.86/8.82`。
  v2のほぼ単色から微細な明暗へ変更。断面emissiveは全画素0。
- UV profile=`wall-face-uv-v3`。新atlas report/hashをscene生成時に確認し、旧atlasとの取り違えを拒否する。
  GLBの形状・法線は本番6/6不変、側面UVはv2候補6/6不変。全scene/Khronos/geometry/UV gate pass。
- `wall-preview-topdown-v1`で縦補正1.16619038を追加。6個の実camera基準点を解析式へ0.01px以内で照合。
  1px/wu換算の上面厚はEW/NSとも9.6px、高さ32の画面寄与は19.2px。meshの厚み変更は不要。
- 1280×960拡大、256×192標準相当、同canvasでcamera scale=5の最遠相当を直接render。
  標準相当では細粒が残り、最遠相当では細部が縮退するが静止画像の輪郭は残る。
  ちらつき／ゲームAAの判断は未実施。Blender静止画像をnative acceptanceへ流用しない。
- 検証: `dev.py verify`は`All quality gates passed`。最後の定数参照整理とruntime定数照合test追加後も
  workflow Python 134/134、`dev.py check`、明示workspace Clippy `-D warnings`、docs gateがpass。
  Rust source変更なし。新rootは原図コピーを含め7.0 MiB、新規validation worktree作成・削除はなし。
  原図は`staging/source/wall-cut-core.png`にも同一bytesを保持している。

| 最新artifact（`staging/`配下） | SHA-256 |
| --- | --- |
| 断面原図（画像生成保存先からの同一bytes） | `3b61f434ba2e1a8dcb0b3ed673af98b72850aa3b1bdcc7dc69cce2d4f07a5dcb` |
| `blend/wall-production-v1.blend` | `c0afc23c19095c9f52f869b6e84fceff13a656533ce23722b8a7c10ba6774268` |
| `exports/textures/buildings/wall/wall_albedo.png` | `ad3f6b75042abec3d27febe8e3a3cbe4e722a1a4db6a4657afdae5fb6f06cf9b` |
| `exports/textures/buildings/wall/wall_emissive.png` | `8d8214b0251b2565fa8b8d376d7d1cc4f84b53ae284aaea40451aa45595d8003` |
| `renders/wall-production-v1-reference-board-neutral.png` | `5a630caa7229e9863698d8ce17ddbb0113808147f3f68612f622f9930a0a0376` |
| `renders/wall-production-v1-reference-board-neutral-standard.png` | `7b04855d53108cd1f8d81ae9acd18847c78d16729f01cb34f11be34313d7a226` |
| `renders/wall-production-v1-reference-board-neutral-farthest.png` | `c88884402118e9a204a2980201fdfa881cf90caaebcf473753a3b559bc8a3bdb` |
| `renders/wall-production-v1-reference-board.png` | `7af773c460a35357b22eda75ff36790e72d4ee3627aaae43361eadd775465c06` |
| `exports/models/wall_isolated.glb` | `08c3a1142096beba97ae373ce3d95d1158f7a12664afa38dbfc2b656f33cc35c` |
| `exports/models/wall_end.glb` | `6837ada9f4b89416076a678746755ea867bbfc041887760c5c7c39d29bc0d4a8` |
| `exports/models/wall_straight.glb` | `ac27be6cc50b12c98f1e4543ec76afcb24b99a771339a3d20e3710ee78425309` |
| `exports/models/wall_corner.glb` | `2d7c3c16b91d29e9931643055818cdfd6cee7e6dd4b2a03dfc6b0d12b1f6cbc9` |
| `exports/models/wall_t_junction.glb` | `eaf4f2793387694045baca1fe6d4b78f4ec74a63a8d692e1888ee550b42124f1` |
| `exports/models/wall_cross.glb` | `69f068e2490847dd29d287441cafb60782bc15235fd316983586bc26d73082c4` |

### M3承認前経路の準備

今回の「OKです」は候補のコミット・実機確認への承認として記録する。アート最終承認や本番反映への
承認へは広げない。既存runtimeはschema-2 ArtPreviewのcompleted phaseを既に表示できるが、
`wall_art_acceptance`はArtPreviewをprovisionalへ固定していた。本設確認用に`--completed-preview`を
追加し、別profileとmanifest/observationの明示flagで標準・最遠の単独previewを束縛する。
`profile_name`はformal authorityとの混在を拒否し、probeとraw performanceの双方でphaseを再検証する。
既存型枠previewの意味は変えない。`provision_wall_surface_preview.py`は本設8 fileだけを差し替え、
旧型枠7 fileと本番geometry/法線を保持する独立したtechnical sealer。旧final manifestの承認は再利用しない。
Help/docsスキルで再確認し、診断用経路追加だけなのでHelpはNo impact。

初回native job `wall-art-20260912T105446Z-97d5ec25`（subject `d15b0156`）はビルド完了後、
起動前の`config.rs`がArtPreviewをprovisionalに限定していたためinvalidとなった。画像は未取得。
表示側の`accepted_wall_phase`だけの確認では不十分だった。値・textureは触らず、設定入口でも既存の
`wall_actual_window_phase_matches`を使い、completed/provisionalだけを受理するよう整合させる。
mixed、actual-windowなし、phaseなしは拒否する回帰testを追加。これは検証用perf起動の成立条件で、
通常版の建設操作やアセット承認権限を変えない。失敗jobは削除・上書きせず保持する。

`5375338c`で再試行した標準job `wall-art-20260912T111622Z-2d627435`は撮影・offline verify pass。
Intel Arc (MTL) / Vulkan / X11 / High / DPI 1、96 production / 0 fallback / 6 mesh / 16 masksを確認。
最遠job `wall-art-20260912T112827Z-6b653703`はperf.pyの`--wall-art-zoom farthest requires
--wall-art-matrix`でinvalid。単独最遠previewにも既存runnerのmatrix carrier flag/environmentを対で
渡すよう修正する。orchestratorは1 caseのままで、formal権限や9-case matrixにはしない。
このharness変更後は標準／最遠とも新subjectで取り直し、前の標準passを新証拠へ流用しない。

### M3: 本設候補のnative ArtPreview結果

2026-09-12、clean subject `87f0e68267588a3eb9ed7848f1be1f244babf505`で両方を再撮影し、
各jobの終了状態`valid`および独立したoffline `verify`の`pass`を確認した。
nativeスキルのdirect kitty launcherを使用。画像は加工なしの1280×720 X11 client capture。
Intel(R) Arc(tm) Graphics (MTL) / Vulkan / Intel open-source Mesa driver / Mesa 26.1.8、
High / DPI 1、camera scaleは標準1・最遠5。96 production / 0 fallback / 6 mesh / 1 material、
16 topology masks各6件をprobeで確認した。型枠7 fileは旧本番と同一bytes。

標準画像では上面の暗い細粒と側面の石積み・下段装飾の区別を確認した。
最遠画像では細部は数pixelへ縮退する。straight ROIのterrain contrast検査はpass
（weakest column sigma=19.691723、terrain sigma=5.297357）。これは静止画の検査であり、
時間方向のちらつき・全quality/DPI・性能baseline・native Memoryの検証ではない。
ArtPreviewは最終アート承認・formal acceptance・release authorityを与えない。

共通のidentity:

- source fingerprint: `c07888b27d9ec906af46774dc811d5061b504bdec4858207f6f6a8ade00743ea`
- harness fingerprint: `6dbc93b109a45d5cb7bad41a78e0ec0f51b3ad6a876d760e5665c8050255e6c3`
- asset view fingerprint: `ef23820f76cc23470fbde93de7ecc2438a738e7996a533515158c6378ab31a28`
- binary SHA-256: `e8a8ecbd1f329ba5ac816b88148f4454ea4d1180ecdd4d7f27cc72bc2cee94b4`
- 隔離generation 11 technical manifest: `044f893190bc3bab106a02144d79f016516b05d38a986aa8b0f0b86539a03ffd`
  （`target/wall-core-texture-NVprLDwY/staging/validation/surface-preview-11-carrier/surface-preview.json`）

job親directoryは`/home/satotakumi/projects/hell-workers-validation/wall-door-joint-83ad3f85/target/native-acceptance/`。

| zoom / job | `manifest.json` SHA-256 | `current-wall.png` SHA-256 |
| --- | --- | --- |
| standard / `wall-art-20260912T113344Z-5d1af382` | `0243d76724833cf69ebe78ee425de74b8bd5604107e37f392c871cf5ab457f59` | `e8f03ef4a75b046b3488a21c7ee6dabed6e041c637362e4a15dd6b5e8123fbf7` |
| farthest / `wall-art-20260912T113518Z-c28498d4` | `319183ff4bb224009d40167d14b76d7b04bc8ef671e08e4c2a6aa61b2263469a` | `2445bfe7f2c2662cac0a3c2be168ce3bb2e6269d85df4a83480b786a49a4dceb` |

最新実装で`dev.py verify`、`dev.py check`、workspace Clippy `-D warnings`、workflow Python
136/136、nativeスキルの全指定self-test、agent-rules、skill validatorがpass。
変更Rustのrust-analyzer診断はerrors=0 / warnings=0。Helpは設定入口のperf専用変更も含め
実経路でNo impactと判定した。通常の建設入力・資材・状態・Help文言は不変。

Wall本番generation 10とDoor generation 7のlocator hashは上記記録から不変。
既存の共通validation worktreeを再利用し、上書き前の隔離asset viewはbackupへ保持した。
新worktree/branch作成・削除なし、回収容量0。M3全体は未完了のため共通worktreeを撤去しない。
次はこの候補画像のアート確認を受け、未検証項目と正式封印・本番昇格を別段階で進める。

### v4出力検証結果（native前）

隔離root `target/wall-readable-RDcceXFi/`（5.8 MiB）へ2枚の画像生成原図、prompt全文
`prompts.md`、atlas、原本、6 GLB、3倍率の白色照明boardを保存した。
画像生成はbuilt-in tool、ImageMagickは機械的縮小・atlas配置のみ。
原図は`stone-art.png` / `core-art.png`で、生成サービス側の原図も削除していない。

- stoneの平均RGB=`49.57/43.20/37.69`、core=`51.89/45.74/40.25`。両面の色差は小さいが、
  v3のcore平均`36.77/33.29/30.85`より明るい候補となった。既存色と同一とは主張しない。
- coreの標準偏差は`3.98/3.30/2.77`（v3は`9.28/8.86/8.82`）。微細な高コントラスト粒子を
  広い低コントラスト内部模様へ置き換えた。両領域emission=0、2領域外は元atlasと全画素一致。
- 6 GLBはscene / Khronos errors=0 warnings=0、geometry / UV-v4 pass。
  本番の全triangleの頂点位置・法線signatureと6/6一致。側面triangleは全152件stone、
  cap全76件core。新たな形状・厚さ・材質スロットは追加していない。
- Blenderの標準相当boardでは鉄板の下段帯と孤立した黒い部品境界がなく、石面が上下へ続く。
  最大zoom-out相当は細部の評価には小さすぎるため、輪郭観察に限定する。
  これらの静止画でゲーム内の可読性改善・ちらつき解消を確定しない。
- workflow Python 137/137、`dev.py verify`の全gate pass。Rust/runtime設定の変更なし。
  本番Wall/Door locator hashは上記値から不変。既存validation worktreeとasset viewには触れていない。
  今回の新規validation worktree / branch作成・削除はなく、回収容量0。
- 次段階: 修正コミットの明示承認後、既存technical sealerで新候補identityを固定し、
  clean validation worktreeで標準／最遠native ArtPreviewを再撮影する。Memoryは未実施。

| artifact（隔離root相対） | SHA-256 |
| --- | --- |
| `stone-art.png` | `7a09a2167893567bb11542180d3bc49100a6b9d2f6d65c4a858295b29610ce6f` |
| `core-art.png` | `35f06da3e7286b8b441d432cc7262ae753f054b3a6a992a01ac7601199a48a23` |
| `staging/blend/wall-production-v1.blend` | `d7d8debc50773cb2a5833518e3005450393a892baf7b35b046035014728f6867` |
| `staging/exports/textures/buildings/wall/wall_albedo.png` | `251094ae523509cc7a0cd258b94925ab1d398636b8c27462f3c6e33c9c01ddc8` |
| `staging/exports/textures/buildings/wall/wall_emissive.png` | `4c87e02fdd79353cf134ceb870e072433f47ea19916acbe9dd93095282db006b` |
| `staging/renders/wall-production-v1-reference-board-neutral.png` | `bf0a8b2e76908d3c29dd05fe29653a6adb5a8aa909a91704b8494f5e9c9b018e` |
| `staging/renders/wall-production-v1-reference-board-neutral-standard.png` | `92ad048383ee1c74f54c427d7f9e2fcaf7f0d9566d3401ca0cb5a158f1014401` |
| `staging/renders/wall-production-v1-reference-board-neutral-farthest.png` | `592955734b3616b11d02f86824eeb91e8fceca6b783e31565135272bf24fe115` |

### v4 native ArtPreview結果

ユーザーの「どうぞ」はv4修正コミット・ゲーム内撮影への承認として扱い、本番昇格へ拡張しない。
修正subjectは`8d93bf11b98b97574115da57f031f9447f79769f`。
`provision_wall_surface_preview.py`で隔離generation 12を新規封印し、型枠7 fileと本番geometry/法線の
保持、atlasの保護画素、出力hashを再検証した。両jobはこの同じ候補で`valid`となり、
独立offline `verify`も両方`pass`。native helperやRust runtimeへの追加変更はない。

- 実機: Intel(R) Arc(tm) Graphics (MTL) / Vulkan / Intel open-source Mesa driver / Mesa 26.1.8。
  X11 client 1280×720、High / DPI 1、camera scaleは標準1・最遠5。
- 対象: completed phase、96 production / 0 fallback / 6 mesh / 1 material、16 masks各6件。
  撮影はdirect kitty launcherで2 processを順次実行。前job終了後に次jobを開始した。
- 標準画像では旧下部の鉄板部品境界・紫装飾がなく、側面が連続した石面として表示される。
  石の目地は控えめで、等倍で留め具のような小部品を読む必要がない。最遠では細部を判読できるとは
  主張せず、straight ROIの輪郭contrast pass（weakest column sigma=16.748148、terrain sigma=5.215739）
  として記録する。前候補より明るい石色であり、色・簡略化の最終受容はユーザー確認待ち。
- source fingerprint: `c07888b27d9ec906af46774dc811d5061b504bdec4858207f6f6a8ade00743ea`
- harness fingerprint: `6dbc93b109a45d5cb7bad41a78e0ec0f51b3ad6a876d760e5665c8050255e6c3`
- asset view fingerprint: `c92d02bbaa47a7ae1242fe3374e3a84cfe5e3fc1ba7278c7ad4c9acb6a8ad2c7`
- binary SHA-256: `e8a8ecbd1f329ba5ac816b88148f4454ea4d1180ecdd4d7f27cc72bc2cee94b4`
  （Rust/profile不変のため既存buildを再利用、旧画像を再利用した意味ではない）。
- technical manifest SHA-256: `5277b189311a000ea31e04a18bd454a9998c877fb3e4abcec2c29bc7861537e1`
  （隔離root内`staging/validation/surface-preview-12/surface-preview.json`）。

job親directoryは`/home/satotakumi/projects/hell-workers-validation/wall-door-joint-83ad3f85/target/native-acceptance/`。

| zoom / job | `manifest.json` SHA-256 | `current-wall.png` SHA-256 |
| --- | --- | --- |
| standard / `wall-art-20260912T145424Z-a2fd8435` | `5df91d05832da1e3e0819dad24b251594b5cba16784babc86b3264ee26f58858` | `6096ee9b78f6e3a974c4eed12de35faff760a8a80bad25fb127db06418b74ca7` |
| farthest / `wall-art-20260912T145546Z-18c4e1ce` | `82a2cc505a3882563f55d20d6edc8ea80efc6d0003ba4768cc52e17f6e7e33f2` | `a2a8506e0b45bf313cb3c630d85e9ff110e843cb903a5cf4c41a099b43818ceb` |

今回の証拠は承認前ArtPreviewの静止画2枚であり、時間方向のちらつき、他quality/DPI、
formal acceptance、性能baseline、native Memoryは未検証。アート最終承認・release authorityを与えない。
既存validation worktreeをcleanな新subjectへ切り替え、旧asset viewの変更対象は隔離rootの
`previous-preview-assets/`へbackupした。新worktree/branch作成・削除なし、回収容量0。
本番Wall generation 10 / Door generation 7のlocatorとcanonical世代は不変。
このコミット後もworkflow Python 137/137、`dev.py check`、workspace Clippy `-D warnings`を再実行してpass。
2つのnative job終了後の`dev.py verify`も`All quality gates passed`。Helpは上記の実経路レビューに基づく
No impactをコミットtrailerへ記録し、今回の隔離候補provisionでも通常の操作・状態・文言は変えていない。

### v4アート承認と正式封印の準備

ユーザーの「OKです。進めてください」を、直前に提示したv4標準／最遠画像への承認として記録。
記録UTCは`2026-09-12T15:08:26Z`（メッセージ送信時刻の推定ではない）。
`record_wall_surface_approval.py`で両jobを再度独立verifyし、
`target/wall-readable-RDcceXFi/staging/reports/wall-surface.art-approval.json`へ保存した。
SHA-256=`441c69a86b1e2dc95ccc3a8aef533373d5d83bccdbe28b263b6c3341981a709d`。
生成原図やゲーム画像は変更していない。これはformal acceptanceや本番反映許可へ拡張しない。

`seal_wall_surface_revision.py`は自己完結v3 payloadに新本設v2 sourceを同梱する専用経路。
承認済み6 GLBの再export一致、型枠7 file不変、geometry/法線/atlas保護画素不変を要求する。
prompt・原図・pack report・承認PNGをpayload inventoryに加え、後で旧stagingに依存せず検証できる。
通常runtimeを変更せず、正式候補を隔離viewへ出す。実封印・formal native結果は別途追記する。
workflow Pythonは149/149 pass（新規12件）。承認recordへの失敗job・別subject・別candidate・別authority・
画像改変・core改変・pair fingerprint不一致・過去時刻／空の承認を拒否する。
revisionの旧generation再利用・型枠変更・非隔離出力、およびrelease内原図の改変も拒否する。
`55dc2f5d`の初回封印は、移動したblendが相対参照するalbedo/emissiveを再export前に配置して
いなかったため、scene gateの`MISSING_IMAGE`で停止した（完成payloadなし）。
値やアートは変更せず、承認済みtextureの配置を先行させる順序へ修正し、専用回帰testを追加する。
最初の失敗root `surface-final-13/`は保持し、再試行は別rootへ出す。
`da6c2ba9`では再exportした6 GLBが全件承認bytesと一致し、scene/Khronos/UV gateもpass。
その後のpayloadコピーで`shutil.copytree`が公開済みdirectoryのread-only modeも引き継ぎ、
新approval report書込が拒否された。directoryを新規作成しbytesだけコピーするよう修正する。
元generationのmodeは変更せず、read-only sourceを使う回帰testを追加する。
この失敗root `surface-final-13-textures/`も保持する。

### v4正式候補の封印結果

subject `edebfc90a7cd6a840dca1b3784948390b47cca9a`で専用sealerがpass。
`target/wall-readable-RDcceXFi/staging/validation/surface-final-13-writable/`へ新規作成した。
本設6 GLBの再exportは承認済みbytesと全件一致。scene/Khronos errors=0 warnings=0、UV-v4もpass。
型枠7 file、本設geometry/法線、atlas保護画素は旧releaseと不変。

- authoring v3 generation 13: `payload/manifest/wall-production-v1.asset-set.json`、
  SHA-256=`ef4f3c62f185570b83b5781470bbb2d7eb8571502c664c18bdcafc39081454cf`。
- 同梱した新本設v2 source: `payload/completed/manifest/wall-production-v1.asset-set.json`、
  SHA-256=`f6aed1325458c934e08893026ee9895f2774a2225fb23a12739d778a343d0cc1`。
- `asset_release_manifest.validate`を独立再実行してpass。promotion collectorは90 fileを列挙し、
  そのうちsurfaceのprompt・原図・pack report・承認PNG・native job/manifestは10 file。
  payloadは9.2 MiB。canonical pointer、receipt、primary runtimeは変更していない。
- 既存validation worktreeを同じsubjectへ移し、`candidate/assets/`をexact 15 core viewとして配置。
  旧preview locatorは`previous-preview.wallset`へ保持。authority=`isolated_candidate`、
  asset view fingerprint=`415a8f0b3e8835dce44c3c16f897b96d1be702eb7dc699b6bd3274cda416ad78`。
- workflow Python 151/151（新規14件）、`dev.py verify`全gate、`dev.py check`、
  明示workspace Clippy `-D warnings`がpass。Rust変更なし。

最初のformal標準matrix `wall-art-20260912T152623Z-f1385a9a`はHigh/DPI 1.0・1.5の2 case完了後、
High/DPI 2.0のcaptureが全面単色（1280×720、8-bit grayscale、1色）となりinvalid。
probeでは96 production / 0 fallback / 6 mesh / completed、generation 13の読み込みは成立していた。
形式検査を緩めず、失敗PNGを保持する。同一subject・候補・バイナリ・設定のまま一度だけ
`wall-art-20260912T153053Z-8cae82f2`でmatrix全体を再実行し、一過性撮影か再現する表示失敗かを切り分ける。
元jobの2 caseだけを9-case gateの合格とは扱わない。

### v4正式標準matrix結果

同条件の再実行 `wall-art-20260912T153053Z-8cae82f2`は9/9 case完了、`status=valid`。
独立offline `verify`も`status=pass / screenshots=9 / evidence_kind=formal`となった。
High／Medium／Low × DPI 1.0／1.5／2.0、1280×720 X11 client capture。
全caseの実adapterはIntel(R) Arc(tm) Graphics (MTL)、Vulkan、Intel open-source Mesa driver / Mesa 26.1.8。
各caseで96 production / 0 fallback / 6 mesh / 1 material、16 mask各6件、completed phaseと
generation 13 / authoring hashを確認。warmup 10秒・measure 10秒×各1回であり、性能比較baselineではない。
subjectは封印時の`edebfc90`に固定。source/harness fingerprint・Capture binaryは先のArtPreviewと同一。

High/DPI 2.0は再試行で正常画像となった。初回の単色capture原因は未確定で、設定や撮影gateは緩めていない。
代表画像（High/DPI 1.0・2.0、Medium/DPI 1.0、Low/DPI 1.0）を目視確認。
Lowでは石面の細部が縮退するが、旧下部装飾の黒い筋はなく、形状の輪郭が残る。
動的なちらつきや最遠倍率の正式全品質matrixを、この静止画検証で合格とはしない。

| artifact | SHA-256 |
| --- | --- |
| job `manifest.json` | `6b760647aac3196b153645e95e06582bb68f63b62d7bb59fd7db185ecc212029` |
| High/DPI 1.0 `current-wall.png` | `8d4073546a8da43aecfd4cd22a2a24084997195ea508fdfb0cba18f2944cd441` |
| High/DPI 2.0 `current-wall.png` | `f0ac0dfe5208b7ebe4be7e11216f3ce28b66fd9d2966a1f558dc93959ce47a47` |
| Medium/DPI 1.0 `current-wall.png` | `58ff82a616d316cd9712e82c7757dc7e762ddacbcbae3ce3d3164431aeaae996` |
| Low/DPI 1.0 `current-wall.png` | `515b813099bd03fd4a0ed0aa6aad9459628e5442d10dc54b5f0aba3d9f217714` |

job親directoryは上記と同じ共通validation worktreeの`target/native-acceptance/`。
9枚の画像、manifest、observation、aggregate CSVを、封印root内`formal-standard-capsule/`（14 MiB）へコピーした。
これは保管用capsuleであり、元job一式なしでnative offline verifierを実行できるとは主張しない。
本番Wall generation 10 / Door generation 7は不変。新規worktree/branch作成・削除なし、回収容量0。
次は既存正式profileで性能Capture／Memoryと時間方向の確認を行い、明示承認を伴う本番昇格へ進む。

### v4性能Captureの停止と再開計画

同じ`edebfc90`／generation 13から正式profile `wall-production-performance-v2`を開始した。
job `wall-production-performance-20260912T154554Z-d5c6f456`はbuild完了後、
completed / small / productionの初回runで`display_paced`判定となり`status=invalid`。
60秒の3,600 sample、60.0 fps、p50=16.657678 ms。先行fallbackは3,110 sample、
51.833 fps、p50=16.817592 msであり、この1 pairだけで比較の合格や性能改善を主張しない。
job.json SHA-256=`7e9187c34196b627e8831cec9f3b31258099ed7057a37ae844b624eed7fe98fe`、
capture-order.json SHA-256=`78b68049c31fac82e30673035ec216a14fa321bffc5aa4c825beeb2e4165fd30`。
parentは上記共通validation worktreeの`target/native-acceptance/`。失敗artifactは保持する。

`window.csv`は開始／終了ともX11 / Intel Arc (MTL) / Vulkan、
requested=`auto_no_vsync`、effective=`immediate`。ただしこれは更新ループが非pacedである証拠ではない。
hostはGNOME Wayland上のXwayland。compositor原因とは断定しない。
ローカルBevy 0.19.0の`bevy_winit/src/winit_config.rs`で、既定`WinitSettings::game()`が
focus時Continuous／非focus時`reactive_low_power(1/60秒)`を使い、VSyncとは独立であると確認した。
失敗時subjectのアプリはこのresourceを上書きしていなかった。

同条件再試行`wall-production-performance-20260912T155229Z-0408c508`を一度だけ開始し、
`xprop`で非破壊にフォーカスを観測した。fallback PID 760650のclientは`0x1800004`、
activeは別PID 741846の`0xe00004`だった（UTC 15:53:31時点でも同じ）。
これにより非focus経路の成立を確認したが、初回失敗runのfocusやcompositor寄与までは確定しない。
再試行もproduction / completed / smallで3,600 sample、60.0 fps、p50=16.659912 msとなりinvalid。
job.json SHA-256=`f147bcd9bb3c35643741190051ffcf4e0ad1bf21a2a840da64f4db07ab05e160`。
両jobのchild終了後にのみ、primaryの`main.rs`へ次の修正を実装した。

次の変更は非focus制限が計測を支配する仮説だけを検証する。perfの実windowに限り
`WinitSettings::continuous()`を設定し、通常play／headlessには適用しない。
通常mode維持とperfのfocused/unfocused両方Continuousをtestする。画面設定・VSync・測定時間・
60Hz拒否gateは変更しない。変化がなければ同仮説の値調整を繰り返さず、表示環境側へ切り分ける。
`perf_window_update_settings`は`perf_enabled && !headless`のときだけresourceを返し、
WinitPlugin設定後に適用する。2 testでfocusの両分岐と通常／headlessの3組合せを検査する。
新sourceでの実機再採取前なので、この修正で実測の60fps固定が解消したとはまだ主張しない。
修正後の`dev.py check`、明示workspace Clippy `-D warnings`、`dev.py verify`全gateがpass。
profiling binaryの上記focused 2 testも2/2 pass。`main.rs`のrust-analyzer診断はerrors=0 / warnings=0。
native helper・asset view・原図は変更していない。新規worktree/branch作成・削除なし、回収容量0。
MemoryはCapture完了後に逐次実行する。ちらつき・本番昇格も未完了で、本番locatorは不変。

### v4連続更新修正後の実機結果

clean subject `552d9c91d290eaa25a6619f2c947a6d0a755ebe4`へ既存validation worktreeを移し、
generation 13とexact asset viewを変更せず、正式24-run profileを再実行した。
jobは`wall-production-performance-20260912T160545Z-bcbfc3ba`、親directoryは上記共通worktreeの
`target/native-acceptance/`。7分42秒のCapture build後、30秒warmup／60秒measureを22 process採取。
22回の各raw runは検証されたが、22回目の反復安定性gateでjob全体が`invalid`となった。
残り2回、全体集約・p95/p99比較、Memoryは未実施。途中session/orderの`running`表示を
成功とは扱わず、終端`job.json`のinvalidを正本とする。childは終了済み。

- source fingerprint: `289c24a6f16203edff7f6df738e0bea9144c5e7759e2919c88f5ba32268ed315`
- harness fingerprint: `6dbc93b109a45d5cb7bad41a78e0ec0f51b3ad6a876d760e5665c8050255e6c3`
- asset view fingerprint: `415a8f0b3e8835dce44c3c16f897b96d1be702eb7dc699b6bd3274cda416ad78`
- Capture binary SHA-256: `182ef8f2df453b8526bc526cd5f50b311d9284d9138c07de7ccb117863622594`
- job.json SHA-256: `6b95a36f84f9ba46a9db61741b4c08a61d02800ed579dcbd56ec5fd78628ab92`
- capture-order.json SHA-256: `3320a90bcba205e2c2a892553914a30c089c3b6369cba517d0dc62c34a2324f6`
- 実adapter: Intel Arc (MTL) / Vulkan / Mesa 26.1.8、X11、1280×720、High / DPI 1、Immediate。

初回completed / smallはfallback 124.767 fps / p50 7.536633 ms、production 121.750 fps /
p50 7.559471 ms。`xprop`でゲームPID 785305／786258のclientが`0x1800004`、
activeが別client `0xe00004`であることをそれぞれ観測した（UTC 16:14:55／16:16:30付近）。
非focusでも60fps固定にならない経路が成立し、全22回の`display_paced=false`を確認した。
これは60Hz制限修正の確認であり、旧invalid job比のゲーム高速化率ではない。

停止条件はfallback-control / provisional / smallのp50
`13.192470 / 8.104464 / 8.342522 ms`、最大／最小比`1.627803`（上限`1.25`）。
同条件productionは`8.840660 / 8.001466 / 8.126925 ms`、比`1.104880`。
本設4 cellは各3反復が揃い、p50比は`1.024〜1.117`だったが、部分結果を全体合格へ流用しない。
fallback / provisional / mediumも採取済み2回の比が`1.299254`であり、基準表示側の
ばらつきはsmallだけではない。閾値・測定時間・case集合・アートを変えて通す修正は行っていない。

次は長いmatrixを無条件に再実行せず、電源・温度・CPU/GPU負荷等を計測中に観測し、
同一fallback条件のbounded診断で変動要因を切り分けてから正式jobを新規採取する。
終了後のread-only確認ではCPU governorは`performance`だったが、計測中の履歴はないため
温度・他process・compositorのどれが原因かは未確定。他アプリ終了やOS設定変更はしていない。
native helper・Rust・asset dataへの追加変更なし。`docs/performance-profiling.md`へ連続更新の
適用範囲とVSyncとの違いを同期した。Helpはperf起動に限定する同じ実経路でNo impact。
本番Wall generation 10 / Door generation 7、画像・原図は不変。新worktree/branch作成・削除なし、
回収容量0。ちらつき確認・正式Memory・本番昇格は引き続き未完了。

### Help impact（実経路レビュー）

`No impact`。`create_prism → GLB UV0 → ResolvedProductionWallAssets →
ProductionWallMaterialPool.complete → apply_wall_presentation_system`を確認した。
本設の貼り方だけを変え、`is_provisional`による型枠／本設選択、topology、建設入力・必要資材・
成立条件・完了結果・Room境界・文言は変えない。`orders_building_zones`のArchitect説明に
この旧UVへ依存した説明はないため、Help providerとsnapshotへ空変更を加えない。
本変更はPython制作経路なのでgateのproduction分類外だが、過去のmixer no-impact passを
本変更のレビューとして代用せず、上記経路で独立判断した。
断面の色合わせも同経路を再確認した。追加の材質設定と白色照明はBlenderの確認画像にだけ作用し、
ゲームの照明・UI・建設機能は不変のため、Help更新は引き続き不要。
v3でも同経路を再確認。変更は断面albedo/emissiveとUV、制作時の比較投影のみで、
プレイヤーの入力・成立条件・資源費用・状態の意味・文言は不変。Helpは`No impact`。
v4も同じproducer/consumerを再確認。`wall_asset_set.rs`のcomplete materialが本設albedo/emissiveを
消費し、`wall_presentation.rs`の`is_provisional`分岐とtopologyは不変。
`orders_building_zones`のArchitect説明は金具模様や発光を状態識別条件にしていない。
下部装飾削除・石面／断面の簡略化は建設入力・必要資材・成立条件・状態の意味・Room境界・文言を
変えないため`No impact`。Help provider / snapshotへ空変更を加えない。
v4承認記録／封印追加も実経路で`No impact`。制作側の承認・隔離候補・release payload検査のみを追加し、
runtime material選択・建設入力・資材・状態の意味・Help本文を変更しない。
性能計測の連続更新も`No impact`。`PerfScenarioConfig::enabled → perf_window_update_settings →
WinitSettings`だけに作用し、通常起動ではresourceを上書きしない。Architect入力からWall完成・
Room境界と`orders_building_zones`の説明へ至る経路は不変。Help provider/snapshotは変更しない。

## 10. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| 2026-09-12 | Codex | 原因・UV修正方針・承認境界を固定 |
| 2026-09-12 | Codex | M1〜M2完了。6 GLB・前後board・128 Python test・全体gate pass、native前のコミット承認待ち |
| 2026-09-12 | Codex | 断面色の追加修正。UV-v2と確認材質の反射補正、紫rim／白色照明の比較画像を別生成 |
| 2026-09-12 | Codex | 追加依頼を調査。本番と候補の厚さ9.6一致・確認boardの縦補正欠落を確認し、断面textureと同倍率比較の案を記録。実装は未実施 |
| 2026-09-12 | Codex | 承認済み案をv3へ実装。専用断面texture・投影補正・両軸／連結／倍率別board、6 GLB検証と134 Python test pass。本番は不変、コミット・native前 |
| 2026-09-12 | Codex | 壁修正と本設preview経路をコミット。起動条件2件を修正後、同一subjectで標準／最遠native ArtPreview pass。画像・hash・未検証範囲を記録。本番は不変 |
| 2026-09-12 | Codex | 実機等倍での可読性指摘を受けv4へ整理。下段装飾廃止・全側面stone・低コントラストcore、6 GLBと137 test・全体gate pass。未コミット、native再撮影・本番反映前 |
| 2026-09-12 | Codex | 明示承認でv4をコミットし、隔離generation 12で標準／最遠native ArtPreview pass。実機画像・identityを記録。本番反映は保留 |
| 2026-09-12 | Codex | v4アート承認を記録。正式封印経路・14回帰testを追加しgeneration 13を封印、151 test・全体gate pass。正式標準matrixは初回単色captureで停止、同条件再試行で9/9 valid・独立verify pass。本番は不変 |
| 2026-09-12 | Codex | 正式性能Captureは2回とも60Hz判定で停止。非focus経路を観測し、perf実window限定の連続更新と2回帰testを追加。実機再採取・Memory・ちらつき・本番は未完了 |
| 2026-09-12 | Codex | `552d9c91`で22回の非paced実機計測を確認。基準表示の反復p50比1.628で正式jobはinvalid。原因未確定・再採取前提、Memoryと本番は保留。性能仕様書を同期 |
