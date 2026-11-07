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
- `numeric_approval_sha256`: M2のisolated_candidate/release_approvedに独立numeric decisionの小文字64桁hashを要求する。
  art_previewと他kindには指定できない。M2 release receiptにも同じhashを要求する。
  production_stateとこのhashはmanifest内容hashに含まれる。安全な数値範囲を満たすだけでは独立採否にならない。
  optional fieldは未指定時にserializeから省略し、非M2の既存canonical bytesを維持する。

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

`tools/blender_ai_workflow/scripts/building_asset_pipeline.py export`は既存のrole別GLB／PNGとrecipeを
包装する。geometryや画像を新規生成しない。source／geometry／artifactの実bytes・hashとrole順を確認し、
content-addressed artifact、locator、provenanceを出力する。`art_preview`は承認なし、`isolated_candidate`は
独立した`building_art_approved`記録とpreview identity完全一致を要求する。exportではreleaseを発行できない。

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

profiling-onlyのproduction raw observerとBridge／M2 probeは共通writerでfresh outputと一時pathを検査し、各snapshotの一時ファイルを
`create_new`で排他的に作成してからrenameする。既存ファイル・dangling symlink・失敗した一時成果を
上書きせず拒否する。これは収集の安全性であり、ordinary-worldの欠落producerや正式受入を補わない。

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
