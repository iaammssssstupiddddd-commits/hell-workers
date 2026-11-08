# 橋以外の建設物・正式ビジュアル導入

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `non-bridge-production-release-plan-2026-10-04` |
| ステータス | `In Progress` |
| 作成日 / 最終更新日 | `2026-10-04` / `2026-11-08` |
| 作成者 | Codex |
| 関連提案 | N/A |
| 関連Issue/PR | TAK-14 / 基盤統合PR #29 |

## 1. 目的

`77f90d22`のfresh `tak14-restarea-maxzoom-art-eis-feedback-20261108-v5`は7checkpointと登録/別実行verifierが成功。
source=`dba53819e43c01de77e3afbf632fe930242767ea9d35be23d5cfff778663b840`、
harness=`e2560847da2f51e10809c29d59ed276bbb4511d6dd4532bb747a5cc30103d94f`、
manifest=`037365f52dc1902922ee9d401d49cb952d0e10ad7feaa437caf82274ac1a5764`、
raw=`9367b78abd6ca0894f2b0ccc76a612ce808ed0af80c4544b43e4c1a8793f612b`。
実施工後40wheelでscale4.999997615814209、同owner133143983773/epoch0のempty/occupiedを撮影。
既存tooltipのResting 0/5→1/5、Dream 0.00→0.50/sとBuilding: RestAreaの選択headerをmain/独立reviewで実見した。
再クリックによる別候補選択後、`world-candidate:133143983773:1:0`を一度選んで同ownerへ復帰し、
全3Left pairのraw座標/時刻/ownerとframe順序を照合した。Intel Arc/Vulkan WARN/ERRORなし、505raw samples。
利用者が共有入力を使わないと明示した比較条件ではポインタ消失なし。旧2失敗の原因は物理操作と断定しない。
限定UI補完として用途保持・seal/finalize/storage pass（allocated304263049216B）。正式art/numeric/performance/releaseは未受入。
用途終了したmaxzoom v1〜v4のdiagnostic consumerを正式releaseし、当該4生成jobだけを撤去した。
削除前duは173944832/153239552/181583872/165187584B、計673955840B。invalid履歴と原因・補正結果は本書/台帳へ保持。
最新v5、候補制作原本、不変codec、共有Cargo cacheは維持する。次は同identityで未確認の明暗・壁とのdepth・
配置previewと完成形の投影/anchor比較を補完し、独立art判断後にM3 numeric決定をexact geometry/art hashへ拘束する。

次の限定比較はRestArea専用の明示static variant `rest-area-art-context` とする。既存galleryの既定36体・
M6・ordinary sessionへ混在させず、同descriptorの完成形とworld preview、前後の既存壁factoryを一度だけ配置する。
変更対象は `assets/building_asset_set/acceptance.rs`、`scripts/building_art_acceptance.py` と新profiling fixture module、
`building_presentation/preview.rs` の共通適用処理、専用observer/verifier・拒否テストに限定する。
previewは単なるPNG貼付ではなく共通descriptor→handle/canvas/Anchor処理を通し、実owner/root/parts、壁transform/depth、
preview role/hash/anchor/custom_size、投影、実light entity/値/影/camera/layerと安定後不変inventoryを照合する。
未知variant・別kind/identity・ordinary/M6混在・fallback・preview role/anchor/size不一致・前後反転・light不一致・
readiness後の再seed/位置補正/inventory変化は拒否する。primary既存build cacheを維持し、Help実レビュー・
変更別CI・Clippy・独立read-only review・fresh owned画像・storageを通す。候補assetやgameplayは変更しない。
重要な境界: 現structural shaderはLightFieldをsampleしていないため、field値だけで室内照明応答を主張しない。
当fixtureのAmbient/Directional比較は `seeded-paused-art-comparison`、accepted/performance/promotion=falseであり、
通常入力ghost・通常照明環境全体・lifecycle・正式art/numeric/release採否の完了とはしない。

専用variantの初期実装は4完成owner/8壁/4preview/6実Soulを別inventoryとして構成し、
config/admission・seed・plugin配線とsidecar契約を明示分岐した。既定36体/M6のlayout・検査は維持する。
独立中間reviewで判明した壁footprint衝突は実tilesのmin/max外側から導出して修正し、
同handle内のPBR材質変更、壁可視性/3D transform/layer、実RtT target/composite、
完成接地点とpreview anchorの既知offsetを観測・拒否へ追加した。
初期実装時点では準備中であり、fresh native画像・最終固定read-only review・正式採否は未実施だった。
`a76c44f3`の最初のplanは、実manifestのartifact roleが`image:world_preview`であるのに
新helperが非prefix名を検索していたため登録前に拒否された。job root・launchは未作成のまま保全した。
実role namespaceへ修正し、欠損/非prefix/重複artifactの拒否テストを追加する。旧計画拒否はnative passではない。
`8386042b`のfresh standard v1は実X11/Intel Arc/Vulkanへ起動したが、readiness後のsnapshot変化を
observerが検出して非ゼロ終了し、probe/PNGは未生成。invalid封印・診断用途保持・finalize済み。
現段階では変更fieldが未記録のため原因を断定せず、最初のJSON差分path/前後値を有界に報告する
readonly診断を追加する。不変性・identity・材質・投影・inventoryの受入条件は緩和しない。

`3356cdee`（source `4a45c3676428ceed73925d9648cb76318fe0064800b2cc7b7a38835179c479b6`）の
fresh standard v2とdim v1は、同じdb0f ArtPreview・同一binary
`02ea33c1c87407d9cc0512dd4bf7027b28f3d7d58cf4cd76af64b8b739de72e9`で技術比較pass。
実光源500/12000対100/2400、4完成owner・8壁・4preview、0/1/5/0 seeded occupantsと
exact材質・壁depth・画像role/hash/Anchor・実RtT target/compositeを照合した。
ownerの投影[400/560/720/880,280]と縦補正済み完成接地点が一致し、previewは同x・y440へ並置。
standard manifest `693aec1b6e4d4eecc2b67ba7dcbb630d1aa956a713d047562eb76e47fb5f54f7`、
PNG `faec96caeb16998d292306a0c6e9aeee6a5b0f1a8649006551a9829854de1d68`。
dim manifest `c9b85ae99d37a45b6b23373b129b126d09cdef67ec770e52cd4d10f1511db436`、
PNG `cb8dbcfff5bb16f05c9af3a939e9817ff2793e9d5f23ddfc16050e7122f0fa13`。
両方とも別プロセスverify、登録verifier seal/finalize済み。main/独立read-only reviewで両PNGを実見し、
暗所でも布屋根・支柱・入口が残ること、固定preview自体は明るさ不変であることを確認した。
複雑な壁交差・全方向depth、人数の形だけの識別、通常入力ghost・LightField・通常照明全体は未証明。
正式art/numeric/release/performance判断は付与せず、利用者へ当該見た目の採否だけを別途照会中。
先行standard v1の変化は診断追加後の同条件v2/照明だけを変えたdim v1では再現せず、原因未断定。
受入を緩和せず、初回invalidのまま一回の有界診断を終了し、今後の変化はfirst-difference診断で拒否する。
最新2画像は具体的採否用途で保持し、旧source/binaryの再実行依存はseal/finalizeで終了した。
初回standard v1の診断consumerは終了し、finalized/invalid・終了process・所有・非symlinkを確認して
当該生成jobだけ140005376Bを撤去した。原本・最新2画像・共有Cargo cacheとinvalid metadataは維持する。
利用者のRestArea見た目採否待ちと並行して、既存SoulSpa ArtPreview generation1
`63e860d2777b1ed7d562835f64452c5fd3d9a00ef5ac1435015e0beafd346e48`の現行source galleryを準備する。
world/catalogは96wu/256px、world Anchor[128,144]・代表OperationalMaskZero、geometry hash
`738afb151a0d55044774a8f22af9b2649784678b61c4ab25e0577869acb1d2f2`、art/numeric/production_stateは未承認のまま。

`df2f2c20`の同source `4a45c367…479b6`、同binary `02ea33c1…de72e9`でSoulSpaのfresh near/normal/farを実行した。
3件ともrun・別実行・登録seal verifierを通過し、用途保持・finalize済み。mainと独立readonlyが全3画像を実見した。
PNGはnear `1dff025ff111abb2c087d8e6cc51e8e18a02776ad4381885b566a9604bbb9f1b`、
normal `fd7aa39caf2b2dca99fd9d2e4788c6a72d625adff7d5ae8cb68d1e4b4d538b4b`、
far `8db517ce77342c0060416323687084727f5467a073be0213403e445acb5cdca2`。
同ArtPreview・36体の静止fixtureでmask0/1/3/15と0/1/2/4人、4区画は読めるが、骨素材の筋・節は弱く
黄土色の直線格子へ寄っている。active slotはSoulに覆われ、発光単独の判読も未達。技術passで美術採否を閉じない。
atlas/UVの実経路を照合すると、上向きbone:shaft 42面中34面で既存の固定cell描線が面内へ届いていない。
長辺共通正規化で細いUV帯になるのに、strokeを固定cell座標へ描きpolygon maskで切り捨てるためである。
次は `building_m3_art.py` と対応testだけでshaft描線を実UV quadの長軸へ沿わせる。
色・geometry・logical footprint・slot順・emissive・runtimeは変更せず、旧描線で失敗する面内画素回帰を先に追加する。
Help実レビュー・変更別CI・独立readonly後、fresh候補を既存制作経路で作り新identityとして再比較する。
旧原本・3距離画像は修正前後比較の用途で保持し、正式art/numeric/release/performanceは未承認のまま。
描線補正`9bbe8abf`はM3関連12試験・変更別contracts/toolingと独立readonlyを通過した。
fresh authoring v1はGLB exporter/validatorが両roleともerrors=0/warnings=0だったが、M3がM2専用の
3/4角面検査を呼び骨の8角capを拒否してexit1。invalid封印・診断保持・finalizeし、旧試行を再送しない。
初期診断の「slotの12角cap」も拒否対象という推測は不正確で、実slotは24三角/12四角であり追加許可は不要だった。
M2入口の3/4角限定を維持した別M3入口で3/4/8角だけを扱い、indexed vertex/UV集合・face内triangle・
枚数・全perimeterの向き・内部edge対消去を従来同様に検査する。checker自身のhashもM3制作reportへ加える。
正常M3 capとM2拒否、逆向き・重複・退化・face跨ぎ・範囲外index・UV変更の回帰を通してからfresh v2へ進む。

`a7356b49`で関連23試験・contracts/tooling・独立readonlyを通過した。authoring v2の制作と内部verifyは成功し、
body/slot GLBは旧候補と同じ`3046407c…f0e6d` / `0f523ea0…39415`、emissiveも旧同一だった。
ただし主担当の登録verifierがoutput rootをenv token内だけへ含め、sealが`verifier does not address this batch's output`
で送信前拒否されたため当batchはinvalidとして保持・finalizeした。台帳やverifierを修正してpassへ変えていない。
独立の正規prepare工程が原本を再verifyしてgeneration2 ArtPreviewを生成し、runtimeの登録verifier seal/finalizeはpass。
新identityは`fe49fae422a101bb796e2e6397b370599cbce33425637d9cae6127a75d407aff`、geometry原本は
`729c7f275619c63db63b1255ab505a09a40b0b5480f4585a9081a035a50f98fe`。
同source/binaryのfresh nearは技術pass、manifest `4e1f86d292d493391a3e8222db9ed93413d8d1eab0b18607240923e3ef524d06`、
PNG `c12a97b94367cc3bd0699796fa246e6358447edc5582ae46d370a81bbead05bd`。別verify・seal/finalize済み。
main/独立画像reviewでは骨としての判読改善は乏しく、描線追加で解決する仮説は一回で打ち切った。
同形状のclay比較も生成・厳密verifyしたが、人間の採否と別の診断でありnative受入とは扱わない。
Blenderのuse_nodes非推奨警告、clay子processのsocket Broken pipe警告を観測した。GLB errors/warningsは0、終了code0。
利用者は新near画像に対して「この見た目を採用する」と明示した。2026-11-08、同世代2の見た目だけを採用し、
追加の骨形状修正は行わない。`tak14-soulspa-uv-runtime-20261108-v1/art-approval.json`へexact identityを拘束し、
保存file SHAは`97ad92b97572fd29abf1c2bd3821e3f07722f1491ff64ad098a0bc2b4ea0f128`、
export/numericへ拘束するcanonical JSON SHAは`cb6fea40f51f488d6759e2a61739e2ef8533f00e6a01650f7357b1a47e14b97f`。
独立readonly数値reviewは、32wu/tileとslot順・中心offset、半径11wu/隣接間隔10wu、高さ0.6/半厚0.3、
body±31.5wu/高さ6.5wu、body420/slot48tri、原本式に対するGLB最大誤差約8.4e-7wuとUV約2.6e-8、
512px atlas/32px cell/4px gutter、preview/Anchor、実tile→slot producerと発光倍率[1,1,1]を独立に照合し採用可能と判断した。
numeric projection canonical SHA=`51ed725c6cdcab66ba548fcf2e23af76f637e522d2859161fd16997fda569a38`、
同art/geometryへ拘束したnumeric decision canonical SHA=`8c3d74bd2ec11e00e7617becb3dc4a0fd4bc63bef2ec6a5b29f6437718bf9cce`。
最初の隔離export v1は主担当が非canonical key順のreceiptを渡し、出力前に拒否されたためinvalid/finalize済み。
原回答fileを保持したまま内容不変のcanonical別fileを作り、正規readと束縛を確認したfresh v2だけを実行した。
`tak14-soulspa-isolated-candidate-20261108-v2`の独立verify・seal/finalizeはpass、隔離候補identityは
`a1b92f31c34a62c034d041738f495669aa3b2d9ff2e41007966afc30da3a9bf8`（generation2）。
両decisionをprovenanceへ内包し、通常assets/locatorは未変更。all-mask/通常lifecycle/性能/release/既定起動は未承認。
次はこの隔離候補の全16maskを、既定galleryの4標本と混同しない明示入口で現sourceへ拘束して照合する。
実装はgallery sessionの任意`spa_mask_page=0..3`をSoulSpa isolated_candidate/通常照明/smallだけへ限定する。
pageごとに4標本をmask[4p,4p+1,4p+2,4p+3]へ一度seedし、actor数は他種8人+実mask popcountから導出する。
未指定は既存[0,1,3,15]/15人・36体gallery、M6・RestArea context・他kind・通常起動を維持する。
別scopeでpage/identity/source/nonce、実tileからのmaskとslot名/mesh/transform/材質・発光画像・倍率、
body/inactive非発光を観測し、全4pageの別process画像と不変snapshotを照合する。これはseeded静止状態の検査であり、
通常利用の状態遷移や性能受入は代替しない。未知page/混在/不足actor/誤slotや発光を拒否する。
Help・変更別CI・Rust check/Clippy・独立readonly後、現候補のfresh4pageを逐次実行する。

