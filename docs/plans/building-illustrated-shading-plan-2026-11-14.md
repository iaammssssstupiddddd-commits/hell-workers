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

非橋設備の構造材質とDoorのopt-in shaderを変更する。骨・布・錆・木の色面、実状態・emission、
alpha・depth・施工clip・光と遮蔽の成立を保つ。Wall/Floor/Bridge、foreground Sprite、
ゲーム論理・形状・manifest世代・asset authorityは変更しない。

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
