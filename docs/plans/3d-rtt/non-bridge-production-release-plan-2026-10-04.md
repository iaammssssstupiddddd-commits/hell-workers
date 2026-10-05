# 橋以外の建設物・正式ビジュアル導入

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `non-bridge-production-release-plan-2026-10-04` |
| ステータス | `In Progress` |
| 作成日 / 最終更新日 | `2026-10-04` / `2026-10-05` |
| 作成者 | Codex |
| 関連提案 | N/A |
| 関連Issue/PR | TAK-14 / 基盤統合PR #29 |

## 1. 目的

通常起動で橋以外の9種が正式ビジュアルになり、完成・状態・配置・移動・施工・カタログ・保存復元が一致する。
原移行計画の受入条件を維持し、橋だけを今回の完了対象から外す。技術previewと正式採用を区別する。

## 2. スコープ

Tank、MudMixer、RestArea、SoulSpa、Door、WheelbarrowParking、SandPile、BonePile、OutdoorLamp。
Wall/Floor、Bridge、gameplay・保存schema・配置条件の変更、廃止統括の再起動、外部公開は対象外。

## 3. 現状とギャップ

- 基点 `30477dab4cd091abfb2ee79cc1cb183e28c936b5`。通常起動はrelease bindingなしで従来fallback。
- Tank/MudMixerの未承認原本・previewは外部asset正本の `staging/imports/tak14-20261004` に保全済み。
- M3制作器とM5専用画像導入器は存在。primaryのbuilding manifestは未導入。
- 独立art/numeric採否、正式candidate受入、昇格・install・rollback、通常起動の既定接続が不足。
- production native toolingは通常開発のprocess/window/capture・host計測producerが不足。旧hostを代用しない。

## 4. 実装方針

mainがprimaryを直列編集し、branch `codex/tak14-non-bridge-release` を使用する。
候補を制作・比較し、独立レビューを経て正式assetへ昇格する。未承認assetを起動時に自動承認しない。
最終導入では承認済みの同一世代inventoryを既定起動へ接続し、明示overrideと失敗fallbackを検証する。
Bevy 0.19の既存loader/pool経路を使う。公開・push・mergeは本依頼の完了条件にしない。

## 5. マイルストーン

### M1: 原本・候補

- [x] 保全原本と現行loader/制作器を調査し、storage checkを実施。
- [x] M2の現行subject候補、M3両種、M5四種を制作・技術検査（未承認art_previewのみ）。
- [ ] 実機比較画像と数値契約を独立レビューし、exact identityに採否を記録。

### M2: 受入・正式導入

- [x] primary validation所有の通常world固定F12入力probeを拒否系試験・実機・独立reviewで確認。
  これはinput/raw輸送の技術境界だけで、正式recipe、domain lifecycleやreleaseの代用ではない。
- [ ] 通常開発の不足producerを実装し、拒否系試験と独立レビュー。
- [ ] 同subjectのCapture→Memory、state/lifecycle、有限pool/cleanup、群別性能を検証。
- [ ] Door既存assetと全consumerの適合監査。
- [ ] 正式promotion/install/release/rollbackを検査。

### M3: 通常起動・完了

- [ ] 承認済みassetの既定起動経路を接続し、環境変数なしで9種の表示を確認。
- [ ] 原仕様・Help・asset記録を同期。橋は未完と明記。
- [ ] 同subjectの変更別検証、Clippy警告0、workspace test、独立レビューとstorage closure。

## 6. リスクと対策

未承認候補・古い技術passを正式採用に流用しない。原本を上書きせず新しい制作名を使う。
ゲームの論理を見た目都合で変えない。失敗・中断は正確に記録し、未検証範囲を残す。

## 7. 検証計画・データ管理