`c2459398666b1247b0623e1043cbb3fef8550252`でこの明示検査を実装した。独立reviewの指摘を受け、
pageだけは初回-only記録にせず毎frame再観測し、初期snapshotと異なれば上書きや修復をせず失敗する。
owner/root/part Entity・実ChildOf・消費世代・root/part pose・同handle内の材質値変化も対象に含める。
既定gallery/M6の初回-only挙動は維持。Python関連21件、Rust spa関連42件・静止fixture17件、
`dev.py check`、通常/profiling Clippy警告0、変更別contracts/tooling/rust（Python605件・制作224件・
workspace通常/profiling tests・Memory/Tracy/RenderDoc compile）が成功した。
変更別検証baseは`248dfc1c980fe212b9de73e6c38eb1a3127904c2`、dirty source fingerprintは
`2040b8dc108e4462dfac3767a5f542bc89a9eebe630739dd22a6f95a22cbd254`。
Helpはexplicit profiling-only sessionから通常入力・UI・保存・利用条件へ到達しないためNo impact。
RAはerror/warning0だがprofiling fileにunlinked hintがあり、対象featureの実compile/testも併用した。
最終tracked diff `4d68c1cd63608eecfd36b4c86804e27f1f9c123ee87398ae0b8aefca0439050d`と
新Spa observer `639a062414c8fdd322c8e6fddf1c59f86f58595272b94309f71bef1ed6bd0508`は独立readonly承認済み。

fresh `tak14-soulspa-mask-page{0,1,2,3}-20261108-v1`は同commit・source
`1dc5cbfdbd26a4758976d37ba7cbe59382d1e9a0d0e8174717df9bd50e2101e0`、driver
`322eac169ba09aba10469ce8ddc4ae50329993efa8f2a18a0119078a78d550e9`、binary
`201d0a6a8b0d82a6aa3a87c414fddd9c313d465c346904e1e951630fbb89334d`、隔離候補a1b92f31世代2へ拘束した。
Intel Arc/Vulkan/X11/High/DPI1・1280x720、30秒warmup/60秒のpaused静止観測を逐次実施し、
30 stable frames後の4画像を主担当が実際に確認した。各pageのactor数は12/16/16/20、実tile maskは0〜15を重複なく網羅。
body/slot0〜3のexact mesh/pose・親子所有・材質/発光・初終不変とnative WARN/ERRORなしを検査し、
recipe内verify、別CLI verify、登録済み独立verify・seal/finalizeはいずれもpass。

| page / mask | manifest SHA256 | owned-client PNG SHA256 |
| --- | --- | --- |
| 0 / 0〜3 | `04761d81596105961abea33e43596a51e8fd786e856d61436eb4addfe60c619d` | `ebb5c94b6b61c82387ed36991370025e34266ccfcabdec391659000cdc54858b` |
| 1 / 4〜7 | `b8b53f9a16c6a13fe689977529316d313cdb34d4888486e44cbad829161400b0` | `e1774e5eeb97709c5fc81cfea70dd549889b05d61b607204bc01532d3c76461c` |
| 2 / 8〜11 | `c30b64bd5c7bf5d0ad7e272340eb8030086a890e264239cde5ce66f8219bba85` | `c4d5290ba58004535e9e59cd5f7ebec98eb1d11ccedf04bb6cdce3bc4a1446f5` |
| 3 / 12〜15 | `c36e0fd01b331a5043fd70abe232266b640449402d42de3af0a7714b3a24bb97` | `30f9d9052644cb8967f95f66c10a3c3e81e651f63260af70de2a5e18f51e4fa8` |

これは全maskのseeded静止presentationに限る。通常の自然入退所・実施工・Save/Load・cleanupや
Memory/GPU/累積性能budget・release/rollback・既定起動を完了扱いにしない。
独立readonly担当も全4PNGを実見し、mask/actor・5part・同candidate・probe/manifest・初終snapshot・
所有window/adapter・hash一致を照合して、この静止技術確認だけをapprovedとした。
Soulが発光面を部分遮蔽する点も確認済みで、通常稼働遷移や性能/releaseの承認には読み替えない。
4pageの画像/snapshot比較は終了した。全process終了・非symlink・候補assetの別原本とのbyte一致を照合し、
4holdの比較consumerを正式releaseした。削除前du合計は568254464B。初回削除では複製Wall/Door世代の
readonly directoryにより一部が残ったため、所有・非symlinkを再確認し、生成物側directoryだけへ
owner writeを戻して撤去を完了した。当該4rootは全て不存在（残存0B）、採用元の権限は変更していない。
削除した生成物自体のbackupはなく、結果と台帳metadataを保持する。採用原本・隔離候補a1b92f31・
利用者が採用したnear画像・通常Cargo cacheは別の現在用途で保全し、変更していない。

用途終了したauthoring v1（1294336B）とclay比較（1130496B）は固有の採用原本を含まず、
既存strokes原本・runtime世代2・採用画像へ必要情報を保持したうえで生成物のみ撤去した（計2424832B）。
両process終了・所有・非symlinkを照合し、v1診断consumerを正式release、clayはseal後撤去・finalizeした。
削除物自体のbackupはなく、採用原本と失敗metadataは保持する。

`c07987e4`のfresh `tak14-restarea-ordinary-art-eis-feedback-20261108-v3`は9checkpoint、
登録verifierと別実行verifyが成功した。source=`43386c30797aac97f88d338f16c76304cb14ff143d9f9111e4dc0eba01361947`、
manifest=`ea7a00561cb8da52693a7a5afeec6b8c5723917534f51668d75b6b047a55d9da`、
raw=`87a03b86750e0d89402495023d6e837e65d3747fc1580333bb83ebad27fea964`。
exact db0f ArtPreviewで通常施工・実入所/Dream・保存復元(1845v0→831v1)・自然退出・
cooldown15秒(仮想85.614576461→100.614576461)を観測。実active/root/part/材質の同一性と
全9画像をmain/独立read-onlyで確認し、Intel Arc/Vulkan WARN/ERRORなし。
保持・seal/finalize/storage pass、allocated=296582414336B。正式art/numeric/release採否は未完。
用途終了した失敗job `tak14-restarea-ordinary-art-feedback-20261107-v1`、
`tak14-restarea-ordinary-art-eis-feedback-20261107-v1`、`tak14-restarea-ordinary-art-eis-feedback-20261108-v2`
は各診断consumerを正式release後、`target/native-acceptance/`直下の当該生成物だけを撤去した。
削除前duは順に149721088/192282624/189358080B（合計531361792B）。invalid metadataと原因・補正結果は
維持し、候補制作原本・最新成功job・共有Cargo cacheは変更しない。

`5e61256f`のfresh `tak14-restarea-maxzoom-art-eis-feedback-20261108-v1`は実施工後40wheelで
scale=4.999997615814209へ到達し、empty hover/selectedとoccupied hoverの6checkpointを取得した。
同owner38654703279の利用中tooltipはResting 2/5・Dream 1.00/sを表示したが、同地点の再クリックは
既存の重なり候補巡回によりSoul Stephanie (4294965449)を選び、occupied-selectedは15秒timeoutでinvalid。
旧送信は再送せず、invalid封印・診断用途保持・finalize済み。正式art/numeric/release採否には使わない。
候補一覧の既存target/revision/epoch付き行をprofiling observerで受動公開し、別対象が選ばれた場合だけ
一覧からexact ownerを一度選ぶ。各UI hover/press、raw Left pairと座標、最後のworld hover復帰を検証する。
未知targetや旧revision、余分なクリックは拒否し、player選択ルールと候補順位は変更しない。
補正`a3ef5226`のfresh `tak14-restarea-maxzoom-art-eis-feedback-20261108-v2`は通常配置後、
step18のmotionを(161,32)でACKした直後、次のpress前にX11 pointerが所有client外となり未送信拒否。
最終observerもcursor=null、focus=true。物理入力、compositor移動、adapter問題のどれかは断定しない。
invalid封印・診断保持・finalizeし、press拒否へ実X11座標/寸法/ownerを追加する。入力guardやcleanupは不変。
診断付き`215f2590`のfresh maxzoom v3ではポインタ消失は再発せず、実施工・40wheel・empty/occupied
hover/selected全7枚と候補一覧からのexact owner選択(step83)を取得した。ただし最終検証の新しいglobal
interval ruleが前段constructionのEscape steps13/14（共にframe261..267のjoint ACK）を誤拒否しinvalid。
これが唯一の重複であり、完了step23後のmaxzoom操作には重複なし。旧結果はinvalid封印・診断保持・finalizeし、
新規interval ruleを完了後の操作だけへ限定する。既存construction検査、全Left/wheel/owner条件は維持する。

最大zoom-out=5でのUI補完を明示legとして実装した。数値camera scale、実wheel、hover/pointer/tooltip
対象とvisible本文を同一sampleへ受動記録し、既存consentの宣言SCROLL capabilityを持つEISだけを使う。
通常入力でzoom→exact owner hover→利用人数/Dream表示→同owner選択をempty/occupiedで確認する。
tooltipを情報panel本文と混同せず、予約数や未表示値は証明しない。scale/stateを直接書き換えず、
wheel/ACK回数上限、window/epoch/owner、UI遮蔽、候補世代、誤tooltip/選択をfail-closedで検査する。
既存recipeはSCROLLを要求しないまま維持する。関連テスト、Help、same-source検証、独立review後にfresh実機へ進む。

`b6e2cfc5`のfresh `tak14-restarea-ordinary-art-eis-feedback-20261108-v2`は通常建設まで通過し、
入所待機120秒でinvalid。予約5人はGoingToRestのまま通常移動していた。rawの最初の予約から
実43.920→166.658秒、仮想34.630→88.401秒、最寄りの予約Soulは(1232,-1552)→(720,-466)、
建物中心(672,32)まで約500wuを残した。停止・入力失敗ではなく有界待機不足と判断する。
入所待機だけ240秒とし、既存全体480秒、実occupant/Dream>=2、raw同一性等の条件を維持する。
待機中の速度変更・actor/進捗/状態の注入はしない。旧結果は診断保持・seal/finalizeし成功へ変更しない。

`ec2c1c2b`のfresh `tak14-restarea-ordinary-art-eis-feedback-20261107-v1`では、宣言済み
logical region=(0,0,1440,900)/scale=2、physical root=(2880,1800)で初回入力を通過した。
通常施工・利用・Dream・Save:Succeeded・Load:Succeededと7checkpointを記録した後、
live exercise_saveが既定の正式raw scopeをcheck_saveへ渡してfeedbackを拒否しinvalid。
ゲーム失敗や候補採否とは読み替えず診断保持・seal/finalize済み。退出/cooldownは未達のまま。
明示ArtPreview入口からlive lifecycle→save checkerへだけscopeを受け渡し、通常入口の既定を維持する。
rawの自己申告からscopeを選ばない。回帰試験はliveのkeyword伝播と旧正式既定の両方を検査する。

