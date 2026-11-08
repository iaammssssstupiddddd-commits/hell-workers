# 建物アセットセットの読み込み

制作方針は[building-art-direction.md](building-art-direction.md)、移行順序は
[移行計画](plans/3d-rtt/non-wall-floor-building-art-migration-plan-2026-09-19.md)を正本とする。
本書はroot `bevy_app::assets::building_asset_set` が所有する入力境界を記す。

## 実装範囲（M1-c候補）

`.buildingset` のschema、authority、依存ファイルの実バイト数とSHA-256を検査する。
`StartupPlugin` はasset型・policy resource・loaderを登録し、`VisualPlugin`はkind単位のpoolと
表示更新systemを登録する。通常起動はrelease binding未指定ならlocatorを選択せず、読み込み要求も
発行しない。`HW_BUILDING_ASSET_RELEASES`を明示した起動だけが、M2の2件、M3を加えた4件、
M5を加えた8件、Bridgeを加えた9件の完全な組を、各kind 1件の
`release_approved` identityとcanonical locatorで要求する。
既存Wall／Doorのloaderと表示経路は継続する。Bridgeのschema・pool接続は実装済みだが、
正式asset導入と固有lifecycle/native受入は別工程であり、実装だけでは承認しない。

manifest loaderの`Loaded`は**manifestと依存バイト列の検証済み**だけを意味する。
M1-a2のpoolはGLBの先頭primitiveを`Mesh`、PNGを`Image`として要求し、依存loadとpreview寸法を確認して
pendingをactiveへ切り替える。M1-bではactive descriptorを3D設備、2D表示、配置・移動preview、
Blueprint、pulse、カタログへ公開する。通常consumerは`release_approved`だけを許可する。
M1-c候補はoffline codec／export、明示profiling候補投入、昇格・install・復旧・切戻しを追加した。
これはTAK-14候補の実装契約である。2026-10-04に通常Orcaのprimary由来の
`codex/tak14-product-integration`へ製品sourceを選択取り込みしたが、現在は統合検証中であり、
独立review、実機受入、正式asset公開、美術受入の完了ではない。
正式assetと状態別のwater高さ・rotor軸等の制作値は、制作時に制作側と照合して確定する。

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
- `numeric_approval_sha256`: M2/M3のisolated_candidate/release_approvedに独立numeric decisionの小文字64桁hashを要求する。
  art_previewと他kindには指定できない。M2/M3 release receiptにも同じhashを要求する。
  production_stateとこのhashはmanifest内容hashに含まれる。安全な数値範囲を満たすだけでは独立採否にならない。
  optional fieldは未指定時にserializeから省略し、全art_previewと非M2/M3の既存canonical bytesを維持する。
  M3はproduction_stateを持たず、保存・利用・Dreamのgameplay数値は変更しない。

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
基盤前後のコスト比較は別々に凍結した静止参照jobを使う。本書更新時点でM1-cの実native受入は未実施。

制作器の `building_asset_pipeline.py verify` はpackaged bytes・provenance・既存承認identityを読むだけで、
承認を作らない。M5 `prepare --destination` はfreshな隔離jobへ出力できるが、既存出力、原画の親子path、
primaryの製品assetsへの出力は拒否する。入力原画はstagingに限定しworld画像を再加工しない。
`verify --isolated` はその出力をread-only検査する。標準の出力先は引き続きstagingである。

### 通常開発におけるproduction受入toolingの境界

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
