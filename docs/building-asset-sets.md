# 建物アセットセットの読み込み

制作方針は[building-art-direction.md](building-art-direction.md)、移行順序は
[移行計画](plans/3d-rtt/non-wall-floor-building-art-migration-plan-2026-09-19.md)を正本とする。
本書はroot `bevy_app::assets::building_asset_set` が所有する入力境界を記す。

## 2026-11-13 非橋9種の正式導入・受入

Tank、MudMixer、RestArea、SoulSpa、WheelbarrowParking、SandPile、BonePile、OutdoorLampは、
個別の独立numeric/art/release判断、隔離promotion/install/rollback rehearsal、本正本へのpromotionとinstallを完了した。
通常binaryの`default_release_bindings.json`はこの8件を使用し、既存Door g7を合わせた9種を採用する。
Bridgeの正式asset・専用lifecycle・通過・性能受入は別工程のままで、全10種の完了を主張しない。

| kind | generation | ReleaseApproved manifest SHA-256 |
| --- | ---: | --- |
| Tank | 1 | `3ecc40fd65f2833fb95860ef31748600c0bc58369ed7dfe6afb2f4cc8b66de0c` |
| MudMixer | 1 | `fe47d51a7689707575611efd6d6440534294d005e8b6a41b9d15339e28c07047` |
| RestArea | 1 | `2c9ab0e7b36d19550c619773a8e0aceb69bf5d1c55ea3ac86c11974eefa80add` |
| SoulSpa | 2 | `cbce26f0fa8342b0b9101b5bc6da4c08d8f535535fd3d1d27a3238ced80bcdf1` |
| WheelbarrowParking | 2 | `a0a848dafecdbb9fd88acb1dc244e3ea4eee0c6c4148a6901c6df41276344e21` |
| SandPile | 1 | `3ca9f5523b1009b70da32052e9b52a6e5b2c07a9637dc42449aff13837fb0a66` |
| BonePile | 1 | `e94b7740741eaf349d2dfa6b5217d160796519b1c24ad5689f86cc63af5c6bc1` |
| OutdoorLamp | 1 | `26b2f804e2e34c72d0d37ada6aa873ecd79abf7af83fc5618377f7b98b25a370` |
| Door（既存経路） | 7 | `4e3f9236db067772b9a60b37ea98437cb29e481f9ec9cbb4d6dcf6517374a449` |

既定8件のinventory SHAは`87866aaa1734ad1445f21765053dbeb56e2e0205f383540bdf67c381a98d073a`。
支持物は既存Wall g13を維持する。M2/M3/M5の通常startup auditを群ごとに採用済みで、
初期worldに存在しないownerを生成・表示済みと扱わない。groupごとの通常lifecycleとアート採否は
以下の個別記録が所有し、静止画像だけから自然施工・全物流の追加成功を推論しない。