新入口のfresh `tak14-restarea-ordinary-art-feedback-20261107-v1`は、exact db0f ArtPreviewを
active descriptorへ読み込んだが、最初の速度ボタンへのpointer motion1件がgame cursorのACKを
得られず15秒でinvalid。クリック/キー/建物配置は未送信。子とportal終了後、診断用途で保持・seal/finalizeした。
全39raw sampleでcursor=null、送信時X11 root=(2878,67)/client=(2398,-363)はgame窓外。
同mapping=(1,2.0)の前成功は初期client=(1102,720)から目標へACKした。scaleを変える根拠はない。
Xwayland座標とcompositorの現在座標の食違いは仮説であり、原因確定や失敗のpass化はしない。
次は開始座標依存を切り離すため、既存のconsented EIS absolute経路を当該ArtPreview lifecycleだけで
明示選択する。既存Notify経路を変更/自動fallbackせず、同じportal permission、declared region/scale、
exact owner/focus/nonce/単発step/game ACKを維持する。transport/hashと実regionを結果へ拘束し、
未知region/混用/他case/false authorityは起動前に拒否する。関連拒否試験・Help・storage・独立review後に
fresh batchで一度比較し、変化がなければこの仮説は打ち切る。

2026-11-07、HEAD `254fba4e` / source `2faf959677b5f9a78e5c25f34e0f68f523d6d4d10f5fd951206754562e068762`
のexact RestArea ArtPreview `db0f5356…2522`をnormal/far/near/overview highとnormal lowの
fresh 5試行で確認した。全て登録・別実行verifierをpassし、owned画像をmainが確認、保持・seal/finalize済み。
独立画像確認ではnormal/far/nearの布屋根・支柱・入口に追加形状修正要求なし。
最大zoom-outでは存在だけが読め、種別/利用状態は断定できないため、実selection/info UIによる補完を未達条件に残す。
静止fixtureの技術passであり、art/numeric/release・通常利用や性能の採否ではない。

次の実装は、採否前ordinary表示の循環を解く明示profiling-only RestArea ArtPreview feedback入口。
`BuildingArtSession`やM2 seeded fixtureを流用せず、exact identityのload policy/pool requestと受動観測だけを
通常生成worldへ接続する。専用session/UI case/observerを一組にし、正式production env、release、M6、
static/perf/M2との混在、非profiling、未知field、別kind/authority/nonce/pathを起動前拒否する。
正式production observerのReleaseApproved allowlistとscopeは維持し、feedbackは別scopeで
accepted/performance/promotion=false固定。要求identityと実active/descriptor/root/part/材質画像を別々に記録する。
UI runnerは原本/manifest/依存bytes、codec/source/harness/nonceを固定した隔離asset viewを使い、既存の
通常施工・利用・Dream・保存復元・退出/cooldown入力を再利用する。材料/進捗/actor/完成を注入しない。
fallback・旧世代・読込前ownerだけでは候補表示成功にせず、変更別検証・Help・storage・独立read-only review後に
fresh実機へ進む。通常起動/ゲーム規則/正式承認条件は不変。全9種の正式採否と導入は未完。
入口/runner/専用scope/実consumer照合を実装し、最初の変更別CIはcontracts/tooling/deps/rust、
通常/profiling workspace試験、Memory/Tracy/RenderDoc構成、通常/profiling Clippy警告0とdev.py checkを通過。
独立reviewでasset-view根の同bytes symlink漏れを指摘され、canonicalなjob配下/祖先/root実体を
copy前/verify時に検査する補正とview/assets/viewport祖先の3拒否回帰を追加した。
関連101試験はpass。最終同subject検証・独立再review・fresh実機前であり、正式採否は保留する。

M3導入経路の追加照合で、既存Pythonのart/release decision比較がnested generationの
true/1.0を正規int1と同一視する欠陥を独立read-only reviewで確認した。M3 numeric側のidentityを
正規intのまま不正artのhashへ結び直しても通るため、採否・候補化・起動前に補正する。
既存art decisionのexport/load/verify/planとrelease decisionをcanonical JSON比較へ統一し、
M2 numericのidentityとbool数値代用も拒否する。新しい承認・asset・ゲーム規則は生成しない。
Rustのprofiling実行ファイル準備と並行する編集はPython/文書だけで、Rust入力は変更しない。
旧コードは追加拒否回帰で7failureを再現。補正後はfocused21件、Python全573+222件と
contracts/toolingの変更別検証を通過した。plan/evidence/approvalと全hashを同時差替えする拒否も含む。
実codec由来candidateへ再拘束した実装は独立read-only reviewで限定承認、HelpはNo impact。
実機plan登録前に最終文書差分とstorageを確認する。profiling executableの準備buildも成功した。

次の実装はM3の独立数値採否を、exact art-preview identity・art decision・geometry原本の
hashと明示数値projectionへ結ぶ経路。RestArea/SoulSpaのcandidate/releaseにもnumeric digestを
要求し、M2専用production_stateは流用しない。未知field・非有限値・bool代用・identity/原本/
数値の不一致は拒否する。proposalのnumeric_freeze=falseは変更せず、実承認は別入力のままとし、
schema実装や合成fixtureを正式採否・releaseへ読み替えない。
この経路は独立read-only code/docs reviewで限定承認。Python 573+214試験、通常/profiling workspace
試験、Memory/Tracy/RenderDoc構成、Clippy警告0、dev.py check、Help No impactを確認した。
数値採否・candidate化・通常起動の視覚世代切替はまだ行っていない。

読込後の残工程は`rest-area-lifecycle`として同じ通常操作へ接続する。利用中save/load 7枚から、
実mapで復元した同じSoulの自然Dream枯渇/退出とcooldown満了まで計9checkpointを取得する。
早期のcomponent消失・入所/予約・timer再設定・世代/epoch置換・時刻逆行は拒否し、
満了後の通常再入所は区別する。状態を注入せず、fresh実機と独立レビュー前は未受入とする。
fresh `tak14-rest-area-postload-lifecycle-feedback-20261107-v1`は全9画像/登録・別実行verifier/
独立read-only確認をpass、seal/finalize済み。実Manual1 496,786 bytes、SHA
`da7457dafd6c5f303106d46f094484f5e0c5c98d221c8e26bb9b7ed0be69c017`とSave416→Load473が一致。
21mapでSoul1856v0→855v1/owner1908v19→1833v1を固定し、Hidden/Dream保持後の自然17放出/
final/退出を確認。virtual91.815720774のcooldown15→106.815720774の初回消失まで正確に15秒。
満了前の再入所/予約なし、Intel Arc/Vulkan、WARN/ERRORなし。灰色fallbackであり正式art/releaseではない。
旧退出/occupied-save比較root計89,088,000 bytesはconsumer解除・process/原本不在確認後に撤去し、
最新64,573,440 bytesだけを正式視覚世代のlifecycle比較へ保持する。コード/原本/台帳履歴は保全。

利用中保存/読込の独立した`rest-area-save` feedback recipeを実装した。
最初の`tak14-rest-area-occupied-save-feedback-20261107-v1`は実保存/読込と7checkpointに到達したが、
receipt→撮影の追加15秒条件（実15.034/17.522秒）でinvalid精算した。各区間18raw sampleの
pause/epoch/仮想時刻/状態は独立確認で不変。時間上限だけを広げる代わりに全raw区間の不変性を
検証し、既存session480秒/240 sample以内へ拘束する。また非保存Soul Visibilityは既存shell再構築で
非入所のVisibleがInheritedへ戻るため、その正確なprojectionをRust試験で固定する。
入所SoulのHidden・vitals/位置/双方向関係は緩和せず、旧invalidをpassへ読み替えない。
通常完成/入所/正のDream放出後にpauseを維持し、F5/F9の既存Manual1操作で保存/読込する。
実bytes/hashと実Entity remapの全Soul/RestArea集合・vitals/位置/関係を照合し、7画像を独立確認する。
fresh v2は登録/別実行verifierと全7画像の独立read-only確認をpassし、seal/finalize済み。
実Manual1 496,766 bytes、SHA `6912a29eac134f34f68f0f0ef3ec7dfe7b018bf4d9a6438074de9fc0d4821f09`、
Save310/epoch0→Load367/epoch1の全21map（20Soul+1RestArea）・4予約・利用中Hidden/Dream保持を確認。
撮影までの全raw区間はpaused/同virtual time/状態不変。Intel Arc/Vulkan・WARN/ERRORなし。
灰色fallbackの限定技術証拠であり、正式art/numeric/performance/releaseは未承認。
既存の自然退出/active cooldownの限定結果を保持し、次は読込後の退出/cooldown満了へ進める。
全inventory・正式9種release/default起動の不足は消さない。

保存/読込の次の境界は、実commit bytesと実Entity remapへ結ぶprofiling限定producer。
`tak14-durable-receipts-feedback-20261107-v1`は通常Manual slot 1の499,109 bytesと
Save/LoadのSHA一致、epoch0→1、全20 Soul map/収集状態/Pool一致を独立読取で確認した。
6画像・登録済みrefund verifierはpass封印/finalize済み。新receipt項目は自動verifierの受入範囲ではなく、
保存時BlueprintでRestArea ownerは0件。次は完成RestAreaの通常利用中save/loadを同じ実mapで
自動照合する。全world canonical、cooldown満了、正式art/numeric/release/default起動は未完。

2026-11-07材料返却工程: 既存のゼロ材料caseを変えず、通常RestAreaの実搬入Woodを受動観測してから
同じF5/F9・二段階取消を行う別feedback caseを追加する。資材付与・進捗設定・AI割当は行わない。
実前状態の材料数と、新規の返却Entity IDs/種類/数量を厳密照合する。材料なし・施工完了・
同subject不一致・返却不足は拒否し、全予約/inventory清算や正式release承認へは昇格しない。
`material-refund-feedback-20261107-v1`は実Wood1の保存読込後、時間再開中のUI遷移で
通常施工が完了して対象を失いinvalid。停止中に対象詳細へ移動する手順へ修正した。
v2は実取消、owner2219v1／epoch1のcleanup、Wood2と返却Entity2411v18・880v6を観測したが、
読込Wood1の後に通常搬入が1個増え、旧verifierのloaded量との一致要求でinvalid。
save/load間の量・progress一致は維持し、取消時はsame-owner/epochの実raw履歴が
loaded量から単調増加し、最後の実材料数と返却ID数・種類が厳密一致する契約へ訂正した。
v3は配置後のSpace入力が時間再開を成立させずtimeoutでinvalid。新caseのみ既存HUDの
Normal（1x）／Pausedを一度操作して実状態を待ち、未知入力は再送しない。
v4は実搬入・保存読込・停止中の対象選択に成功したが、Normalを押して時間再開後、
release時にBevy cursorがnullとなりgesture guardが拒否。取消は送っていない。
4試行ともinvalidのままseal/finalizeし、それぞれの修正・原因調査用途だけを診断保持した。
限定コードは独立read-only review、55 focused tests、tooling gateを通過。profiling-only Rustの
材料snapshotはworkspace通常/profiling試験・両Clippy警告0で確認済み。非ゼロ返却native受入は未達。
次は入力ガードを緩めずfocus/cursorを切り分け、独立して実搬入branchのtransaction producerを追加する。

実搬入producerの実装方針: 手運びとwheelbarrowの既存Blueprint搬入成功branchでのみ、
actor/assignment/item/owner/epoch、実増分の前後量とfootprintをprofiling限定で記録する。
明示観測resourceがない通常起動は収集しない。item消費のdeferred適用とowner/material整合を
確認してからbounded receiptへ加え、未消費・foreign epoch・cap超過は観測失敗として閉じる。
状態差から搬入commitを推測せず、全予約清算・BuildWork・art/releaseを付与しない。
既存搬入・疲労・chain・予約解除・施工速度を変えず、拒否系単体試験、変更別検証、
Help No impact判断、storageとexact-source独立reviewを通してから新しい実機証拠へ結ぶ。

配送producerの手運び（通常終了・Build chain）／猫車実handler試験、bounded receipt拒否系、
Pythonのidentity/count/epoch/item照合を追加し、Soul AI 115件、通常/profiling workspace試験、
Memory/Tracy/RenderDoc構成、両構成Clippy警告0、tooling、Help No impactを確認した。
独立read-only reviewはコードと関連仕様を限定承認。rust-analyzerの既存Query呼出しの一時エラーは、
コード変更なしのfresh診断で0件へ戻った。変更別CIは既存依存advisory 2件で引き続き未通過。
fresh `tak14-rest-area-delivery-refund-feedback-20261107-v5`は実Wood配送4件を記録し、
保存・ロードWood1一致後、新epoch1のowner810v1へWood4まで通常配送、返却4IDとowner/request
cleanup・4tile walkability復元まで到達した。しかし旧Soul1845v0を触るdeferred commandのWARNで
厳格なgame-log検査が拒否し、invalidとしてseal/finalizeした（診断保持43,352,064 bytes）。
入力ガードは変更せず、この試行ではfocus/cursorを維持。警告を無視した受入や旧invalidのpass化はしない。
次はロード前Entityを保持する遅延会話cooldown等のproducerを実再現で特定し、修正・fresh実機を行う。
全予約/inventory・通常起動の正式asset・art/numeric/performance/release受入は未完。

