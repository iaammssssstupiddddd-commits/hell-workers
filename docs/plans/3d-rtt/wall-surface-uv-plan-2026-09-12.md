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
側面は面内の水平距離と高さで等密度に展開し、上面は同系色の内部粒子・短い亀裂で
外壁ではなく断面と読める表現を検討する。確認画像の厚み比較はゲームと同じ投影条件を必要とする。

## 2. スコープ

- 本設6 familyのUV生成、専用回帰検査、隔離Blender原本・GLB・確認画像。
- 型枠7 file、Door、形状・厚さ・高さ・接続・material数は変更しない。
- 現在のWall generation 10を上書きしない。新しいアートの承認と本番昇格は別段階。
- 承認済み追加範囲: 本設albedo/emissiveの未使用領域へ断面textureを配置する。
  領域外の全画素は保持する。従来の「元texture画像不変」はv2までの実績。

## 3. 現状とギャップ

`create_wall_production_scene.atlas_uv`のstoneはXYだけを参照しZを無視する。
側面UVが線に潰れ、上面だけが2次元の石積みを表示する。rust/purpleもX固定面ではUが潰れる。
既存post-export gateはUVの型・有限値しか調べず、この退化を拒否しない。

## 4. 実装方針

- 外壁は輪郭辺の接線方向をU、高さをVとする。長短の辺を同じ画像幅へ正規化しない。
- 石面の上下は同じ高さ座標を使い、中段で模様をリセットしない。
- 上下面はatlasの専用粒子・微細亀裂領域へ平面投影する。
- 断面の色は側面の目地を除く石の代表色へ合わせる。Blender確認材質の不要な反射を除き、
  runtimeのroughness=1 / reflectance=0に対応させる。照明の違いは別に明記する。
- 従来の下半分の装飾面選択は維持し、rust/purpleも面内で等密度とする。
- 専用UV profile検査を追加し、旧世代・型枠の検証契約へ遡及適用しない。
- Bevy runtime/API変更なし。GLBのUV0を既存lit materialが消費する経路を維持する。

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
- [ ] nativeで標準／最大zoom-outの見え方と時間方向のちらつきを確認する（M3対象）。

### M3: 実機・本番反映

- [x] コミットの明示承認を得る（候補をコミットし実機確認へ進める質問への「OKです」）。
- [ ] clean subjectを固定し、候補を検証用asset viewへ封印する。
- [ ] 新しい本設UVを扱う承認前preview経路を用意し、nativeスキルのdirect kitty launcherで観察する。
- [ ] 新アート承認を記録し、final封印・正式native検証後、明示承認のある本番昇格へ進む。

## 6. リスクと対策

UVの線への退化、面ごとの密度差、上面への目地混入は専用検査で拒否する。
旧型枠sealerは本設8 file不変を要求するため、その承認を新しい本設meshへ流用しない。

## 7. 検証計画

全6 familyで側面のUV面積が正、面内2軸の密度が等しいこと、上下面が内部色領域内であることを確認。
GLBの輪郭・port・厚さ・triangle数は旧geometry gateで別に確認する。
`python3 scripts/dev.py check`、`python3 scripts/dev.py verify`、
`python3 scripts/dev.py cargo -- clippy --workspace --all-targets -- -D warnings`を実行する。
native未実施のBlender画像をゲーム内の完成証拠へ格上げしない。性能baselineは今回の対象外。

## 8. ロールバック方針

承認前はcanonical/runtimeに触れない。昇格時はgeneration 10のpointerとruntime locatorを
別々に保存し、immutable世代を残す。既存Wall/Door共通validation worktreeはこの小修正だけで削除しない。

## 9. AI引継ぎメモ

- 開始subject: `c58d2aed`、開始時worktree clean。
- 現在地: M1完了。v3の断面texture・投影条件を実装し、6 GLBとBlender比較を検証済み。
  M3のコミット・実機確認は承認済み。新本番昇格は未実施。
- 参照必須: `docs/blender-setup.md`、`docs/assets_workflow.md`、native/Help/docsスキル。
- 初回検証ログ: workflow Python 128/128 pass（新規7 testを含む）。6 GLBのscene/Khronosは
  errors=0 / warnings=0、geometry＋新surface UV gate pass。旧GLBとの全triangleの頂点位置・法線を
  頂点分割順に依存しない比較で照合し、6/6不変を確認した。
- 2026-09-12: `python3 scripts/dev.py verify`は`All quality gates passed`、
  `python3 scripts/dev.py check`および明示的なworkspace Clippy `-D warnings`もpass。
  Rust変更なし。native Capture / Memoryは未実施で、過去世代のpassを新UVへ流用しない。
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

### Help impact

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

## 10. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| 2026-09-12 | Codex | 原因・UV修正方針・承認境界を固定 |
| 2026-09-12 | Codex | M1〜M2完了。6 GLB・前後board・128 Python test・全体gate pass、native前のコミット承認待ち |
| 2026-09-12 | Codex | 断面色の追加修正。UV-v2と確認材質の反射補正、紫rim／白色照明の比較画像を別生成 |
| 2026-09-12 | Codex | 追加依頼を調査。本番と候補の厚さ9.6一致・確認boardの縦補正欠落を確認し、断面textureと同倍率比較の案を記録。実装は未実施 |
| 2026-09-12 | Codex | 承認済み案をv3へ実装。専用断面texture・投影補正・両軸／連結／倍率別board、6 GLB検証と134 Python test pass。本番は不変、コミット・native前 |