最終受入は、正式9種galleryのHigh/DPI1・normal/far（36 ownerとWall/Floor/Soul/実携行icon）、
Door EW/NS/tieの3独立worldでのcatalog/ghost/実配置Blueprint、J1の支持壁変更・撤去復元・paused loadを
それぞれ原raw・所有window画像と独立verifierで採用した。galleryとJ1の明示seeded contextは自然施工の証明ではない。
`65117e02`、runtime source `cfae3fced24f254a3249222539bf9b4b07ea7943e20a476bf1adfa83b848f31f`で
fresh Capture12→Memory12の固定予算比較と、代表EWのpause/input/resume回帰も独立採用した。
性能の数値と測定範囲は[性能仕様](rendering-performance.md#非橋9種の最終累積静止性能受入2026-11-13)、
採用画面は[制作仕様](building-art-direction.md#非橋9種の採用画面2026-11-13)を参照。

復旧はcanonical exportsのjournal/pointer/locator/receipt/payload/provenanceを正本とする。
8件すべてで、製品側へ保全したexact codec `601b0fd770b29d00cef29fc47c21e8c8353f0315fad80c40812be37f0d0646ce`を使い
`recover`は`applied`、`rollback`は`planned`を`--apply`なしで確認した。
実rollback/reapplyや新たなpromotionは行っていない。初回導入の旧authority不在へのrollback契約を維持する。
終了済みtransactionは元candidate jobを復旧依存とせず、正本の検証とcodec SHAを引き続き要求する。
原本とcodecの配置は[アセット運用](assets_workflow.md#非橋9種の制作原本と復旧情報)を参照。

以降の段階別履歴にある「未完」「保持中」はその記録時点の結果であり、現在の非橋9種の採否は本節を正本とする。
旧invalid・未測定の結果をpassへ変更せず、用途終了後の原本pathを将来の実行義務にも扱わない。

## runtime契約

`.buildingset` のschema、authority、依存ファイルの実バイト数とSHA-256を検査する。
`StartupPlugin` はasset型・policy resource・loaderを登録し、`VisualPlugin`はkind単位のpoolと
表示更新systemを登録する。通常起動はコンパイル済み
`default_release_bindings.json`の完全なrelease組を、既存poolへ要求する。
現在の一覧には独立release承認・promotion/install済みのTank、MudMixer、RestArea、SoulSpa、
WheelbarrowParking、SandPile、BonePile、OutdoorLampの8件を採用した。
M5を加えたcompiled8は`078908ea`の全4群CI・独立reviewと通常startup確認を通過した。
後続の正式9種画像・Door・累積性能受入も上記の最終採否で完了した。
独立release承認・promotion/install後に `building_asset_pipeline.py bindings` のexact出力を採用して初めて
既定起動へ反映する。一覧は起動時の文書/外部JSON読取りではなくbinaryへ組み込む。
`HW_BUILDING_ASSET_RELEASES`を明示した場合は一覧全体を置換し、空文字・空配列・非UTF-8・不正JSONを
既定値へ縮退させず拒否する。既定/明示ともM2の2件、M3を加えた4件、M5を加えた8件、
将来Bridgeを加えた9件の完全な組、各kind 1件の`release_approved` identityとcanonical locatorだけを許す。
候補/群gallery/ordinary art/M2/native UI・save/load・deconstruct・notification/production観測、
および有効なperformance sessionでは暗黙一覧を選ばず、専用sessionの明示入力を維持する。
profiling featureだけを有効にした通常起動は既定値を使う。
rollbackでは既存locator/receiptと対応する旧binaryの一覧を同時に復元し、別世代の混在を導入しない。
既存Wall／Doorのloaderと表示経路は継続する。Bridgeのschema・pool接続は実装済みだが、
正式asset導入と固有lifecycle/native受入は別工程であり、実装だけでは承認しない。

manifest loaderの`Loaded`は**manifestと依存バイト列の検証済み**だけを意味する。
M1-a2のpoolはGLBの先頭primitiveを`Mesh`、PNGを`Image`として要求し、依存loadとpreview寸法を確認して
pendingをactiveへ切り替える。M1-bではactive descriptorを3D設備、2D表示、配置・移動preview、
Blueprint、pulse、カタログへ公開する。通常consumerは`release_approved`だけを許可する。
M1-c候補はoffline codec／export、明示profiling候補投入、昇格・install・復旧・切戻しを追加した。
これはTAK-14候補の実装契約である。2026-10-04に通常Orcaのprimary由来の
`codex/tak14-product-integration`へ製品sourceを選択取り込みした履歴がある。
その統合事実だけでは独立review、実機受入、正式asset公開、美術受入の完了を意味しない。
正式assetと状態別のwater高さ・rotor軸等の制作値は、制作時に制作側と照合して確定する。

### M2の正式世代（2026-11-13）

| kind | generation | ReleaseApproved manifest SHA256 | 固定制作値 |
| --- | --- | --- | --- |
| Tank | 1 | `3ecc40fd65f2833fb95860ef31748600c0bc58369ed7dfe6afb2f4cc8b66de0c` | Partial Y=12、Full Y=22 world unit |
| MudMixer | 1 | `fe47d51a7689707575611efd6d6440534294d005e8b6a41b9d15339e28c07047` | 回転軸 `[0,1,0]`、1.5707964 rad/s |

正本は `~/Sync/hell-workers-assets/exports/`、runtime mirrorはprimaryの`assets/`。
両正本transactionを先にappliedへ進め、個別正式native原本のlive検証を終えてからmirrorした。
各rootは`transactions/building-{tank|mud-mixer}-1.json`、authority、locator、receipt、
原本・geometry・art/numeric approvalのprovenanceを持つ。独立release decisionはcandidate identityと
個別正式manifest digestへ拘束し、performance/group galleryの技術passから自動生成しない。
同じ採用bytesの隔離promotion/install/初回rollbackを実行し、locator/authority不在への復帰と
immutable payload/provenanceの保持を独立確認した。本正本のgeneration1はrollbackしていない。

同sourceの群別paused比較はN/4N各3対、Capture→Memoryの全24件が固定予算内で独立承認された。
これは通常worldの継続稼働性能や実GPU確保総量の測定ではない。旧lifecycleの限定採用履歴、
現sourceの表示同期回帰、個別画像と群18条件を分けてrelease判断へ使用した。
M2の通常起動は`14d9c813`のowned実機でcompiled_default選択・2pool activeと依存residentを確認し、
独立review・変更別CIを通過した。初期worldの対象ownerは空であり、全consumer表示の証明ではない。
群の導入進捗、残るDoor監査・正式9種画面・累積比較は
[最終累積性能結果](rendering-performance.md#非橋9種の最終累積静止性能受入2026-11-13)で管理する。

### M3の正式世代（2026-11-13）

| kind | generation | ReleaseApproved manifest SHA256 |
| --- | --- | --- |
| RestArea | 1 | `2c9ab0e7b36d19550c619773a8e0aceb69bf5d1c55ea3ac86c11974eefa80add` |
| SoulSpa | 2 | `cbce26f0fa8342b0b9101b5bc6da4c08d8f535535fd3d1d27a3238ced80bcdf1` |

M2と同じ正本へ両promotionを完了してからprimaryへinstallし、正規`bindings --release-group m3`の
exact4件をcompiled inventoryへ採用した。正本・runtimeは各transaction、authority、locator、receiptと
原本・geometry・art/numeric approvalのprovenanceを保持する。隔離された同bytesの8操作で、
両promotion→両install→両rollback→両mirror rollbackと初回preimageへの復帰を独立確認した。
本正本をrollbackして再applyしたものではない。

`14d9c813`で正式個別2画像と群18条件を確認し、既存の限定lifecycle採用履歴・現表示回帰、
独立に固定した群別予算とCapture→Memoryの24試験を合わせ、個別manifestに拘束した独立release判断を得た。
計測result SHAは`738e31ae69f1d32f9d40654ff111f9dbec5ef913dfa316bd1b28d176921dcd2f`。
N/4Nのp95増分は+0.145793/+0.014279ms、p99は+0.268455/+0.289262ms、
Rust peakは+4041624/+4932072B、RSSは+13208/+41852KiB、最大相対MAD2.923%。
Mesh+3/41816B、Image+7/4194304 CPU・descriptor推計B、material+4/1856 shallow Bは
事前見積りと一致し、初終・N/4N・instrument間で共有資源がplateauした。
通常worldの継続稼働性能や実GPU確保総量の合格には読み替えない。
RestAreaの利用・Dream・cooldown、SoulSpaの4区画/利用枠・worker状態・save/load契約は変更しない。
compiled4は`eebb55c5`の全4群CI・Clippy警告0・独立exact reviewと通常startup確認を通過した。
startup manifest SHAは`70e1371a58a89046d1fdf368f65ac4cff0cfe2e65df4cd8860a45e5052d9d6a3`。
compiled_defaultの4pool active/依存常駐を確認したが、初期対象ownerは空で全consumer表示の証明ではない。

### M5の正式世代（2026-11-13）

| kind | generation | ReleaseApproved manifest SHA256 |
| --- | --- | --- |
| WheelbarrowParking | 2 | `a0a848dafecdbb9fd88acb1dc244e3ea4eee0c6c4148a6901c6df41276344e21` |
| SandPile | 1 | `3ca9f5523b1009b70da32052e9b52a6e5b2c07a9637dc42449aff13837fb0a66` |
| BonePile | 1 | `e94b7740741eaf349d2dfa6b5217d160796519b1c24ad5689f86cc63af5c6bc1` |
| OutdoorLamp | 1 | `26b2f804e2e34c72d0d37ada6aa873ecd79abf7af83fc5618377f7b98b25a370` |

`2b3828df`のfresh個別4画像・群18条件と、planning履歴／現表示同期回帰、
独立numeric/art採用と固定予算の性能比較を合わせ、4件それぞれに独立release判断を得た。
control12の最大MADは1.9716%。候補比較前に理由・12指標を変更せず採用した予算SHAは
`827b76fc340eedc08853ad710789dea41370cec3454071fbf5383f5a9401201d`。
paired24 result SHAは`e95fbb0f64aced13cc8f4c19469dbc5489f9a34498ef3cfcf2ff0c6ed6f0bca5`。
N/4Nのp95増分は+0.159012/+0.051943ms、p99は−0.013465/+0.066670ms、
Rust peakは+2492835/+3180489B、RSSは−27912/+69124KiB、最大相対MAD2.776806%。
追加は9枚の256² RGBA8画像、2359296 CPU・descriptor推計Bで、追加Mesh/material/accessorは0。
全共有資源は初終・N/4N・Capture/Memory間でplateauした。通常継続AI・全GPU確保総量の合格とはしない。

隔離16操作（全4promotion→全4install→全4canonical rollback→全4mirror rollback）で
初回preimage不在への復帰、payload・4provenance・receipt・journalを独立確認した。
その後、凍結sourceのまま本正本へ全4promotionを完了し、各applyの個別art原本検証を終えてから
旧live-source依存を閉じ、primaryへ4件をinstallした。本正本の世代をrollback/reapplyしていない。
既存runtime bytesは全て保持し、資源・配電・駐車・anchor・save/loadのgameplay契約を変更しない。

正規`bindings --release-group m5`出力8件（SHA
`87866aaa1734ad1445f21765053dbeb56e2e0205f383540bdf67c381a98d073a`）をcompiled一覧へ採用した。
`078908ea`で通常startup manifest SHA
`bdf9884c9460fdd3341aec32f0a7fc8f4309062bca3d72db5220e5ea654571c7`を独立採用した。
全8poolがactive/依存常駐、初期Parking ownerとchildの採用画像接続を確認し、`visible=false`を保持した。
初期worldにない他consumerやParking表示成功を主張しない。当時の後続だったDoor監査・正式9種画面・累積比較は上記最終採否で完了した。
既存Door g7は別loaderのまま、Bridgeの正式導入は対象外とする。

### 通常起動defaultの受動audit

profiling buildでも専用session/performance/明示overrideを使わない通常起動は、非空のcompiled
inventoryを選んだ場合だけ `BUILDING_DEFAULT_RELEASE_AUDIT` JSONログを残す。
選択元compiled_default・inventory bytes SHA・完全bindings・game PIDを一度記録する。
実poolのactive/descriptor/依存Mesh・Image residencyが全件一致し、pending/failureがなく60frame
継続した場合だけactiveを記録する。load failure/120秒timeout/観測上限は別の診断で、ゲームを修復・
承認・強制終了しない。空default、明示override、専用session/performanceではこのauditを登録しない。

active時には実在する通常ownerのroot/leaf identity・依存handleと2D Spriteのworld imageも記録する。
実Building owner inventoryを先に採取し、実在3D ownerごとのroot exactly1と実在2D ownerごとの全Sprite targetを
独立側で全件被覆する。2D targetは既存publisherと同じowner Spriteとdirect VisualLayerKind childで、
childのSprite欠損もtargetに残す。ownerごと1件以上、owner/parent/layerの一致、重複/余剰/欠落拒否と有限capを要求する。表示consumerが欠落したownerを、world自体に存在しないkindと混同しない。
その時点で存在しないkindを生成したり、全consumerの成功と見なしたりしない。
`scripts/building_default_release_acceptance.py` は正規validation launcherから、専用envもperf argvも
release overrideもない通常binaryを起動し、exact installed pointersとcompiled JSONの一致、
所有X11/Vulkan画像・ログ・binary copy・sourceを独立照合する。private asset viewは正式install済みassetsの
同bytesコピーだけで、locator・候補・fixtureを追加しない。admissionで読んだplanを凍結し、build前後・
起動前・最終verify前および原本の独立verifyで完全一致を要求する。変更は別job rootや起動へ進めない。初期世界にないconsumer、全kind lifecycle、
アート・性能・release採否はこの接続検証の対象外で、通常authority storyboard等の別証拠へ残す。

### 正式9種の受入ツール契約

`building_release_art_acceptance.py`は全8 ReleaseApprovedと既存Door g7だけを受け付ける
独立gallery入口で、High/DPI1のnormal/farを各owned windowで観測する。
`HW_BUILDING_RELEASE_GALLERY_SESSION`は閉じたprofiling契約であり、候補許可や通常gameplayの設定ではない。
36 ownerのmesh/AABB・画像alpha投影を完全照合する。壁・床・Soulと実InventoryのWood携行iconは
明示seeded contextで、自然施工・物流の証明にはしない。Floorと携行準備は未公開段階の各1回のみ。
その後の既存30frame安定とcontext60frame安定を取り直し、probe/ACK/final公開後の修復は拒否する。
通常group・performance側のreadinessを巻き戻す入口にはしない。
専用のFloor witnessは唯一のowner・非provisional kind・pose・独立floor層の一致を検査し、
Transform欠損も含む全Building inventoryでcore/support＋当該1件だけを許す。通常group・performanceは
従来core/support数のままで、任意のFloorや余剰ownerを除外しない。
追加Soulは既存生成systemへ通常messageを送る。生成resolverのradius5先頭走査が宣言した対象cellへ
到達するrequestを送信前に検証し、生成後にも実位置・idle task・空Inventory・無関係を照合する。
terrainや既存Soulを動かさず、生成規則を変更しない。
停止中は通常LogicのInventory mirror同期も止まるため、未公開の実pickup直後に既存同期systemを一度だけ呼ぶ。
携行Sprite自体は通常Visual systemへ任せ、公開後や重複準備での再同期・修復は許さない。

`door_release_ui_acceptance.py`は既存Siteの空3×3を選び支持壁2/2/4だけを一度準備する。
Door本体・Blueprint・施工完了を生成せず、実OS入力のcatalog・EW/NS/tie ghost・同owner Blueprintを検証する。
producerはpre-armから毎frameの入力を保持し、hostはarm nonce/PID/epoch/frame、EIS client、実placement receiptと
4枚のowned画像を結合する。停止中のSelectBuild/配置禁止は維持し、empty/catalogをPauseで撮影後、
既存Normalボタン→Door選択→ghost→実配置→Escape→既存Pauseボタンの順に操作する。
各操作の前後Pause状態、時間ボタンの実Pressed ACK、全入力prefixを照合し、resume前からpause後ACKまでを
20 real秒以内に制限する。最終Blueprintは再Pause後のprogress0/material未完を要求し、自然施工を主張しない。
既存`blueprint_scale_animation_system`のprogress0 scale0.9・Z_AURA=0.2をBlueprintへ、
ghostのunit scale・Z_SELECTION=2.0をghostへ別々に拘束し、実world/local行列も照合する。
共通UI観測のviewportはVec2の有限数値としてexact1920×1080を検証し、bool・非有限・端数差を拒否する。
物理PNG/window/clientの寸法検査は別に維持する。
支持壁のseam・変更・撤去・loadは既存J1 release6の別受入とする。
J1のゲーム子環境は`RUST_LOG=info,wgpu=error`を固定し、親の抑止filterで実AdapterInfoを欠測させない。
独立verifierは原ログの単一adapter・Vulkan・WARN/ERRORなしを引き続き要求する。
各`--case`はEW/NS/tieの閉じた必須enumで、別々の正規batch/helper processが1 game/window・1 PortalSession・
1 ConnectToEISだけを所有する。前caseの完全verify/seal/保持/finalize後にだけ次を登録し、失敗や不明結果では停止する。
既存opt-in同意journalのconsume/成功時rotationだけを使い、revoked/unknown sessionへ再接続しない。
`verify-group`は順序付き3原本の完全verifyと同一source/binary/normal inventory、固有PID/nonce/sessionを照合する。
これは3個の独立worldの確認であり、単一worldや単一入力sessionの継続とは主張しない。

`building_cumulative_acceptance.py`は既存adjacent比較の閉じたprofileを使い、旧referenceと正式8世代＋Doorの
Capture/Memory各N/4N・3対を比較する。両sourceの完全raw oracleの成功後だけ固定の論理投影を比較し、
元presentation digestを残す。codecは最初の登録と保存済みSHAの再照合を分離し、実際のrelease transactionから
追加asset pathだけを導出する。旧計測harness・共通asset bytes・失敗履歴は変えない。
累積比較の旧資源補足は、承認済み`29e58ebf`のsource-only verifierから元の`verify()`をisolated子processで実行する。
原basis root/manifest SHA、固定commit・entrypoint、5module実SHAと旧harness、同Git所有・clean/read-only・
ignored bytecode不在を前後照合する。current計測のsource/harnessとは別にplanへ固定し、旧12run＋補足2runを
完全再検証する。旧共有mesh失敗・accepted=falseと固定予算は維持し、任意module/commandやcatalog置換を許可しない。
この専用checkoutはbuild/asset導入/agent起動に使わず、累積採用後に用途終了として撤去する。
各ツールの存在や合成fixtureの成功は、画像・実入力・性能・art/releaseの承認を与えない。

累積verifierは同じ公開操作内だけのprivate plan/release-context proofを使う。
最外操作の冒頭・終了で完全live source/assets/drivers/codec/歴史basisを照合し、
stage・build receipt・各caseのraw入力・台帳の変更もmetadataで拒否する。
全caseの完全raw oracleは省略しない。proofはCLI/JSONへ出さず、別操作・process間・永続cacheへ流用しない。
単独公開verifierは毎回完全検証し、CLIは最終照合後だけ成功結果を出す。
旧累積v3は個別24件の検証後、Memoryの精算間隔が約339〜347秒となり固定180秒で全体失敗した。
同じplanの入れ子再検証を減らすhost修正であり、旧失敗の時刻・予算・採否を変更しない。
修正後v4は間隔条件を満たしたがMemory N差21,430,227 bytesが16MiB予算を超過した。
停止中のwindow buffer蓄積を実TimePluginで再現し、通常runtimeのframe/runner所有3型だけを補正した。
修正後v5は固定予算のCapture12/Memory12を通過した。旧v3/v4失敗とart/releaseの独立判断は維持する。

### Bridge配置の成立条件（統合候補）

Bridgeの配置・完成・通行契約はasset poolの採否から独立する。配置resolverはlive地形の2列を読み、
両列の川が連続し、合わせた川幅が5タイル以内であることを確認する。既存2×5 footprintと
南北4岸セルについて範囲、建物、Stockpile、raw障害物を検査し、岸は川ではなく通行可能でなければならない。
previewとcommitは同resolver/geometryを使い、不一致・成立不能なら予定を作らない。
施工予定の予約だけでは川を通行可能へ変えず、完成・load・撤去は既存lifecycleの実データを使う。
このコードの取り込みはBridge専用asset・アート・通常配置/通過のnative受入を証明しない。

## 種別と必須role

roleは下表の順で、meshを先に、imageを後に並べる。余剰・欠落・重複・並べ替えは拒否する。
各artifactのrole文字列は`mesh:`／`image:`を接頭辞とする。

| kind | mesh | image | part | 代表状態 |
| --- | --- | --- | --- | --- |
| Tank | body, water | albedo, world_preview, catalog | body, water | Empty |
| Bridge | body | albedo, world_preview, catalog | body | Complete |
| MudMixer | body, rotor | albedo, world_preview, catalog | body, rotor | IdleAngleZero |
| RestArea | body | albedo, world_preview, catalog | body | Empty |
| SoulSpa | body, slot | albedo, slot_emissive, world_preview, catalog | body, slot0〜slot3 | OperationalMaskZero |
| WheelbarrowParking | なし | world, catalog | なし | WithoutVehicle |
| SandPile / BonePile | なし | world, catalog | なし | Static |
| OutdoorLamp | なし | world_off, world_on, catalog | なし | Off |

9種以外はdecode時に拒否する。Wall／Doorは別schemaを維持する。制作contract fixtureのrole・leaf数・代表状態との一致をtestで照合する。
SoulSpaの4 slotは同一slot meshを参照し、part側で個別transformを持つ。

## manifest契約

- `schema_version = 1`、`asset_set_id = building-<slug>-v1`。
- `identity`: kind、正のgeneration、authority、manifest_sha256。
- `source_sha256`、`export_sha256`、`geometry_contract_sha256`: 制作側の照合用identity。
  このloaderはhash書式だけを検査し、外部制作原本との照合は行わない。
- `art_approval_sha256`: art_previewではnull、それ以外では承認identityのhashが必須。
- `artifacts`: role、path、正のbytes、sha256。
- `parts`: name、mesh_role、material_role、translation_wu、rotation_xyzw、scale。
  GLB local originをpivotとし、移動量は既にworld unit。TILE_SIZEを再乗算しない。
  移動量は有限、scaleは有限かつ正、quaternionの長さ二乗は1からの差が0.0001以下。
  materialは通常`opaque_albedo`、SoulSpaのslotだけ`spa_slot`。
- `world_preview`／`catalog_preview`: image_role、canvas_px、canvas_wu、anchor_px、representative_state。
  canvasは正、world sizeは有限。anchorは左上起点のpixel座標でcanvas内。
  catalogは正方形・中央anchor、worldは上表のworld用画像（Lampはworld_off）を参照する。
- `receipt`: release_approvedのみ必須。それ以外はnull。
- `production_state`: Tankの有限な `0 < partial_y_wu < full_y_wu <= 2 * TILE_SIZE`、MudMixerの有限なunit axis
  （長さ二乗の誤差0.0001以内）と `0 < radians_per_second <= TAU`。art_previewと他kindでは存在してはならない。
- `numeric_approval_sha256`: M2/M3/M5のisolated_candidate/release_approvedに独立numeric decisionの小文字64桁hashを要求する。
  art_previewとBridgeには指定できない。M2/M3/M5 release receiptにも同じhashを要求する。
  production_stateとこのhashはmanifest内容hashに含まれる。安全な数値範囲を満たすだけでは独立採否にならない。
  optional fieldは未指定時にserializeから省略し、全art_previewとBridgeの既存canonical bytesを維持する。
  M3/M5はproduction_stateを持たず、保存・利用・Dreamのgameplay数値は変更しない。

JSONはRustの型宣言順に`serde_json::to_vec`したcompact形式＋末尾LFと完全一致させる。
未知field、別順序、余分な空白は拒否する。SHA-256は小文字hex64桁。
manifest_sha256は、identity内の同fieldを空文字にし、receiptをnullにしたmanifestを
同じcompact形式（**末尾LFなし**）でserializeした内容のhashとする。
receiptとの循環参照を避け、receipt自身は別途バイトhashとidentity一致で検証する。

artifact pathは`building_sets/<slug>/<generation>/<sha256>.<extension>`と完全一致を要求する。
slugはkindのkebab-case（例: `mud-mixer`）。extensionはmeshがglb、imageがpng、receiptがjson。
絶対path、親参照、URL、別asset source、GLB subasset label、別kind／generationへの参照は不可。
同じ内容のimage role間で同じpathを共有することは可能。

## authorityと読み込み順

1. manifestのcanonical形式、内容hash、role・part・preview契約を検査する。
2. loader登録時にroot resourceから取得した`BuildingAssetLoadPolicy`を検査する。
   `release_approved`は次のreceipt検査へ進む。
   `isolated_candidate`は`allowed_candidate`とのidentity完全一致が必要。
   `art_preview`はさらにprofiling build限定。既定policyでは両候補とも拒否する。
   `.meta`から権限を設定できず、登録後のresource変更でも既存loaderの権限は変わらない。
   導入前の9種静止比較だけは、別の`HW_BUILDING_M6_CANDIDATES`から検査済みexact8候補の集合を
   loader登録時に固定する。profiling/BuildingArtStatic限定で他sessionとの併用を拒否し、
   candidate側だけpoolへの読み込みと表示を許可する。通常起動とrelease bindingの許可は広げない。
   [静止比較契約](building-art-static-reference.md#導入前の9種混在比較)の別profile/sidecar/oracleで
   制作validator自体のhashと全asset viewを固定し、技術passをproduction authorityへ変換しない。
3. release receiptの実バイト数・hash・canonical形式を検査する。
   receiptはschema_version 1、manifestと同じidentity／art_approval_sha256、decision `release_approved`が必須。
4. 全artifactを`LoadContext::read_asset_bytes`で読み、実バイト数とSHA-256を検査してからmanifestを返す。

候補の権限不一致は依存ファイルを読む前に失敗する。欠落・破損・receipt不一致も失敗として返し、
loader自体はfallbackの変更や部分公開を行わない。
receiptは製品に同梱する承認記録の整合性検査であり、署名や外部承認機関の認証ではない。
正式な承認・生成器による昇格判断は下記M1-cの責務で、runtimeが承認を作ることはない。

## typed residencyと世代pool（M1-a2／M1-b）

- `BuildingAssetPool::request`はidentityとmanifest handleをpendingとして保持する。同kindで試行済みの
  generation以下を拒否し、`Started`／`PendingOccupied`／`AlreadyAttempted`を返す。
- meshは`Mesh0/Primitive0`、imageは`Image`として解決する。load／再帰依存失敗とpreview実寸不一致は
  世代失敗とし、既存activeを保持する。全typed assetが揃った時だけ一括で切り替える。
- 旧activeのstrong handleと失敗したpendingはdropし、pool内にretired cacheを残さない。
  `invalidate_active`はidentity完全一致のactiveだけを解除し、他kind／pendingは変えない。
- `PostUpdate`でpool poll、3D同期、2D／preview同期、`ApplyDeferred`をchainし、Transform伝播とUI prepareより
  前に完了する。後着generation、pause、新規consumerを同境界で揃える。world置換時はpoolを維持する。
  各同期systemのworld固有`Local`でQueryStateとscratch容量を再利用する。通常`iter/get_mut`で
  後着archetypeへ追随し、root/child/consumerの一覧と重複検査集合は毎call内容をclearする。
  owner indexや採否/stateを保持せず、paused・candidate未loadでも同期を省略しない。
  一時割当の削減自体をRSS予算の合格や正式asset導入と扱わない。
- Tank／MudMixer／RestArea／SoulSpaはmeshを持たない`EquipmentRoot`とpart childを使う。
  `Building3dVisual`はrootだけに付け、partのlocal transformをworld同期で上書きしない。
  generation交換はpart集合単位、失効時は従来mesh／位置／材質／状態表示へ戻る。
  owner消失・重複root・world再構築を整理し、水面・回転・施工状態・稼働maskは既存gameplay stateに従う。
  TankのPartial水面だけは低い位置の陰として専用の暗いmaterial tintを使い、俯瞰の通常倍率での
  Fullとの識別を補助する。判読性は変更後の実機画像で要確認。水量判定・数値高さ・共通albedoは
  変えず、世代ごとにmaterialを有限保持する。
- 小物4種のworld、8種のghost／Blueprint／pulse／移動先／catalog、Tank partner ghostを同期する。
  Lampは既存通電状態でoff/on画像を選び、旧gray tintを二重適用しない。image／size／anchorのbaselineを
  entityごとに保存し、失効／mode変更で復元する。再利用ghostは対象切替時にbaselineを更新する。
- 論理footprint、通行、生産、光源、save schema、Wall／Door／Bridge専用経路は変更しない。

## offline生成と明示候補投入（M1-c）

`bevy_app --building-asset-codec`は通常app／window初期化より前にJSONを入出力するoffline入口。
`project_building_asset_json`の`seal`／`validate`がruntimeのRust型・f32形式・canonical serializerを再利用する。
codecのreceipt生成は形式の整合性だけであり、美術・release承認を行わない。

Python側のart/numeric/release decisionのidentityはcanonical JSONで比較し、generationの
int `1`とbool `true`／float `1.0`を同一視しない。art原本・numeric原本・manifestのhashを
結び直しても不正型を正規identityへ読み替えない。M2 production_stateの数値にもboolを許さず、
既存int/float入力とRustのf32範囲検査を維持する。candidate evidenceとinstall時のrelease decisionも再照合する。

`tools/blender_ai_workflow/scripts/building_asset_pipeline.py export`は既存のrole別GLB／PNGとrecipeを
包装する。geometryや画像を新規生成しない。source／geometry／artifactの実bytes・hashとrole順を確認し、
content-addressed artifact、locator、provenanceを出力する。`art_preview`は承認なし、`isolated_candidate`は
独立した`building_art_approved`記録とpreview identity完全一致を要求する。exportではreleaseを発行できない。

M3のcandidateには別入力のcanonical `building_m3_numeric_approved` decision（schema_version=1）も必須。
fieldはschema_version、decision、art_preview_identity、art_approval_sha256、geometry_contract_sha256、
numeric_contractの6個だけ。geometryの実原本bytes/hashと、exact preview/art decisionへ拘束する。
`building_m3_numeric.numeric_contract`はversion 1 proposalの厳密なfield/list形状からunits、scale、
tolerance、tile順/中心、emissive multiplier、atlasと対象kindのgeometry/preview/role budgetを取り出す。
paletteとreject文は独立art側に残す。数値は有限なint/floatだけでbool代用・未知field・値の不一致を拒否する。
proposalのnumeric_freeze/art_approved/runtime_published=falseは書き換えず、decisionを自動生成しない。
独立採否の存在・真正性はreview運用の責務で、validatorやsynthetic testの成功は採否にならない。
export/load/plan/recovery/installはnumeric原本とdigestを保持・再照合する。runtime codecはmanifestと
release receiptのdigest一致を検査し、geometry原本の採否を新しく行わない。既存M2数値型/範囲は維持する。

M5も独立入力の`building_m5_numeric_approved`を要求する（同じ6field/schema_version=1）。
`building_m5_numeric`はversion 1 proposalの原本とexact preview/art decisionを照合し、
canvas_px/wu、anchor_px、catalog margin、world role順、代表状態を別numeric_contractへ拘束する。
実manifestのworld/catalog descriptor、空partsと画像role順も照合し、別値・別kind・未知field・
bool/floatの整数aliasやM2 production_stateを拒否する。export前に検査し、load/plan/recovery/installで
numeric原本・digestを保持/再検査する。未承認ArtPreviewは引き続き承認なしで扱えるが、M5 candidateと
releaseにnumeric digestがない旧形式は拒否する。この実装は4種の独立採否や正式releaseを発行しない。
新M5 candidateのnative planは`numeric_verifiers`へpipeline/M3・M5 validatorと参照contract原本の
相対path→SHA256 inventoryを固定する。共通game/harness hash外の依存であり、同じdirty pathの内容変更も
run/verify共通のplan再検査で拒否する。旧ArtPreview/feedbackにこの検査の成功を遡及付与しない。

`profiling` buildの`HW_BUILDING_ART_SESSION`だけが、mode・kind・generation・authority・manifest hash・
locator・nonce・status pathを明示照合してload policyとpoolへ候補を許可する。
`feedback`／`art-preview`は`art_preview`、`candidate`は`isolated_candidate`に限る。
poolのpresentation許可もそのidentity完全一致に限定する。通常起動のproduction bindingは
`HW_BUILDING_ASSET_RELEASES`のJSON配列で明示し、次のいずれか一つの完全な組だけを許可する。

- legacy M2形式: TankとMudMixerを各1件、合計2件。
- M2+M3形式: Tank、MudMixer、RestArea、SoulSpaを各1件、合計4件。RestAreaとSoulSpaは片方だけを
  追加できず、M2の2件を省略できない。
- M2+M3+M5形式: 上記4件にWheelbarrowParking、SandPile、BonePile、OutdoorLampの全4件を加えた8件。
- Bridgeを含む形式: 上記8件にBridgeを加えた9件。Bridge単独bindingや8件未満への追加は拒否する。
  Doorは別loaderであり、この9件に含めない。全対象10種の受入ではDoor固有監査も必要。

各identityは正のgeneration、小文字64桁manifest hash、`release_approved` authorityを持ち、locatorは
`manifests/building-<kind>-v1.buildingset`と完全一致しなければならない。対応9種以外、件数2／4／8／9以外、
必須kindの欠落、同kindの重複、未知field、非canonical locator、`HW_BUILDING_ART_SESSION`との併用は、
App/plugin初期化とpool requestより前に拒否する。未指定時はno-opで従来fallbackを維持する。
指定が妥当な場合だけStartupで配列内の全件をpoolへrequestし、一部だけを暗黙採用しない。
runtimeはこの入力から承認を生成せず、正式assetの導入と通常起動への有効化は別の明示工程である。

## native evidenceの範囲

`scripts/building_art_acceptance.py`の`plan → run → verify`を使用する。planはcommit／source／harness／
asset view／driver／codec／candidate identityを固定する。feedbackだけdirty sourceを許容し、正式候補証拠に
転用しない。art-preview／candidateはclean sourceが必要で、candidateは別途美術承認との照合も行う。
起動は既存native helperの資源guardとno-prompt launcherを使い、基準asset viewを隔離コピーする。

比較時の `--view overview|near|normal|far` は明示profiling sessionのカメラだけを選ぶ。
明示building-art sessionがある間は左の開発パネルも非表示にし、近景で先頭の
状態標本が隠れないようにする。通常起動とsessionなしのM6比較のパネル状態は変更しない。
既定overviewは従来の全体位置・scale 5を維持する。near／normal／farは候補kindの先頭4棟を中心に、
scale 0.5／1／2で固定する。対象36棟・状態・論理配置・時間pauseは変更しない。
overviewでは全対象のvisibilityを確認する。focused viewは候補の先頭4棟のvisibilityを要求し、
画面外の他種・後続棟についてはload／owner／geometry／状態inventoryを検査するが画面内表示を主張しない。
runtimeはview、camera center／scaleと30 stable framesを観測し、driverは要求view／scaleと照合する。
入力によるカメラdriftを毎frame修復せず拒否する。通常起動のカメラ、zoom操作、Helpには影響しない。
近景の画像取得も技術比較に限り、人による美術採否やordinary lifecycleの代用ではない。

検証対象は**pauseしたsmall静止fixture（36棟）のpresentationのみ**。exact identity／nonce、30 stable frames、
初期・終了inventory一致、owned X11 clientの1280×720画像、指定adapter／Vulkan、警告・エラーなし、
binary／source／asset／原本hash一致を検査する。jobは`promotion_authority=false`であり、自動美術承認ではない。
この証拠は稼働状態、save/load、施工・撤去・移動全経路、各kindの性能予算、全体移行完了を保証しない。
M5四種の新gallery planは `foreground_observation.schema=1` を明示する。
別Python観測moduleの実file SHA256もplanへ拘束し、共通harnessリスト外のmodule改変を拒否する。
実owner/子Sprite・parent・画像role/handle/原本artifact・descriptor canvas/anchor、実sRGBA、
local/global poseと親子合成matrix、visibility/layersを受動採取し、ready後の変化を拒否する。
offline verifierは型を保持したsource manifestとのcanonical JSON一致と独立TRS合成を照合する。
整数inventoryのbool/float aliasを拒否する。有限の非白色は診断値として保持し、
白へ補正したり美術的に正しい色と承認したりしない。旧planはこの追加証拠なしの履歴であり、
新schemaの観測済みと読み替えない。これは2D表示経路の切り分けで、明暗/密集/preview/UIや素材採否を閉じない。

明示 `--variant m2-art-context --view normal|far --lighting standard|dim` はTank/MudMixer専用の
feedback/ArtPreview/small比較で、他のsessionとの混在とcandidate/releaseを拒否する。
4対象と前後8Wall、8idle Soulを一度だけ準備し、Mixerの既存2Refining workerは別に数える。
原本parts/geometry digest、Tankの実水量0/25/50/50・capacity50・空水面hidden・Y12/22・既存Partial tint、
Mixer実Refining関係と停止時の原本rotor poseだけを明示検査する。未知の材質/pose差は許可しない。
M2のresourcesは全体asset数でなく当該setの全mesh/image roleと常駐handleを固定し、欠損を拒否する。
対象part/previewのhandleと独立に照合し、`global_pool_evidence=false`とする。全体資源pool/性能の受入ではない。
RestArea既存contextの全体counter不変検査は変更しない。
共通のcamera/RtT/descriptor previewと30frames安定観測を使うが、RestAreaの白材質/occupant契約は不変。
previewはEmpty/Idle代表画像であり、全状態のpreviewが存在する証拠ではない。
これは画像採否の比較材料のみで、UI/通常入力・自然利用・lifecycle・性能・独立採否・releaseを閉じない。

別入口 `scripts/building_m2_ui_acceptance.py` の `m2-ui-tank-empty|partial|full` と
`m2-ui-mixer-refining` は、seeded最大zoomの既存tooltipをOS EIS hoverで確認するfeedbackだけを扱う。
専用 `ui-art` legは合法な施工setupを共用するが、旧M2の時間駆動注水・再開・lifecycle driverを登録しない。
起動時の複数native profile禁止には、この専用M2 probe/UI observerだけの厳密な例外を置く。
同case/kind/nonce・probe=1・ui-art・feedback/ArtPreview/gallery/normal/standardをread-only照合し、
他scope・第三profile・性能scenarioは拒否する。後段の完全なidentity/path検証とload前guardは省略しない。
一度だけ実Water/StoredInまたは実Refining/WorkingOnを準備してpauseし、カメラscale5を設定する。
通常のhover resolver、tooltip生成、player文言は変更せず、static artのhover抑止も解除しない。
Tankは0/25/50の実Water正逆集合とcapacity50、Mixerはactual SoulのRefiningとexact mixer/task identityを照合する。
同source/identity、owned X11/window/nonce、EIS ACK、実Tooltip rootとText祖先/clip/alpha/UI camera、
Lastで公開した同観測、production root/mesh/材質/poseをcapture前後へ拘束する。
各caseはfresh登録jobとし、旧caseや他authorityとの混在を許さない。
これはzoom操作による最大値到達、自然施工/搬送/割当、selection、lifecycle、性能、独立art/numeric採用、
promotion/install/releaseの証拠ではない。通常起動へ新しいasset locatorやUI表示は追加しない。

2026-11-09の独立画像/数値監査と主担当判断で、M2の現原本を美術/数値だけ採用した。
Tankの空/半量/満水は既存Water tooltip、Mixerは実workerのRefining tooltipで最遠時の状態を補完する。
Mixer自身にIdle/Active文言は追加しない。canonical承認と原本/geometry/依存を内包した隔離候補は以下であり、
通常startupへのinstall・promotion/release・自然lifecycle・性能受入は未完のままである。

| 種別 | 隔離候補manifest SHA256 | 採用production state |
| --- | --- | --- |
| Tank | `6cf7e62bc7b8a9f75c31ab8eb8a036956e16e249f03b70a6e15e9757d4a5b12e` | partial Y12 / full Y22 |
| MudMixer | `b6b01fd6a79d780a1764239b3ea733f5e02b1dfd4335137004842028b5a564a0` | Y軸 / 1.5707964 rad/s（f32 bits `0x3fc90fdb`） |

回転速度の実f32値は1.5707963705062866で、manifest/承認JSONはruntime codecの正規表記1.5707964を使う。
全精度decimalの旧承認からは出力前のstrict比較で拒否されており、新承認digestでfresh候補を生成した。
数値を丸めて検証条件を緩めたのではない。独立監査・承認digest・失敗/成功と整理の履歴は
[現在の正式導入・受入記録](building-asset-sets.md#2026-11-13-非橋9種の正式導入受入)に記録する。

別の通常世界入口 `m2-mixer-candidate-planning` はmode `m2-candidate-planning` /
scope `ordinary-m2-candidate-planning-v1` に限定し、採用済みMudMixer IsolatedCandidateだけを許可する。
準備は合法anchorへのcamera/pauseのみ。実EISのcatalog→合法ghost→Blueprint→占有ghost→配置mode取消→
同Blueprint inspectionを、packaged art/numeric/production-state・入力/PlacementOutcome・実preview consumerへ拘束する。
実PlacementOutcomeのraw sampleを境界として、全途中sampleで配置前の対象不在、配置後の同owner/4tileの
未完成Blueprint1件と予約map、厳密型epoch/window・有限単調clockを検査する。通常の資材搬入は拒否しない。
初期完成Mixerや途中完成を拒否し、通常の資材/worker/task/水/完成を注入しない。
TankのCompanionPending/Committed、自然施工/搬送/精製、save/load、move/cleanup、性能/releaseは別工程である。
旧M5のArtPreview契約・旧seeded M2のfixtureをこのcandidate証拠へ流用しない。

通常観測のprofiling専用 `m2_context` はMixerのSand/Rock/Mud、Stockpile/StoredItems、
全ResourceItemの実種別とStoredIn/StoredByMixer/LoadedIn、Refine actorのtyped phase/progress・identity・
WorkingOn/TaskWorkersの両方向を有界収集する。欠損はnull/明示不在、非finite/overflowはfailureとし、関係を修復しない。
`ordinary-mixer-refine-branch-v1` は実Refining完了branchのWater消費ID・Mud生成5ID・branch内の
Sand/Rock各−1とMud＋5を、deferred適用後の消失/ResourceItem/StoredByMixer/Doneと照合して記録する。
WorkingOnは当branchで解除される。branch内counterと、同frameの別配送を含み得る最終snapshot counterを混同しない。
出力には既存の寿命があるため、後の個数だけを精製完了の証拠にしない。receipt resourceが無ければ記録せず、
このproducerやunit testだけでは実機の自然精製/lifecycle・性能・アート・releaseを受入しない。

Sand/Rock搬入は `ordinary-mixer-material-branch-v1` / `MixerMaterial` として、実手運び・猫車の
`add_material`成功branchでのみ捕捉する。実item型、格納/積載/所持の直前値、worker/assignment/target、
MixerとSand/Rock/Mudのbranch内before/afterを、deferred消費後のitem消失と同epoch/完成Mixerへ照合する。
手運びは所持item、猫車は同じ所持車両へのLoadedInを要求し、payloadだけの型推定や失敗の成功化を拒否する。
各itemの受入増分は1、対象外counterは不変、現在の材料capacity5を超えないことを検査する。
同frameの別配送/精製があるため、最終snapshot counterをbranch内afterへ一致させない。
満杯でのdropは成功receiptを作らず、猫車の部分投入は実消費できたitemだけを記録する。
Refineと共通の4096件上限とsticky failureを使い、同epoch/itemの再利用を独立parserが拒否する。
これは材料→counter変換の証拠であり、通常伐採/採掘/砂採取、配送要求の全lifecycle、帰還、自然精製、
全9種の実機・性能・正式releaseを証明しない。observerの非承認flagsとゲーム規則は変更しない。

水供給の受動inventoryは別の `water_inventory_schema=1` として同 `m2_context` へ追加する。
TankとBucketStorageを各最大512件、BucketTransport actorを最大128件読み、実Stockpileの種別/capacity、
StoredItems（不在はnull）、専用BelongsTo、未完成PendingBelongsToBlueprintとitemのDeliveringToを区別する。
BucketTransportはRiverまたはTank/needs_fill、TankまたはMixerのdestination、bucket/amount、全6phaseと
Filling/Pouringの実progress、Inventory/WorkingOn/ActiveTaskIdentityを記録する。欠損/消失した参照と非Soulも
観測したまま残し、所属・資材・taskを修復しない。非finite/上限超過は収集失敗。
これはstate inventoryだけであり `water_transfer_receipts=false` を明示する。
`ordinary-tank-pour-branch-v1` は実Tank注水branchのworker/assignment・bucket・新Water5Entityを記録する。
既存の注水/cleanup command適用後、同epochの実Tank/capacity50、水のResourceItem/StoredIn/StoredItems正逆、
空bucketと運搬関係解除を照合する。非profilingでは収集せず、観測Resource不在では記録しない。
bucket_beforeとRiver由来taskの実payloadを記すが、Riverでの取水自体を観測した証拠ではない。
raw parserは同sampleの実水ID・Tank所属と重複使用を検査し、このreceiptを全lifecycle witnessには昇格しない。
作業IDは実GatherWaterToTank request（assignment/targetとanchor Tank）へ拘束し、bucketは別のpayloadとして扱う。
同sampleの実Soulとexact GatherWater TaskCompletedのworker/assignment/request-target/時刻を照合する。
初期観測器のbucket-target固定は通常assignment経路と不一致だったため修正した。ゲームのtask割当は変更しない。
River取水、自然供給/精製、Tank親子の実入力/施工受入は下記の専用caseで個別に確認する。
Tankからの取出しとMixerへの投入は、profiling限定の実分岐receipt
`ordinary-tank-take-branch-v1` / `ordinary-mixer-pour-branch-v1`で受動記録する。
同epochの実DeliverWaterToMixer request、Soul/Inventory/ActiveTaskIdentity、専用bucket/Tank/Mixerを拘束し、
消費Water ID、実搬送量、投入前個数/capacity、生成Water ID、帰還可否を保持する。
Deferredでは取出し記録を作らず、現行handlerがUnreachableでも消費する結果はroute_found=falseで残す。
部分投入や帰還失敗も実結果として記録し、ゲームの数量・pathfinding・失敗cleanupを修正しない。
独立parserは実branchで捕捉した搬送bucket/消費Waterの型と格納先、適用後sampleの所有・型・在庫正逆・実時刻と一意IDを照合する。
周期rawの直前sampleを実処理の直前と同一視せず、その間の正常な取得・移動・搬入を拒否しない。
別のlossless-chain検査はraw検査を先に実行し、同worker/request/bucket/epochの先行取出し、
Found/帰還継続、消費数=搬送量=生成数を要求する。投入だけをTaskCompletedへ読み替えない。
全receiptを同じ4096件上限へ数え、欠損/不整合はsticky failureとする。
これらのunit testは合成世界の分岐同値性であり、通常worldの供給/自然精製、帰還完了、
renderer性能や正式releaseの証拠ではない。`water_transfer_receipts=false`と非承認flagsは維持する。

限定case `m2-tank-candidate-supply` / `ordinary-m2-tank-candidate-supply-v4` は、
採用済みTank候補の通常施工3checkpointに、別の合法なMixer配置・自然施工・Tank取水由来の最初の精製の3画像を加える。
Mixerは基準assetsの表示を維持し、Mixer候補のadmissionや美術採否をこのcaseへ混入しない。
準備はcamera/pauseとread-onlyな2つのanchor選定だけ。Tank/保管所とMixerの間に1セル以上の間隔を保ち、
初期Mixer不在を要求する。資材・actor・task・完成・水を付与せず、既存Yardの通常自動採取需要を使う。
Tank施工prefixは同process/scope/indexのまま再検証し、後半は既存HUDと一度だけの実Mixer配置入力を使う。
Mixerの前に通常SuperでTank Water50と専用bucket5個の返却を最大180秒待つ。
`setup=ordinary-tank-full-returned-buckets-v2`を固定し、Pausedの実release sampleから
Normal直前まで同owner・在庫正逆・own storage・非積載/非配送/非所持を同一rawで要求する。
全Inventoryの`inventory_holders`を受動観測し、BucketTransport以外の帰還運搬による所持も除外する。
追加2操作を含む専用motion上限20（宣言19＋任意Dev最小化1）をsource契約へ固定する。汎用上限16は不変。
Full専用画像は追加せず、この前提確認はraw証拠だけである。Empty/Waterの実型を変換・破棄しない。
Mixer施工/供給は明示Super条件でそれぞれ最大180秒待ち、全体480秒・1200sample・4096event上限は維持する。
`refinement_policy=first-tank-origin-after-complete-prefix-v1`を固定する。全raw分岐と全先行精製を検証したうえで、
撮影以前の同MixerでTank取水由来のWaterを消費する最初のRefineCommitから実Water IDを遡り、
同worker/request/bucketの損失なしTankTake→MixerPourと、再利用しない各RiverFill→TankPourへ結ぶ。
各bucketのRiverFill/TankTake→TankPour/MixerPourをcycle順で全件追い、消費済みの満水履歴を再利用しない。
全MixerPour出力と全Refineの実Water IDを一意に結び、先行する満水bucketの正常な再利用も分類して残す。
Sand/Rockは全実搬入と全先行Refineのzero起点数量ledgerを照合し、同frameの精製後搬入を
精製前の材料へ数えない。Mud搬出は別経路なので未観測差をnormalizeした3counter一致を主張しない。
各Refineの実before/after/出力はraw parserで別途検証する。別bucketの未完配送を完了へ数えない。
全後半sampleで同Tankのmap・専用bucket5個・保管所・在庫正逆・active candidate rootを保持する。
このrecipeの実装/testは実機成功ではなく、fresh実行・登録verifier・独立画像/raw reviewを必要とする。
帰還完了、全採取/配送lifecycle、Mixer候補の通常表示、性能、9種の正式release/installは別条件である。
`e9863406`のfresh `tak14-tank-mixer-supply-20261110-v1`はMixer施工待機180秒でinvalid終了。
通常Tankの完成とMixer配置、Wood3/4の実消費は記録されたが、残るWoodが供給されず完成しなかった。
494 samples/1104 events、最終real242.282747855 / virtual321.127758323、実RiverFill11/TankPour10。
MixerMaterial/MixerPour/RefineCommitはなく、給水から精製までの受入成功には数えない。
入力再送・資材追加・上限延長はせず、通常需要/自動採取/割当の診断として192774144 bytesを保持した。
invalid封印/finalize済み。旧passへの読み替えや独立採否の変更は行わない。
追加診断v2（`5ee802b5`）も同じ180秒でinvalid、194744320 bytesを診断保持しfinalizeした。
通常Wood需要1、地上/予約/搬送供給0、Tree候補41は存在したが、新規伐採割当はなかった。
自然生成seed20260914の再現では、Yardから到達できるTreeは0/41、Rockは13/52で、
初期・Tank/専用置場完成後・Mixer予定footprint後の全段階で同じだった。fresh/warm cacheとA*も一致した。
地形生成の現契約はTree到達を保証しておらず、この試験worldでは橋なしの追加Wood供給を成立させられない。
一時INFO診断は撤去し、gameplayの地形・資材量・自動採取・歩行規則は変更しない。
供給caseだけ自然生成seed42/attempt0/fallback=falseへ固定し、layout SHA256
`d8eff05ae2b6c56d8bbfa39570c6ce6e7174aae0c69f2c5ab53527add5dcff72`をcontractと実観測で照合する。
準備時のread-only経路投影では全3段階でTree15/Rock12へ到達可能で、実WorldMapは更新しない。
他caseや通常起動のseedは変えない。これは特定worldの試験成立条件であり、任意seedでの供給保証、
全物流や地形生成修正の受入ではない。
`ec590035`のfresh供給v3では両建物が通常施工で完成したが、精製待機180秒でinvalidとなった。
最終real394.520277787/virtual445.421038043、1172 events。Sand5は末尾で猫車へ積載され、帰路を進行中だった。
MixerPour1は川で満水になったbucketを保管後に別workerが拾い、Fillingを省略した通常分岐であり、
Tank内Waterの取出しではない。TankTake/MixerMaterial/RefineCommitは0で、成功へ読み替えない。
invalid封印・診断用途204750848 bytes保持・finalize済み。Sand採取指定不足や停止と推測せず、
次はSupply専用の限定受動観測でsolid requestの実owner/demand/workers、Rock実指定/到達、
採取/手運搬/猫車の実task/移動先を調べる。入力・数量・上限・採否条件は変更しない。
供給v4（`f6270ae7`）ではYard発行Rock需要5→到達可能なMine指定→実採掘/手運搬→Rock搬入まで確認した。
Sand5も猫車の帰路を進行し、real417.811793415で上限終了した。invalid封印・比較用途保持・finalize済み。
採取/搬送実装の欠落やSand指定不足は立証されず、一時solid診断は撤去する。
512 samples時点の公開コストは収集9476us、全履歴encode903873us、write33624usだった。
profiling observerの履歴をowned append-only型へ移し、event_indexをappend時に確定して一度だけencodeする。
過去rowへのmutable参照を公開せず、入力/receipt全経路を共通appendへ通す。全JSON内容/順序/authority、
1200samples/4096events/完成JSON64MiB上限、排他的temporary・同期write・renameは不変。
byte/encode失敗はstickyで、欠損を捨てて後続成功にしない。headerのfailureは公開時に生成する。
各公開の`encoding_us`は新規row encodeとenvelope組立の合計、`row_encoding_us`は前回公開後の追記分。
`collection_us`は当observe内で実行したrow encode分を差し引き、callback由来のencodeはencoding側だけへ含める。
全面bytesコピー/writeは残り、総累積処理の線形化やゲーム/renderer性能合格を意味しない。
新sourceの実機結果は未確認であり、旧成功/invalidを新受入へ流用しない。
encode最適化v5（`4b93f47a`）は384samplesでencode4109us（旧698895us）に改善したが、
背景Dream通知による予定間隔前のsnapshot増殖で64MiB上限へ達しinvalidとなった。
896samples/64287642bytes時点のencode31745us（うちrow2690us）で、TankTake1/MixerPour1とRock実搬入はあった。
正式invalid封印・診断比較用途保持・finalize済み。精製成功/全物流/正式性能には読み替えない。
v6（`ce651f35`）では全Dream通知を保持してsnapshotを既存間隔へまとめ、186samplesまで抑えたが、
real73.234568079/virtual193.924290605で4096event上限へ達した。4099eventsのうち3833が実Sleeping Dream通知。
Tank/Mixer完成・TankTake1/MixerPour1・Rock5搬入はあったが精製未達。invalid封印・174993408bytes診断保持・finalize済み。
供給だけを扱うv2ではsource contractとraw headerにexact
`event_contract={"id":"m2-supply-events-v1","excluded_producers":["DreamTransferredVisualMessage"]}`を宣言する。
検証済みSupply preview sessionから固定導出し、任意producerの除外を外部JSONで選択できない。
このscopeだけobserverのDream readerを最初から購読せず、製品側の生成・他reader・他scopeは変更しない。
v1のcoalescingは撤去し、対象eventの即時公開を元に戻す。input・施工・水・solid・精製・未知eventは除外しない。
独立verifierは現行scope/source/headerの完全一致を要求し、旧scope、契約欠損/変更、Dream producer/operation混入を拒否する。
この証拠からDream移送・休憩lifecycle・全世界の完全履歴を証明できない。旧v1 invalidは履歴のまま維持する。
1200samples/4096events/完成JSON64MiBと待機上限は変更しない。
v7（`37515837`）はこの通知契約で490samples/254events、観測は上限内・raw failureなしだったが、
精製待機180秒でinvalidとなった。最終real231.907792928/virtual821.816274802。
両建物完成・Rock5実搬入・MixerPour1は記録された一方、Sand5は猫車905v0に残り、TankTake/Refineは0。
実停止点[72,49]=(720,-16)とMixer中心(672,-64)は67.88wu離れ、中心距離閾値57.6wuを越えた。
猫車は有効な隣接pathを得ていたが中心距離だけで到着判定しており、Unloadingへ進めなかった。
独立read-only調査でも同じ不整合を確認した。identity失効の直接原因は観測がなく断定しない。
通常のMixer猫車branchだけ、Found後に手運びと同じ実終点も到着判定する修正を行う。
旧invalidは封印/finalizeし、200327168bytesを修正比較用途に保持。新source実機成功や全受入には数えない。
v8（`c673ff6a`）は猫車の実Sand5搬入とRock1、初回RefineCommitで泥5生成まで進んだが、
55入力中17motionsが旧汎用16上限を越え、独立検査でinvalid。179samples/263events、raw failureなし、
初回精製real68.398932860、最終real69.450875111/virtual180.860686409。
使用水は満水バケツの正常な直接再利用でTankTakeは0。搬入修正の限定結果を供給全体の成功に読み替えない。
invalid封印・177369088bytesを明示入力予算と取水前提の比較用途に保持しfinalize済み。
v9（`e89707bc`）ではWater50到達後、専用置場にBucketEmpty4とBucketWater1、bucket task0の
正常終状態が続き、全Empty前提の180秒待機でinvalidとなった。433samples/177events、raw failureなし、
最終real211.397865047/virtual749.625251064。満水Tankへ注げない余剰バケツを満水のまま返す既存仕様である。
185516032bytesをcycle provenance比較用途で保持しinvalid封印/finalize済み。v4はこの正常分岐を捨てず全件検査し、
最初のTank由来精製を固定選択する新しい契約であり、旧v3結果や別精製を成功へ読み替えない。
v10（`e9ac61c7`、source `9d843ab755c0a617d48fb901dc437e1ca3bc967ba348bc1d98ad324f6df937a3`）は
通常Full/返却前提を満たし、両建物完成・先行するRiver満水bucketの投入と2回の精製へ進んだが、
real93.546954555のTankTakeで5Waterを消費した後、同worker/request/bucketのMixerPourが
Mixer内3/5に対して2Waterだけを生成し、strict lossless検査でinvalidとなった。
実route_found/returningはtrue。最終230samples/294events、real94.384383478/virtual250.933614114、raw failureなし。
`pouring.rs`の`min(amount, available)`と全bucketのEmpty化が残3の消失原因で、実handler回帰も現行部分投入を保持している。
数量/待機/予約/取消の変更は観測同値修正ではなく別のgameplay bugfix判断を必要とする。
良いcycleだけを選んでこの損失を隠さず、lossless供給、全物流、正式性能・release/installは未達のままとする。
181051392bytesのv10原本を数量契約の診断用に保持し、invalid封印/finalize済み。

観測encode改善とMixer猫車隣接到着修正は、変更別contracts/tooling/Rust gate、profiling Clippy警告0、
Help実経路No impact、独立read-only reviewを通過した。v8/v10では実Sand搬入/精製まで到達し、
観測負荷と到着不整合の限定修正は確認できたため、2つの一時計画は本節へ結果を集約して撤去する。
供給全体の合格を意味しない。解決済みv1〜v6の診断consumerは正式releaseし、元pathの
1201537024bytesを復元可能なTrashへ移した（物理空き容量の増加とは主張しない）。
v7〜v10、採用candidate/codec、通常Cargo cacheは別の具体的用途を維持する。primary storage checkはpass。

自然施工は別の `m2-mixer-candidate-completion` / `m2-candidate-completion` /
`ordinary-m2-candidate-completion-v1` で採用済みMixer候補だけを受付する。準備はcamera/pauseだけとし、
実配置receipt・Wood4個の実搬入・同BlueprintへのBuild assignment/完了・新ownerへのmap移管を記録する。
完成時は同候補のbody/rotor mesh・albedo・材質・初期poseを確認する。既存planning caseの完成拒否は維持し、
このcaseを精製・save/load・move/cleanup・性能・releaseの証拠へ流用しない。
このconsumer観測はlocal pose・owner一致とpart ViewVisibilityに限定し、global行列・root親・render layersを
通じた描画chain全体の証明ではない。Normal/Pausedの実raw状態・speed1と施工区間のvirtual前進、
全checkpointの同PID/epoch/window・paused同clock、入力のreal phase境界を検査する。

Tankの自然施工は独立した `m2-tank-candidate-completion` mode/case、
`ordinary-m2-tank-candidate-completion-v1` scopeで採用済みisolated candidateだけを受付する。
本体クリックのCompanionPendingでは予定/保管場所を作らず、右隣2セルへの実第二クリックの
CompanionCommittedから本体4セルと保管場所2個を追跡する。Wood2個の搬入とBuild完了、新Tankへのmap移管、
同じ保管場所のPendingBelongsToBlueprint→BelongsTo、実座標・capacity10・StoredItems正逆を検査する。
完成sampleで専用空バケツ5個と格納/非運搬関係を照合し、その後の自然運搬を停止・修復しない。
空/予定/完成の3画像と同候補body/waterを検査し、Emptyではwater partの存在・非表示を要求する。
準備はcamera/pauseのみ。供給が先に始まりEmpty条件を失えば失敗とし、水を削除しない。
`0a51f1b3`のactual-windowでこの限定施工受入は成功。二段入力からWood2配送、同じ保管場所の昇格、
専用空bucket5個、候補body表示/water非表示の3画像とrawを独立確認した。
job `tak14-tank-candidate-completion-20261109-v1`、source `bbc4ccd6051760b38dc866ef57a06804e6e217a5b9d08b48d1daccb506208890`、
manifest `e16b6ae3e201f4c58e41fe2d2d29af295c0ca50b2bbf5551b7a51c1e2618e81d`。
Intel Arc/Vulkan/X11、WARN/ERRORなし。封印・用途登録・finalize/storage check済みで、後続変更の成功へ流用しない。
Tank水運搬、全描画chain、性能、正式releaseの受入を意味しない。

次の `m2-tank-candidate-inflow` / `ordinary-m2-tank-candidate-inflow-v1` は別の明示case/scope。
同processの施工prefixをscope/index不変のまま検証し、追加入力はNormal→自然TankPour待機→Pausedだけ。
既存TankPour branch・正確な作業ID/TaskCompleted・専用bucketと新Water5個を照合し、全実注水の生成集合と
同Tankの現在StoredIn/StoredItemsを一致させる。実水量により水part可視・Partial高さ/色またはFull高さ/色を確認する。
通常取水/供給を注入・修復せず、180秒待ちの不成立は失敗。川取水成功そのもの、TankからMixerへの搬出、
全物流、全描画chain、性能、正式releaseは証明しない。
`aba57f91`のfresh実機 `tak14-tank-candidate-inflow-20261109-v2`で限定成功、4画像/rawを独立確認した。
sample228の同worker1864v0/request1957v36/bucket2413v24に続き、229でTank1896v44への
実TankPour・同request完了・Water5個生成を記録。以降の同Tank格納集合が一致し、同rootのwaterはHiddenから
Visibleへ、Y12/Partial色[.58,.70,.73,1]へ追従した。Intel Arc/Vulkan/X11、WARN/ERRORなし。
source `729343f055081e28e723bc42e394d748cebc3d88b34d52c41f2a4c8989461142`、
manifest `c10eabcb51a2a98d2fa5d5c65d91b9104cc832b3bc52e1dbdf4063401576e44d`、
raw `534baa14a067952ac5cca9a482f5b48a29f356956b0a3b5dd5b971dfac578e3f`。
170229760 bytesを残るFull/Mixer供給との比較用途で保持し、封印/finalize/storage pass済み。
全9種の標準起動locator/install/releaseは未変更であり、通常起動の全移行完了ではない。
Full50専用の `m2-tank-candidate-full` / `ordinary-m2-tank-candidate-full-v1` は、
同じ通常施工prefixから自然充填を最大180秒待ち、実5Water注水10件と現在50個のexact集合、
同候補水partのY22/whiteを検査する。旧inflowの最初の注水条件は変えず、case/scope/checkpointは別とする。
既存のclock/owner/map/関係・sample/event上限を維持し、未達なら失敗とする。
`0cf63187`の `tak14-tank-candidate-full-20261109-v1` で限定成功。実Tank2439v21への10件の注水、
一意Water50個と同在庫、同水partのFull Y22/whiteを確認した。4枚の所有窓画像を確認し、
Intel Arc/Vulkan/X11でWARN/ERRORなし。461 samples/899 eventsで既存上限内だった。
source `c4f1d2ebe3828cf39a0d4dfdb92f36789d553dee7ce8946f46a335862a0700d8`、
manifest `8a203b9ef1f9a9978f9be7763ea34ac437b861046fd019af8c70bad6b10a95f7`、
raw `1138a050671a2668e9ef5056b24cecfa24b2ebb97347413f323f15e3655cbf98`。
登録verifierで封印しfinalize/storage pass済み。186470400 bytesはFull→Mixer供給の比較とvisual feedback用に保持し、
その用途終了後に整理する。川取水branch、Mixer供給/精製、性能、正式releaseは依然として別の未達条件である。
後続の受動観測として `ordinary-river-fill-branch-v1` / `RiverFill` を追加する。
RiverのFilling branchで通常の帰路探索がFoundとなり、実空bucketをBucketWaterへ変更した場合だけ記録する。
deferred適用後に同worker/request/Inventory、専用bucketのBelongsTo/DeliveringTo、同Tank/epoch、
GoingToDestinationを照合し、欠損・不一致はsticky failureにする。通常gameplayと非観測sessionは不変。
receiptはbucketの型変換を証明するだけで、`data.amount=1`を水量と読まず、川地点・到達経路や
注水/精製の成功を推定しない。Full検証器は新sourceから各注水の直前の同bucket/worker/requestの実RiverFillを要求し、
再利用・欠損・別作業のreceiptを拒否する。旧成功jobをこの新条件の証拠へ読み替えない。
`7c661318`のfresh v2はNormalで40/50のまま180秒に達しinvalidとして封印・診断保全・finalize済み。
472 samples/898 events、RiverFill11件・TankPour8件、終了直前も水入りbucketが同Tankへ運搬中だった。
この失敗を上限延長や旧passで解消しない。後続Full contractは必須 `supply_speed="Super"` を封入し、
施工prefixは既存条件のまま、貯水だけ実Super→Pausedの二操作へ分離する。切替前raw速度1、
Released sample以後は停止後も4を要求する。全上限・実汲水相関・一意Water50条件は不変。
これは4x条件の技術表示確認であり、Normalの180秒内充填や性能の確認ではない。
`b152108d`のfresh `tak14-tank-candidate-full-20261109-v3`で新条件が限定成功。
実RiverFill10件とTankPour10件が同worker/request/bucketで対応し、Tank2218v18に一意Water50個を格納。
同root2216v17のFull Y22/white、4枚の所有窓画像を独立確認した。465 samples/877 events、
sample154のReleasedで速度1→4、最終Pause後は末尾464まで停止/速度4/仮想時刻固定。
Intel Arc/Vulkan/X11、WARN/ERRORなし。HUD FPS1は性能承認ではない。
source `7d934e9e1dbb9964b6485909ee3f9ebd230c283e8fd238ecc9aa315609009fbb`、
harness `6113fb1b62a492d8a3b4084b1c387f0b09d6896d5a44d01027fef0c709bde8d2`、
manifest `26f1add51e320a61996ae0c9ac25bda1c66791362f5fdf612cc2adcd7e5e87db`、
raw `1bdd5d3af35b9f68e6ceebe83eb0ef045f65b21bd90f563cfb79dc005f84a693`。
登録verifier、独立read-only review、封印、finalize/storage pass済み。最新job186478592 bytesは
Tank→Mixer供給/visual feedback比較用に保持し、その用途終了後に整理する。
施工単独v1・Full v1・Normal未達Full v2は用途を正式解除し、計534450176 bytesをゴミ箱へ移動した（回収可能）。
結果の成功/invalid履歴は維持し、採用原本/候補/codec/Partial比較入力と最新v3は保全した。
次は実Tank取出しとMixer注水、Sand/Rock搬入のbranch結果を採取し、通常伐採/採掘と
自然RefineCommitへ接続する。川地点への地理到達、全物流、全描画chain、性能、正式releaseは未達。
最初の実機 `tak14-tank-candidate-inflow-20261109-v1`（`41253e73`）の結果はinvalidのまま維持。
実注水と4画像は得られたが、周期観測に一瞬のPouring phaseを要求した検証器が拒否した。
修正では実TankPour branch証拠を維持し、直前sampleの同依頼/作業員/水入りbucket/行先を拘束する。
phaseは到着中またはPouringとし、任意の過去sampleから作業IDを拾わない。旧jobを成功へ変更せず新jobで再確認した。
診断終了後、旧job167124992 bytesは正式releaseしてゴミ箱へ移動。採用原本/候補/最新証拠を削除せず、失敗履歴は本書と台帳へ集約した。

M5専用の明示 `--variant m5-art-context --view normal` は別のseeded比較である。
ArtPreviewの4種だけをfeedback/art-preview・standard照明で許可し、candidate/releaseや通常入力証拠へ転用しない。
対象4棟、前後8Wall、8idle Soul、descriptor経由の4previewを一度だけ準備し、以後は受動観測する。
Lampだけは元galleryの列別Spa2棟（mask3/15）と追加6発電Soulを保持し、既存のOperational/Generating準備、
Dream100の明示seedから実power_output/grid_recalcを通す。全14Soul/14建物の別inventory、tile/worker双方向関係、
発電元/消費先/Yard/電力網とoff列への供給不在を固定する。通電ミラーを上書きせず、ready後の変化は拒否する。
元の地形を変更せず、全Soulの既存task/関係があれば拒否する。8idle SoulのDream値は調整しない。
actual foreground、地形/footprint、Wall/Soulの論理位置とrenderer root/local/global行列・材質・layers・visibility、
RTT camera、preview画像/canvas/anchorを同subjectで記録し、30 stable frames後の変化を拒否する。
別verifierの実file hashもplanへ拘束する。これは2D/RTT合成の重なりを画像で判断するための比較材料であり、
正しい奥行き順、全地形での可読性、通常のPlacementGhost/選択入力、配電、lifecycle、性能、独立採否を保証しない。
基盤前後のコスト比較は別々に凍結した静止参照jobを使う。本書更新時点でM1-cの実native受入は未実施。

明示 `--variant lamp-supply-art-context --lamp-supply-target 0|2` はLampの美術判断用に限る別feedback。
ArtPreview/OutdoorLamp/standard/small、normalまたはoverviewだけを許可し、既存M5 contextのnormal-only条件は維持する。
同じseeded配電graphでsetup時だけcompleted ownerを選択し、通常presentation/inspectionが作るheaderと供給Textを読む。
Text/ViewModel/PowerSupplyStateは注入しない。実selected/unpinned/inspected/model/grid owner、供給state/需要と
未clip・可視・無変形の実Text矩形、primary window/DPI1をexact foreground/identityへ拘束する。
30 stable framesと初期/終了一致を維持し、ready後の変化を補正せず拒否する。
normal/overview×off/onのowned画像によるreadabilityだけを扱い、OS選択・自然配電・ordinary lifecycle・性能や採否を
成功としない。通常起動、player操作/文言、配電rule、Helpに変更はない。

制作器の `building_asset_pipeline.py verify` はpackaged bytes・provenance・既存承認identityを読むだけで、
承認を作らない。M5 `prepare --destination` はfreshな隔離jobへ出力できるが、既存出力、原画の親子path、
primaryの製品assetsへの出力は拒否する。入力原画はstagingに限定しworld画像を再加工しない。
`verify --isolated` はその出力をread-only検査する。標準の出力先は引き続きstagingである。

### 通常開発におけるproduction受入toolingの境界

#### M5 ordinary planning ArtPreview feedback

UI helperの`--case m5-sand-planning|m5-bone-planning|m5-parking-planning|m5-lamp-planning`
は`--smoke --input-backend portal-eis --m5-art-source <exact ArtPreview> --art-codec <codec>`を要求する。
`ordinary-m5-art-planning-v1` / `m5-feedback`は一kindのidentity、source/codec/verifierの実bytes、
base assetsと新規asset-viewだけを認め、candidate/release、他kind、seeded gallery・M6・productionとの混在を拒否する。
既定起動のlocator/admissionや製品writerは変更しない。
準備は生成済みworldの製品placement validatorで合法anchorを選び、cameraとpauseを一度設定するだけ。
Site/Yard、地形、資材、Soul、task、建物、完成や施工進捗は注入しない。
生成済み同kind建物は最大16ownerのID/local/global poseを準備時に固定し、消したり新設対象に数えたりしない。
全phaseでそのowner集合と姿勢・world consumerを保持し、新しい完成owner、差替え、欠損は拒否する。
既存world画像は初期ready時のexact descriptor/親/poseへ拘束し、今回の合法anchor・新Blueprintと分離する。
実Primary windowへ向く単一active/full-viewport/layer0 cameraと実cursor gridを確認し、通常入力による
catalog→合法ghost→一度だけBlueprint配置→同tileの占有拒否ghost→同Blueprintの情報表示を6画像で記録する。
実ImageNodeの未clip・whole-image・白tint、Spriteのdescriptor/image/size/anchor、施工/合法色、
owner/parent/local/global pose・layersと表示実体を独立検査する。
pulseは既存writerがBuilding状態で作った場合だけ存在し、それ以前は存在してはならない。
施工が進んだ実progress/搬入は保存するが、pulse未出現の試行をpulse受入と読み替えない。
実入力は同nonce/PID/window/epochとphase別step/frame/real区間、原press/release、
原Pressed以後の配置receiptに拘束する。Last flush待ちは入力を再送しない。
このfeedbackはaccepted/performance/promotion=falseであり、完成/採取/車両入出庫/通電/保存、
世代切替/cleanup、独立art/numeric decision、正式candidate/releaseや通常起動への公開を証明しない。

採用済み4候補には別の`m5-sand-candidate-planning`、`m5-bone-candidate-planning`、
`m5-parking-candidate-planning`、`m5-lamp-candidate-planning`を使う。
mode `m5-candidate-planning` / scope `ordinary-m5-candidate-planning-v1`だけが
IsolatedCandidateを許可し、旧ArtPreviewの4case/mode/scopeと相互に代用できない。
packaged art/numeric approval・原本・全依存を再照合し、numeric validator inventoryと
共有art driverの実SHAをsource contractへ固定する。旧previewへこのmetadataを持ち込めない。
通常生成worldのcatalog/ghost/実Blueprint/占有拒否/同owner inspectionと既存Parking保持を
同じ入力guardで確認する。新完成ownerの出現は引き続き拒否する。
これは候補の通常入力・表示経路の技術受入だけで、自然施工以降のlifecycle、通電/入出庫、
性能、正式promotion/install/releaseを成功とする入口ではない。fresh実機結果は未確定。

#### 採否前のRestArea ordinary art feedback

通常UI helperの`plan --case rest-area-lifecycle --smoke --input-backend portal`へ
`--rest-area-art-source <隔離ArtPreview root> --art-codec <準備済みcodec>`を明示した場合だけ、
exact RestArea ArtPreviewを通常生成worldで表示する。通常起動の有効化でも正式候補の承認でもない。
planは実codec検証、原本/geometry/manifest/依存bytes、基準asset viewとcodec hashを固定し、runは
そのdigestを再照合する。fresh job内へ基準assetsと宣言依存だけをcopyし、製品assetsを変更しない。
verifyは基準全ファイル＋宣言overlayのexact inventoryを再計算し、追加/欠落/改竄/symlinkを拒否する。

profiling限定`HW_BUILDING_ORDINARY_ART_SESSION`はUIのnonce/root/固定caseと一組で、
`feedback`/RestArea/ArtPreview/正規locatorのみを許可する。static ArtSession、M2/Bridge probe、
M6、release、正式production observer、perfとの混在、非profiling、未知fieldや既存出力/tmpを拒否する。
loader登録前にexact policyとpool requestを準備し、材料/完成/actor/利用/Dreamは一切seedしない。
既存の通常配置・施工・入所・Dream・利用中保存復元・自然退出/cooldown入力を用いる。

共用の受動collectorは専用Resourceから別scope `ordinary-rest-art-feedback-v1`を選ぶ。
public production JSONは従来どおりReleaseApprovedだけを許可し、scopeを自己申告できない。
両scopeともaccepted/performance/promotion=false固定。通常scopeには新しいart項目を加えない。
feedbackだけで要求identity、実active/pending/descriptor、実owner-linked rootの世代、part mesh、
親子/transform/可視性、material/albedoと不透明・非発光・白tint等の実値を別記録する。
採用checkpointは実completed ownerからexact root1件を引き、pending/未ready/旧世代/fallback、
読込前owner参照、別mesh/texture/material値を拒否する。従来の正式raw verifierは新scopeを受理しない。
これは限定された候補表示/lifecycle feedbackであり、全consumer、最大遠景selection、照明/壁depth、
independent art/numeric採否、Capture/Memory性能、promotion/install/release/rollbackは別途必要である。
`c07987e4`のfresh ordinary EIS feedbackは9checkpointを通過し、登録verifierと別実行verifyが成功した。
source=`43386c30797aac97f88d338f16c76304cb14ff143d9f9111e4dc0eba01361947`、
harness=`3edf28c113da7c416c45af5f9053de50b83c53d7320ec8bd10562883863d5cae`、
exact候補は`db0f…2522`。通常施工・利用・Dream・実Entity remapを伴う保存復元、
自然退出と15秒cooldown（仮想85.614576461→100.614576461）を観測した。
同ownerのroot/part/画像/材質接続と9枚のowned画像をmainと独立read-onlyで確認し、
Intel Arc/VulkanでWARN/ERRORなし。限定feedback結果は保持・seal/finalize/storage pass済み。
通常起動への正式採用や前段のart/numeric承認、全consumer/性能の完了とは扱わない。

最大遠景UI補完の受動観測は、cameraの数値transform/projection、primary window、実hover／pointer target、
tooltip reducerが実際に保持するWorldEntityとworld epoch、owner中心の投影点を記録する。
tooltip mirrorはprofiling UI observerが存在するときだけ更新し、UI／placement／対象なしではworld targetを消す。
実MouseWheelは独立MessageCursorで読み、window・unit・delta・phase・frame・epochを最大128件記録する。
上限超過はsticky overflowで公開し、黙って欠落を成功にしない。他consumerの入力を消費せず、
カメラ／hover／tooltip／actorの状態を書き換えない。これだけではzoom操作や遠景の識別成功を証明しない。

明示`--case rest-area-maxzoom --smoke --input-backend portal-eis`は同じArtPreview/codec指定を必須とし、
保存復元lifecycleとは別の有界feedback caseである。通常建設3checkpointの後、最大48回の1刻みwheelで
数値scale=5へ到達し、empty/occupied各状態のhoverと同owner選択の4枚を確認する。
SCROLL capabilityはこのcaseだけが明示要求し、既存caseは要求せずwheel/button4/5を拒否する。
libei 1.6の宣言SCROLL(16)とscroll_discreteのint32/120単位だけを使い、Notifyやsmooth scrollへfallbackしない。
各wheelはpresend/ACKのwindow/epoch/foreground、実MouseWheel、raw camera sampleを照合して一度だけ送り、
同stepや不明送信を再送しない。camera一意性・active/primary target、実hover/pointer/tooltip対象・可視本文、
利用人数と既存Dream rate、選択ownerと可視inspection headerを照合する。倍率やactor状態を直接設定せず、
このcaseのfresh実機成功、アート・数値・性能・正式releaseは実装/回帰テストだけから主張しない。
checkpointはframe、入力step、raw sample、実時間の順序とpause中のvirtual時間不変を要求する。
新しい入力区間の非重複検査は建設完了後のmaxzoom操作だけに適用する。前段の既存construction検査は維持し、
Driver.keyが意図的に押下/解放をまとめてACKするEscapeを、新規wheel/click区間の規則で誤拒否しない。
hover→selectedには同区間の実MouseButtonInputのLeft press/release、同window/epoch/ownerと実cursor座標を
要求し、既存選択の表示だけをクリック成功に読み替えない。empty→occupiedは通常simulationの進行を要求する。
同地点の再クリックは既存の重なり候補巡回なので、異なる対象が選ばれた場合はworldクリックを再送しない。
既存候補一覧のtoggleと対象行をtarget/revision/epoch付きで受動観測し、可視hover/pressと実入力を一度ずつ照合して
当該ownerを明示選択する。最後にworld ownerへpointerを戻し、同じtooltipと選択headerを撮影する。
一覧を使う場合も全Left pairのraw window/epoch/owner/座標/時系列を検査し、別target・旧revision・追加クリックは拒否する。
press前の所有client外ポインタ拒否には実window/PID、client寸法、X11 client/root座標とmaskを記録する。
拒否時は入力を送信・成功記録せず、cleanup releaseは維持する。これは原因観測の補強であり、guardの緩和ではない。

2026-11-08のfresh `tak14-restarea-maxzoom-art-eis-feedback-20261108-v5`では、通常施工と40wheel、
最大scale4.999997615814209で同owner133143983773のempty/occupied hover/selected全7checkpointが
登録/別実行verifier pass。既存tooltipのResting 0/5→1/5・Dream 0.00→0.50/s、可視header、
候補一覧のexact owner/revision/epoch行からの明示選択をmain/独立read-onlyが実画像とrawで照合した。
Intel Arc/Vulkan WARN/ERRORなし。quiet shared-input条件で旧ポインタ消失は再発しなかったが原因は断定しない。
結果は用途保持・seal/finalize済み。建物形状だけの最遠識別、明暗・壁depth・配置previewの比較、
独立art/numeric採否、性能、正式releaseと通常起動への採用を証明するものではない。

同UI recipeの明示`--input-backend portal-eis`は当該ArtPreview lifecycle/maxzoomだけで既存EIS absolute
経路を選ぶ。通常portal Notify経路からの自動fallbackではなく、混用・他caseへの拡張は拒否する。
既存のpointer/keyboard同意を使い、ConnectToEISは一度だけ。宣言logical region×scaleと
physical root寸法、実owner/focus/nonce、単発stepとgame cursor ACKを維持する。
結果へabsolute transport種別と実region/scale/root寸法を記録し、独立verifyで照合する。
EIS helperも三つのharness inventoryでhash固定する。未知regionや座標不一致で入力を再送しない。
最初のNotify試行は候補load後、初回motionのACKなしでinvalid精算。配置は未送信であり、
Xwayland/compositorの原点不一致は未確定仮説。開始座標依存を除く比較であり、承認の代用ではない。
`ec2c1c2b`のfresh EIS試行は実施工・利用・Dream・保存と読込まで進んだが、live save checkerが
既定の正式raw scopeでfeedbackを照合しinvalidとなった。旧結果は変更せず、live exerciseにも
明示keywordでfeedback scopeを渡す。raw自身のscopeから推測せず、正式呼出しの既定と拒否は不変。
読込後の退出/cooldownは当該試行では未検証であり、修正後のfresh比較で確認する。
次のfresh試行では入所前に120秒の待機が終了した。予約5人はGoingToRestを維持して実際に移動し、
123実秒で54仮想秒だけ進み、最寄りも中心まで約500wuを残した。固定待機の不足として入所待機を
240秒へ有界化する。速度・Soul位置・予約・Dream・成功predicateは変えず、全体480秒上限を維持する。
旧timeoutはinvalidのままで、240秒は入所成功や性能budgetの証明ではない。

`python3 scripts/building_production_native_acceptance.py capabilities` はlocal supportを読むだけで、
旧request/Run/hostへの照会、台帳更新、plan生成、build、launchを行わない。現在は `available=false`。
ordinary-world process/window・capture/ACK producer、GPU/native/RSS/application-handle instrument producer、
通常開発のauthenticated admission adapterが未提供である。旧統括を再起動して補完しない。

`building_production_validation_context.running_context` は通常primary検証台帳のread-onlyな
runner所有確認を提供する。実行中batchの直接childと親coordinatorの出生identity、owner、literal
command/verifier、exact出力root、登録時plan hash、現在head/source、policy/controllerを照合する。
環境変数や保存JSONだけでは受理しない。grandchild/wrapperも現在は拒否する。
`building_production_local_probe.py` はこの入口を通常開発の直接child検証へ接続する技術観測器である。
cleanな同一source／profiling binary／登録済みplan／fresh rootを要求し、隔離した作業dirで通常worldを起動する。
所有PIDのX11 client画像、製品rawのsample/event順序、子processのRSSとfd数だけを採る。
入力は駆動せず正式identityも渡さない。GPU／native allocator／application全体のhandle、
Capture→Memory、建設・配電・保存復元・cleanupの成立、art/numeric採否、release/promotion authorityは
この観測器から得られない。別の実装・独立受入を終えるまで `capabilities available=false` は維持する。
`building_production_local_input_probe.py` は別の技術専用入口で、同じprimary validation所有の
直接childから通常worldの所有X11 windowへ固定F12を一度押下・解放し、製品rawの対応する
`KeyPress`／`KeyRelease`、前後の実window画像、子processのRSS/fdを照合する。任意入力recipeや
正式asset identityを受け付けず、domain outcome、Capture/Memory lifecycle、GPU/native allocator、
application全体のhandle、性能、art/numeric/releaseを受入済みにしない。入力受付が実機で確認できても
`capabilities available=false` のまま、必要なordinary-world legとinstrument producerを別途実装する。
同helperの明示`--scenario architect-cancel`は固定された別profile
`building-production-local-architect-portal-v1`を計画する。通常生成worldへBの押下・解放で
Architectを開き、Escapeの押下・解放で閉じる。任意key/command、fixture、asset authorityは渡せない。
profiling raw observerは実際のMenuState／PlayMode／TaskModeを読み、存在しないresourceはnullとする。
同epochのHidden→Architect→Hidden、Normal／None、各キーの実raw witnessと所有clientの3画像を検査する。
catalog画像の前後にはfresh sampleを要求し、その全区間でArchitectが不変でなければ拒否する。
操作モードは通常入力が参照する`TaskContext.0`から読む（`TaskMode`単体resourceを仮定しない）。
撮影前には単一Architect panelの表示と正の有限layoutが2連続sampleで成立することを要求する。
閉じた画像も2連続のHidden／panel非表示を待ち、撮影後のfresh sampleまで同状態を検査する。
このreadinessは実画面の独立確認を代替せず、menu stateだけの変更や描画前の画像は採用しない。
開始画像はHidden／Normal／None／同epochのfresh撮影区間を拘束し、中央160pxのblankを
既存M2 pixel検査で拒否する。8秒・最大16回以内のblank再撮影は同じowned processに限り、
拒否した画像と理由を保持する。寸法違いやraw区間の不成立は再撮影せず失敗する。
runとverifyはbefore／catalog／afterの全画像へ同じnonblank検査を適用する。
画素の非一様性はasset readinessやアート採用を証明しない。
これはordinary入力でのcatalog開閉に限り、9種の候補表示・配置・施工・性能・正式採否ではない。
この実装の実機成功はfresh batchの独立verifyと画像確認後にのみ記録する。
profiling rawには`BuildContext.0`の実選択と、通常Architectの9種の実cardを読み取る。

`UiIntent`／`ArchitectBuildingPanel`から対象を解決し、primary windowのphysical pixelで
rendererのclipと可視ancestorを適用したrectを記録する。Bridge、非表示、未layout、変形matrix、
過大集合・可視重複は受理しない。`MouseButtonInput`はbutton/state/windowを実原メッセージとして
記録し、ButtonInput由来のPointerPress/Releaseとは区別する。rectや入力だけでclick成功、
placement commitや正式asset受入を推定しない。通常入力・UiIntent・gameplay・Helpは変更しない。

profiling観測sessionの`PlacementReceipts`は、通常UI配置分岐が実際に返した結果だけを最大4096件で保持する。
観測sessionなしではresourceを作らず、入力・配置条件・保存・正式authorityを変更しない。
Tankの最初の有効clickは`CompanionPending`でありBlueprint成功ではない。companion配置成功時だけ
実親Blueprint／子storage IDs／footprintを記録し、失敗時は拒否または親rollback済みを記録する。
他7種の実Blueprint結果と、SoulSpaの実site／4tile spawn結果は別statusとする。
rawの`PlacementOutcome`にはdeferred command適用時のworld epoch／primary window／実時間と、
raw観測時のsample indexを付ける。click時点との原子性やraw event順による因果関係を保証しない。
overflow、欠落・多重window、計測context欠落はfail closed。原MouseButtonInputとの因果照合、
deferred entityのbefore/afterとoccupancy検証、通常lifecycle全体の受入は別の未完工程である。
offline raw parserは同window／epoch／finite時刻とstatus別payloadを検査するが、
`PlacementOutcome`をlifecycle domain witnessの許可一覧へ追加しない。pending・分岐結果の
形式検査を通常placement legの成功へ読み替えない。原MouseButtonInputも入力証拠のまま扱う。

`architect-select-place-cancel --kind Tank`は限定first-click feedbackである。
選択後のowned 1280×720 clientの固定world点[640,300]へ一度だけmotion／Left press／releaseし、
同window／epoch・原入力と同sample／実時間の単一RejectedまたはCompanionPendingだけを検査する。
明示されたBlueprint一覧が不変であることとEscape解除を要求し、committed ownerを認めない。
raw全体のindexed-state-delta形式も検査し、欠落／重複／合成入力・branchは拒否する。
これは最初の配置分岐の技術診断であり、親子commit、完成、lifecycle、正式アート・releaseではない。
開始画像は非blankだけで判定せず、固定normal startupのmap内viewportで
GamePlugin正本のClearColor（srgb 0.1）の近傍が半数以上残る画像を拒否する。
actor／光／UIだけの描画をworld表示済みと誤認しないための補助拒否gateであり、
地形の完全性・品質や独立画像reviewの代用ではない。既存16回／8秒の撮影上限を維持し、
拒否原画像と理由を保存する。入力の再送はしない。

通常worldの`placement_context`は既存Yardのentity／min／maxを最大32件と、
単一active MainCameraによるlogical viewport四隅の投影を読み取るprofiling限定観測である。
同じ明示sessionのBlueprint取消観測は、実refund IDsとdeferred後のowner cleanupを
`BlueprintCancellationApplied / OwnerCleanupApplied`として保持する。Blueprint rawには
UI側の実Entity IDと照合する`entity_bits`も付ける。全予約/inventory清算やSoulSpa取消、
正式lifecycleの`CancelConstruction / Committed`は未対応のまま、partial記録から合成しない。
RestArea planning feedbackは同nonce/PIDのraw hash、実前状態とowner/map tombstoneを検証するが、
ゼロ材料取消の実機証拠と非ゼロ返却の単体試験を混同せず、art/release authorityを付与しない。
2026-11-07の通常RestArea feedbackではSave/Load後のepoch1 ownerに対し、実二段階取消から
配送request1件・actor参照4件の解消、Blueprint/map消失・4tile walkableを同process rawと5画像で
独立確認した。これは進捗0・実搬入材料0の限定passであり、完成assetや全清算の承認ではない。
別case `rest-area-refund`は通常のWood搬入を最大90秒待ち、実搬入数と施工進捗を保存・読込で
保持してから取消し、同ownerの実返却Entity IDs・種類・数量を照合する。資材付与や作業完了を
準備しない。実搬入欠落・完成済み・返却不足は拒否し、このcase追加自体を実機成功へ数えない。
save/load間の材料・progress一致と、再開後の追加通常搬入を区別する。取消時のsame-owner/epoch
raw履歴はloaded量から単調増加し、取消直前の量と実返却ID数・種類が一致しなければ拒否する。
新caseの時間操作は既存HUDのNormal（1x）／Pausedを一度使い、実状態を待つ。取消詳細までは
停止中に移動し、失効・pointer移動・時間遷移timeoutでは入力を再送しない。
2026-11-07の4試行は施工完了競合、追加搬入と旧量条件の不一致、時間再開未成立、
post-release cursor消失でinvalidのまま記録した。v2の部分的な実返却観測をnative passへ転用しない。
実搬入はprofiling-onlyの`hw_jobs::delivery_observations::BlueprintDeliveryReceipts`へ
手運び・wheelbarrowの既存成功branchからのみ記録し、item消費適用後の同epoch/owner/material整合を
確認する。rawの`MaterialDelivery / Committed`はこの限定branchの実結果であり、全site、BuildWork、
入力因果や正式lifecycle全legを証明しない。量のstate deltaだけをcommitへ変換しない。
cursor消失の切り分けには、明示UI observerの`window_focused`を実Windowから受動記録する。
focusを設定・強制したり、cursor/focus guardを免除するものではない。
別case `rest-area-completion`は通常配置後、最大120秒の受動待機で実材料配送・通常施工の
完了を待つ。準備で材料・worker・進捗を付与しない。原domainの`BuildingCompleted`と
同Blueprintを対象にした実Build task終了、5つの実Wood消費receipt、旧owner消失と新buildingの
4tile占有・通行不可（既存`BuildingType::blocks_movement`契約）を同epoch/raw・3枚のowned画像へ拘束する。rawのBuilding `entity_bits`と
Blueprint `materials_complete`は実値の読み取りであり、生成・進捗変更・承認を行わない。
actorsの実`ActiveTaskIdentity`（assignment／current target／work type）も受動記録し、
Build完了を直前の実assignmentへ結ぶ。chain後もassignmentの起点は元配送requestのことがあり、
Blueprint同値へ書き換えない。最終配送以降のtask sampleとdomain完了sampleの順を要求する。
同sampleはExecute後のdomain callbackが先、Lastでのtask message読取りが後になるため、
rawの記録時刻順を実処理の発行順と取り違えない。異sampleの時刻逆転は拒否する。
同kindのstate差やUI完了表示だけでは合格させず、全予約/inventory・完成後の利用/Dream・
art/numeric/performance/releaseは別受入とする。このcaseの追加だけでは実機成功ではない。
明示production観測sessionのrawは通常500ms間隔、実際のheld WASD中は50ms間隔で採る。
これはcameraの通常速度・入力・worldを変更せず、read-only観測の間隔だけを切り替える。
1200 sample／4096 eventのcapは維持する。hostが停止帯通過を観測したら即時拒否して
元のkeyをreleaseし、逆向き補正や再送で成功へ変えない。
通常Manual1ロードのfeedback captureは、実Succeeded receipt後の同window／新epoch／
Hidden・Normal・Closedを2つの後続raw sampleで確認し、最低500msのsettle後に取得する。
verifierも後続sampleを要求するが、rawとGPU呈示の原子的同期を保証するものではない。
全owned PNGを別途独立に実見し、確認dialogが残る旧buffer画像をロード完了証拠へ採用しない。
Yardの作成・拡張、カメラ移動、配置入力、候補点の自動承認は行わない。
欠落／多重／inactive cameraや投影不可はnullとして明示し、viewport点を推測しない。
非finite・反転したYard bounds／camera位置、件数超過は観測失敗とする。
同sampleのprimary window／dpi／epochと組み合わせて通常配置の前提を切り分けるための情報であり、
Yard内という理由だけで占有・配置可否・実commit・release受入を認めない。
同contextの`target_window`はRenderTargetの実window参照でありforeign targetをprimaryへ置換しない。
`logical_viewport_size`、WorldMapによる`grid_min/max`と対応する`grid_world_min/max`も実観測する。
画面外の有限projectionを画面内へclipしたり、欠けたprojectionを再構成したりしない。

`architect-select-place-yard-cancel --kind Tank`は別の限定feedback profileである。
単一の既存・grid整列Yardとowned primaryの1280×720 logical viewport／DPI1を要求する。
Yard内の固定相対trial grid（min X+3、max Y-3）を実quadから投影し、必要な軸だけ
通常W/A/S/Dを各軸1回・最大4秒で送ってworld viewportへ到達する。停止・context異常時も
元の保持keyを解放し、入力を再送しない。原KeyboardInputとcamera変位方向を独立検査する。
選択後はprojection全体が不変であることをpress前に再検査し、原単発Left入力と同frame/timeの
一致grid／CompanionPendingだけを認める。Rejectedや親子commitを成功へ読み替えない。
Blueprint一覧不変とEscape解除を要求し、正式配置・完成・lifecycle・art・releaseのauthorityはない。

`architect-select-place-companion-cancel --kind Tank`はその後の単発Left入力を追加する別feedback。
profilingの`companion_context`は実際の`CompanionPlacementState`と最大512の`BucketStorage` root、
`PendingBelongsToBlueprint`／`BelongsTo`、実stockpile占有を読むだけで、missing resourceはavailable=false。
全BucketStorage markerを数え、Transform欠損はinventoryから隠さず観測failureとする。
pending anchor／center／radiusと実Yard投影を照合し、現Tank 2×2の右隣へ2tile保管所を試行する。
press直前までwindow／epoch／選択／投影／pending contextを再検査し、入力を再送しない。
原Leftと同frame/timeのCompanionCommitted、実Tank Blueprint 4tile、2つの実child rootと
pending parent参照／stockpile占有、親map予約を独立照合する。Escapeは選択だけを閉じ、
確定した計画の親子inventoryと親map予約を残すことを検査する。拒否・rollback・不明結果は成功にしない。
これは通常配置の計画確定だけで、材料配送・建設完了・save/load・cleanup・正式art／releaseは証明しない。

`architect-select-cancel --kind <9種の固定集合>`は別profile
`building-production-local-equipment-feedback-eis-v2`で、`dev.py feedback --build-only`の
profiling feature付き`target/debug/bevy_app`を利用する限定feedbackである。
formal Capture/Memoryや性能のbinaryとは混同しない。入力は既存pointer／keyboard consentから
一度だけConnectToEISし、`native_ui_eis.py`の単一physical monitorと一意なabsolute regionで
配送する。planはEIS transport hashも固定する。
X11のphysical rootに対しEISのlogical region寸法が宣言scaleで一致する場合だけ、
absolute targetを同scaleでlogical pixelへ変換する。regionから任意倍率を逆算しない。
外側のX11 owner/focus/nonce/単発stepと製品のexact cursor ACKは維持し、
Notify混用・再送・region推定・authority付与はしない。
終了時は通常配送guardを再使用せず、原deviceに記録した保持key/buttonのreleaseだけを送り、
stop・device close・connection teardownする。pause/removal済みdeviceへは注入しない。
固定Bで開き、2連続rawで安定した実cardの中心へ一度motionを送り、製品cursorのfresh ACK後に
Left押下・解放を一度だけ送る。primary window／epoch／DPI 1／cardの不一致ではクリックしない。
実MouseButtonInputとBuildContext／PlayModeを同区間で検査し、SoulSpaだけは既存の
TaskContext SoulSpaPlaceを使う。選択画像の前後区間を拘束した後、Escapeで通常modeへ戻す。
Bevy 0.19の`ComputedNode::resolve_clip_rect`はVisible軸を下限−∞／上限＋∞とする。
Architectの`Overflow::scroll_y()`の横軸も同じであり、正規の外向き無限clipだけを有限node／
viewportへ交差させる。NaN、内向き無限値、反転範囲、空矩形、変形matrixは拒否する。
任意座標・任意command・Bridge・fixtureは受け付けない。失敗を再送せずfresh診断に保持する。
これは選択のfeedbackのみで、配置commit、施工、asset readiness、正式art/numeric採否ではない。
同probeの初期7試行はいずれも正式にはinvalid。v6 `tak14-ordinary-input-probe-b885f623-20261005-v6`
では所有X11 windowへのF12押下・解放が輸送側receiptに残り、製品rawには49 sample中に
`KeyPress`／`KeyRelease`がない。押下時間・focus待ち・tap順を変える仮説はここで打ち切る。
v7 `tak14-ordinary-input-probe-69aa99d1-20261005-v7` はprofiling限定observerが
Bevy 0.19の `KeyboardInput` 原メッセージを `ButtonInput` 由来の状態イベントと区別して記録したが、
所有X11 windowへのXTest F12 receiptに対し、48 sampleにはいずれの入力イベントもない。
原 `KeyboardInput` まで届かない観測であり、Xwayland上流の配送を疑う技術診断である。
輸送とobserverを独立に実証した原因確定でも、製品入力成功でもない。
新profile `building-production-local-input-probe-portal-v2` は既存 `native_ui_portal.py` の
同意付きRemoteDesktop輸送へ固定し、所有window、F12、両入力transportのhash、primaryの
所有者専用同意ジャーナルをplanに拘束する。旧invalid batchを再解釈せずfresh rootで試す。
`tak14-ordinary-portal-probe-dcac0f75-20261005-v8` はsource
`dcac0f7541b26ec788b3bee93973e0448c5eac96` の通常world所有windowで同意付き
portal grant、F12押下・解放receipt、製品rawの原 `KeyboardInput` 押下・解放と
`ButtonInput` の `KeyPress`／`KeyRelease` を5 sample／4 event内に記録し、独立verifierで
technical passとして封印・finalizeした。同じprofiling binaryでXTest失敗に対するportal配送成立を確認したが、
技術probeの `accepted`、`performance_evidence`、`art_approved`、`promotion_authority` は全てfalse。
通常worldのCapture/Memory lifecycle、host GPU/native/RSS/application全体handle、9種の性能・
art/numeric採否、release/install/rollbackを証明するものではない。
2026-10-04の通常world技術batch `nonbridge-ordinary-probe-20261004-v1` は
source `ee9410363d4e8f7bd144506a64ac3fb3a71527dd`、Intel Arc/Vulkanの所有X11 clientを撮影し、
nonce/PID一致のraw 3 sample／0 eventと子process RSS/fdを独立verifyしてfinalizeした。
画面は建設物のない通常開始時点であり、9種の表示・入力・lifecycle・性能受入ではない。

offline acceptance、collector、lifecycle verifierは、trustedなin-process `registration_adapter`を明示的に
渡せる。plan/receipt bindingの共通検査は維持し、CLI/spec/environmentから任意adapterを選択できない。
raw lifecycle verifierはprofiling observerが出す `KeyboardInput` 原メッセージを、key／
Pressed/Released／repeat／windowを持つ入力観測としてだけ許可する。これをdomain outcome、
ownerに紐づいたlifecycle witness、または正式入力recipeの成立証拠へ昇格しない。
offline production planは `building-production-results-portal-v2` に更新し、X11基底transportと
Wayland portal transport双方のhashを固定する。収集器は同意付き `PortalX11Input` を使い、
所有者・focus・入力stepのraw ACKを維持する。action logには機密tokenでなくgrant devices、
persistence、session pathだけを記録し、独立verifierはgrant欠落・不正payloadを拒否する。
これは収集器内部の輸送契約であって、normal-developmentのauthenticated admission、
所有capture/ACK、host GPU/native/RSS/application-handle計測器や9種の正式recipeはまだない。
既定のoffline入口は未対応hostをfail-closedで拒否する。歴史的recipe adapterは保存済み証拠の一致検査であり、
JSONの一致は認証ではない。通常の正式実行経路として提供・承認したものではない。
runnerはmodule globalの関数を変更せず、各呼出しにadapterとinstrumentを明示して渡す。
Capture/Memoryはそれぞれのbinary hashで束縛し、開始時と収集終了時のsource再検査を行う。
receiptのbatchはintentのid、時刻は正のintかつplan freeze後でなければならない。
Memory lifecycleもCaptureと同じkind/leg coverageとdomain述語を通す。sessionの整合性だけではleg成功としない。
技術verifyはart/release/promotion authorityを付与しない。

offline verifierの `non-bridge` scopeは先行9種（M2/M3/M5の8種＋別loaderのDoor）を扱う。
Bridgeだけを除外し、Door固有を含む全kind/legをCaptureとMemoryの双方に要求する。
`full` の10種条件は変更しない。9種も累積比較であり、群別の同binary比較へ縮小せず、
異なるsourceの比較元・24 run matrix・全計測値・有限budget・release inventoryを要求する。
9種の構造数はcopyごとに5 root／candidate 11 part（legacyは5 part）。
scope追加は不足producerやadmissionを提供せず、正式実行・美術採用を可能にする変更ではない。

## 昇格・install・復旧・切戻し

保存復元の限定feedback `architect-select-place-companion-save-load --kind Tank` は、既存F5/F9と
実Manual1ボタンを通常入力で使用する。profiling rawはSaveCatalogUiのmode/session、実slot
capabilitiesとcomputed UI camera／primary window／有効・可視rectを読むだけ。古いsession、
別window、非表示・無効祖先、custom camera viewportをclick対象へ変換しない。
fresh job内 `runtime/saves/manual-1.scn.ron` の実バイトhashとManualCatalog／NormalCatalog
成功receiptを束ね、epoch更新・親子Entity置換・同じBlueprint進捗／材料／footprint／保管占有を
検査する。実saveはゲームだけが書き、hostは有界read-only hashのみ。symlink／hardlinkや
既存slot上書きを拒否する。これは計画保存復元の技術feedbackであり、完成lifecycle・全世界の
canonical serialization・性能・アート／数値・release承認へ昇格しない。
`89f40fe2`のfresh通常Tank計画Save/Loadは独立画像reviewと登録verifierで限定technical pass。
原親と2childはLoadで新IDへ置換し、同footprint・進捗0・空材料・保管占有を保持、旧mapは空。
ロード確認dialogなし／Loaded Manual slot1をowned clientで確認した。これは材料配送・完成・
取消のterminal refund・正式asset authorityを受け入れたものではない。
通常RestAreaの予約配置・Manual1保存復元・二段階取消は、`1756c610`のfresh feedbackで
独立owned画像/raw reviewと登録verifierを通過した。ロードで親Entity/epochを置換し、
同じ4tile・進捗0・材料未完を保持、取消後は4tileの占有解除とwalkability復元を確認。
これは既存取消requestと予約cleanupの限定証拠であり、完成資材返却のterminal receipt、
全actor/pool cleanup、正式art/numeric/performance/release承認ではない。

- `plan`はisolated candidate、別美術承認、source／geometry／export inventory、codec hash、現在のauthority／
  locatorを照合する。sourceと導入先は分離し、generationは失敗を含む全割当済み世代より大きくする。
- `apply --apply`は同identityのcandidate-mode native証拠を原本から検証し、その証拠hashに結び付いた
  独立の`building_release_approved`記録を要求する。planの生成やcodec sealだけで昇格しない。
- root単位のlockと永続journalで前状態を保存し、immutable payloadとprovenanceを先に配置する。
  authority更新後、runtime locatorを最後に切り替える。途中失敗を完全な導入とみなさない。
- `recover`はplan／journal／現状態を照合する。dry-runは状態確認のみ、`--apply`で同一の保存入力から
  途中のapplyを継続する。完了済みtransactionは導入済みprovenanceで照合でき、外部競合は拒否する。
- `rollback`は途中applyの復旧後に実行する。旧receipt／payloadを検証して旧locator／authorityへ戻す。
  初回導入ならlocator／authorityを解除する。割当世代とimmutable payloadを削除・再利用しない。
- `install`はrelease-approvedの正本transaction／証拠／承認を検査してruntime asset rootへ複写し、
  locatorを最後に切り替える。既定はdry-run、変更には`--apply`が必要。
  `--rollback`によるmirror切戻しも正本のrolled-back journalを要求し、任意の世代低下を許可しない。
- Wall／Doorの既存promotion/install dispatcherは9種をこの入口へ分岐する。Bridgeのasset処理が可能でも、
  地形・通過・lifecycle・性能・美術の固有受入を他kindから流用しない。

## 検証とHelp

profiling-onlyの`observations_rest`は通常RestAreaのcapacity、両方向のoccupants/reservations、
Soulの実Dream/fatigue/stress/IdleState/Visibility/座標/入口walkability/cooldown、DreamPoolと
実/仮想時刻をread-onlyで採取する。owner最大512、Soulと各relationship最大128とし、
上限超過・非finite状態は収集失敗、欠損component/resourceはnullのままにする。
空のoccupants targetがBevyにより除去された後も、空配列や成功へ置換しない。

同じ明示profiling sessionの`spa_context`はSoulSpa専用の受動観測である。
site最大512、tile最大2048、Soul/WorkingOn source最大128、grid最大512、tile worker最大128を要求する。
phase、必要/搬入Bone、active_slots、PowerGeneratorの出力/係数、実poseとmap owner footprint、
durable `SoulSpaTile.parent_site`、TaskWorkersと全WorkingOn source、GeneratePowerのtile/phase/座標、
SoulのDream/疲労/ストレス、designation/task slots、grid generation/consumption/接続の実値を記録する。
site/tile/Soul/PowerGridのEntityはDebug表現と実`to_bits()`を併記し、保存復元の実mapと数値identityを照合する。
Children/ChildOfからtile集合を推測せず、load後に表示階層がなくても同じ論理ownerを観測できる。
欠損component/resourceと空配列を区別し、orphan parent、非Soul worker、active_slots範囲外なども
正常化や成功判定をせず原状態で公開する。非finite値と件数超過は収集失敗とする。
`outcome_channels`は実cancel/slots message resourceの存在だけを示し、未登録channelを空の成功へ置換しない。
独立MessageCursorが`SoulSpaConstructionCancelOutcome`/`SoulSpaSlotsChangeOutcome`を一度ずつ読み、
既存domain resultと実refund数/適用値を記録する。他consumerのmessageをdrainせず、要求・消失・
state差から成功を合成しない。`Canceled`の返却個数だけでは返却Entity IDs/全cleanupの証明にならない。
sample_index/real_secondsはobserverがmessageを読んだ時点であり、domain適用時刻や入力との因果を表さない。
raw verifierはこの2種の厳密な型・status/payload一致・有界な観測時刻だけを検査し、
domain witnessの許可一覧には追加しない。範囲外の有限値も合法値へ補正せず、別の受入predicateで判断する。
このproducer追加は通常アセット採用、保存schema、配置/利用/発電ruleやHelpを変更せず、
通常入力lifecycle・Save/Load・性能・releaseを受入済みにするものではない。

`soul-spa-planning`は既存Yardから製品validatorで南向き2×2の合法配置先を選び、
準備はcamera/pauseだけを変更する。通常OS入力で未搬入の配置、Save/Load、確認付き取消を実施し、
全4tileのdurable parent/pose/占有、walkability、実cancel結果を検査する限定feedbackである。
実save/normal-load commitの受動receiptは保存バイト列hash/sizeと`spa_context`を持ち、
`spa_entity_remap`は実適用mapからSoulSpaSite/Tile/PowerGridだけを抽出する。既存Soul/RestAreaの
`entity_remap`は変更しない。不完全・重複・曖昧なmap、epoch/原保存バイト/paused stateの不一致を拒否する。
入力journalのnonce/PID/window/step/frame、実controlのhover/pressと対象・session・確認ticket/epochを
原KeyboardInput/MouseButtonInputへ一対一に結ぶ。配置直前は最新の投影点・window/epoch・合法footprint・
SoulSpa modeを再確認し、実placement receiptのowner/4tileも照合する。timeoutや不一致で入力を再送しない。
新helperはnative/Capture/RenderDocの全harness fingerprintへ含め、内容変更で旧証拠を再利用できない。
取消前後の実source relationも検査するが、Bone搬入後の返却、自然利用、発電/通行、全cleanup、
候補アート採用・性能・正式releaseの証拠をこのrecipeから推測しない。

別case `soul-spa-completion`は同じcamera/pause準備と通常配置入力から、実Bone自然搬入を最大180秒待つ。
empty/placed/completedの所有窓画像と同nonceのrawを結び、同site/4tileの維持、Bone0→12の単調増加、
Operational、各tileのGeneratePower/slot1・占有/歩行、workerと実task/逆参照、派生出力とgrid接続を照合する。
途中rawも同epoch/window、site/4tile・pose/親/mapを維持し、単調な実/仮想時刻とHUD通常/停止入力に
拘束された実unpaused区間を要求する。全区間pausedの仮想進行や途中だけ異なるsubjectを拒否する。
資材・worker・完了をseedせず、不成立時にtimeout後の入力再送をしない。これは通常施工の限定feedbackで、
継続利用・Dream消費・枠変更・Operational保存復元・deconstruct・候補アート・性能・releaseは別の受入とする。

`soul-spa-usage`は通常速度の施工3checkpointを再検証し、既存HUDのFast（2倍速）入力で
最大300秒待って実際のGeneratingを確認する。using撮影後は通常速度へ戻して消費を測る。
同じSoul/稼働tileを維持するusing/usedの2画像と全中間rawを追加し、正の出力・grid接続・逆参照、
3〜15仮想秒のDream消費0.5/秒を照合する。隣接rawの減少率と総量を検査し、同仮想時刻での
Dream変化、停止中の消費への付替え、途中のworker/親/map/window/epoch置換を拒否する。
再開/停止は既存HUDの単発入力だけで行い、Dream・worker・発電状態を注入しない。
通常AIの割当条件Dream30以上と自然な移動を待つため、timeoutは成功や再送許可にならない。
通常速度の120秒/300秒比較が既存Dream条件・移動の完了前にtimeoutしたため、延長反復はしない。
受動`relative_speed`を実Timeから記録し、欠損時はnullのまま。rawとUI/actionで実1→2→1と
単発HUD入力区間を照合し、測定区間のunpaused rawが1倍速でない場合やbool代用/不明速度を拒否する。
全session480秒とraw1200 sampleの上限、未成立時のinvalidは維持する。速度を準備コードから設定しない。
撮影入力中の自然な疲労離脱を成功と誤認しないよう、収集開始は実GeneratingのDream20以上・
疲労0.6以下・stress0.65以下を観測できた場合だけにする。測定窓の選択であり、数値設定・
割当・回復・離脱ruleを変更しない。十分な利用者が現れない場合も同じ上限で失敗する。
これは通常利用の限定feedbackであり、枠変更、Operational保存復元、撤去、候補画像の採用、
Capture/Memory性能、正式releaseの証明ではない。実機成功はfresh jobの独立検証後にのみ記録する。

`soul-spa-persistence`は通常速度で施工を最大300実秒待ち、3checkpoint後の**空のOperational施設**を
F5/F9の実catalog入力で保存・復元する。従来completion/usageの施工待機180秒は変更しない。
完成以降は停止を維持し、原保存ファイルのhash/bytesと正規commit、epochの更新、site/四区画/PowerGridの
完全なID置換、pose/slot/指定/占有/歩行/grid出力を照合する。worker/taskがある場合、入力追加・枠変更・取消、
停止中の仮想時間変化や保存間の状態変更を拒否する。未搬入取消caseの既定条件は変更しない。
これは空施設の永続化だけを扱い、occupied task/Soul remap・自然離脱・撤去・候補表示・性能・releaseは別受入とする。

`soul-spa-slots`は自然施工/利用5checkpointを再検証後、通常worldの同siteを実選択し、
既存情報パネルの枠ボタンを通常速度で4→3→2→1→0と単発操作する。重なり時は既存の
epoch/revision付き候補pickerでexact ownerを選び、盲目的なworld click再送はしない。
全4件のSoulSpaSlotsChangeOutcomeを原MouseButtonInput、対象、値、sample/timeへ結び、
ゼロ枠で停止撮影→通常3〜15仮想秒→停止撮影を追加する。全tailのowner/4tile/grid/window/epochと
同Generating Soul/tileの維持、ゼロ枠以降のtask割当集合不変を要求し、新規割当や追出しを拒否する。
GoingToTileはWorkingOn追加前から数え、閉鎖前に割当済みのSoulの到着を新規割当と誤認しない。
全session480秒/raw1200上限は維持する。これは有界な既存利用継続だけで、自然離脱・再開放時の
割当・occupied保存復元・候補表示・性能・正式releaseを証明しない。実機成功は別途記録する。
profiling observerは実4tile中心とsite中心の最大5点を既存SelectionResolverへ読み取り照会し、
同ownerがDirect Objectとなる点と実順位を`selection_target/selection_point`へ出し、先頭となる点を優先する。
実owner/所有tileの座標と投影点へ拘束し、不明・非有限・別対象・Snapped・解決点なしはpress前に拒否する。
選択・候補順・world/map・actorを変更しない。visible controlsはclip外の候補行を含まないため、
controls不在だけで施設が選択候補から欠落したとは断定しない。
全5点で先頭にならない場合も、実候補に存在する点から一度だけ通常clickし、同ownerを含む実pickerの
epoch/revision/anchor/inventoryを照合する。clip外なら既存ScrollAreaへ最大8回の実下wheelを送り、
実MouseWheel witnessと輸送press/releaseを1対1で拘束する。release前のwheel、別window、再構築、
無進行、overflow、camera/world変化を拒否し、fully-visibleのexact owner行だけを一度選ぶ。
ScrollPositionや選択状態を直接変更せず、通常UI/入力/候補順位は維持する。
利用枠caseだけ入力ACKを「次のfreshな同対象観測」へ拘束し、従来の各motion/button後の固定3frame索引待機を
除く。cursor/pressの実一致、所有/nonce/原input対と全上限は維持し、clientへACK方針を記録する。
他caseの待機は不変。利用枠操作前に、初期Generating全員が既存の安全な利用測定条件を満たすことを要求する。
一人だけの健全な測定窓から他の疲労した利用者の継続を推定せず、自然離脱と枠変更を混同しない。

UI feedbackの`rest-area-usage`は通常施工の3checkpointに入所・放出の2checkpointを追加する。
camera/pause準備以外は既存入力と通常AIを使い、occupant・Dream・材料・完了状態を注入しない。
UIとproduction rawは同じread-only collectorを使い、paused・同nonce/PID/epoch・同仮想時刻・
有界な実時刻差で結ぶ。正しいcapacity/inverse relation、非表示かつ歩行可能な入口にいる同一Soulの
連続Restingと、既存`DreamTransferredVisualMessage`のexact owner/source/amountを照合する。
0.5以上のSoul Dream減少とmessage総量、DreamPoolの増加を確認し、移動・世代置換・
Sleeping由来や異ownerのmessage・不整合な時刻/数量を拒否する。
UIのPostUpdate撮影後、Lastのraw writerが同じcheckpointの状態・時刻を記録するまで
最大15秒だけ待ってから子を終了する。入力は再送せず、1秒以内の実時刻差と全状態一致を
緩和しない。foreign nonce/PID・収集失敗・timeoutは検証失敗とする。
これは通常利用と正のDream放出に限定した技術feedbackであり、退出/cooldown、利用中save/load、
全状態・美術/数値・性能・正式releaseの全受入を代替しない。実行・独立verify・画像reviewは別途必要。

別caseの`rest-area-exit`は通常利用の5checkpointをすべて再検証してから、同Soulの自然な
Dream枯渇・退出後の1checkpointを追加する。同ownerの正のfinal transferと総放出量、
関係の双方向解除、可視復帰、walkableな座標、0より大きく15秒以下のcooldownを照合する。
全放出量以上のPool増加を要求し、退出後Dreamは既存awake最大0.5/秒・15秒cooldownの
実経過と適用frame最大0.5秒から得る再生成上限以下とする。厳密な残量0とは主張しない。
これら定数とslow step上限が変わる場合はRust契約試験が失敗し、verifierの再検討を要求する。
raw区間内の世代/epoch置換・退出後の再入所・cooldown再増加を拒否する。
退出を注入せず、cooldownの満了や利用中save/loadの受入までは主張しない。

明示production観測sessionではsave ownerの非保存`DurableReceipts`も登録する。
実atomic commit後の`DurableSave`はstreamed container全bytes、実normal-load成功の`DurableLoad`は
read container全bytesと実Entity remapをrequest/epoch/RestArea snapshotへ結ぶ。
raw observerは`SaveLoadApplySet`後にだけdrainする。失敗/rollback/recoveryを成功にしない。
上限・除外範囲は[save_load.md](save_load.md#明示profiling-sessionの保存読込診断)を正本とし、
このproducerの存在だけで利用中save/load、全world canonical一致や正式releaseの受入を宣言しない。

`ui_usability_acceptance.py --case rest-area-save --smoke`は通常の完成/入所/Dream放出5checkpoint後、
pause中のF5/F9と既存Manual slot 1選択/確認だけを行う。7 owned画像、保存実bytes/hash、
Save/Load commit receiptと全Soul/RestArea実mapを照合し、vitals/位置/Idle/可視性/Poolと
両方向の入所/予約関係がremap後も一致することを要求する。名前や近い座標から同一性を推測しない。
利用中の同一Soul/RestArea pair、paused raw/checkpoint、request/epoch/時刻の不一致を拒否する。
Visibilityは非保存shellなので既存rehydrateのprojectionを固定する。入所Soulは保存前後ともHidden、
非入所SoulのVisible/Inheritedは読込後Inheritedだけを許し、Hiddenや欠損を同値扱いしない。
receiptから撮影までの全raw sampleでpause/epoch/仮想時刻/収集状態が不変なことを要求する。
観測区間は既存UI sessionの480秒と240 sample以内に制限し、単純な撮影待ち秒数を鮮度に代用しない。
これは限定feedback recipeであり、実機で未実行の段階を受入済みとはせず、全world canonical、
全inventory、cooldown満了、art/numeric/performance/releaseへ昇格しない。

2026-11-07の`tak14-rest-area-occupied-save-feedback-20261107-v2`は登録/別実行verifierと
全7画像の独立read-only確認をpassし、seal/finalize済み。実Manual1は496,766 bytes、SHA-256
`6912a29eac134f34f68f0f0ef3ec7dfe7b018bf4d9a6438074de9fc0d4821f09`で両receiptと一致。
Save sample310/epoch0→Load sample367/epoch1、全20Soul+1RestAreaの実mapと4予約を確認し、
Soul `1842v0→807v1`はowner `2320v34→1955v1`への入所関係、Hidden、Dream
`1.1400237083435059`を保持した。Save310→撮影327とLoad367→撮影384の全rawは停止/状態不変。
Intel Arc/Vulkan、WARN/ERRORなし。表示は未承認assetではなく灰色fallbackで、美術採否ではない。
読込後の自然退出/cooldown満了・全inventory・正式9種導入の不足は残る。

次の限定`rest-area-lifecycle` recipeは上記7checkpointの後、実mapで復元された同じSoulの
自然なDream枯渇/退出/正のcooldownと、cooldown componentの実消失までを2checkpointで延ばす。
退出は実final Dream messageと総放出量/Pool増加で照合し、満了までは再入所/予約・timer再増加や
停止を拒否する。remainingと仮想経過は既存slow-step最大0.5秒以内の差だけを許す。
初回消失がその期間より早ければ拒否し、その後の通常再入所は早期入所と混同しない。
全区間のsame Soul generation/epoch、単調な実/仮想時刻と最終paused全contextを照合する。
recipe実装や単体試験だけで満了を受入済みとせず、fresh実機/独立確認を必要とする。

`tak14-rest-area-postload-lifecycle-feedback-20261107-v1`は通常配置/搬入/完成/入所/Dream放出、
利用中save/load、実mapの同Soul退出/cooldown満了まで全9画像と登録/別実行verifier、
独立read-only確認をpassしseal/finalizeした。実Manual1は496,786 bytes、SHA-256
`da7457dafd6c5f303106d46f094484f5e0c5c98d221c8e26bb9b7ed0be69c017`、両receiptと一致。
Save416/epoch0→Load473/epoch1の全21map/収集状態を確認。選定Soul `1856v0→855v1`は
owner `1908v19→1833v1`へのHidden入所とDream `2.0300469398498535`を保持した。
読込後17放出の合計`2.03004778549`とPool増分が整合し、唯一finalはsample520。
同sample/virtual `91.815720774`でcooldown15.0、sample607/virtual `106.815720774`で初回消失。
満了前の再入所/予約なし・同世代/epoch・単調減少を確認。Intel Arc/Vulkan、WARN/ERRORなし。
灰色fallbackの限定技術証拠であり、正式art/numeric/performance/releaseや全inventoryは未受入。
旧退出/保存比較jobは用途終了後に撤去し、結果は履歴として残す。最新jobだけを正式視覚世代の
ordinary lifecycle退行比較に保持し、旧binary/verifierを再実行する依存は作らない。

profiling-onlyのproduction raw observerとBridge／M2 probeは共通writerでfresh outputと一時pathを検査し、各snapshotの一時ファイルを
`create_new`で排他的に作成してからrenameする。既存ファイル・dangling symlink・失敗した一時成果を
上書きせず拒否する。これは収集の安全性であり、ordinary-worldの欠落producerや正式受入を補わない。
production rawは累積samples/eventsを借用Serialize envelopeへ渡し、中間Valueへの全履歴複製を行わない。
JSON値・順序・ordinal・公開頻度と64MiB/1200samples/4096events上限、同期write/sync/rename、
失敗tmp保持は不変。128samplesごとに収集/encode/writeの処理時間とbytesを有界に記録するが、
これはobserver自体の負荷診断で、ゲーム/renderer性能や正式受入の証拠ではない。

`tak14-soulspa-ordinary-slots-20261108-v5`は全7actual-window画像/独立確認と登録verifyを通り、
同ownerの4→0実入力・5.25仮想秒の既存2worker維持/割当集合不変を確認した。
これはfallbackの限定非追出し証拠だけで、wheel分岐・occupied保存・採用asset表示・正式性能/releaseを補わない。
旧4診断jobは用途終了後に個別撤去し、結果は現行計画/台帳へ残す。

別case `soul-spa-occupied-save`は通常usage prefix後の停止中Manual1保存/読込を検査する。
実Soul/Spa remapで以前のGenerating Soulの永続vitals/poseとsite/4tile/map/gridを拘束し、
AssignedTask/WorkingOn/TaskWorkersの正規resetは永続値と分ける。
停止中のLogic停止により残る派生出力は保存値とのexact比較を保ち、0へnormalizeせず稼働継続とも扱わない。
receipt/capture間の全rawの永続projection・runtime reset・pause/仮想時刻/epoch/window不変、
実bytes・正規request/session・原入力を要求する。既存empty caseのoccupied拒否は維持する。
これは新しい検証経路の契約であり、fresh実機pass以前は利用中保存を受入済みとしない。
通常再開後のdirty rebuild/再割当・自然退出・採用asset表示・性能/releaseは別の未検証範囲である。

RestAreaの採用済み隔離候補は、別case `rest-area-candidate-lifecycle` / mode `candidate` /
scope `ordinary-rest-candidate-lifecycle-v1`でのみ通常worldの検証用admissionへ渡す。
既存ArtPreview feedbackのauthority/mode/case受付と、通常起動のlocatorは変更しない。
主担当の計画・実行・再verifyでpackaged art/numeric approvalとsource/geometry/依存bytesを既存pipelineにより検査し、
同じmanifest/codec/base assetsのoverlayだけを作る。自然建設/利用・Manual1占有保存/読込・自然退出の
既存lifecycleへexact consumer identity/root/part/材質検査を加える。
candidate scopeは専用validatorが明示し、rawの自己申告から選ばない。未知scopeやpreview/release authorityの混入を拒否する。
実機pass前はこの入口の実装だけで受入済みとせず、pass後も性能・promotion/install/release権限を付与しない。

SoulSpaの採用済み隔離候補は別case `soul-spa-candidate-occupied-save`、mode `candidate`、
scope `ordinary-spa-candidate-occupied-v1`に限定する。通常施工/利用/占有Manual1保存復元の既存7checkpointを
維持し、source/codec/制作validator原本・基準assetsと隔離overlayを固定する。既存RestAreaとSpa fallbackの
scopeは変更せず、独立validatorからだけ専用scopeを渡す。body+4slotの実mesh、parent/pose、白/opaque/lit、
albedoとemissive画像を、同checkpointの実4tile TaskWorkers maskへ照合する。保存復元後はruntime worker resetに
従う非発光を要求し、保存派生出力から稼働継続を推測しない。candidate原本や通常worldへ資材/worker/完了をseedしない。
この追加は実機成功・再割当/自然退出・性能・正式release受入ではなく、fresh実行と独立画像/raw reviewを必要とする。

後続の`soul-spa-candidate-resumed-use`は`candidate-resumed` modeと
`ordinary-spa-candidate-resumed-v1` scopeで同じoccupied prefixを再検証し、通常4倍速入力による
以前のworkerの実remap後の再割当・追加Dream消費・Dream閾値付近の関係解除を3場面へ追加する。
入力対、epoch/window、構造、派生出力の通常Logic再構築と実slot発光を照合し、保存派生出力だけから
再開発電を推測しない。観測された状態差は未収集のdomain終了receiptとは区別する。
旧caseの受入・通常起動・gameplay・保存形式・480秒/1200sample上限を変えない。
2026-11-08の全工程case v1は退出前に上限へ達してinvalidであり、受入成功へ読み替えない。

全工程の固定上限に収まらない場合も上限を緩めず、`soul-spa-candidate-loaded-use`を別subjectの
fresh jobとして使う。`candidate-loaded` mode / `ordinary-spa-candidate-load-v1` scopeは検証済み
occupied jobの原本・7画像・raw/input/checkpoint・候補依存hashを固定し、実Manual1だけを新sessionへ
コピーする。過去施工を現sourceの証明に数えず、今回の通常F9/選択/確定の唯一Loadと保存時Soulから
今回のremapを検査する。初期worldと保存worldのanchor/footprint/draw_pos不一致は拒否する。
停止中reset→自然再割当（Super）→追加消費（Normal、3–15 virtual秒）→自然解除（Super）の5画像で、
実window/epoch/入力対・構造・発光・Dream消費と関係解除を検証する。Save/資材・worker・Dream seedなし。
480秒/1200sample・入力上限を維持し、historical入力と今回の限定結果を分離する。正式release/性能は別条件。

2026-11-08のloaded-use v1（source `a0f2c5ce1a4f76245c0734a6c370bb2f5238791232bf89908c55620d5554ca40`）は
5画像/rawの独立reviewと登録verifierを通過した。保存時1849v0→今回854v1、同tile2349v1で
6.088962112 virtual秒にDream26.38408470→23.33959770、直前Dream9.78866768から作業解除を観測。
別workerは残存しており全員退出ではない。過去施工、未収集domain終了receipt、性能・releaseはこの結果に含めない。

M2 probeは有効化された検証sessionだけで0.1秒ごとのdue sampleを即時公開する（最大1800 samples、
categoryごと最大128 entities）。観測・同期書込みの負荷を含むため性能証拠には使わない。
M2 native runnerは画像の撮影前後でraw prefix/sessionを照合し、最大2秒の待機で新sampleを要求する。
全観測区間のstate/pause/camera/identity/owner/epochが不変な画像だけを採用する。
固定1280x720 clientの中央160x160が単色の画像もrun/verify両方で拒否する。
拒否画像・理由は別pathに残し、checkpointは最大48試行／24採用、最終画像は最大8試行に制限する。
このblank拒否や安定raw区間はrenderer frameの原子性・対象の描画完了・美術採否を保証しない。
独立画像比較を別に要求し、fixture投入による水量・精製状態をordinary-input lifecycleへ読み替えない。
この検証専用補正は通常起動・player input/UI・gameplay・save・Helpに影響しない。

`assets::building_asset_set`のunit testは9種×3 authorityのschema、異常入力、policy、receiptを検査し、
Bevyのmemory AssetServer経由で欠落・改竄・正常ロードを確認する。
M1-a2はdecode可能な最小GLB／PNGでtyped load・寸法・世代・失効を、M1-bは表示consumer・root／part・
状態・cleanup・world resetを検査する。M1-cはcodec互換、exact候補許可、昇格／復旧／切戻しと
native証拠の拒否条件を検査する。これらの合成fixture試験を正式assetの美術・描画受入とは扱わない。

本節のasset入力境界と2026-10-04のadapter refactorはHelp影響No impact。Bridge配置契約を取り込んだ変更は
別途Update requiredとして既存provider/snapshotへ反映した（[Help契約](help-screen.md)参照）。
release binding未指定の通常起動はlocator／load requestを発行せず従来fallbackを維持する。
明示的な`HW_BUILDING_ASSET_RELEASES`は独立承認済みの2種／4種／8種／9種の完全な組を
既存consumerへ接続する運用入口であり、player操作やUI設定ではない。不正なbindingは
fallbackへ暗黙縮退せず起動前に拒否する。
建築種類、通常操作、成立条件、gameplay結果、save schema、配置rule、文言は不変なので、Help catalog／provider／
coverage snapshotは更新しない。候補投入は引き続き明示profiling sessionだけで、codec／export／promotion／installは
開発用offline入口である。binding対象kindやplayer-visible契約を広げる場合は改めてHelp影響を判定する。
将来のmanifest追加をHelpレビュー対象から漏らさないよう、`.buildingset`をruntime data分類とtestへ追加した。

2026-11-10に利用者が別途許可したTank→Mixer数量保存修正はgameplayの補給成立条件を変更する。
Boolean水入りバケツを全杯5個に統一し、取水/投入を全量だけに制限する（[物流契約](logistics.md#44-mudmixer-水搬入-deliverwatertomixer)）。
これは上記asset-only変更のNo impact判断とは別の`Update required`で、既存Helpとsnapshotへ反映する。
旧sourceのv10で観測した5個取水→2個投入はinvalidの診断証拠として保持し、修正後sourceの成功へ読み替えない。
fresh sourceの通常入力native結果と独立reviewが揃うまでは、この修正も実機受入済みとは扱わない。
修正commit `8ba10ec6`のfresh Supply v11は部分投入を起こさず、製品Mud10満杯でWater3のまま停止してinvalid。
全量保存の実handler/割当/需要回帰と品質gateは通過したが、Tank由来の投入・精製の全工程証拠は未達である。
通常StockpileはStasisMudを受け入れないため、その追加を搬出解決と扱わない。
次の通常入力campaignには正規の床/壁需要・TaskArea・材料/猫車経路を明示し、旧試験を再解釈せずfresh subjectで確認する。

### M5採用候補の通常入力planning（2026-11-12）

`47e48c405e0b191b5b607b1650ac334fee7513bc`、source
`0216714f9037b8873c690f45c604e500f65484ea6a0876c66dec29f4786a67dd`の4専用caseは、
各6画像/21実入力を完了し、登録verifierによるsealと独立画像/rawレビューが成功した。
scopeは`ordinary-m5-candidate-planning-v1`、全caseでepoch/window/PID/nonceと同Blueprintの選択を維持した。

| 対象 | raw samples/events | manifest SHA256 |
| --- | --- | --- |
| SandPile | 30/80 | `215c4481f954c9121680ef9df85c027a586a12b23ad860c93f12721b557c4df2` |
| BonePile | 32/73 | `c63e2100e8c03f8b1ad470b2bc8d1b6b1585c11d4bce24e80aefc657c96bf15c` |
| WheelbarrowParking | 31/68 | `58ce2ae6981f3ae3836f1a6bb51101dca3c2fa0fb71ba40d98834d621d67de86` |
| OutdoorLamp | 35/83 | `13a7655f0c705eef911d31ad36879769510355fe9e966a2ad980700abe9d632d` |

catalogは各採用候補の画像、配置後は青い未完成Blueprint、選択後は同kind/進捗0の実InfoPanelを確認した。
Parkingの既存ownerも同candidate画像・poseを保持した。小さい半透明ghostはBlueprintと重なるため、
画像単独の色判別には限界があり、rawの合法/占有色・実画像・可視性との対応で評価した。
WARN/ERROR/panicなし、Intel Arc MTL/Mesa26.2.6/Vulkan/X11。自然完成、採取/車両利用/配電、
save/load、稼働性能、独立art再採否、promotion/install/releaseの証拠には含めない。

### 通常worldのdevice allocator観測

明示profiling観測sessionのsampleへ`gpu_allocator_observation`を追加する。
Bevy0.19のmain WorldのRenderDevice/RenderAdapterInfoから、wgpu29のsafeな
`generate_allocator_report()`を使い、Vulkan device allocatorの瞬時allocated/reserved bytes、
allocation/block件数、実adapter/driverを`wgpu-vulkan-allocator`として記録する。
両totalの実一覧とのchecked sum、block範囲とcapacityを検査する。device/adapter欠損、非Vulkan、
report None、算術不整合は明示unavailableで、値を0やRSS推定へ置き換えない。
実際の空reportだけはavailable/0とし、取得不能と区別する。
これは全GPUメモリ、専用VRAM、peak、描画完了やGPU時間の計測ではない。
backendのlockとreport一覧生成の観測費用を伴い、全allocation名をrawへ出さない。
既存`gpu_measured_bytes:null`/`performance_evidence:false`、正式性能oracleのsource受付と
art/release権限は維持する。fresh nativeによる接続確認と通常worldの正式計測producerは別工程である。

`ea3c8077`の採用候補Lamp planning接続試験では31sample全件でavailableを確認した。
最終sampleはallocated 771,944,984B / reserved 1,543,503,872B、allocation 521件 / block 12件。
Intel Arc MTL/Mesa26.2.6/Vulkan/X11、6画像/21入力の原本を登録verifierと独立read-only reviewで
照合し、pass封印/finalize済み。既存`gpu_measured_bytes`は全件null、`performance_evidence`はfalseを維持した。
これはdevice allocator DTOの実接続確認だけで、peak・全GPUメモリ・正式性能budgetへの合格ではない。

### 専用galleryのDPI契約

`building_art_acceptance.py plan --variant gallery --scale-factor 1|1.5|2`は新planへ
`window_contract=building-art-window-v1`と`scale_factor`を一緒に固定する。
有限な数値の厳密集合だけを受け付け、bool・片方の欠損・別contract・未知DPIを拒否する。
両fieldのない旧planだけはDPI1として解釈する。既存context/UI variantは引き続きDPI1専用である。
物理窓とowned PNGは1280×720のまま、実Windowの開始/終了DPI、logical寸法、RTT品質と
scene/mask target、camera viewを別々に照合する。DPIをPNG寸法へ掛けた推測値を採用しない。
この入口の実装だけでquality/DPI全matrixや群内調和、最遠での判読性、独立art/release採否を完了とはしない。
新planはtyped `HW_BUILDING_ART_SESSION`にもwindow contractとDPIの対を渡す。
Rust static configは完全なsessionを読み、requested DPIとの一致を要求する。1.5/2はgalleryだけで、
context・Spa mask page・通常性能fixtureはDPI1のまま。新契約のsessionは実static workloadへ限定する。
初回`bff4440e`のDPI1.5試行はこのRust接続前に描画前拒否となり、invalid封印済み。この初回では実機接続を証明していない。
接続補正`cffd9fe3`のfresh試行は登録verifier・独立画像/raw review・pass封印/finalizeを完了した。
実Windowは開始/終了DPI1.5、physical1280×720、logical853.333313×480、High RTT1280×720。
Empty/Partial/FullのTank候補を確認し、WARN/ERROR/panicはない。winitの推定DPI2ログは実観測値と区別した。
source `fac859ddd7bb861d43a21bafa98671c83daf8945c0be8934ffb6745421472bfe`、
manifest `95b7a072aafc12cbffcbba2609ec4d05b5df7678ceb2838bf2ff8d311f1de752`。
別fallback群の上端は一部clipされており、群内調和・正式全matrix・性能・releaseの証明にはしない。