次の限定修正は`hw_visual::speech::conversation::end_conversation`のcooldown遅延処理。
Bevy 0.19の独立DelayedCommandQueueはworld replacement後も期限まで残るため、
期限到達時にexact Entity generationの生存を読み直してからmarkerを除去するclosureへ変更する。
ログfilterや全遅延queue破棄は行わない。まず旧実装で置換後の実warning捕捉試験を失敗させ、
通常の期限、Virtual pause/resume、index再利用、実visual reset、中断時cooldownなしを回帰検証する。
会話時間・報酬・操作・保存schemaは維持し、Help判断・workspace検証・独立reviewとfresh nativeを再実施する。
旧実装はworld置換・pause/resumeの2試験で実WARNを再現、修正後は全4試験pass。
独立read-only reviewはこの限定修正を承認。fresh nativeと全体受入は別に確認する。
fresh `tak14-rest-area-cooldown-refund-feedback-20261107-v6`はsource
`31111610b5121cb963beb4aefd41727cde05415fdae63fcaee6cee73709390a4`で、
通常配送Wood0→1、保存読込Wood1、epoch1でWood1→2、owner2330v8の取消と
返却2102v10／2360v3各Wood1、request2098v9閉鎖・4tile通行復元を確認した。
Intel Arc/Vulkanの全6画像・原入力・rawを独立reviewし、登録verifierとstrict log検査pass、
seal/finalize済み。今回WARN/ERRORはなく、旧Soulへの遅延command警告は再現しなかった。
v5はinvalid履歴のまま保持する。v6の29,396,992 bytesは通常施工・全予約/inventory観測の
次工程比較だけをconsumerとして保持し、旧source/binaryの再検証依存は精算した。
通常/profiling workspace、Memory/Tracy/RenderDoc構成、両Clippy警告0、Help No impact、
rust-analyzer 0件、contracts/toolingを確認。全体CIは既存依存advisory 2件で未通過。
この限定feedbackを全予約/inventory・art/numeric/performance/release承認へ読み替えない。
原因修正とfresh v6の確認後、v5診断consumerを正式解除し、process不在・原本なし・
symlinkなしを確認して生成raw/画像/検証用saveだけを削除した（43,352,064 bytes）。
失敗metadata・採用コード・制作原本・最新v6証拠・通常Cargo cacheは保全した。

次の通常施工feedbackは既存`commit_completed_building`→`publish_building_completed`の
domain event記録を利用する。経路確認で元Blueprint・新building・kind・footprintのproducerが
既に存在すると分かったため、重複producerやゲーム処理は追加しない。
新しい`rest-area-completion` caseは通常配置後に実配送・施工を待つだけとし、完了event、
実Build task終了message、同epochの旧Blueprint消失・新ownerと全map占有をrawと画像で照合する。
状態差やUI通知だけを施工commitへ昇格せず、施工条件・速度・spawn・予約は変更しない。
全予約/inventory清算・正式asset/releaseとは分け、拒否系試験・Help判断・独立review・fresh実機を経る。
独立reviewで配送前Build完了を許す抜けを発見したため、最終配送以降のsample/time、
実ActiveTaskIdentityとの完了前一致を必須化した。元配送assignmentを保持するchainは維持し、
同sampleでのdomain callback→Lastのmessage読取順も別に検証する。関連59 tests pass。
初回`tak14-rest-area-construction-feedback-20261107-v1`は通常Wood5搬入・施工完了に到達したが、
verifierがRestAreaを誤って通行可能と期待して拒否したためinvalid封印/finalizeした。
既存`BuildingType::blocks_movement`とWorldMap完成処理は4tile通行不可が正しいため、
ゲームを変えず新caseの述語・fixtureだけを訂正し、通行可能を拒否するtestを追加する。
20,246,528 bytesの初回診断をfresh確認まで保持し、旧invalidをpassへ変更しない。
fresh `tak14-rest-area-construction-feedback-20261107-v2`はsource
`860333f9ad5043357e48a901ebdac833d59b66a99a07f1f6e3cbab20e158688d`、harness
`16199865666c335e2e14d8ba01470d7f84044a521bbfbf8015a82527dcd0384c`で限定pass。
Blueprint2233v0へ5個の実Woodを配送し、actor1863v0／起点assignment2234v0のBuild完了と
新building2283v42への4tile占有移管をsample206で確認した。domain event359→Lastのtask361、
実時刻は双方59.600192742。旧owner消失・全4tile通行不可、全3PNG/原入力/rawを独立review、
登録verifier・strict log検査pass、seal/finalize済み。Intel Arc/Vulkan、WARN/ERRORなし。
20,033,536 bytesは通常利用/Dreamと残lifecycleへの比較用途だけを保持し、旧source/binary依存は精算。
完成画像は現行fallbackの簡素な表示であり、正式art/numeric・全予約/inventory・性能・releaseは未承認。
通常/profilingのworkspace試験・両Clippy警告0、Memory/Tracy/RenderDoc構成、関連59 tests・tooling・
Help No impactを確認。rust-analyzerはcfg-disabled hintのみ、エラー0。全体CIは既存依存2件で未通過。

次のCI復旧は依存の最小補正として分離する。`futures-util`は同0.3系列の保守版へ更新して
`pin-utils`経路を解消できるか確認する。`ruzstd`は0.9.1以上の修正版が必要だが、Bevy 0.19の
`bevy_image`は^0.8指定のため、エンジン全体の版変更・feature削除・advisory無視は行わない。
必要なら既存vendor方式でBevy 0.19のimage crateだけを正規原本から固定し、APIを変えず
decoder依存だけを更新する。license・原本hash・実差分・削除条件と拒否系decode試験を残し、
Help判断・通常/profiling検証・native画像読込・独立review・storage・変更別CIで検査する。
依存補正は美術承認や9種の正式releaseを代替しない。
修正前の実Bevy KTX2回帰試験では、正常1024 bytesとtruncated拒否はpass、
1KiB windowに対する1025 bytesの不正RLE blockを旧ruzstd 0.8.2が受理してredとなった。
巨大なbombを生成せず拒否境界を再現した。公式bevy_image 0.19.0の全Rust sourceを保持し、
optional ruzstd指定だけを0.9.1へ変更するvendor patchで補正する。出典・除去条件は
`vendor/bevy_image/PATCH.md`を正本とし、修正後の結果は別途確認する。
修正版ruzstd 0.9.1では同3試験がすべてpass。`dev.py deps`はadvisories/bans/licenses/sources
すべてokとなり、RUSTSEC-2026-0441/0344の2件を解消した。既存duplicateとyanked版
（clipboard-win 5.4.1、spin 0.10.0）のwarningは別扱いで残している。
`image-decoder-feedback-20261107-v1`はsource `97b343980d38…`、通常画面1PNGを
mainと独立reviewerが実見し、登録verifier・seal/finalize pass。Intel Arc/Vulkan、WARN/ERRORなし。
manifest `8f974eeeb5b3…`、PNG `3c0e16303f48…`。入力なしの描画と起動時KTX2 LUT読込に限定し、
全asset形式・art/numeric・性能・9種releaseを承認しない。最終review中の3,985,408 bytesだけを保持。
Helpは公式API/featureを維持する不正画像拒否・futures保守更新で、入力/UI文言/Help到達性/
gameplay/save schema不変のためNo impact。全体の変更別検証は引き続き実施する。
依存補正は`c0267a15`で保存済み。base `37c76a3dd04cc1c7baf91faa5779598f05fae3c6`からの
変更別CIはcontracts/tooling/deps/rust全groupを通過した。通常/profiling全試験、Memory/Tracy/
RenderDoc構成、Clippy警告0、tooling 538+208試験、`dev.py check`、primary storage pass。
最終比較が終了したためdecoder native holdを正式releaseし、生成物3,985,408 bytesを撤去した。
コード・出典・限定pass metadataは保持し、この旧jobを再検証する義務にはしない。

次工程は通常RestArea施工完了の後の利用・Dream放出である。準備によるcamera/pause以外は
既存入力と通常AIだけを使い、SoulのDream/疲労/idle/Visibility、RestingIn/予約/owner側occupantsと
DreamPoolをread-onlyで採取する。既存DreamTransferredVisualMessageのexact Soul/RestArea/amountと
同process/nonce/epochのbefore/afterを結ぶ拒否系試験を追加し、通常入所と実放出を実機確認する。
occupant/Dream/materialの注入、強制完了、gameplay変更、art/numeric/performance/releaseの代用はしない。
変更別検証・Help・独立read-only review・native seal/finalize・storageを次の同sourceで再実施する。
通常/profiling workspace試験・両Clippy警告0と全groupの変更別CIを通過した。
初回`tak14-rest-area-usage-feedback-20261107-v1`は通常Wood5搬入・施工・Soul1867v0の入所と
Dream9.4800758→6.8300657、Pool79.4395065→87.3095398を記録したが、最終UI checkpointより
raw writerが1.091728332秒手前で終了し、1秒以内の厳密照合でinvalidとなった。
invalidをseal/finalizeし、38,535,168 bytesを同checkpointのwriter競合調査に限定して保持した。
UI PostUpdate撮影後、Lastのraw記録が既存の同状態/時刻条件を満たすまで最大15秒待つ補正を行う。
許容幅を緩めず、未到達だけを待ち、foreign identity・時刻/関係破損・timeoutは即拒否する。
旧invalidを再解釈せず、拒否系回帰と独立review後にfresh batchで確認する。
補正後`tak14-rest-area-usage-feedback-20261107-v2`はsource `b77a1c8b1538…`、harness
`d2e2f1487740…`で登録verifierと別プロセスのverifyが限定pass、seal/finalize済み。
通常Wood5配送・施工後のRestArea2275v32へSoul1863v0が入所し、同世代・同入口座標の
Hidden/Restingを維持してDream4.1100345→1.4600365、Pool72.4896851→75.1398468を確認した。
実RestArea sourceのmessage総量と減少量、同epoch・停止中UI/rawの状態/時刻が一致する。
全5PNGをmainが実見し、strict log検査はWARN/ERRORなし。画像は現行の灰色fallbackであり、
美術・数値採用・性能・正式releaseの証拠ではない。36,794,368 bytesは退出/cooldownと
利用中save/loadの次工程比較だけを保持し、旧binary再実行依存は精算した。
通常利用は`5486abf6`で保存し、変更別CI全groupと独立code/画像reviewが通過した。
v1のwriter競合調査は完了したため診断consumerを正式解除し、生成物38,535,168 bytesを
撤去した。invalid metadata・採用コード・最新v2証拠・通常Cargo cacheは保全した。
次は独立した`rest-area-exit` caseで実Dream放出を最後まで待ち、同Soul/ownerのfinal message、
関係解除・可視復帰・正のcooldownと歩行可能な入口をraw/画像で結ぶ。材料/Dream注入・
強制退出・duration変更は行わず、既存利用caseと正式art/release未達を維持する。
退出verifierは独立reviewで見つかった残量/Poolの拘束不足を補正し、rawとUIを同時改変しても
不正残量・Pool加算なしを拒否する回帰を追加した。通常再生成は実cooldown経過と最大rate/
slow-stepからだけ許容する。関連68 tests、実Rust定数契約1件、変更別CI全group、
`dev.py check`、profiling Clippy警告0、Help No impact、rust-analyzerエラー0を確認した。
`tak14-rest-area-exit-feedback-20261107-v1`はsource `dd827e2d22be…`、harness `b76c0ece1f34…`で
実行と登録verifier・別verify・独立全6PNG/raw reviewが限定pass、seal/finalize済み。
Soul1867v0／epoch0の残Dream2.7300391に対し実RestArea放出22件は計2.73004065、
sample414のfinal時にDream0・関係解除・Visible/walkable・cooldown15を確認した。
撮影時は通常再生成Dream0.35／cooldown11.50、Pool84.98965→102.41963で全量以上の増加。
UI/raw境界の時刻は完全一致し、Intel Arc/Vulkan、WARN/ERRORなし。灰色fallback表示であり、
cooldown満了・利用中save/load・art/numeric/performance/releaseは未承認。
43,528,192 bytesは残lifecycle比較用に保持した。旧施工v2と利用v2の比較consumerを最新batchへ
移管・正式解除し、生成物20,033,536＋36,794,368 bytesを撤去。結果metadataとコード/原本は保全した。
利用中save/loadの次工程では、明示production observerがあるprofiling buildだけに、
実atomic-saveのheader/body bytesのdigestと、成功normal-loadが使用した実Entity remapを記録する。
通常requestのoperation/slot/source、epoch、時刻、RestArea/Soulの実snapshotへ束縛し、
rollback/recovery・未成功・異request・欠損map・上限超過から成功receiptを作らない。
保存schema・ロード判断・rehydrate順・terminal outcome・gameplayは変えず、観測失敗だけを
fail-closedで記録する。producerの拒否系と実transaction回帰を先に検証し、独立review後に
通常UIの保存/読込とexact file bytesへ接続する。名前一致や状態差はEntity同一性の代用にしない。
施工完了v1の誤ったwalkable期待値はv2で修正・独立review・実機passとなったため、
旧v1 diagnostic consumerを正式releaseした。稼働process・symlink・原本がないことを確認し、
`target/native-acceptance/tak14-rest-area-construction-feedback-20261107-v1`の生成物
20,246,528 bytesを撤去した。invalid履歴と採用コード、最新v2の比較用途は保持している。

