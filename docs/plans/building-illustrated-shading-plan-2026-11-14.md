# 建設設備を面塗り主体の描画へ変更する

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | building-illustrated-shading-2026-11-14 |
| ステータス | In Progress |
| 作成日 / 最終更新日 | 2026-11-14 / 2026-11-15 |
| 作成者 | Codex |
| 関連Issue | TAK-14 導入後のシェーダーフィードバック |

## 1. 目的

g3の材色変更でも3D感が強いという利用者の観測を制約にする。丸みを連続的な陰影で示すPBRから、
描いた色面を主体にした平面的な挿絵へ変更する。材色の再調整では解決済みと扱わない。

## 2. スコープ

指摘されたTank/MudMixer/RestArea/OutdoorLamp/SoulSpaの表現方式を段階的に変更し、
非橋9種の混在・性能で最終確認する。骨・布・錆・木の色面、実状態・emission、
alpha・depth・施工clip・光と遮蔽の成立を保つ。Wall/Floor/Bridgeとゲーム論理は対象外。
Doorと他の小物3種は既存成果を保ち、混在時の統一感と退行を確認する。
最初の照明試作では世代を変えなかったが、画像投影の正式導入では§8に従い新世代と独立採否を要求する。
Tank付属Spriteの計画tint修正は§7の確定原因に限定し、選択・保管・搬送を維持する。

## 3. 現状とギャップ

structural shaderはPBR照明と後段shadow styleを使う。Tankだけ法線0.82混合をするが、
他設備では面の幾何法線による丸み・箱の明暗がそのまま残る。compositeはtextureの転送だけ。
未使用のindoor light field fragment readは起動失敗の履歴があり、今回再有効化しない。

## 4. 現行方針と比較基点

照明だけのNPR仮説は§7の結果で打ち切った。係数調整や同じ試作の再実行はしない。
現行方針はTank一種のbodyへ手描き画像を投影する2.5D pilotである。
body専用materialを追加し、Emptyではbody、Partial/Fullでは既存の実water mesh12/22WUを使う。
画像が色面と描線を、元geometryがdepth/外形/遮蔽/shadowを担当する。
正式galleryや通常buildとは別の閉じたprofiling sessionと実material witnessを作り、
新paintの有効性を確認してから正式schema/role・他設備への展開・性能受入を計画する。
同目的branch codex/tak14-non-bridge-releaseを再利用し、基点c520c2fa6f1ac31d20b4d84054db042e40295287。
主担当が編集・検証・Gitを所有、独立reviewはread-only。公開・push/mergeは行わない。

## 5. マイルストーン

### 閉鎖済みの照明試作（履歴）

- [x] 白面参照と色面を分離したNPR、対象限定、発光とalpha/depth/clip保持を実装した。
- [x] 適用・除外・状態材質の回帰19test、check/Clippy/Help/技術reviewを通した。
- [x] normal/farの実機比較を行い、外観はchanges_requestedとして不採用にした。
- [x] 通常PBRとTank0.82を元のbytesへ戻し、打切り理由を恒久仕様へ記録した。

### 次の画像投影pilot（実装・検証中）

- [x] exact identity/mesh/paint/投影を束縛するprofiling専用入口・planと拒否試験を作る。
- [x] body専用projected material、water分離、ownerから実materialへの初終witnessを実装する。
- [ ] normal/far・0/Partial/Full・Actor/Wall前後・別位置/moveで原画と投影の成立を比較する。
- [ ] 独立外観採用後、正式schema/role・他設備への展開と変更別性能・Help/CI/storageを閉じる。
- [ ] 採用方式を恒久仕様へ集約後、この一時計画と不要な検証出力を整理する。

## 6. リスク・検証

陰影を減らして開口や状態を失うリスク、暗所の発光化、段階境界のちらつきを通常/遠景で確認する。
無変化なら同系統の値調整を続けず、幾何投影/画像方式を再評価する。
shader/APIはローカルBevy0.19一次sourceと既存実装で確認する。
dev.py check、cargo clippy --workspace --all-targets -- -D warnings、変更別CIをprimary guard経由で実行する。
Help Skillとnative Skillを適用し、validationのretain/plan/seal/finalize/checkを使用する。
旧終了jobは再開せず、新subjectはfresh jobへ拘束する。実画面の最新提示・修正用cacheはフィードバック中保持する。
閉じた比較のrawは恒久仕様へ結果を集約後に整理し、原本と採用assets/旧世代は保全する。

## 7. 最初の試作の結果と再計画

