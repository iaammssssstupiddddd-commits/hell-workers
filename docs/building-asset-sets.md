# 建物アセットセットの読み込み

制作方針は[building-art-direction.md](building-art-direction.md)、移行順序は
[移行計画](plans/3d-rtt/non-wall-floor-building-art-migration-plan-2026-09-19.md)を正本とする。
本書はroot `bevy_app::assets::building_asset_set` が所有する入力境界を記す。

## 実装範囲（M1-b）

`.buildingset` のschema、authority、依存ファイルの実バイト数とSHA-256を検査する。
`StartupPlugin` はasset型・policy resource・loaderを登録し、`VisualPlugin`はkind単位のpoolと
表示更新systemを登録する。ただし通常起動からのlocator選択と読み込み要求はまだない。
既存Wall／Doorのloaderと表示経路は継続し、Bridgeは別件解決まで対象外とする。

manifest loaderの`Loaded`は**manifestと依存バイト列の検証済み**だけを意味する。
poolは、明示的に`BuildingAssetPool::request`を呼ぶconsumer向けに、GLBの先頭primitiveを
`Mesh`、PNGを`Image`として要求し、依存を含むload成功とpreview寸法を確認してから世代を
activeへ昇格するpoolを追加した。pending世代はactiveと分離し、失敗時は既存activeを保持する。
M1-bではpoolを`VisualPlugin`へ登録し、`release_approved`のactive descriptorだけを3D設備、2D表示、
配置・移動preview、Blueprint、pulse、カタログへ同じ更新境界で公開する。
production向けprojection／promotion生成器も未接続であり、現時点のschemaは未公開の初期版である。
正式assetと状態別のwater高さ・rotor軸等の制作値は、制作時に制作側と照合して確定する。

## 種別と必須role

roleは下表の順で、meshを先に、imageを後に並べる。余剰・欠落・重複・並べ替えは拒否する。
各artifactのrole文字列は`mesh:`／`image:`を接頭辞とする。

| kind | mesh | image | part | 代表状態 |
| --- | --- | --- | --- | --- |
| Tank | body, water | albedo, world_preview, catalog | body, water | Empty |
| MudMixer | body, rotor | albedo, world_preview, catalog | body, rotor | IdleAngleZero |
| RestArea | body | albedo, world_preview, catalog | body | Empty |
| SoulSpa | body, slot | albedo, slot_emissive, world_preview, catalog | body, slot0〜slot3 | OperationalMaskZero |
| WheelbarrowParking | なし | world, catalog | なし | WithoutVehicle |
| SandPile / BonePile | なし | world, catalog | なし | Static |
| OutdoorLamp | なし | world_off, world_on, catalog | なし | Off |

8種以外はdecode時に拒否する。制作contract fixtureのrole・leaf数・代表状態との一致をtestで照合する。
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
正式な承認・生成器による昇格判断はM1-cの責務で、runtimeが承認を作ることはない。

## typed residencyと世代pool

- `BuildingAssetPool::request`はidentity（kind・generation）とmanifest handleをpending世代として
  保持し、同じkindで試行済みのgeneration以下を再要求しない。戻り値`BuildingAssetRequest`は
  `Started`／`PendingOccupied`／`AlreadyAttempted`の受付結果を表す。
- mesh roleはglTFの`Mesh0/Primitive0`、image roleは通常の`Image`として読み込む。
  AssetServerのload失敗・再帰依存失敗はいずれも世代失敗として記録する。
- world／catalog previewはmanifestの`canvas_px`と実画像寸法を照合する。不一致は昇格しない。
- 全typed assetが揃った時だけpendingをactiveへ原子的に切り替える。旧activeのstrong handleは
  切替時に即座にdropし、pool内へretired cacheを保持しない。失敗したpendingも破棄し、
  同じkindの既存activeを維持する。
- `invalidate_active`はidentityが完全一致するactiveだけをdropする。他kindとpendingには影響しない。
- 通常起動はlocatorをrequestしないためfallbackのまま。明示投入されたactiveのうち、
  `release_approved`だけを次節のproduction consumerへ公開する。

## production表示接続

`PostUpdate`でpoolのpoll、3D structure同期、2D／preview同期、`ApplyDeferred`を順にchainし、
Transform伝播とUI prepareより前に完了する。後着generation、pause中の切替、新規consumerも
同じ境界でactive descriptorへ揃える。world置換ではpoolを維持し、world entity側を再構築する。

### 3D設備

Tank、MudMixer、RestArea、SoulSpaはlogical ownerごとにmeshを持たない`EquipmentRoot`を1つ持ち、
descriptorのmesh、local transform、共有generation material、`RenderLayers`、`Visibility`を持つpartを
childとして生成する。`Building3dVisual`はrootだけに付くため、既存のworld transform同期がpartの
local transformを上書きしない。generation切替ではpart集合を一括交換し、active失効時は従来の
mesh・位置・中心高・材質・状態表示へ戻す。owner消失、重複root、world再構築も同期時に整理する。

Tankの水面、MudMixerの回転部、SoulSpaの施工状態と4区画maskは既存gameplay stateを正本としてpartへ
反映する。descriptorは論理footprint、通行、生産、光源、save schemaを変更しない。

### 2D設備とpreview

Parking、SandPile、BonePile、OutdoorLampのworld spriteに加え、8種の配置ghost、Blueprint、pulse child、
移動先preview、Tankのpartner ghost、表示中のcatalog cardへdescriptorを適用する。Lampは既存の通電状態で
off／on画像を選び、旧gray tintを二重適用しない。適用前のimage、size、anchorをentityごとに保持し、
active失効やmode変更では従来値へ復元する。再利用ghostはDoor、対象設備、bucket／Wall等の非対象へ
切り替わるたびにbaselineを入れ直し、前のgenerationのanchorやHandleを漏らさない。

production consumerはpoolとは別にauthorityを再確認する。候補authority、欠落／失効したactive、
locator未接続の通常起動では常に従来fallbackを使用し、Wall／Door／Bridgeの専用経路を変更しない。

## 検証とHelp

`assets::building_asset_set`のunit testは8種×3 authorityのschema、異常入力、policy、receiptを検査し、
Bevyのmemory AssetServer経由で欠落・改竄・正常ロードを確認する。M1-a2のtestは実際にdecode可能な
最小GLB／PNGを使い、typed load、preview寸法拒否、active/pending分離、失敗時のactive保持、
世代昇格、invalidateと旧handleの解放を確認する。これはruntime契約のtestであり、制作物の見た目や
描画受入を代替しない。

M1-bのfocused testはkind別atomic publish、後着generation、active失効、ownerごとのroot／part数、
状態part、world／ghost／Blueprint／pulse／移動先／catalog、owner cleanup、world resetを検査する。
release-approved fixtureによる接続試験であり、正式assetの美術受入ではない。

Help影響はNo impact。表示consumerは接続済みだが、通常起動にはlocator／load requestと正式release assetがなく、
player-visibleな表示は従来fallbackのままである。建築種類、操作、成立条件、文言、成功・失敗結果も不変。
将来のmanifest追加をHelpレビュー対象から漏らさないよう、`.buildingset`をruntime data分類とtestへ追加した。