2026-11-07再開工程: 通常Blueprint取消が実際に発行したrefund Entity IDsと、
deferred適用後のowner/request/storage/map・関連actor状態を結ぶprofiling-only観測を追加する。
通常RestAreaの既存input recipeへ同nonce/PIDのrawを接続し、ゼロ材料予約の取消を実機確認する。
単体試験の非ゼロrefundと通常worldのゼロ材料取消を区別し、完全な予約/inventory清算、
SoulSpa専用取消、全9種のart/numeric/performance/release未達は維持する。
取消10件と通常/profilingのworkspace試験、Memory/Tracy/RenderDoc構成、両構成Clippy警告0を確認。
全体gateは既存依存のRUSTSEC-2026-0441（pin-utils）とRUSTSEC-2026-0344（ruzstd）で未通過。
例外追加や依存の無検証更新はせず、正式完了条件として残す。
初回`cancellation-witness-feedback-20261107-v1`はbuild成功後、ゲーム起動前のjson未importで
invalidとしてseal/finalize。設定生成関数を直接呼ぶ回帰テストを追加し、修正後はfresh試行で確認する。
fresh `cancellation-witness-feedback-20261107-v2`は5PNG/rawの独立reviewと登録verifierで限定pass、
seal/finalize/storage pass。Intel Arc/Vulkan、same nonce/PID118045、新epoch1のowner1834v1
（UI Entity ID8589932757）についてsample86で実applied witnessを確認した。
配送request1839v1とactor4件の参照解除、owner/map tombstoneと4tile walkability=trueを照合。
進捗0・材料0・refund0の実操作であり、非ゼロ返却や全予約/inventory清算の受入ではない。
次は同じ通常取消経路で非ゼロ実搬入材料の返却と全清算を検証する。9種の正式導入は未完。
この新証拠を次の比較用に保持し、用途を終えた旧RestArea zero-material証拠と初回build-only
診断をconsumer release後に削除した（計20,017,152 bytes）。制作原本・最新Tank/RestArea証拠・
通常Cargo cacheは保持。削除後storage checkは404,728,475,648 bytes、339 batch、未分類0でpass。

2026-10-12の最新限定進捗: `89f40fe2`で通常Tank親Blueprintと2childの計画commit、
F5/F9のManual1保存復元、dialogなしのロード後画面をactual-window/rawの独立review・
登録verifierで確認した。次は通常計画取消のowner/付属保管所/map cleanupと実terminal receipt。
完成・正式asset・9種全体受入は未完。
`e9bc838f`の初回はF5直後のcatalog layout未確定（sample65、可視controls空）をクリック前に拒否し、
save/loadなしのinvalidとして精算・診断保持した。入力を再送せず、2つの実computed-layout sampleが
同じwindow／epoch／session／controlを持つまで最大3秒のpassive waitを追加してfresh subjectで再確認する。
`d58de249`のfresh実機は描画待ちを通過し、Manual1のSave成功（sample57、session1、497110 bytes）を
実見した。Loadはsample71/session2でInvalidData、epoch0のまま拒否。
原saveにはYard発行のDeliverToBlueprint requestとFamiliarのManagedByが混在していた。
Soul割当applyの`IssuedBy(=ManagedBy)`片側上書きが原因で、requestのproducer ownerを
deferred適用時に保持する修正を進める。原失敗・saveは診断保持し、load validatorは変更しない。
修正`ba1d8e3a`は変更別CI、Clippy警告0、workspace checkと独立reviewを通過。
fresh実機はSave以前にcameraのKeyW区間で停止した。sample22→23が0.505秒空き、
Yard投影が96→348pxへ移動してfeedback停止帯を通過した。通常worldやinputを変更せず、
明示profiling observerだけheld WASDを50ms間隔で読む。sample/event capは維持し、
hostは停止帯通過を即時拒否して元keyを一度releaseする。旧launchの再送・補正入力はしない。
`562a151b`のfresh実機では通常Save sample67/session1・Load sample82/session2がSucceeded、
epoch0→1、親1898v2→1984v1と2childの新ID、footprint・空材料・map占有の復元をrawで確認。
実Manual1は501865 bytes。ただしloaded.pngはLoad確認dialogが残り、独立画像reviewが拒否した。
cycleはinvalidとして精算し、原raw／save／画像を撮影同期修正の診断に保持した。
Load実receipt直後のcaptureをやめ、2つの後続Closed sampleと500ms settleをpassiveに要求し、
fresh画像を独立確認する。raw成功を画像成功・正式construction・releaseへ読み替えない。
fresh `tak14-save-tank-present-feedback-20261012-v1`は独立9PNG reviewと登録verifierで限定pass、
seal/finalize/storage check済み。Save sample67/session1、Load sample81/session2、capture sample84は
receipt後566ms・後続3sampleのClosed、新epoch1。原親2367v1→798v1、付属2childのID置換、
4tile footprint・空材料・進捗0・stockpile占有を保持し、旧実entity/map残留はない。
実Manual1は494407 bytes、SHA-256 `541c691df5a3ce70428244177c39238702a4e1bbc7dd26c45bebc91879998835`。
通常Intel Arc/Vulkan、WARN/ERRORなし。全world canonical serialization・完成・art/numeric/perf/
正式releaseではなく、次の取消／cleanup比較用途の限定raw/save/latest画像だけを保持する。
既存通常RestArea recipeのfresh試行は配置・Save/Loadの4checkpoint後、Task Dashboard取消の
Left押下でtargetが再生成され、pressed観測を失ってinvalid。取消receiptはない。
本文のdirty再描画をheld Left中だけ保留し、dirty要求をreleaseまで保持する修正を進める。
simulation snapshot・既存同対象release guard・live capability拒否を変更せず、fresh実機で再確認する。
修正`76209f80`は同sourceの全変更別CI（contracts/tooling/Rust）、profiling全target Clippy警告0、
workspace checkと独立read-only reviewを通過した。fresh `rest-area-gesture-feedback` v1/v2は
どちらもrecipe入力前の初期WM activationで拒否（owned `0x1600004`、focus `0x400003`）。
配置・保存・取消のcheckpointはなく、invalidとしてseal/finalize済み。第三の盲目的な再起動、
強制focus、旧launch再送はせず、通常取消の実機確認は未達として保持する。
通常Tankの保存復元修正で用途を終えた旧生成job6件（placement-yard、companion、saveの
initial/layout/owner/cadence）を、終了済み・利用processなし・追跡原本なし・symlinkなしと確認して
正式consumer release後に削除した。実測合計77,844,480 bytes。履歴結果と採用仕様は維持し、
最新Tank raw/save・RestArea取消診断・制作原本・通常Cargo cacheは保持、storage checkはpass。
初期activation拒否は、公開Shell状態のread-only観測でOverviewActive=true、画面ロックfalseと
切り分けた。通常の公開Properties.SetでOverviewだけを閉じてfalseをreadbackし、新規jobで
既存owned focus guardを通過。強制focusやtimeout調整はしない。
`1756c610`のfresh `tak14-rest-area-gesture-overview-feedback-20261012-v1`は通常配置・Manual1の
Save/Load・二段階取消・4tile復元を独立5PNG/raw reviewと登録verifierで限定pass、seal/finalize済み。
親8589932601→8589932382、epoch0→1、進捗0・材料未完の予約を保持。
frame562で新親/epoch1のCancellationRequested、frame576で予約なし・4tile owner=null/walkable=true。
取消ボタンは各press/releaseで同task payloadを維持し、ロード後画像に確認dialog残留なし。
完成材料のterminal refund、全actor/pool cleanup、art/numeric/performance/releaseは未達。

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

### SoulSpa通常worldの観測補完（2026-11-08）

`66330d0d`のfresh `tak14-soulspa-ordinary-usage-20261108-v1`は施工・Generating・1W出力の
全5checkpointを得たが、ゲームログのdangling relationship警告1件でinvalidとなった。
Soul `1856v0`→消滅対象`1950v11`の警告時刻は23:06:55 UTCで、同Soulはraw上で集会行動中、
Spa task/WorkingOnは全400 sampleでNoneだった。site `2053v2`は別ownerで、
実利用Soul `1878v0`はtile `2040v2`でGeneratingを維持したが限定passへも昇格しない。
旧batchはinvalid seal/finalize、41,324,544Bを修正比較用に保持した。
管理Recruitには適用時の存続guardがある一方、idle Join/Arriveにはなく、同じ消滅参照を作れる。
この経路を適用時のSoul/spot確認へ揃え、拒否時の休憩関係・Visibility・通知不変を回帰試験する。
警告の型名がdebug featureなしで匿名のため、当該経路との完全な因果確定はfresh比較まで留保する。

全16maskの静止確認後、通常利用/lifecycleを実値で検査するため、profiling-onlyの
`observations_spa.rs`を通常raw observerへ接続する。siteのphase/Bone搬入/active_slotsと発電値、
durable parent_siteによるtile/worker/WorkingOn逆参照、実GeneratePower taskとSoulのDream/疲労、
WorldMap占有・walkability・grid接続を有界に読み取る。表示hierarchyに依存しない。
不足component/resourceはnullとして残し、欠損からmask・成立・cleanup完了を推測しない。
所有・座標・有限値・上限の不整合は検査できる生の状態として保持し、非有限値/件数超過はfail closed。
既存のdomain cancel/slots outcomeを独立MessageCursorで記録し、受理要求や消失を成功receiptへ変換しない。
通常起動の候補採用・通常動作・保存schema・公開入力は変更しない。関連unit/拒否試験・Help実レビュー・
Rust check/Clippy・変更別検証・独立readonlyを通してから、別の通常入力シナリオを補完する。
このproducer補完だけでは実施工・自然利用・Save/Load・Capture/Memory・releaseを完了にしない。

次に専用`SoulSpa`通常配置feedbackを追加する。生成済みYard内の南向き2×2成立条件を
製品と同じvalidatorで探し、初期準備はcamera/pauseだけに限定する。既存OS入力で配置、
未搬入状態のSave/Load、確認付き取消を行い、4つのdurable tile、owner remap、実map占有と
walkability、実cancel outcomeを照合する。資材・task・worker・Dreamは注入しない。
この限定recipeは候補アートをadmitせず、発電/利用/搬入後返却/正式releaseを証明しない。

初回`tak14-soulspa-ordinary-planning-20261108-v1`は`f647da95`で実行し、配置press前に
`Spa placement context changed`でinvalid終了した。原観測frame279/epoch0ではArchitect menu、
SoulSpaPlace(None)、合法footprint `[(70,50),(71,50),(70,49),(71,49)]`、cursor `(944,524)`、
world capture/pointer-over-UIともfalseであった。試験のHidden menu要求が製品契約と違ったためで、
配置pressやSave/Load/取消は未実施。旧結果はinvalidのままseal/finalizeし、原入力と観測を修正比較に保持する。
製品状態を変更せず、試験は実Architect menuを正としてmode/footprint/window/epoch/入力guardを維持する。

次の`tak14-soulspa-ordinary-planning-20261108-v2`は`c34f4da4`で通常配置・保存・復元・復元owner選択まで
観測したが、取消controlの探索でtimeoutしinvalid終了した。実UIは
`CancelSoulSpaConstruction { source_task: None, target: 865v1 }`を公開しており、試験が逆のfield順を期待した。
取消/確認入力は未送信。保存/読込の実receiptは同じ500945 bytes/SHA256
`e57639856f4fbe7f3c58377775b8f2f98551a26839bba1ee824dd691f943855d`を示し、
旧site `2237v0`→`865v1`、4tileとgridの実remapを記録したが、全recipeの成功とは扱わない。
この失敗もinvalid seal/finalizeして比較用に保持し、識別keyを実UiIntentの宣言順に集約して修正する。

`tak14-soulspa-ordinary-planning-20261108-v3`は`59ad5f2c`で5段階の操作・画像を取得したが、
独立verifierの配置mode確認でinvalid終了した。UI press前6.447秒にはSoulSpaPlace(None)で、
直前raw 6.216秒にも同modeがあるが、配置commit/press観測6.498秒では製品がmodeをNoneへ戻していた。
試験側の「action開始以後にもmodeが残る」前提を撤去し、UI guard時点以前の最新rawを1秒以内、
同window/epoch、Architect/SoulSpaPlaceに拘束する。古い一致行への遡及やpost-commit行の代用は拒否する。
元batchはinvalid seal/finalizeのまま保持し、修正版は新subjectの別batchで実測する。
同じ原記録の診断で、F5/F9のraw読取時刻が短いUI ACKの後に来る場合も確認した。
keyの再送や検査window拡張ではなく、同nonce/PIDの実Pressed/Releasedを最大10秒待ってから
actionを閉じる。独立reviewの当初の「loaded時に施工予定表示がない」という所見は、
5画像の再照合で撤回した。ROI `(912,492)-(992,573)` の淡青表示はplaced/saved/loadedで一致し、
empty/cancelledでは地形へ戻る。誤認を根拠に製品visualを変更しない。