`d68a72c5`のlighting-only galleryは技術passだが、独立画像レビューはchanges_requested。
3D感を十分減らせなかったため係数調整を止め、通常描画を元へ戻す。実uniform値の観測も不足していた。
比較画像と結論はbuilding-art-direction.mdへ集約し、旧verifierのlive source依存は封印・独立確認後に終了する。
生成paint候補はdocs/design/building-art/tank-projected-paint-candidate-20261114.pngへ隔離保存した。

次はTank bodyの画像投影pilotを作る。body専用materialを1つ分離し、Full水面とopaque共用しない。
明示的profiling session、exact Tank identity/body GLB/paint PNG・decoded RGBA・canvas/anchor、
source/binary・nonce/owned windowをpinする。normal/released gallery/M6/candidateとの混在を拒否する。
owner→Root→body/water→実material→mode/texture/path/digest/local/global poseを初終で観測し、
declared値のechoだけを証拠にしない。paintのalphaをgeometryの穴にせず、無効RGBや黒縁を拒否する。
0/Partial/Full、normal/far、別owner/move後、Actor前後を閉じた比較で確認してから正式schema/roleへ導入する。

専用入口は`HW_BUILDING_TANK_ILLUSTRATION_PILOT`、契約は`building-tank-illustration-pilot-v1`。
`scripts/building_tank_illustration_pilot.py plan/run/verify`がsource/driver/assetとowned windowを束縛する。
最初の静止比較は4位置・Empty/Partial/Fullのnormal/farだけを確認し、move/Actor前後や性能を証明しない。
元paintは変更せず、opaqueな木色へalpha合成した後256pxへ縮小し、透明RGBの黒縁と高周波の縮小を避ける。
material mode=2とRoot相対のcamera投影を使い、初終の実image bytes/materialと既存geometry観測をentityで結合する。
通常sessionとの混在、別Tank世代、body/water共用、foreign image、image未load・変更を拒否する。
Help判断はNo impact。入口はprofiling専用で通常起動から到達せず、材質pilot以外の操作・状態・保存・
失敗理由・Help reachabilityを変えない。正式導入時は実プレイヤー経路を改めて照合する。

2026-11-15のpilot v1はnormalのowned X11/Vulkan画像と実mode=2材質まで取得したが、
検証器がRust codecのbuildingsetをPython canonical metadataとして読んだためexit=1/invalidで封印した。
farは未実行でありpassへ読み替えない。検証器は既にcodecと全asset bytesへ拘束されたplan内の
decoded manifestから水面画像を解決するよう修正する。source/identity/assetの再照合は維持する。
このgalleryの画像寸法は1920×1680、DPI1であり、通常ゲームのviewport受入とは別である。

修正後`7c92ebe0`のpilot v2はnormal/farとも登録済み独立verifyとsealが成功した。
実mode=2、256px paint、Root/part/material/ChildOfの一致、Empty0/Partial25/Full50×2を確認した。
独立目視は手描きの輪郭・木板・帯の線と遠景のEmpty対水面の差を認め、方式の方向だけを採用した。
完成art/numeric/release/通常lifecycle/性能は未承認である。

黄土色の上縁はpaint未被覆と仮定したが、その仮説は実geometry投影のcoverage検査で否定された。
全17889texelがpaintのalpha>=128に入る。実cameraで付属BucketStorageの黄色2D Spriteを投影すると、
Emptyではx696–760/y1408–1440となり、問題の(728,1432)に重なる。
MainCamera order2がRtT合成order1の後に描くためであり、原画や投影値は調整しない。
通常Visual phaseへ完成Tankの計画tint正規化を追加し、旧保存・撤去preview復帰・Pending不変を検証する。

## 8. Tankから通常版へ接続する

v3（`8da324ef`）のnormal/farと8付属Spriteの実RGBAゼロを独立verify/sealで確認し、
独立目視でも黄色帯の消失と手描き方向を採用した。静止pilotの方向採用であり正式releaseではない。

- [x] Tank限定のoptional `body_paint_projection`を追加し、省略した旧manifest/receipt bytesを維持する。
  `world_preview_v1`方式とmatteだけを持ち、body・world_preview参照に限定する。
  canvas/anchorは既存previewから導出し、RtT合成の縦補正をshaderで一度だけ適用する。
- [x] 新しい手描きworld_previewを透明のままbodyとpreviewで共有し、bodyだけopaque matte合成する。
  GLB・albedo・水位12/22・partsを保持し、body専用materialと水面を共用しない。
- [x] 保存済みg3編集可能原本bundleをSHA固定で新stagingへ読み、新paint・mask・設定・producerを記録する。
  元bundle/canonical/旧復旧codecを変更せず、新sourceと新codecを別identityで保存する。
  producerは既存blender-safeを直接使用し、networkを分離する。launcher/Flatpak executableと
  app/runtimeのref・deployment commit・location・metadataを固定し、外部audit前後に再照合する。
  base/source/recipe/report/producer/packerの監査中差替えも拒否する。