primaryの `dev.py validation` で制作/実機batchを登録、独立verify、seal、finalize/checkする。
M3最初の原本候補batch: `nonbridge-restarea-authoring-20261004-v1`。
現行RestAreaはv3（無地入口レビュー後の描線）、SoulSpaはv2（施工＋16mask）。どちらも技術候補で正式採用ではない。
小型4種は固定256px canvasの専用原画生成器を追加。worldのcrop/rescaleや既存item画像の変更はしない。
M5 v1のBonePileは板に見えるため不採用とし、関節と軸の形状を補正して再生成する。
RestArea actual-window v2は画像取得後、AdapterInfoログ不足でinvalid。失敗を保持し、子専用INFOログへ補正。
候補計画のready返却欠落とpackaged-byte verifyの承認schema検査を修正し、独立read-onlyレビューを受けた。
関連40件のunit test成功。これらはordinary lifecycle/性能/art/releaseの完了を意味しない。
RestArea actual-window v3はIntel/Vulkan・exact identity・静止inventoryのtechnical verify pass。
ただし遠景scale 5の画像は採否に不十分なため、明示profiling-onlyの近景／通常／遠景viewを追加して比較する。
M5 v2 BonePileは関節と湾曲により独立read-only比較で板状の指摘を解消。人の正式art承認ではない。
小型4種のruntime packageをfresh隔離rootで検証済み。SoulSpaは候補名不一致の出力前失敗をinvalid記録後、
正しい既存原本名による別batchでpackage検証済み。原本や製品assetは上書きしていない。
M2 albedoのface normal由来の焼付け明暗を除去。materialの色差だけを残し、normal反転でpaint bytesが
変化しない回帰テストを追加した。旧候補を新しい美術・数値承認として流用しない。
Help実経路レビュー: 未承認staging制作・byte検査・明示profiling比較cameraのみの変更で、通常起動、
player操作、文言、gameplay、save schema、Help到達性は不変（No impact）。正式導入時は改めてレビューする。
現行のTank水面tintとatlas塗装も未導入のArtPreview候補に限る。開発用panelはprofiling条件下だけで、
通常起動のasset解決、操作、表示文言、状態遷移、保存形式、Help到達性には変更がない（No impact）。
Tank/MudMixerのv2原本とruntime packageを検証。Tankの水面は面全体に連続する2本の波線に修正し、
三角形ごとの放射状筆跡を除去した。原本・数値・ゲームの水位判定は別契約のまま維持する。
RestArea/Tank/SoulSpa/Parkingの近景、Sand/Bone/Lamp/MudMixerの通常zoom実機比較はtechnical passで
独立verify・用途登録・finalize済み。普通の利用・精製・採取・配電lifecycleを実行した証拠ではない。
Lampは通常zoomで素材・籠の判読が弱く、追加の拡大比較が必要。SoulSpaの全16mask、
最遠・異なる照明、実際のDream／配電／回転・保存復元・owner cleanupは未受入。
offline verifierへ先行9種の明示non-bridge scopeを追加。10種fullの条件を維持し、
9種でも異なるsourceの累積比較・両instrumentの全leg・全計測値を要求する。不足producerは未実装。
package importの孤立テストで露呈したnative helperの相対importを補正し、CLIの既存入口は維持する。
利用者の継続指示に基づき候補の方向性は統括が判断する。独立art/numeric承認や正式releaseとして記録していない。
まとめたPython関連92テストとprofiling m2 probeのRust3テスト成功。通常／profilingのClippy警告0。
profiling既存警告の条件式・test配置だけを整理し、Bridgeの挙動・制作は変更していない。
通常validationのlive直接child/親出生・command・sourceを照合するread-only入口を追加。
これは未接続の所有確認のみで、ordinary-world producerや不足計測を実装済みとはしない。
入口の独立read-onlyレビューで起動時の子公開raceと未検証plan再hashを修正、拒否系10件成功。
街灯v2の追加近景はtechnical passだが、独立画像レビューで骨の節・籠の内部判読不足が残った。
縮尺の経路は正常なためM5 v3原画で曲がった骨支持と太い籠の帯・2区画を再制作した。
32wu canvas・中心接地点・off/on同一alpha・内部のみ点灯を維持し、上端余白も回帰検査。
原画と街灯runtime v2を登録済みbatchでbyte検証、用途保持・finalize済み。
街灯v2の通常距離 `nonbridge-lamp-normal-20261004-v3` もactual-window technical passだが、
籠が数pxの横帯に潰れることを実画像で確認。縦長籠へ修正した原画v6、runtime v3、通常距離
`nonbridge-lamp-normal-20261004-v4` を独立byte/actual-window verifierでtechnical pass、用途保持・finalizeした。
画像の独立read-onlyレビューでは点灯状態と縦長輪郭の改善を確認した一方、通常距離の骨・錆鉄素材は
判読不足。正式art採用は未承認で、遠景・暗い背景・密集配置・実配電は未検証。
街灯v3の近景 `nonbridge-lamp-near-20261004-v3` もtechnical passしたが、独立画像レビューでは
籠が笠、骨支柱が短い脚に見えたため正式art候補として不採用。旧v6原画とruntime v3の専用出力は
比較終了後に正式holdを解除し、復元可能なゴミ箱へ移動した。原画generatorと画面証拠は保全する。
原画v7では骨の曲がり・関節と、背景が実際に見える鉄籠の隙間へ形状を再設計。
`nonbridge-m5-originals-20261004-v7` と `nonbridge-lamp-runtime-20261004-v4` は
独立byte verifyのtechnical passであり、後者の identity は `art_preview`、manifest
`8cc4fec0d4bed81f3f0b47c44082c6b307a0411f76377805aef5c9a5c323323e`。
同identityの近景 `nonbridge-lamp-near-20261004-v4`、通常距離
`nonbridge-lamp-normal-20261004-v5`、最遠 `nonbridge-lamp-far-20261004-v4` を
Intel/Vulkanの実ウィンドウで独立technical verify後、用途登録・finalizeした。
近景/通常距離の独立read-only画像レビューでは開いた籠・曲がった骨支持・off/on差が残り、
正式art評価へ進める候補とした。ただし静止profiling画面に限る。最遠の独立画像レビューでは
種類・off/onを画像だけで確実に判読できない。この `far` はcamera scale 2であり、
Bevy 0.19 `PanCamera` の製品既定最大scale 5ではない。最大倍率側の比較と、選択した
Outdoor Lampの既存情報パネルにある種類・Supply状態の実操作経路を確認する。
既存UI実装とHelp本文の存在だけでは代替経路を受入済みとしない。明暗・重なり・低品質、配置preview、実配電、
art/numeric正式採否は未完了。
同一identityの最大zoom-out scale 5 `nonbridge-lamp-overview-20261004-v4` はIntel/Vulkanの
実ウィンドウでtechnical verify passした。画像では街灯とoff/onがほぼ判読できないため、
情報パネルの建物種別とSupply状態を実選択で確認する受入が必要。既存の
`append_building_model` / `append_power_consumer_model` は値を構築するが、
最遠での選択可能性や実配電との同期をこの静止画像から推定しない。
原画の出力先を生成前に `chdir` した試行はexit 125で成果なしと確認し、invalid seal/finalize済み。
同一候補の比較入力holdが後続の正規planを保守的に拒否したため、各plan前に対象holdのみ正式releaseし、
出力を保持して再登録、plan完了後に保持用途を戻した。旧passや空の試行を採否へ数えていない。
変更別 `dev.py ci check --base 30477dab4cd091abfb2ee79cc1cb183e28c936b5 --mode auto` は
街灯v7と情報パネル回帰テスト後のsubjectでcontract/tooling/Rust群pass。
これはこの検証結果を計画へ追記する直前のsourceであり、追記後の最終差分は再検証する。
Bridgeの整形だけの差分は除外した。情報パネルが同一OutdoorLamp entityの種類と実際の
PowerSupplyState変更を表示する回帰テストを追加したが、最遠の実ウィンドウで選択できる証拠ではない。
同sourceの独立read-only実装レビューはblocking findingなし。ただし実機操作、art/numeric採否、
ordinary lifecycle／性能、正式releaseは未受入であり、実装レビューから承認を推定しない。
既存static画像はその撮影時subjectの比較用であり、後続のtooling差分を検証したnative受入には流用しない。
通常worldの最初のhost観測器 `building_production_local_probe.py` を追加。
primary検証batchの直接child／登録planとclean sourceを照合し、隔離cwdの通常起動から
所有window、raw sample/event、子process RSS/fdを採る設計。入力なし・正式identityなしの技術probeで、
全lifecycle、Capture→Memory、GPU/native/アプリ全体handle、正式美術・数値・releaseを受け入れたものではない。
拒否系unit testと現subjectの実機観測・独立reviewを経るまでM2のproducer条件は未完とする。
同観測器は拒否系5テストと独立read-only reviewで阻害なし。正規batch
`nonbridge-ordinary-probe-20261004-v1` はsource `ee9410363d4e8f7bd144506a64ac3fb3a71527dd`で
Intel Arc/Vulkanの所有X11画面、raw 3 sample／0 event、子process RSS/fdを技術verify passとして
seal/finalizeし、比較用途をstorage holdへ登録した。画像は通常開始時点で建設物がなく、
操作・建設・配電・save/load・release identityは観測されていない。0 eventをlifecycle passへ読み替えない。
Door g7はprimaryの既存locator `21075f6b5c6e4412ccc17632ecc3b44bcb619bb901a06062fa0905c0be303002`、
receipt `95ba6a19494d84825db54075cf8735d2024d18e6983dc54343cdc71c19a046ad`と
6 core fileの保存済みhashが現時点で一致。現行HEADのghost/catalog focused test 4件もpass。
この静的監査は通常配置・load後・pool cleanup・性能の実機受入を代替せず、M2のDoor監査を閉じない。
8種の保存済みactual-window画像を独立read-only監査したところ、全て`art_preview`で正式採用は未決。
Tank近景の空状態は開発パネルに遮蔽され、水位差も弱い。SoulSpaの骨素材／全mask、
Lamp最遠の種類・状態と情報パネル経路も重点未達。明示building-art session中だけの
開発パネル非表示を補正し、同一新subjectでTank近景を再観測する。通常プレイヤーUIは変更しない。
`c606733989c0f21b6fba4dea7138190f5f873409`の明示art-previewでは、保存済みdebug表示が
再適用されてもDevPanelだけをLayout前に隠す実装・回帰テスト・固定read-only reviewを通過。
Tank近景 `tak14-tank-near-c6067339-20261004-v1` と通常倍率
`tak14-tank-normal-c6067339-20261004-v1` はIntel/Vulkan実window・同manifest
`87997feeafa4c99dddeab227d0e8e60bfc371d8572e208f0266a652abccc9596`で独立verify pass、
seal/finalize/storage check済み。近景の空槽遮蔽は解消し4標本を確認できた。
ただし通常倍率ではPartial(25)／Full(50)の水位差を判読しづらく、近景の木部材・鉄帯と
空槽内面の三角形状も正式画風採否には不足。静止画像は背景・preview・実給排水・性能を含まない。
この候補の正式art/numeric採用とreleaseは保留し、まず通常倍率で状態差が読める
表現／確認経路と内面描線を修正し、fresh identity・同subject画像で再評価する。
通常倍率の判読不足を受け、Tankの既存`TankPartial`状態にのみ低水位の陰を示すmaterial tintを
追加した。水量・高さ・asset identityは変えず、世代交換／owner cleanupの既存回帰を維持する。
これは候補補正であり、変更後のactual-window再撮影と独立採否までは未受入。
`29fcf958b0f661e8edc56979cff10c472ce6ffd9`の通常倍率feedback
`tak14-tank-tint-feedback-29fcf958-20261004-v1`は、Intel/Vulkan実windowの静止画像を
独立verifyしtechnical passとしてseal/finalizeした。Partial水面がFullより暗く、以前の
同倍率画像より区別しやすい。ただしfeedback-onlyであり正式art、数値、releaseの権限はない。
同sourceのart-preview試行`tak14-tank-normal-29fcf958-20261004-v1`は、recipeが旧証拠の
`target/profiling/bevy_app`を上書きするビルドを始めたため、そのcodec SHAが変わる前に
該当ビルド工程を停止しinvalidとしてseal/finalizeした。画像やpassは存在せず、診断ログだけ保持。
旧2件のcodec SHAは不変。正式previewは別worktreeの専用targetで作る。
独立画像レビューでは状態差の改善を確認したが、暗色が水質差に見える可能性、鉄帯と空槽底の
面割れが残る。数値fixture・geometry・水量を変えず、槽底を連続した同色・world基準の木目に、
外壁だけを控えめな錆鉄帯にするpaint候補と拒否系テストを追加した。原本の新規生成・近景／通常
比較と独立採否まで正式候補ではない。
`b8dbce220e18ecf39e80fa5d04d185ae014925cd` でTankのpaint候補を新規生成し、
原画とruntime packageのbyte/UV/geometry/state検証をtechnical passとして保持した。
runtime identityは `art_preview`、manifestは
`22ff091af4af76527361c9b6c6da8c224c792a9fd81e58e7b3a3a9cd4755329f`。
専用worktreeの初回actual-windowはignored assetの不足でinvalidとし、失敗記録を保全。
primaryと同一内容のasset viewを復元したfresh batchでは通常倍率と近景がIntel/Vulkanの
所有X11 window・独立verifierでtechnical pass、用途登録・finalize・storage check済み。
独立画像比較では槽底の放射状面割れが減り、Partial/Fullの色差も増した。一方、近景の
PartialはhoverのMove tooltipとselection強調に重なり、鉄帯のface境界に細い切れ目と
太さのばらつきがある。背景・照明・遠景・実給排水とstate transitionも未受入。
従ってこれは静止技術証拠であり、独立art/numeric承認、正式候補、promotion/installではない。
近景のhover重なりは通常worldの美術欠陥と混同しない。明示profilingのbuilding-art静止
sessionだけhover更新を抑え、通常起動・通常inputは不変のまま同identityをfresh sourceで再撮影する。
`64f69762c3edbbd3147bac725e8f8792af97c078` の明示ArtPreviewでhover由来の
Move tooltip・selection強調がないTank近景をfresh subjectのIntel/Vulkan実windowで取得し、
独立verifierのtechnical pass、用途登録、finalize、storage checkを完了した。
独立read-only画像評価ではEmpty/Partial/Fullの区別と槽底の改善を確認したが、
外壁の錆鉄帯にUV面境界の切れ目と幅の不揃いが残り、正式art採否は不可。
静止草地画面であり、異なる背景・照明・遠景、実水量遷移、numeric freeze、releaseは未検証。
この指摘に限って外壁の2帯を同じ高さ比率の面へ塗り、UV境界外に描線を延長する候補を
`534fd9b30c6bd5050bde567faae40affb92ec110` で補正した。12面の両側で帯がgutterへ届く
拒否系回帰テスト、変更別CI、Help No impactを確認した。新規のTank原本
`nonbridge-tank-authoring-20261005-v4` とruntime package
`nonbridge-tank-runtime-20261005-v4` を同sourceから生成・独立検証し、technical passとして
seal/finalizeした。runtime manifest identityは `art_preview`、manifest SHA-256は
`edc65d14fe5f97b8953ccdd1487b62b1fcb51d9112915968d5994c2edf29f9cb`。
数値freeze、独立art approval、production candidate、promotion authorityはいずれもない。
専用worktreeと同一codec/identityのIntel/Vulkan所有X11 windowで近景、通常倍率、遠景、最大ズームアウトを
それぞれfresh batch `tak14-tank-band-near-534fd9b3-20261005-v1`、
`tak14-tank-band-normal-534fd9b3-20261005-v1`、
`tak14-tank-band-far-534fd9b3-20261005-v1`、
`tak14-tank-band-overview-534fd9b3-20261005-v1` として撮影した。4件とも登録済み独立verifierで
technical pass、用途登録、finalize、storage check済み。独立read-only画像評価では、近景・通常倍率に
帯の重大な断絶は見えず、Empty/Partial/Fullも色差を伴って判別でき、正式art採否へ提出する
候補には進められる。遠景の草地上でも三状態の色面と桶の外形は判別できるが、木目・水位高低差・
鉄帯の連続性や素材感は判定不能。一方、通常倍率の鉄は細い灰色線で錆の質感が弱く、最大ズームアウトでは
建物が十数pixel程度のためPartialとFullを確実に区別できず、鉄帯も判定不能。黒背景の静止画から
通常地形の最遠判読性は証明できない。これらは静止ArtPreview比較に限り、通常worldの
給排水遷移、異なる背景・照明・重なり、低品質設定、native性能、数値freeze、正式art採否、
release/install/rollbackは引き続き未受入。
通常world固定F12入力probeの初期7 batchは全てinvalidとして独立にseal/finalizeし、診断保持と
primary storage checkを完了した。v6 `tak14-ordinary-input-probe-b885f623-20261005-v6` は
所有X11 windowへの押下・解放receiptを残したが、製品raw 49 sampleには対応する入力イベントがない。
focus待ち・tap順の調整は同じ欠落を再現したため打ち切った。v7
`tak14-ordinary-input-probe-69aa99d1-20261005-v7` は原 `KeyboardInput` メッセージも記録したが、
XTest F12 receiptに対して48 sampleで原メッセージ・ButtonInput状態イベントがともに欠けた。
Xwayland上流の入力配送を疑い、次の技術probeは既存の同意付きWayland portal輸送へ固定する。
所有windowと同意、製品rawの順序を新sourceで確認する。これは9種のlifecycle、正式recipe、
art/numeric承認、releaseを完了させるものではない。
新source `dcac0f7541b26ec788b3bee93973e0448c5eac96` の
`tak14-ordinary-portal-probe-dcac0f75-20261005-v8` はportal同意と固定F12輸送receipt、
製品rawの原メッセージ・ButtonInput状態遷移を5 sample／4 eventで確認し、独立verify pass、
用途登録、finalize、primary storage checkを完了した。これは入力配送境界の技術証拠で、
通常worldの施工・保存・cleanupや9種の正式受入を代替しない。次はこの成立経路を用いた
通常world Capture/Memory producerとhost instrumentを別工程で実装・受入する。
現行の正式raw verifierは新しい原 `KeyboardInput` イベントを拒否していたため、厳密な
key/state/repeat/window payloadを持つ入力観測としてだけ追加し、domain witnessには流用しない。
拒否系fixtureと独立reviewを経ても、通常worldのhost admission／capture ACK／instrument
producer欠落は残る。補正前のcollectorはX11 XTest固定であり、portal輸送の実証だけでは正式driverにならない。
本補正ではoffline production planをportal専用v2へ更新し、collectorを固定同意journal・
所有window・focus・raw ACKのportal輸送へ接続する。grantをaction logへ残して独立検査するが、
host admission、actual-window capture/ACK、外部GPU/native/RSS/application handle計測器は
別途必要であり、このコード補正だけでは正式campaignを開始できない。
同一Tank preview identity `edc65d14fe5f97b8953ccdd1487b62b1fcb51d9112915968d5994c2edf29f9cb` の
近景・通常・遠景・最大ズームアウト4枚を独立read-only美術再評価した。近景の槽底・鉄帯の大きな断絶は
解消し、通常倍率でEmpty／Partial／Fullを区別できるため、追加の形状変更は現時点では必須でない。
ただし最大ズームアウトではPartial／Fullの判別が不確実で、黒背景の静止画像は通常地形との分離を
証明しない。正式art採用は `changes_requested` とし、実操作による既存情報UIの確認経路、異なる明暗・
壁／Soul／隣接設備との重なり、低品質表示、完成と配置previewの接地・縮尺、実状態遷移を同identityの
追加証拠で評価する。技術画像の再評価を数値freeze、lifecycle、release承認へ転用しない。
現行source `0b4738dd8f831bbdfdc981fb140903ddfd89f870` のRestArea通常倍率
`tak14-restarea-normal-0b4738dd-20261005-v2` は、同じ未承認ArtPreview identity
`364c1ad44187d074fe4bc7251f3a919a40659cf49da59187662ce733e7179860` を
Intel/Vulkan実ウィンドウで撮影し、独立verifierのtechnical pass、用途登録、finalize、
storage checkを完了。独立read-only画像比較では入口開口と支柱の形は読めるが、
屋根が板または皮革に見え、縫い目・継ぎ布・皺と木支柱との素材差が通常倍率で不足。
正式art採用は `changes_requested`。配置geometry・利用状態・保存契約を変えず、
布atlasの色面と少数の世界座標基準の継ぎ目・皺だけを新候補へ補正する。
この撮影は静止比較のみで、利用者/Dreamの実遷移、異なる照明・背景、最遠・重なり、
低品質、preview、numeric freeze、ordinary lifecycleやreleaseを証明しない。
原画補正 `77e5bd2870261c6489bb92cc983024b84913fb34` は布色を木材から分離し、
世界座標基準の継ぎ布・2本の横断継ぎ目・少数の皺／縫い目を追加。geometry、入口、利用状態、
SoulSpaには触れていない。変更別contracts/tooling CI、回帰7件、Help No impact、
独立read-only実装レビューはpass。新原画 `tak14-restarea-authoring-77e5bd28-20261005-v1` と
runtime package `tak14-restarea-runtime-77e5bd28-20261005-v1` は独立byte/geometry verify、
用途保持、finalize済み。新manifest `db0f535611de1140cc83dd82f08128052da82e5d8dd2642d77f3075d5fae2522`
は依然 `art_preview` で正式authorityなし。同identityの実画面通常倍率
`tak14-restarea-normal-77e5bd28-20261005-v1`、近景
`tak14-restarea-near-77e5bd28-20261005-v1` はIntel/Vulkan所有windowで独立technical verify pass、
用途保持・finalize・storage check済み。独立read-only画像比較では旧候補の茶色板／皮革感は解消し、
淡い補修布、暗い木支柱、継ぎ目と入口が両倍率で読めるため、この2 viewの追加paint補正は必須でない。
正式art採用は未実施。情報UI、明暗／重なり／低品質、配置preview、
実際の利用者・Dream状態遷移、数値freeze、ordinary lifecycle／性能、promotion/install/rollbackは未受入。
同manifestの遠景 `tak14-restarea-far-35a35f5a-20261005-v1` と最大ズームアウト
`tak14-restarea-overview-35a35f5a-20261005-v1` も現行clean headのIntel/Vulkan実windowで
独立technical verify pass、用途保持、finalize、storage check済み。独立画像評価では遠景で淡いテント屋根・
入口・支柱が隣の四角い設備と区別でき、追加paint補正は不要。一方、最大ズームアウトでは
種類と入口、支柱、利用状態を初見で確実に読むには小さすぎる。屋根を誇張する前に、
通常worldの実操作で既存選択・情報UIが建物種別とResting／Dream状態を示す代替経路を確認する。
黒いviewport余白の原因や通常起動での挙動、明暗・重なり・低品質・配置preview・実利用状態は
これらの静止画像からは未検証。正式art/numeric採否とreleaseは保留する。
RestAreaの通常world経路は既存分類で `Temporary`、配置域はSiteではなくYard。
通常生成worldのSiteから探した初回テストは配置成立せず、分類とproduction validationを照合して
Yardへ修正した。terrainやzoneを受入用に書き換えず、実際のYard内で既存判定が通る
2×2候補だけを受動観測するUI caseを追加中。プレイヤーの配置、予約、保存復元、取消を
実ウィンドウ入力で確認する工程であり、完成建物のResting/Dream、最大ズームアウトの
情報パネル、正式asset表示、性能やreleaseの代用にはしない。
初回バッチ `tak14-restarea-planning-2aa4edef-20261005-v1` は通常worldの配置予約、
保存・再読込を実操作で観測したが、再読込直後に可変top barの OpenEntities 位置が
pointer移動中に変わり、hover guardで終了した。exit 1／invalidとして封印・診断保持済み。
取消の実入力と独立verifyは未完了であり、部分観測を正式lifecycle受入へ昇格しない。
UI driverは移動だけを最大3回再取得し、実際にhoverを観測するまでpressしない補正を実施。
修正後のsource `3038dee7a347bf37d49f2d6a9ac26273c195890a` でfresh batch
`tak14-restarea-planning-3038dee7-20261005-v2` を起動し、実ウィンドウの5 checkpoint
（空き→配置→保存→読込→取消）を取得。Yardの2×2 Blueprint予約、保存後の同一対象復元、
取消後のowner解放を固定独立verifierがpassし、用途保持・finalize・storage check済み。
対象は完成前のBlueprintであり、Resting／Dream・数値／art採否・正式導入・性能を証明しない。
同一MudMixer `art_preview` manifest `4007d3fb4ebb93b9c6a4ce73aec3a9ac1e53736f8e19fc71211dd74ca45b094a` の
近景 `tak14-mud-mixer-near-86a0953a-20261005-v1` と遠景
`tak14-mud-mixer-far-86a0953a-20261005-v1` は、現行clean subjectのIntel/Vulkan所有windowで
独立technical verify pass、用途保持、finalize、storage check済み。独立read-only画像評価は
回転部の十字輪郭とIdle／Refiningの静止色差を認める一方、固定槽が緑の平箱に見え、
木・鉄・泥、槽の内側と架台の区別が近景でも不足すると指摘した。正式art判断は
`changes_requested`。数値geometryやゲーム状態を変更せず、槽内面・木の縁と外板・鉄架台・
回転部をalbedoの大きな色面で分け、fresh候補を同一条件の実画像で再比較する。
遠景の小さな内部線は承認根拠にせず、通常worldの実精製時だけ回転する状態、
明暗・背景・低品質・配置preview、numeric freeze、ordinary lifecycle／性能、正式releaseは未受入。
`cf265dd2a12315be818bb553da86ccf4de0e0afa` ではMudMixer既存body/rotorの部材別albedoを
変更し、数値geometryと状態ruleは不変。変更別contracts/tooling CI、Blender回帰204件、
Help No impact、独立read-onlyコードレビューはpass。fresh原画
`tak14-mud-mixer-authoring-cf265dd2-20261005-v1` とruntime package
`tak14-mud-mixer-runtime-cf265dd2-20261005-v1` は独立byte/UV/geometry verify、
用途保持、finalize済み。manifest `5fdf119b668270d6d898fce5613aa2e9e9e4b58f23b293060df69356253f0d28`
は依然 `art_preview`。同identityの通常倍率
`tak14-mud-mixer-normal-cf265dd2-20261005-v1` と近景
`tak14-mud-mixer-near-cf265dd2-20261005-v1` はIntel/Vulkan所有windowで独立technical verify、
用途保持、finalize、storage check済み。独立画像比較では旧「緑の平箱」は解消し、木枠・暗い内面・
十字攪拌部、近景の鉄支持部を区別できる。一方、暗い底面は泥か木床かを判読できない。
現行数値原本に泥の独立した面・状態がなく、`docs/building-art-direction.md` の容器とは別の
水・泥表面契約を満たしたとは言えない。泥の状態・面の採否を原本側で決めるまで
正式artは `changes_requested` とし、色面だけから泥の存在や実精製中の回転を推定しない。
最大zoom-out、明暗／背景／低品質、配置preview、実精製と保存復元、数値freeze、
ordinary lifecycle／性能、promotion/install/rollbackも未受入。
`281a6fc50111c79963348558854fd3780240f9ee` の2本の泥汚れ仮説は、
fresh原画 `tak14-mud-mixer-authoring-281a6fc5-20261005-v1`、runtime候補
`tak14-mud-mixer-runtime-281a6fc5-20261005-v1`、通常倍率actual-window
`tak14-mud-mixer-normal-281a6fc5-20261005-v1` で検証した。byte/UV/geometryと
実機technical verifierはpassしたが、前候補との独立画像比較では泥と木床の判読性が
実質改善しなかった。この描線仮説は不採用とし、同種の描線・明度調整を重ねない。
制作器は前の部材別albedoへ戻し、候補のtechnical passを正式art/数値承認へ昇格しない。
独立泥面が必要なら数値原本と状態契約の別途改訂・受入が必要。
次の比較用として旧v1原本を保全し、`building-mud-mixer-v2.geometry.json` と
`building-m2-v2.numeric-candidate.json` に固定の薄い泥残留面をbody内へ加えた。
槽底上面Y=12と攪拌部の全角度最下端Y=14の間に泥面Y=12.25〜12.75を置き、
床と0.25wu、攪拌部と1.25wu、内壁X/Zと各2wuの離隔を検査する。
body/rotorの2role、外形・接地点・preview anchor、回転・Refining条件、保存／gameplayは不変。
これは泥量や新しい充填状態ではなく固定の残留面という比較仮説であり、
Pythonの算術・拒否系検査だけでは数値freezeにならない。M2制作器に明示`--revision v2`を
加え、旧v1は既定のまま固定し、新候補の原本・GLB・UV・runtime previewを別identityで
生成できるようにした。独立実装レビューでaccessorだけでは泥面未描画を拒否できないと分かったため、
実際のindexごとにface所属・有向周縁・逆向き一対の内部対角線を照合し、
泥面のindex削除／誤接続／逆巻きを拒否してから
離隔値を出す。`fee69559e958fe36dd22c0cb7a88e307f1ed8cf0` のclean sourceから
原画 `tak14-mud-mixer-v2-authoring-fee69559-20261005-v1`、preview package
`tak14-mud-mixer-v2-runtime-fee69559-20261005-v1` を別identityとして制作し、
index付きGLB・UV・離隔・byteと独立verifierがpassした。bodyは132三角形、rotorは36三角形、
manifest `43b1fe0f75b7176870f9d72663a5c75119d0e81989ba49be0ab96a73697177f1` は
`art_preview`、`numeric_freeze=false`、`art_approved=false`、`production_candidate=false`。
同identityの通常倍率 `tak14-mud-mixer-v2-normal-fee69559-20261005-v1` と近景
`tak14-mud-mixer-v2-near-fee69559-20261005-v1` はIntel/Vulkan所有windowで
独立technical verifier pass、用途保持・finalize・storage check済み。v1との独立画像比較では
槽底と別の連続面は両倍率で前より読めるが、木枠と近い黄土色の平面で「泥」の素材判読は
まだ不確実。近景では紫の状態表示が右2標本に重なるため、泥色の観察は左2標本を重視する。
この静止技術比較を独立art/numeric承認に読み替えない。次の色面仮説は、木材とは異なる
低彩度の濁色と広い不均一な流れを、独立mud面だけに与えること。微細描線を増やすのでなく
通常倍率・近景の同条件画像で素材の分離を一度検証する。実精製中の動作、通常world状態、
full lifecycle／性能、正式releaseは未受入。
`bda34aadc43815f531e5b4a55e5da48bb98c1b31` では独立mud面だけを低彩度の濁色と
広い不均一なflowへ補正。数値geometry、rotor、旧v1、通常gameplayは不変。
fresh原画 `tak14-mud-mixer-v2-flow-bda34aad-20261005-v1` とruntime package
`tak14-mud-mixer-v2-flow-runtime-bda34aad-20261005-v1` を独立byte/UV/index verify後に保持・finalizeした。
manifest `faa969b7af23d93644d7e58cf86c000e35c3113d2c89ecd7f7105d0958fcb876` は
`art_preview` のままで、art/numeric/production authorityはない。
同identityの通常・近景・遠景 `tak14-mud-mixer-v2-flow-{normal,near,far}-bda34aad-20261005-v1` は
Intel/Vulkan所有1280×720 windowで独立technical verifier pass、用途保持・finalize済み。
独立画像レビューでは通常・近景の木製槽底との混同は解消し、遠景scale2でも木枠・泥面・
十字攪拌部と隣接種を区別できた。追加paint調整はここで止める。紫色表示は静止状態差だけで
実精製の証拠ではない。最大scale5、低品質、明暗・重なり、配置preview、実状態遷移、
正式art/numeric採否、ordinary lifecycle／性能、release/install/rollbackは未受入。
次の画像検証のため `building_art_acceptance.py plan --quality high|medium|low` を追加する。
指定argvだけでなく実window CSVの開始／終了品質、Scene解像度・scale、所有window寸法、
Vulkan/X11/adapterを既存window verifierで検査する。旧planはhighとして扱う。
静止性能参照はhigh限定を維持し、lower qualityは明示building-art sessionだけで許可する。
品質指定は検証子processだけの制御で、通常起動やプレイヤーの設定・Help契約を変更しない。
追加経路の実装レビューと現subjectの低品質画像取得までは、低品質受入を完了扱いにしない。
`e234fbe0aa2b80a98d1baacd59d0838b30a4459c` の低品質normalとhigh最大scale5を
同manifestで撮影。`tak14-mud-mixer-v2-flow-{low,overview}-20261005-v1` は独立technical
verifier pass、具体用途保持・finalize済み。low画像SHA
`81748907ad80948d7fe162c93fd832dc5bfea51bb29722a66a9559d242b5201b`、overview画像SHA
`25fffea7babf76914a3f4d425179e5bac81c297a7f4a5f72e78498ca01709e94`。
独立画像レビューではlow normalの木枠／泥／rotor／紫の静止差と隣接種の識別は維持され、
追加paint調整は不要。ただしoverview scale5は素材・種・状態を確実に読めず、画像可読性は未達。
細かなpaint追加で解決したとせず、既存選択・情報UIによる種類・状態の確認を実操作で検証する。
いずれも通常worldの精製・停止・再開、明暗・重なり、正式art/numeric採否やreleaseの証拠ではない。
次の状態観測に向け、`building_m2_acceptance.py plan --mixer-revision v2` を追加する。
Tankはv1、MudMixerは明示revisionと数値原本hashをkind単位で固定し、manifestと実provenanceの
一致をverifyする。旧field欠落planはv1のみと解釈し、hashからv2を推測しない。
top-levelの旧`numeric_candidate_sha256`は共通v1基点で、v2の根拠ではない。検証結果の
`numeric_candidates`を各kindの根拠とし、inspectも同じrevision拒否規則を使う。
unknown revision、Tank v2、v2 hash欠落・v1への取り違え、manifest／provenance不一致を拒否する。
これは未承認feedback候補の状態観測準備だけで、数値freezeや正式art/releaseを付与しない。
補正`6836e916f49dcfefc84d403208cc317395c61acd`のfresh refining batch
`tak14-mud-mixer-v2-refining-6836e916-20261005-v1` はRust producerのv1固定hashで拒否されinvalid。
ログの `M2 probe descriptor does not bind the exact numeric candidate` を診断保持しfinalizeした。
同batchの再起動・結果の読み替えはしない。producerへ明示`HW_M2_ACCEPTANCE_REVISION`を渡し、
Tank v1/MudMixer v1/v2だけを許可、選択hashとdescriptorを照合する補正を行う。
raw traceにも同数値hashを残し、v2 verifierはhash欠落・v1取り違えを拒否する。
通常起動や精製システムは不変。修正後のfresh subjectと独立実機verifyまで精製観測は未達。
出力root: primary `target/native-acceptance/nonbridge-restarea-authoring-20261004-v1`。
owner: main / consumer: M3候補の独立採否・現行subject導入。次工程: 原本・画像を比較しruntime候補を生成。
release_when: 採用原本のasset正本への集約と候補フィードバック終了後。旧batchのpassを新subjectへ流用しない。
検証: `dev.py check`、Clippy、workspace test、`dev.py ci check --base <基点full SHA> --mode auto`、
必要なactual-window/native性能受入、Help実レビュー、`git diff --check`、storage check。

## 8. ロールバック

kind単位の正式rollback記録で復旧し、loader失敗時の従来fallbackを維持する。

## 9. AI引継ぎ / Definition of Done

現在はM1。旧統括・Run・終了済み担当の再配車は禁止。採否は独立レビュー、製品編集・検証・Gitはmain所有。
参照: 原移行計画、building-art-direction.md、building-asset-sets.md、assets_workflow.md、validation-storage-workflow.md。

- [ ] 9種の通常起動表示と全受入を確認。
- [ ] 正式identity・採否・release/rollback・恒久文書を同期。
- [ ] Help/native/変更別検証/独立review/storageを完了し、追加差分なし。
- [ ] 不要jobを整理し、保持用途を引継いで計画をarchiveまたは撤去。

## 10. 更新履歴

2026-10-04: 利用者の「橋以外で最後まで」指示により先行9種の実作業を再開。storage pass、専用branch作成。