`tak14-soulspa-ordinary-planning-20261108-v4`（`ef3dff4f`）は入力照合を通過したが、PowerGrid観測に
numeric Entity bitsがないためdurable remap照合でinvalidとなった。site/tileと同様に実`Entity::to_bits()`を
受動producerへ追加し、grid接続・逆参照を持つfixtureと欠損/異ID/異参照の拒否試験を補完する。
Debug文字列からbitsを推測せず、mapping条件も緩めない。旧結果はinvalidのままseal/finalizeする。

補完後の`tak14-soulspa-ordinary-planning-20261108-v5`は`6e612c1bd94f832aa9ee345563d295986c307a7f`で
5checkpointと別実行の独立CLI verifyが成功し、primary seal pass/finalizeした。
Intel Arc MTL / Vulkanの所有X11窓で通常fallbackの配置表示を取得し、全画像を目視した。
site `2226v0`→`909v1`、epoch0→1、4tileとPowerGrid `1884v0`→`1858v1`の実remap、
500731 bytes/SHA256 `8d29562112841ff9440426e09960f1b849bd952c5a9f5561b884e611f73611e3`の
同一保存原本、未搬入Bone0・必要12・active_slots4・ゼロ出力・歩行可能な4区画を照合した。
確認付き取消は復元ownerへの実outcome `Canceled { refunded_bones: 0 }`、site/tile/map owner消失と整合する。
manifest SHA256は`a4a0734ebde473487b61a008bfa039c519ab647c83cc043f487949bcfb7b4ac9`、
raw SHA256は`05cf5522bb35ebbd6929fff863ad964787efb8e8b2a3f3a3027ce1a84618a334`。
これは未搬入の施工予定だけの限定技術受入であり、候補アートの通常適用、自然搬入/完成/利用、
搬入後返却、全owner cleanup、Capture/Memory性能、正式releaseは未完のまま。
v5を次の自然施工・通常利用との比較基準に保持し、診断を終えた旧v1〜v4のjobは保持解除して撤去する。
旧4jobの実測83,501,056 bytesを撤去し、storage checkを通過した。失敗metadataと簡潔な診断は保持し、
採用candidate・authoring原本・最新v5・primary Cargo cacheは変更しない。

次の限定工程は`soul-spa-completion`の通常施工確認とする。既存の配置入力guardを共有し、
empty/placed/completedの3画像を取得する。初期準備はcamera/pauseだけで、通常AIによるBone搬入を
最大180秒待つ。完了は同じsite/4tile・Bone12/12・Operational・GeneratePower designation/slot・
map占有/歩行・実worker逆参照/発電値の整合を同nonceのrawと照合する。入力の再送、資材注入、
完了seed、候補自動admissionをしない。期限内に成立しなければ未完の実観測として停止する。
この工程だけでDream消費・長時間利用・稼働枠変更・Operational保存復元・deconstruct・性能・releaseを閉じない。

`tak14-soulspa-ordinary-completion-20261108-v1`を`c9ab174c62a1fff7a4839c20142086dc2d84888e`で実行し、
3画像と別実行の独立CLI verifyが成功、primary seal pass/finalizeした。
通常fallbackのsite `2162v1`（bits8589932429）、tile `2156v1/2157v1/2158v1/2159v1`、grid `1884v0`を
保持し、自然搬入Bone0→12/12でOperationalとなった。完成snapshotは実時刻88.441秒/仮想53.365秒、
各tileにGeneratePower/slot1、歩行可能な同一占有を観測し、worker0/出力0でgridと整合した。
Intel Arc MTL / Vulkanの所有X11窓で3画像を目視し、淡青の予定表示から4区画の完成fallbackへ変わることを確認した。
manifest SHA256 `107e97868fcb801643348fd7aa809dbe87f6663033c7a0202c0968f3811ce973`、
raw SHA256 `9fd7a6161e4e6452c95a02dedfa1e0b9e7004be6281c471d10e356da3495af3a`。
完成時HUDのFPS表示4は性能budgetの証拠ではなく、性能受入は未実施のまま。自然利用・発電時Dream消費・
枠変更・Operational保存復元・deconstruct・候補アート通常適用・性能・releaseを未完として維持する。
最新完成jobを通常利用/Operational保存復元との比較に保持し、比較を終えた未搬入planning v5のjobは撤去する。
planning v5の生成物25,108,480 bytesを保持解除後に撤去し、primary storage checkを通過した。

次の限定工程`soul-spa-usage`は自然施工の3checkpointを継承し、HUD Fastで最大300秒待って実Generating
workerを観測する。using/usedを追加し、同じSoul/tileで3秒以上の実仮想時間、正出力とgrid、
Dream消費0.5/秒を照合する。全中間rawの同epoch/window/site/4tile、時計、実HUD通常/停止入力を拘束し、
Dream/workerの注入や割当強制をしない。収集上限1200sample、全session480秒を維持し、上限到達はinvalidとする。
枠変更・Operational保存復元・解体・候補通常適用・性能・releaseはこの限定利用試験に含めない。

警告修正後`32805bdc`のfresh usage v2は警告/エラーなしで通常施工を完了したが、
その後120実秒で仮想50.387571644→90.637571644、全SoulのDream最大27.01988410949707に留まり、
既存割当条件30未達のままtimeoutした。invalid seal/finalizeし36,749,312Bを待機比較に保持した。
旧結果を成功へ変えず、通常速度・判定・数値を維持して待機だけ300秒へ有界化する。
全session480秒・1200sample上限は不変であり、この変更自体を自然利用の成功とは扱わない。

usage v3（`28b64854`）も警告/エラーなしだが、300秒の待機末尾で仮想129.775353527、
最大Dream30.01014518737793に達した直後にtimeoutし、Generatingは未成立だった。invalid精算し
44,642,304Bを経路比較に保持した。待機延長の反復はここで打ち切り、既存HUDのFast=2倍速を
条件待ちだけに使い、Dream消費は通常速度へ戻して測る別の明示経路にする。
受動Time.relative_speedをraw/UI共通contextへ記録し、実Fast/Paused/Normal/Paused入力と
raw速度1→2→1、Normal限定の消費区間を結ぶ。未観測速度や未知入力で成功を作らない。
ゲームの時間ルール・Dream値・割当条件・速度の直接設定は変更せず、全480秒上限を維持する。

usage v4（`a0a370f5`）は実倍率2を観測し、Soul1844v0が仮想79.482〜83.482でGeneratingと
Dream消費を示したが、疲労0.742994→0.802993の直後にtaskNone/回復へ移り、using撮影時は未利用でinvalid。
警告/エラーなし。invalid精算し38,072,320Bを疲労窓比較に保持した。成功条件を弱めず、収集開始を
実Generating・Dream20以上/疲労0.6以下/stress0.65以下という観測窓に限る。測定継続は全rawで検査し、
回復・worker・数値・離脱条件を注入/変更しない。既存300秒/全480秒上限と無再送を維持する。

`bea0b570`のfresh `tak14-soulspa-ordinary-usage-20261108-v5`は5checkpoint・別実行/登録verifierと
main/独立read-onlyの全画像確認を通過し、限定technical passとしてseal/finalizeした。
source `81e913b300df79a4f170f17bdd5b412b8262d7f97720ac2843a75c5c50054fd5`、harness
`bbdc6194935a38bdfc0d1640217caa0a2b6e0ca96eb9a18816b1d52f2bd46a42`、manifest
`8dde11884e22bee96a8e1d4faa864246363924dad00318ad3dcf2f1ee5d26ea5`、raw
`23edf02b177f40758f1822bf94540b7e7353480c5699aeb83b22f1f9a410bfb6`。
site2258v0/四区画を維持し、通常Bone0→12・Operational後、Soul1846v0/tile2261v0が
Normal速度1で仮想133.152545029→139.902545029の6.75秒を継続した。Dream26.81013298→23.43513298、
減少3.375は0.5/秒と一致する。後から別Soulも入り出力は1→2。Intel Arc/Vulkan・WARN/ERRORなし。
人物・範囲オーラ・粒子が重なるため画像だけで個体や数値は読めず、対応はrawによる限定観測とする。
通常起動はfallbackのまま。候補art・枠変更・Operational保存復元・性能・releaseは未受入。
最新job44,785,664Bを枠変更/Operational保存の比較基準に保持する。旧usage v1〜v4の診断は
この比較で終了し、結果履歴を維持したまま生成jobを整理する。

次は独立case `soul-spa-persistence`で自然施工3checkpointにempty Operationalのsaved/loadedを追加する。
既存F5/F9/catalog実入力だけで原保存バイト・正規commit・同window/nonce・epoch0→1と
site/四区画/PowerGridの完全remap、slot/指定/pose/map/出力を検証する。完成以降は全区間pausedで、
workerがいる場合は拒否し、利用中task/Soul remapの証明に読み替えない。枠変更・取消・未知入力も拒否する。
Help実レビュー・拒否系試験・変更別CI/Clippy・独立read-only review後、fresh nativeで確認する。
この準備は保存受入成功や候補通常導入ではない。
独立コードreviewでsaved→load commit間の一時的な状態差替えを拒否できない点を検出し、
正規load receiptのsampleを境に全rawの旧/新epochと施設/四区画/gridを固定する補正を追加した。
同区間の孤立Soul task/WorkingOnも空施設条件として検査する。中間Constructing、ghost task、早いepoch置換の拒否を追加。
診断終了したusage v1〜v4は正式consumer解除・終了process/open fileなし・非symlink・生成物だけと確認し撤去した。
du合計160,788,480B減（原本・候補・最新v5・共有Cargo cacheは維持）。invalid metadata/結果履歴は保全する。

`2ba8332a`のpersistence v1は施工待機180実秒でtimeoutし、保存/読込へ未到達のinvalidとして
seal/finalizeした。最終real204.229823432/virtual68.017569753、site2276v1/Bone0。WARN/ERRORなし。
rawの実AssignedTaskを照合するとSoul1848v0/1854v0は別cart909v0/910v0でBone10ずつを自然採集し、
同siteへのGoingToDestinationを継続していた。1848v0は仮想6.673/位置[-432,-432]から
仮想67.767/位置[560,144]まで移動しており、搬入producer不在や同cart競合と断定しない。
今回は実時間上限に対する仮想進行の遅さと実移動経路が観測できたため、persistence専用の施工待機だけ
300実秒へ一度有界化する。施工raw/receipt/空施設/保存検査は不変で、既存completion/usageの180秒は維持する。
全480秒/1200raw上限を維持し、fresh v2でも不成立なら同系統の待機延長を打ち切る。
旧v1の26,980,352Bは移動経路と次の一回比較の診断用途で保持し、失敗を成功へ読み替えない。

`74f396dc`のfresh persistence v2は通常搬入でsite2332v1/Bone12/Operationalまで進んだ
（real206.460834539/virtual74.232638957）。ただし停止ボタンへの単発motion step24で
要求physical[118,32]に対し製品cursor[234,360]となり、クリック前の到達ACKがtimeoutした。
最終frame564/focused=true/paused=false、WARN/ERRORなし。F5/F9と完成checkpoint撮影へ未到達であり、
試験全体をinvalid seal/finalizeし28,581,888Bを当該ポインタ比較用途で保持した。
物理的な利用者操作と原因を断定せず、座標offsetやACK条件・時間上限を緩和しない。
施工待機の延長はこれ以上行わない。次のfresh試験前に共有入力が静止する時間を確認する。
独立して進める次の設計は、実site選択→Normal→既存SetSoulSpaActiveSlotsの4→0入力と正規outcomeを
結び、既存workerを追い出さず新規assignmentだけを閉じる通常契約を検査するものとする。
prefixの通常利用証拠・全tailのslot/worker/逆参照/owner/grid/epochを維持し、state注入を行わない。
この設計記録は実装・実機成功を意味せず、occupied保存・撤去・候補通常表示・性能・releaseは未受入のまま。

利用者が入力を控えられると明示した新比較 `tak14-soulspa-ordinary-persistence-20261108-v3`
（`4ae97505`、source `527f0d2d4b1752cdfba16b97f5fab781daf7151b35067784e7053feefaf8b60a`、harness
`b2d31eb0e48d96c731fee53cdfa66267fde678c84cbcf20959899c0bd7f83fe0`）は全5画像・別実行/登録verifier、
main/独立read-onlyの画像/raw確認を通し、**空Operationalの論理永続化だけ**をpass seal/finalizeした。
manifest `dbb07cfd9b3facebc1e55723c7bf1e012fa605b413700aa083f7f43671aad876`、raw
`23b630251d84353cc4c86e5dcc28e0c2bb99bbbfcd2708b78b9b358af9a52f77`、Manual1保存501040B/hash
`a2a2cfef1a4a206e8cbea71e81bac0cdbf5ff14f69764d4cbe328e7c9984b1b1`。
site2318v0→1902v1、grid1884v0→819v1と全4tileの実remap、GeneratePower/slot1/worker0、Bone12、
paused tail103行の仮想52.076294648不変を照合した。Intel Arc/Vulkan、WARN/ERRORなし。
画像の黄土色→淡青の差は実在するが、保存前selected Familiar1834v0→load後nullの条件差もあり、
選択オーラが消えている。共通fallbackは両方Neutral/mesh10、sourceの材質選択も同じequipment_materialである。
画像だけから材質/ロード退行と断定せず、同一selectionでの表示同等性は未検証とする。正式artや候補導入ではない。
最新38,580,224Bは表示条件/occupied保存の比較基準に保持し、旧v1/v2の有界診断は終了した。
旧v1/v2は正式consumer解除・process/open file不在・非symlink・生成物のみと確認し、exact 2jobを撤去した。
du計55,562,240B減。原本・最新証拠・共有cacheとinvalid metadataは維持する。

