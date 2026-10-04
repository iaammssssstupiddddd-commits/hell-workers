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

検証対象は**pauseしたsmall静止fixture（36棟）のpresentationのみ**。exact identity／nonce、30 stable frames、
初期・終了inventory一致、owned X11 clientの1280×720画像、指定adapter／Vulkan、警告・エラーなし、
binary／source／asset／原本hash一致を検査する。jobは`promotion_authority=false`であり、自動美術承認ではない。
この証拠は稼働状態、save/load、施工・撤去・移動全経路、各kindの性能予算、全体移行完了を保証しない。
基盤前後のコスト比較は別々に凍結した静止参照jobを使う。本書更新時点でM1-cの実native受入は未実施。

### 通常開発におけるproduction受入toolingの境界

`python3 scripts/building_production_native_acceptance.py capabilities` はlocal supportを読むだけで、
旧request/Run/hostへの照会、台帳更新、plan生成、build、launchを行わない。現在は `available=false`。
ordinary-world process/window・capture/ACK producer、GPU/native/RSS/application-handle instrument producer、
通常開発のauthenticated admission adapterが未提供である。旧統括を再起動して補完しない。

offline acceptance、collector、lifecycle verifierは、trustedなin-process `registration_adapter`を明示的に
渡せる。plan/receipt bindingの共通検査は維持し、CLI/spec/environmentから任意adapterを選択できない。
既定のoffline入口は未対応hostをfail-closedで拒否する。歴史的recipe adapterは保存済み証拠の一致検査であり、
JSONの一致は認証ではない。通常の正式実行経路として提供・承認したものではない。
runnerはmodule globalの関数を変更せず、各呼出しにadapterとinstrumentを明示して渡す。
Capture/Memoryはそれぞれのbinary hashで束縛し、開始時と収集終了時のsource再検査を行う。
receiptのbatchはintentのid、時刻は正のintかつplan freeze後でなければならない。
Memory lifecycleもCaptureと同じkind/leg coverageとdomain述語を通す。sessionの整合性だけではleg成功としない。
技術verifyはart/release/promotion authorityを付与しない。

## 昇格・install・復旧・切戻し

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