- [x] 新preview identityへ独立art/numericを束縛する。
- [ ] 候補実機・配置previewの輪郭・move/遮蔽と性能を確認する。
- [ ] 新generationの正式promotion/install/rollbackを既存guardで行い、既定binding・恒久仕様を同期する。

他設備は部品構成と状態を個別に扱う。特にMixer rotorへ固定した完成図を貼って回転を失わせず、
Spaの実利用slot/emissionとRestAreaの入所Soul非表示を維持する。Tankの静止passを流用しない。

この正式接続コードのHelp判断はNo impact。producer→manifest→pool→Tank body materialと既存
preview consumerを照合し、画像/描線以外の操作・水量状態・施工・配置・移動・保存・fallback・文言・
Help到達性は変わらない。付属SpriteのHelp更新は`8da324ef`のまま維持する。

`d3b9d4b7`のTank g4はnormal/farのclean-source ArtPreviewを独立verify/seal/finalizeし、
別途の画像・形状レビューでartと水位12/22を採用した。ArtPreviewは`de153240ebd8d5d306a3740365b2f1704db67f5f53de8abc148ffd61c39fdf36`、
新しい隔離candidateは`b8f6087f99c641995eb733052f1906efdf9e3acf4fa92c85a4bce2ee113bf661`。
geometry/UV/GLBはg3不変。新source・mask/paint/config・承認原本は別stagingに保持する。
通常起動はまだg3であり、candidateのlifecycle/性能/releaseは未承認。
最後の9種受入は他設備の表現が揃ったsourceでまとめ、見た目調整の各反復で全性能matrixを繰り返さない。

## 9. 回転するMixerは部品に描線を固定する

カメラ固定の完成図をrotorへ投影すると回転の見た目を失うため、既存M2 v3のUVとlocal Y軸を使う。
新しい512px atlasへ円筒の継ぎ目・傷・非対称な摩耗印を描き、既存face-cellのgutterまで連続に埋める。
triangle/UV島を一枚ずつ黒枠にする方法と、面法線由来の明暗をalbedoへ焼く方法は使わない。

- [x] identityに拘束した閉じた`surface_paint/uv_ink_v1`と、通常observerの実mode/texture検査を追加する。
- [x] 既存編集可能原本と数値契約を保持し、新atlas・world-up diffuse・4姿勢previewを新規制作する。
- [ ] body固定、rotorの描線追従、normal/far、pause/resume/load/move、preview整合を実機で確認する。
- [ ] 採用時だけ新art/numeric/candidateへ進み、最終sourceの混在9種・性能・切戻し・正式導入を閉じる。

投影Tankのmode3と旧None経路は変更しない。UV方式はalbedoを主役にし、
照明参照normalだけを固定する。実geometryのnormal/depth/影bias、emission、alpha、post処理は保持する。


`8702f3fe`のMixer g4はbody/rotor/geometryの旧g3全bytes一致、四主姿勢の別hashを確認した。
normal/far contextを登録済み独立verify/seal/finalizeし、固定read-only reviewerが新artと既存の
local Y軸/毎秒1.5707964radを独立採用した。previewは`5d3a40b9364b38509853e0f769c4fc3ec169c39a6f2ef522e54902f25dbf953e`。
連続稼働・通常lifecycle・最終sourceのcandidate/性能/releaseは未承認であり、既定g3を維持する。

## 10. 布小屋・骨の四区画・2D灯具を揃える

- [x] M3へ明示illustrated-v1、固定toolchain・verify後差替え拒否、全gutterのlocal描線を実装する。
- [x] 既存geometry/UV/part/数値を維持し、Spa発光atlasは旧producerの全bytesを保持する。
- [x] Rest/Spaの通常hostへmanifest由来mode4/atlas/base値を要求し、既存利用/発光検査を残す。
- [x] LampはForeground2dのままポールの立体的な明るい面を抑え、既存全alpha/anchor/点灯状態差を保持する。
- [ ] 新原本を生成し、旧GLB/UV/発光との一致とnormal/farの見た目を独立確認する。
- [ ] 独立art/numeric採用後、全変更種の最終source candidate/lifecycle/混在9種/性能・切戻し・正式導入を閉じる。

この制作・検証拡張のHelp判断はNo impact。明示候補の制作から既存manifest/pool/表示・状態材質まで追い、
操作・前提・利用/休息/Dream/発電・保存・失敗結果・文言・Help到達性を変更していない。
新しい見た目の採否と正式導入は別工程であり、候補の技術passで通常版の変更を完了とは扱わない。