枠変更の次の回帰試験は既存UiIntent→domain commit経路を使い、4区画のGoingToTile/Generatingを
保持したままactive_slotsを4→3→2→1→0→4へ変更する。worker双方向参照、task/phase、
区画designation/slot、Bone/OperationalとSoul状態が変わらず、満員/ゼロ枠の新規受付判定と
各変更outcomeが一致することを単体試験する。停止中の枠変更は消費・拒否され、再開後に遅延適用しない。
これはUI domain境界の回帰範囲であり、通常スケジューラの継続・実クリック・実機枠変更の受入ではない。
通常起動、入力仕様、Help本文と正式art/performance/release条件は変更しない。

独立case `soul-spa-slots`の7checkpoint経路を追加する。既存の自然施工/利用をprefixとして
再検証し、同施設の通常選択（必要ならexact ownerの既存候補picker）→Normal→4回の枠減少→停止→
Normalの短時間継続→停止を原入力と受理outcomeへ拘束する。全tailの構造/関係/時刻と
同Generating Soulの維持、ゼロ枠以降のtask割当集合不変を検査する。既存GoingToTileの到着は許し、
WorkingOn追加前から割当を数える。数値・actor・taskを注入せず、
誤対象、欠損/重複receipt、bool代用、epoch変更、追出し、新規worker、短い測定、余分な入力は拒否する。
これは実機試験の準備であり、独立review・fresh実行・画像/raw確認・登録verify前は未受入のまま。

`72b18d36`のfresh slots v1は通常Bone12/Operational、Soul1862v0/Generatingまで進んだが、
owner2220v0/bits4294965075の中心投影[960,540]への単発選択がSoulへ当たり、既存pickerを開いた後も
表示controlsに施設が現れず、待機上限でinvalid精算/finalizeした。終了real444.400936429、
active_slots4、slot操作なし。54,042,624Bを選択経路の診断だけに保持し、受入成功とはしない。
独立read-only照合でも候補行は32px、scroll viewport478px/offset0で先頭15行だけが露出し、
ObjectはSoul/Resourceより後順位であることを確認した。全membership/施設の行番号は観測しておらず、
候補自体の欠落やゲーム選択不具合と断定しない。ゲームの順位/選択ruleは変更しない。
次はprofiling限定で既存resolverへ四区画中心/施設中心の最大5点を受動照会し、施設が先頭Direct Objectとなる
点だけで通常クリックを一度行う。解決点なしはpress前に拒否し、任意座標や候補・選択の注入はしない。
単体回帰、Help No impact判断、独立コードreviewとfresh実機を経るまで、枠変更は未受入のまま。

`801ef1ae`のfresh slots v2はsource `6c4a146668f9ac330383168f4c49bce20dd44e83e4c538a23cea06311f265356`、
harness `4f3ff9e8e2ce77fce8f730b357ddc6837d6fd2660f857dbad75165c12343037b`へ拘束した。
通常Bone12施工後、Soul1872v0/1878v0のGenerating・出力2と通常6.75仮想秒の利用prefixを観測したが、
5点のfirst-Direct-Object probeはnullで選択motion/press前にinvalid停止。real298.970581711、
virtual145.838831046、active_slots4のまま。子終了確認・invalid seal/finalize、49,233,920Bを補正比較に保持した。
probe nullは先頭候補なしと内部照会不成立を区別しないため、施設欠落やResource遮蔽とは断定しない。
先頭Direct点を探す仮説はここで打ち切り、既存pickerへの通常スクロールを明示試験経路にする。
同じ5点で施設がDirect Object候補内にいることを受動確認し、先頭なら優先、重なれば一回clickして
実picker inventoryの同owner/revision/epochを照合する。専用ScrollArea keyと実MouseWheelを受動公開し、
最大8wheel pairで進行・所有window・不変camera/選択/worldを確認してからfully-visibleのowner行を一回選ぶ。
無進行・再構築・不明owner・送信前wheel・別window・overflowは拒否。順位変更やscroll/選択の直接設定はしない。
fresh実機の全7checkpointと登録verifyを終えるまで、利用枠・正式art/performance/releaseは未受入。

`62067e1f`のfresh slots v3はsource `1b6c4125ce2e3881f23cdb38c0197a442a146a1ce68487f72dac93650fb95411`、
harness `d6006a63a94293a80908c7c9e740aea5c1089beeaa8c912110c114f04f7f37e2`へ拘束した。
通常施工・利用後、実候補pickerの3行目からowner2245v1/bits8589932346を選び、
370.236581144/402.14459734/435.347474251秒の3/2/1と、その後0の全4受理outcomeを記録した。
最後の実観測はreal476.42679768/virtual139.717833185/slots0/出力1で、closed/held撮影前に全体480秒上限でinvalid。
子終了確認・seal/finalize済み、56,360,960Bを入力待機と自然離脱の診断に保持。全受入は未達のまま。
used時の2人中、Soul1860v0はDream26.9749/疲労0.4052、Soul1876v0はDream31.0248/疲労0.7327だった。
後者の離脱を枠による追出しとも自然離脱とも断定しない。次は全初期Generatingに既存測定の安全条件を要求する。
UI observerは3frameごとだが各入力がさらにframe+3超を待ち、1枠操作に約32実秒かかっていた。
利用枠caseだけ次のfresh実観測でcursor/pressの同一ACKを検査し、固定の追加frame待機を除く。
他caseと原入力照合・所有/nonce・fail-closed・全体480秒は不変。新方針をclientへ拘束し、
旧attemptは再送せず、変更別検証・Help No impact・独立review後にfresh subjectで一度比較する。

## 5. マイルストーン

### M1: 原本・候補

- [x] 保全原本と現行loader/制作器を調査し、storage checkを実施。
- [x] M2の現行subject候補、M3両種、M5四種を制作・技術検査（未承認art_previewのみ）。
- [ ] 実機比較画像と数値契約を独立レビューし、exact identityに採否を記録。

### M2: 受入・正式導入

- [x] primary validation所有の通常world固定F12入力probeを拒否系試験・実機・独立reviewで確認。
  これはinput/raw輸送の技術境界だけで、正式recipe、domain lifecycleやreleaseの代用ではない。
- [ ] 通常開発の不足producerを実装し、拒否系試験と独立レビュー。
  - 11月7日: Blueprint取消の実返却ID・owner cleanup観測を実装/検証中。全lifecycle成功ではない。
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
producer補正`2fd6bb381cafb525898def9393ad2e43d600b851`のfresh batch
`tak14-mud-mixer-v2-refining-2fd6bb38-20261005-v1` は160秒観測後、renderer/GPU identity
欠損でinvalid。rawはv2 hash・Idle/Active/pausedを記録したが、actual-window logにAdapterInfoがなく
正式観測passにはしない。旧失敗と別の診断保持・finalizeを行った。
M2 runnerの子processへ既存nativeと同じ`RUST_LOG=info,wgpu=error`を明示し、rendererの証拠を
採取する補正を行う。renderer欠損／不一致の拒否は維持する。inventory単独もrawの数値hashを
照合し、v2 traceのhash欠落やv1取り違えを拒否する。修正後のfresh実機verifyは未達。
補正`5468dbbc928f48993986b362e7929021d6e553c0`のfresh
`tak14-mud-mixer-v2-refining-5468dbbc-20261005-v1` と
`tak14-tank-water-5468dbbc-20261005-v1` はIntel Arc/Vulkan所有1280×720 windowで
各160秒観測、登録済み独立verifier pass、比較用途保持・finalize・primary storage check pass。
MudMixer rawは1288 samples、同一v2数値hash・ArtPreview identityでIdle/Active、17 paused samples、
rotor姿勢変化と再開を記録。独立画像レビューでは木枠・泥・rotorと紫のActive表示差が読め、
追加paint欠陥は見当たらない。一方、owner itemはrawの33 samplesにあり、画像では独立した形が
明瞭でなく、item/rotor重なりの可読性は未達。Idle画像にはConstruction Complete表示の重なりがある。
checkpoint 1の撮影前raw index39はpaused=falseだが画像は既に一時停止表示で、checkpoint 2も
停止画像である。現在の撮影前sample bindingは画像の撮影時点を固定しないため、画像だけで
停止前後の運動を証明しない。撮影前後のraw区間と描画時点の整合を確認する補正が必要。
Tank rawは955 samples、同一v1数値hash・ArtPreview identity、同一ownerでEmpty/Partial/Fullを記録。
通常倍率のcheckpoint画像では空の槽底、暗めの低水面、明るい満水面が見えるが、独立画像採否と
撮影時点は別取得である。独立画像レビューでも3状態と木枠／水面の分離を確認し、追加paint修正は
不要と判断した。raw index3/39/75は同じownerのEmpty/Partial/Fullで、水面Hidden→高さ12→高さ22と
画像変化が整合するが、完全な同時刻証拠ではない。EmptyのConstruction Complete表示は上縁・内面を
遮り、その部分の美術評価は未達。水のfixture投入やMudMixerのfixture task/state操作は通常入力の証拠ではない。
両passはfeedback技術観測だけで、正式art/numeric freeze、ordinary-input給排水・精製lifecycle、
全leg/性能、promotion/install/rollbackや通常起動の本設導入は引き続き未受入。
2026-10-12再開: 撮影前sampleだけへの結合を修正する。M2 runnerは撮影後のraw prefixを再取得し、
前後でsampleが進み、全区間でstate/pause/camera/consumer表示・identity・fixture owner・root owner・
world epochが不変な画像だけをcheckpointと最終画像へ採用する。途中で変わって戻る区間も拒否する。
不安定なcheckpoint画像は理由付きで診断保持し、同名で上書きせず最大48試行／24採用画像へ制限する。
最終画像も最大8試行の別pathへ撮り、拒否画像と理由を上書きせず保持する。
session/prefix改変は再撮影で隠さず失敗させる。新verifierは撮影後indexがない旧画像を受入証拠として拒否する。
これはrawの安定区間への結合であり、renderer frameと完全に原子的な撮影や通常入力の証拠ではない。
旧passはその時点の限定結果として維持する。fresh実機観測と独立画像評価を終えるまで本補正の実機結果は未達。
Help実経路: 子processの撮影・raw検証のみの変更で、通常起動、player input/UI、gameplay、保存、Helpは不変（No impact）。
撮影補正`9b1d518d5d9c07447fe73e777ddf2863478b3bec`のfresh 30秒batch
`tak14-mixer-capture-interval-9b1d518d-20261012-v1`は`native bounded stop/capture failed`でinvalid。
拒否画像と前後indexを保全し、同sampleの最終撮影213/223/232を拒否したことを確認、診断保持・finalizeした。
rawの公開間隔より撮影が短いため、撮影後のstrictly newer sampleを最大2秒だけ待つ補正を追加する。
timeoutでは不採用を維持し、prefix/session改変、区間内の状態変化やowner交替は引き続き拒否する。
待機補正`4e2a39635cfadee28a3a7fe4acf6d38ffd7e2c91`のfresh 30秒batch
`tak14-mixer-capture-interval-4e2a3963-20261012-v1`は最終画像を採れたが、unpaused Active画像が
状態切替を跨ぎ`required leg state/pause client captures missing`でinvalid。診断保持・finalize済み。
observerは0.1秒ごとに採るsampleを1秒おきにしか公開していなかったため、検証専用M2 observerで
各due sampleをそのまま公開する。サンプル／entity cap、失敗時のfail-closed、通常起動の無効化、
performance_evidence=falseを維持し、精製ルールやfixtureの材料・task・pause時刻は変えない。
公開間隔補正`85c7574f14d8da720758fe31fc6b95c0ade3c43e`のMudMixer/Tank fresh batchは
技術verifier pass・seal/finalize済み。ただし独立画像レビューでMudMixer Idle raw4..5の画像は
UIだけで中央viewportがclear色のままと判明し、Idleの視覚証拠として不採用とした。
paused Active46..49とunpaused Active62..65は木枠・紫表示・rotorが見え、rawの停止／進行と限定整合する。
中央160x160が単色の画像を採用・verify双方で拒否し、元画像・理由を保持して既存上限内で再撮影する。
この最小blank検査は対象の描画完了や美術採否を保証しないため、fresh画像の独立比較は必須のまま。
最終補正`0619cd65a741e64e082a049fbd10d31ef62d90fd`のfresh 30秒batch
`tak14-mixer-capture-ready-0619cd65-20261012-v1`と
`tak14-tank-capture-ready-0619cd65-20261012-v1`は登録済み独立verifier pass、seal/finalize済み。
同source `0d20a4d2d8fb744832076ff8a292568eb4638a962b6d1cb5f010432a4591fed8`、
同codec `9d846a9b7df7c804dfaf29eb9b01d0960e4d87ca836af0be9bd66872b0b60e84`で逐次実行した。
独立画像レビューでもMudMixer Idle5..6、unpaused Active37..38、paused Active43..45の3画像に
terrainと対象を確認し、同owner2187v0／epoch0と停止・進行の限定整合を確認した。
Tank Empty6..8／Partial39..41／Full78..80も対象が見え、同owner2186v0／epoch0で
水面Hidden→高さ12→高さ22と画像変化が整合する。startup blankはなく、撮影補正の限定受入を閉じた。
ただしConstruction Complete表示の上縁遮蔽、MudMixer紫表示下の泥とitem重なりの判読、
通常入力lifecycle、正式art/numeric freeze・性能・promotion/install/rollback・通常起動導入は未受入。
修正の独立コードレビュー、Python tooling 437件／asset tooling208件、変更別CI contracts/tooling、
observer変更時のworkspace test・Clippy・dev.py checkは成功。HelpはNo impact。
診断が終了した本修正の旧4jobは結果・制限を本節へ集約して整理し、最新2jobだけをM2候補画像比較用途で保持する。
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

2026-10-12継続: 9種の正式導入を完了対象として残す。独立候補監査は完成形の部分評価のみ可で、
全候補の正式art承認は未達。通常worldのArchitect開閉を実入力・実UI状態・所有画像で収集する
固定scenarioを追加する。ゲームを変更せず、MenuState／PlayMode／TaskModeをprofiling observerへ読取り追加する。
defaultのF12 probeは維持し、別profileだけB→EscapeでHidden→Architect→Hiddenを検査する。
独立コードレビュー、fresh実機／画像review、Help No impact・変更別CI・storageを通すまで新経路は未受入。
これはcatalog開閉の収集基盤であり、正式identity、配置・完成・全lifecycle／性能、releaseの代用ではない。

2026-10-12実機: `tak14-architect-input-da3da1c4-20261012-v1`はinvalidとして正式seal/finalize。
B/Escapeと同epochのHidden→Architect→Hiddenは実rawで確認したが、TaskMode単体resourceの
読取りはnull、catalog画像にはpanelが未表示だった。失敗画像/rawを具体的診断用途で保持する。
通常操作が参照するTaskContextへ観測先を補正し、panel表示・layoutの2連続sample成立を撮影前に
要求する。失敗をpassへ読み替えず、修正後のfresh sourceで再検証する。9種正式導入は未完。

`tak14-architect-input-50ae73ff-20261012-v1`はexit0でも独立画像reviewでchanges_requested。
開いたpanelは表示されたが、raw Hidden直後のafter画像にpanelが残ったためinvalidを正式記録。
閉鎖側にも2連続Hidden／layout非表示と撮影後fresh sampleまでの安定区間を要求する。
Rustの観測先補正は成立しており、この追加補正はPython撮影driverと拒否系testだけで行う。

`tak14-architect-input-a47fa8ab-20261012-v1`は開閉の画像が成立したがbeforeが全面黒だった。
独立画像reviewで3画像全体はchanges_requested、invalidとしてseal/finalizeして診断保持。
既存M2のnonblank検査を開始・開・閉の全画像へ適用し、開始はfresh Hidden区間を拘束する。
blankだけを8秒・16回以内で再撮影し、拒否画像・理由を保持する。別batchの再launchではない。

`tak14-architect-input-32b0bd72-20261012-v1`はfresh sourceで3画像がnonblank、通常worldの
開始／Architect表示／閉鎖が実画像と同epochのrawに整合し、独立画像review・登録verifierで
限定technical pass、seal/finalize済み。固定B/Escapeは実入力であり、建物選択・配置は未実行。
このjobの画像/rawだけを次の収集producerの対照として保持し、primary sourceやbinaryは凍結しない。
次はprofiling observerに実際の建設cardの可視・clip済みphysical rectとBuildContextを
read-only追加する。既存UiIntentと実layoutから対象を解決し、hidden ancestor／未layout／
重複・過大集合を拒否する。座標の推測、worldや施工結果のseed、Bridge対象化はしない。
その後、所有windowの実pointer操作と選択後のauthoritative stateを同区間で拘束する。
選択観測の準備だけではordinary placement/lifecycleや正式導入を完了扱いにしない。
独立reviewで変形UIのrect不整合を指摘され、finite translation＋identity matrixだけを受理する
最小補正と拒否testを追加。原MouseButtonInputのbutton/state/windowも状態由来入力と分離する。
原因解消済みの旧3診断jobは正式consumer解除後に生成物だけを整理し、du 11,964,416 bytes減。
invalid metadata・コード・原本・現行比較証拠は保全し、primary storage checkはpass。
観測追加`b94e88a3`は独立read-only review、profiling観測15 test、変更別CIのcontract/tooling/Rust、
通常・profiling Clippy警告0、Help No impactとdev.py checkを通過した。
次の選択feedbackは既存debug cacheを利用する別profileとし、正規validationの所有／source／
plan／driver／binary hashを維持する。9種の実cardだけを固定集合として扱い、pointerの製品ACK、
原MouseButtonInputのprimary window、選択後modeを検査する。通常worldをseedせず、正式採否を付与しない。
`tak14-equipment-tank-feedback-2be539bb-20261012-v1`はクリック前にcard inventory空でexit1、
invalid seal/finalize済み。実Architect PNGには12 cardが表示された。Bevy 0.19の
`ComputedNode::resolve_clip_rect`と実`Overflow::scroll_y()`から、Visible横軸の正規±∞を
観測器が拒否する前提誤りを確認。外向き無限clipを有限node／viewportへ交差させ、
NaN・内向き無限・反転・空・変形を引き続き拒否する。値調整やUI変更では解決しない。

`c21b916a`のfresh feedbackでは9 cardの観測が成立した。一方、移動前はゲーム外の
X11 cursor／製品cursor nullで、相対motion要求[663,563]に対し製品cursor[596,575]。
クリック前にtimeoutでinvalid、正式seal/finalizeして座標経路の診断用途で保持する。
経験的なoffset補正や到達条件の緩和は行わない。既存pointer／keyboard consentで
RemoteDesktopのConnectToEISを一度だけ使う限定equipment profileを追加する。
libei senderは単一seat・一意なactive device・単一virtual region・同monitorのphysical
extent/scaleだけを受理し、pause/removal/monitor変更は拒否する。EIS接続後のNotify混用、
入力再送、未知regionの推定は禁止。原inputと製品の厳密なcursor ACK・選択state、
独立画像review・登録verifierは維持する。実機受入前であり9種のreleaseは未完。
初回独立コードreviewはmonitor変更時の保持入力cleanupを指摘。releaseだけを通常guardから
分離し、原deviceの押下記録を解放してcloseする補正とpause／revocation拒否系testを追加する。
`f16d3e87`のfresh EIS実機は接続・device取得後、physical region一致guardで入力前拒否。
input_receiptsは空、invalid seal/finalize・診断保持済み。拒否にactual rect/scaleとexpected値を
含めるだけの観測補正を追加し、同じ拒否条件を維持して次のfresh実測で単位を確定する。
`8709e085`のfresh入力前実測はEIS logical rect=(0,0,1440,900)、scale=2、X11 physical
root=(2880,1800)を記録してinvalid精算。一次libei APIのlogical pixel契約に従い、
declared scaleとexact logical extentが一致する時だけphysical targetをscaleで割る。
任意倍率の逆算や到達ACKの緩和はしない。半logical pixelも丸めず、ゲーム側のexact
physical cursor ACKを維持する。独立review／fresh実機前で9種の正式導入は未完。

`8b5db4f6`のfresh EIS feedbackはphysical cursor [663,563]、原Left入力、
Tank選択、Escape解除をrawで確認した。closed capture直前の2 sampleのうち
先行sampleがHidden/BuildingPlaceだったためstrict verifierが拒否し、invalid精算済み。
待機側だけを2連続Normal／task None／選択解除済みへ補正する。撮影interval verifierは
変更せず、別subjectのfresh撮影と独立reviewで確認する。正式アート／releaseは未承認。

`ffded26d`では選択・解除rawとclosed intervalは成功したが、独立画像reviewは
catalog.pngのpanel未表示を拒否。exit0を採用せずinvalid精算して診断保持した。
layout readyとOS撮影の表示時点は同一とは限らない。限定equipment profileの撮影に
theme正本のdefault card色とraw rectによるpanel存在確認を追加し、最大8回の撮影だけを
試す。hover／tooltipを除く7 card以上を要求し、失敗画像を保全する。入力再送なし。
これはpanel存在の補助gateであり、文字／アート採否や独立画像reviewの代用ではない。

`b9ac0823`のfresh Tank実機はカタログ実表示／Tank配置説明／Escape閉鎖を独立画像reviewで
確認し、原MouseButtonInput・同window/epoch・選択stateと登録verifierもpass。限定UI選択技術証拠。
選択時の紫cardは移動後cursor下のBonePile hoverであり、Tank採否やselection色の証拠にしない。
原因解消済み6診断jobはconsumer解除・使用process不在・原本不在を確認し生成物だけ削除。
du合計24,985,600 bytes減、台帳metadataと採用コード・原画・最新証拠は保全、storage pass。

次のordinary placement producerはUI処理が実際に得た成功／拒否／Tank companion待機を区別する。
profiling観測sessionがある時だけbounded receipt queueを有効化し、実owner/footprint、
grid/kind、原primary windowとreal timeを記録する。Tank親の仮生成を成功と扱わず、
companion成功時だけ親子の同時commit、失敗時はrollback済み拒否を記録する。
SoulSpaの4tile ownerも実spawn結果に拘束する。ordinary input rawとは別のdomain結果として
結合し、state deltaから成功を推測しない。ゲーム配置条件／spawn順／saveを変えず、
材料配送・cancel/refund・move・save bytesやhost metricsまで受入済みとしない。

次の限定feedbackは`architect-select-place-cancel --kind Tank`。原UI選択後の単発world入力と
Rejected／CompanionPending、Blueprint不変、Escape解除だけを確認する。committed ownerや
正式lifecycle成功を許可しない。producer／入力probe hashとfresh subjectを固定し、
実機raw／画像の独立reviewと登録verifierを通す。正式9種導入の未完条件は維持する。

`5c48a6de`の実機は原Left入力と同frame/timeのTank／NotInYard拒否、Blueprint不変、
Escape解除を実見。ただしbefore画像の地形未描画を独立reviewが拒否し、batchはinvalid精算。
非blankだけではactor／光で偽陽性になるため、normal startupの固定map内viewportに
正本ClearColorが過半残る画像の補助拒否を追加する。撮影だけ有界で再確認し原失敗を保持する。
storage subject fingerprintとnative plan.sourceは別digest体系であり同一hashと記録しない。

`1b529faa`のfresh実機は未描画startup原画像を拒否・保全し、地形描画後の開始から
カタログ／Tank選択／NotInYard拒否／Escapeまで全5画像とrawを独立review、登録verifierで
限定pass精算しstorage pass。正式配置・親子commit・9種releaseは未受入。
成功経路は既存Yardの位置と現カメラprojectionを実観測して通常操作で到達する。
`placement_context`は最大32の実Yard boundsと単一active MainCamera projectionを読み取るだけで、
新Yard・ownerのseed、カメラ強制移動、入力・受入authorityを追加しない。missing projectionはnull。

既存Yardへの次の限定recipeは`architect-select-place-yard-cancel --kind Tank`。
実target window／logical viewport／WorldMap grid対応を追加し、必要なcamera移動を原WASDで
各軸1回・4秒以内、解除まで観測する。Yard trial点はmap生成を変えず実投影と同一contextに拘束。
Yard内の最初のTank clickはCompanionPendingだけを検査し、Blueprint commitはしない。
正常配置の次の親子commit・占有・材料配送・完成・move/save/cleanupと性能・正式releaseは残す。

`7fc5697b`のfresh実機では通常D→W入力で既存Yardへ到達し、grid `[73,60]`で原Leftと
同sample37／time `7.633592636`／window `69v0`／epoch0のCompanionPendingを取得。
全6owned画像と拒否startup原画像・rawを独立reviewし、登録verifierで限定pass封印／finalize、
primary storage pass。Blueprintは空のままであり、正式導入の承認ではない。
次の`architect-select-place-companion-cancel`は実pending stateとBucketStorage root／map占有を
読み取り、通常2回目のクリックによる親Blueprint・2childの計画確定だけを検査する。
既存world／材料／actor／建設結果のseedはせず、完成・性能・art／numeric・releaseは未達のまま保持する。
