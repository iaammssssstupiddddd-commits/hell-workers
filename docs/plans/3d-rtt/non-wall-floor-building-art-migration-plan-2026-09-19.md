# 壁・床以外の建築物のアート移行計画

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `non-wall-floor-building-art-migration-plan-2026-09-19` |
| ステータス | `Draft` |
| 作成日 | `2026-09-19` |
| 最終更新日 | `2026-09-19` |
| 作成者 | `Codex` |
| 関連提案 | `N/A`（ユーザー依頼による計画） |
| 関連Issue/PR | `N/A` |
| 制作仕様 | [building-art-direction.md](../../building-art-direction.md) |
| 調査基点 | `94ccdf23`＋同sessionで定義した制作仕様の未commit差分 |

本書は全10種の移行順、制作物、表示接続、受入条件を所有する。実装・制作・実機受入は未着手。
個別意匠の最終判断はゲーム内の候補画像で行い、本計画の作成をアート受入やreleaseとして扱わない。

## 1. 目的

- 解決したい課題: 設備の共通Cuboid、建物間の画風差、完成表示と旧previewの不一致により、地図上で用途・状態を読み取りにくい。
- 到達したい状態: モデルが外形・開口・可動部を、テクスチャが手描きの線・材質・色面を担当し、全対象が同じ世界の設備として読める。
- 成功指標: 全10種の完成表示・配置・施工・カタログが整合し、実状態・save/load・撤去に追従する。新しい表示が論理占有・通行・生産・照明の意味を変えない。

## 2. スコープ

### 対象（In Scope）

`BuildingType::ALL`のうちWall / Floorを除く10種。Doorは既存productionの適合監査と不足経路の修正を行う。
建築用asset原本、モデル、テクスチャ、2D画像、preview、asset読み込み・切替・fallback、表示用状態、検証用scenarioを含む。
TankのBucketStorage、Parkingの実物の猫車、休憩者やSpa workerは既存entityを維持し、表示の位置・重なりを確認する。

### 非対象（Out of Scope）

- Wall / Floorの形状・テクスチャ・asset世代の変更。比較画面には現行releaseを固定して置く。
- 材料、建設時間、生産量、回復・発電量、通行、Room、Light Fieldのrule変更。
- 新BuildingType、橋の長さ変更・回転、Doorの新操作や連続開閉animation、移動可能建物の追加。
- Soul / Familiar、資源item、猫車本体のアート再制作。Stockpile / Yard等のゾーンも対象外。
- 全小物の3D化、新しいoutline・照明renderer、過去RtT全段階の再受入。

## 3. 現状とギャップ

寸法は表示画像の余白ではなく、`hw_jobs/src/placement_geometry.rs`の論理占有を示す。
旧アセットマイルストーンの1×1設備表や全GLB候補表を、新規制作の寸法・表示分類の正本にしない。

| 対象 | 占有・経路 | 現状 | 移行で必要な表示 |
| --- | --- | --- | --- |
| Tank | 本体2×2＋別配置のcompanion 2×1、Structural3d | 共通Cuboid、空／途中／満杯の材質交換 | 桶・内壁・独立水面、3水量状態、バケツ置き場と干渉しない接地 |
| MudMixer | 2×2、Structural3d | 共通Cuboid、Idle／Activeの材質交換 | 固定槽・架台＋独立攪拌部、実精製中の回転 |
| RestArea | 2×2、Structural3d | 共通Cuboid、既存Dream粒子 | 布屋根・支柱・入口、空／利用中と既存粒子の整合 |
| SoulSpa | 2×2、Structural3d、完成後も歩行可能 | 建設中から共通Cuboid、専用site/tile経路 | 建設中／稼働可能／実稼働区画。低い4区画と骨の構造 |
| Bridge | 2×5、Structural3d、RiverYMin anchor | 茶色Cuboid、中心高0.09 tile | 橋床・支持部・必要な縁、両岸接続とSoulの通過 |
| Door | 1×1、Structural3d、EW/NS | generation 7の承認済み3状態GLB＋preview | 現行アートの適合監査、カタログを含むpreview統一。必要な差分だけ改修 |
| WheelbarrowParking | 2×2、Foreground2d | 専用PNG＋別entityの猫車 | 木枠・轍・駐車枠。実物の猫車がある／ない状態を妨げない |
| SandPile | 1×1、Foreground2d | 砂item iconと同じPNGを参照 | 建物専用の低い砂山。無限sourceのため残量減少表現を付けない |
| BonePile | 1×1、Foreground2d | 既存PNG、Lampでも流用 | 建物専用の骨山。少数の大きな骨で輪郭を作る |
| OutdoorLamp | 1×1、Foreground2d、歩行可能 | 完成本体はBonePile画像流用、通電で色変更 | 専用の骨支柱・籠、点灯／消灯。既存論理光源と同期 |

### 実装から確認したギャップ

- 設備4種は`building_completion/spawn.rs`で同じ2×2 meshを使い、Bridgeは別の2×5 Cuboidを使う。
- `building3d_cleanup.rs`は`Building3dVisual`に対してowner絶対座標・固定中心高を適用する。可動childへ同markerを付けるとlocal位置を壊す。
- Tank / Mixerの材質syncはroot上のmesh materialを要求する。複数partの状態同期は未実装。
- 通常ghost、Blueprint、pulse child、移動preview、load再構築、既存カタログに画像handleを持つ経路が分散している。
- カタログは生成時に`UiAssets::building_preview()`のhandleをcloneする。Doorも旧`door_closed`を返し、production previewの後着・世代切替を反映する経路が必要。
- SandPileの建物と砂item iconは同じ画像ファイル、BonePileはLampの代用画像として共有される。既存ファイルの上書きでは無関係な表示まで変わる。
- SoulSpaは通常Blueprintを経由せず、Constructingから`Building`と3D visualを持つ。完成eventだけを監視すると段階変化を取りこぼす。
- `docs/building.md`にはTank本体2×1の古い記述がある。M0で実占有2×2、companion 2×1へ同期する。

Doorの既存release・未完closeは[Door計画](production-door-art-plan-2026-09-05.md)と
[preview修正計画](door-preview-alignment-plan-2026-09-11.md)が所有する。本書はそれらの実施済み受入を再計上せず、
新仕様との差と今回変更する経路だけを扱う。旧型枠trackの残件を本計画へ移さない。

## 4. 実装方針（高レベル）

### 4.1 制作と表示分類

モデル・テクスチャ・光の分担は[制作仕様](../../building-art-direction.md)に従う。
新規GLBはTank / MudMixer / RestArea / SoulSpa / Bridgeの5種とし、Doorは既存3GLBを監査する。
小物4種は2D表示を維持し、手描き原図または同じ視点の制作モデルから専用PNGを作る。
小物用の制作モデルを用いても、そのGLBをruntimeへ持ち込む必要はない。

| 対象 | 制作物と初期構成 | 状態の正本・同期方法 |
| --- | --- | --- |
| Tank | 固定桶1 mesh＋水面1 mesh、albedo、代表preview | `StoredItems`とcapacityの既存3分類。空は水面非表示、途中／満杯は所定高さへ。正確な連続水量表示とは主張しない |
| MudMixer | 固定槽・架台1 mesh＋攪拌部1 mesh、albedo、停止姿勢preview | `MudMixerVisualState.is_active`（実`Refining`）。可動部だけVirtual Timeで回転しpauseで停止 |
| RestArea | 固定body 1 mesh、albedo、preview | 実occupantsから空／利用中を得る。既存Dream粒子を入口・屋根に合わせ、予約を入所扱いしない |
| SoulSpa | 固定body 1 mesh＋共有の稼働部meshを4配置、albedo、必要なemissive、preview | `SoulSpaPhase`、tileのdurable parent、実`TaskWorkers`から建設段階＋4bit稼働mask。建設中は共有施工材質＋既存骨材進捗、稼働部は消灯。`active_slots`は実稼働の代用にしない |
| Bridge | 固定body 1 mesh、albedo、preview | 完成・建設中・撤去は既存lifecycle。長軸と通行域は変更しない |
| Door | 現行3状態mesh、albedo、EW/NS previewを優先再利用 | 既存Door state・topology consumer。カタログ代表画像はEW Closed |
| Parking / SandPile / BonePile | 各専用world PNG＋同じ原本によるpreview | 静的表示。猫車・資源itemの実体と画像を分離 |
| OutdoorLamp | 専用点灯／消灯PNG＋代表preview | `PoweredVisualState`。新画像と旧gray tintを二重適用しない。光源位置・半径・効果は既存契約 |

表のmesh数は制作開始時の部品構成上限であり、実測draw call数ではない。
固定部の結合・共有を優先し、可動部・状態部以外で増やす場合は理由と予算をM0の契約へ記録する。
triangle数、画像解像度、表示高さ、preview canvas・anchorは無地モデルの投影で決め、着色前に確定する。
壁の240 trianglesやDoorの256px canvasを全設備へ流用しない。

### 4.2 asset単位と切り替え

新規9種には共通schema・loaderと型別role一覧を用い、建物種別ごとに独立した世代・readinessを持たせる。
Doorの既存doorsetは維持する。9個のloader複製や全建物一括でしかreleaseできない構造は作らない。

- 各setにkind、generation、原本・exportの識別、mesh/texture/preview role、partのlocal transform・pivot、canvas・anchor、hashを持たせる。
- 同種全instanceは有限のmesh・texture・material poolを共有する。Spaの各区画も共有meshとoff/on材質を使い、ownerごとのmaterial cloneを作らない。
- 同一kindの全必須roleがreadyな同じ世代だけをactiveにし、完成表示とpreviewの解決結果をまとめてpublishする。各consumerが同じrevisionをrender extract前に反映する順序を固定する。
- asset欠落・不正・世代不一致は、そのkind全体を既存fallbackへ戻す。正常な他kindのreleaseやDoorは巻き込まない。
- 切替前のセットを旧参照が残る間に破棄しない。切替後は非active partと強参照を解放し、世代切替の反復でpoolが増えないことを確認する。
- 既存のstaging、ArtPreview、承認済みcandidate、releaseのauthorityを新設備にも適用する。未承認assetの通常起動への流入を防ぐ。既存Wall/Doorの検査を緩めない。

### 4.3 3D rootと部品の所有

新規3Dの5種は、logical ownerとは独立した`Building3dVisual`を1 rootだけ持ち、描画部品をその子へ置く。
rootは表示modeに対応した基準transform・visibility・状態を持ち、meshを持たない。各描画partに別markerを用いる。
Door / Wall / Floorのroot形式は変更しない。

- production rootのXZは既存logical ownerへ対応、接地Yは0。制作GLBは足元中心を原点とし、部品のpivotはlocal座標で定義する。
- fallbackは同じroot entityを使い、root Yを旧中心高（設備0.4 tile、Bridge0.09 tile）、Cuboid childのlocal Yを0とする。これにより中心高をscaleと独立に保ち、完成bounce中も旧transformを再現する。
- ground rootに旧中心高の固定child offsetを足す方式は採らない。root scaleをs、旧中心高をhとすると中心がs×hへ動いてしまうためである。mode切替時はroot原点・part集合・previewを同時に切り替える。
- ownerの移動・回転・完成bounceはrootへ一度だけ適用し、水面高さ・攪拌角は子localへ適用する。描画childへ`Building3dVisual`を付けない。
- 各描画partに`RenderLayers`と必要なLight Field sampling tagを明示する。回転する羽根の座標から照明の論理anchorを再定義しない。
- legacyのroot material更新をこの5種から分離し、表示用状態→part consumerでmaterial・visibility・transformを決める。
- owner消失、cancel、解体、world replacement、世代交換で不要childを除去する。通常生成・初期配置・Instant Build・loadは同じfactoryを使う。
- 現行diagnosticsのroot数と描画part数を区別する。meshのないrootが存在するだけで表示成功とは判定せず、各leafのresident handle・visibilityを確認する。

上記変更はM1でfallbackの見え方を保ったまま成立させる。Bevy 0.19の`ChildOf` cleanup、visibility伝播、
`RenderLayers`、glTF primitive取得は実装時にローカル一次資料／docsrs-mcpで再確認する。

### 4.4 previewと状態の全経路

画像、描画canvas、anchor、代表状態、active generationを共通の表示記述から取得する。
論理占有は既存shapeを参照し、余白を含むPNGの外形から配置可否を計算しない。

更新対象は、カタログの既存`ImageNode`、配置ghost、Tankの確定済み相方ghost、通常Blueprint、
施工pulse child、Tank/Mixer移動preview、SoulSpa専用配置・建設表示、load後のshellである。
世代変更・asset後着はこれら全consumerへ届くrevisionとして公開する。
3Dの同一原本から59°・yaw 0・RtT縦補正を合わせてpreviewを生成する。2Dは同じ原図を用途別canvasへ配置する。
建設中の色、資材カウンタ、progress bar、配置不能理由は既存表示を維持し、装飾へ埋め込まない。

状態の読み取りはpause中にも行う。アニメーション時間だけVirtual Timeへ従い、load時はdurable stateから
再構成する。asset handle・part entity・回転角をsave schemaへ追加しない。
Spaの3D childを論理`SoulSpaTile`の階層へ混ぜず、ConstructingのcancelとOperationalの解体を別経路で確認する。

### 4.5 crate ownershipと候補ファイル

| 所有先 | 作業 |
| --- | --- |
| `bevy_app/src/assets/` | 新共通assetset loader、authority、readiness、root所有の画像解決。既存Wall/Doorと必要な検証部だけ共有 |
| `bevy_app/src/plugins/startup/`、`plugins/visual.rs` | pool注入、初期化、state publication→presentation→Transform伝播の登録順 |
| `hw_core/src/visual_mirror/` | root/leaf間で必要な表示専用状態。domain型やasset authorityを流入させない |
| `hw_jobs` / `hw_energy`とroot adapter | 既存正本から表示値を投影。生産・回復・発電ruleを新visual側へ複製しない |
| `hw_visual/src/` | 建物part、アニメーション、材質・表示同期、preview用handle契約。rootへの逆依存なし |
| `bevy_app/src/systems/jobs/building_completion/spawn.rs`、`systems/visual/building3d_cleanup.rs` | factory、既存fallback移行、owner変換とpart consumerの分離 |
| `systems/logistics/initial_spawn/facilities.rs`、`interface/selection/`、`systems/save/rehydrate/` | 通常以外の生成、移動、専用Spa、Blueprint shell、resetへの接続 |
| `systems/jobs/deconstruction/`、`systems/jobs/soul_spa_construction/` | 既存owner lifecycleから全partを解放、建設段階の投影 |
| `bevy_app/src/assets.rs`、`hw_ui/src/setup/submenus.rs`とUI adapter | カタログのready後／世代変更後の画像更新。ゲームECSの読み取りはroot adapter |
| `tools/blender_ai_workflow/`、asset同期tooling | 原本・モデル・UV・opaque albedo・preview・manifestの生成と検査 |
| native Skillのhelper、`scripts/perf_tool/`、profiling fixture | 新設備用のcurrent-source recipe・独立verifier。旧凍結profileは変更しない |

候補ファイルは変更責務の入口であり、既存の大きなrootファイルへ全処理を追記する指示ではない。
新module名・共有型はM1で責務に沿って決める。新crateは作らない。

## 5. マイルストーン

推奨順は **M0 → M1 → M2 → M3 → M4 → M5 → M6**。
M2の2設備で制作・動作・preview・releaseまで一巡させ、その確定した方法を後続へ適用する。
各群を受入後に独立導入できるようにし、全10種の制作が終わるまで先行群を未releaseに留めない。

### M0: 寸法・状態・制作境界を確定

- 変更内容: 全10種のfootprint、接地、可視高さ、作業位置、許可された向き、状態一覧を表に固定。Tank/Mixerの無地ラフで投影を確認し、画像・mesh予算を決める。
- 変更ファイル: 本計画、`docs/building.md`、`docs/art-style-criteria.md`、`tools/blender_ai_workflow/fixtures/`（新設備契約）。原本は外部staging。
- 完了条件:
  - [ ] 表の全10種と`BuildingType::ALL`が一致し、Tank寸法・旧アセット計画の新規制作範囲を文書同期。
  - [ ] 5種の接地・part構成、9種のasset role一覧、全previewのcanvas/anchor、役割別資源上限が確定。
  - [ ] 性能fixtureの分布・状態・環境・比較方式・数値budgetを、結果取得前に固定。
  - [ ] Door g7の継承範囲と、既存計画所有の残件を記録。
- 検証: schema/geometryのfocused検査、docs検査。無地の原本previewを本番表示の合格証拠にはしない。

### M1: 共通の読み込み・part・preview経路

- 変更内容: §4.2〜4.5の最小共通実装と新設備用feedback/受入recipeを用意し、Tank/Mixerの技術候補を通す。
- 変更ファイル: §4.5のasset・visual・factory・preview・save入口、authoring/tooling、native helperとprofiling fixture。
- 完了条件:
  - [ ] fallbackが従来と同じ形・位置・状態を保ち、5種のroot/child管理と通常生成・load・cleanupが通る。完成bounceの開始・中間・終了とmode切替中も旧pivotを照合する。
  - [ ] 同一kindのmesh/texture/previewをatomicに切り替え、欠落・不正・世代混在・復旧の経路が検証される。
  - [ ] root countとpart count、pool上限、pause中の後着・load、既存カタログ更新を確認。
  - [ ] `building-art`用recipe（新規）のfeedback、art-preview、正式candidateを区別できる。既存Door helperへ設備を偽装しない。
- 検証: loader/state/lifecycle/previewのfocused test、§7共通gate、fallback同等性のactual-window確認。

### M2: Tank・MudMixerの制作と先行導入

- 変更内容: 無地形状→素材の色面→面別UV→線と筆跡の順で2種を制作し、3水量・回転状態と全previewを接続。
- 変更ファイル: 原本・2種assetset、asset catalog、building preview・move経路、Tank/Mixer visual consumer、関連仕様。
- 完了条件:
  - [ ] 通常表示・最遠表示で2種を識別でき、色と模様が形を埋めない。
  - [ ] 空／途中／満杯、精製開始／停止、pause／再開、水・泥itemと可動部の重なりが正しい。
  - [ ] Tank companion、移動成功／拒否／取消、建設・load・解体で足元とownerが一致。
  - [ ] §7の当該行と当該変更の性能budgetを満たし、アート判断・candidate受入・2種releaseを記録。
- 検証: 状態・移動・資源正本のfocused test、2種gallery・lifecycle、対象性能比較、通常authorityで表示確認。

### M3: RestArea・SoulSpaの制作と導入

- 変更内容: 休憩所の屋根・入口と既存粒子、Spaの建設段階・4区画を制作。実occupancyの表示用投影を追加。
- 変更ファイル: 原本・2種assetset、`hw_core/src/visual_mirror/`、rest/energy adapter、Spa専用配置・施工・cancel・save経路、関連仕様。
- 完了条件:
  - [ ] 休憩者の既存非表示・復帰とDream発生位置を保持し、予約数を利用中表示へ含めない。
  - [ ] SpaのConstructing→Operational、4bit全組合せの対応、代表0/1/4区画の実画面と停止中loadを確認。
  - [ ] Spaの歩行可能な4tile、骨の搬入、発電出力・配電は既存正本と一致。
  - [ ] M2と並べて画風を確認し、当該受入・Help判断・2種releaseを完了。
- 検証: phase/maskとcancel/deconstructのfocused test、利用開始／終了storyboard、対象性能比較、通常authority確認。

### M4: Bridge制作・Door適合監査

- 変更内容: 2×5固定橋のモデル・texture・previewを制作。Doorはg7を制作仕様へ照合し、旧カタログ画像をproductionへ接続。
- 変更ファイル: Bridge原本・assetset、factory/transform/preview入口、DoorのUI画像解決、関連仕様。不適合がある場合だけdoorset revision。
- 完了条件:
  - [ ] Bridgeの両岸接続、橋端／中央でのSoul通過、隣接橋、完成bounceを確認。高さと描画が通行を誤解させない。
  - [ ] Bridgeの通常建設／Instant Build／施工cancel／load／解体後の川の通行復元が既存ruleと一致。
  - [ ] Doorの両軸×3状態と現在の接続・固定枠が新仕様へ適合。適合した既存assetは再制作しない。
  - [ ] Doorの完成・ghost・Blueprint・pulse・カタログを同じ世代に統一。改修scopeに応じた再受入を実施。
- 検証: Bridgeのgeometryとlifecycle、Doorのpreview後着・UI更新。Doorのmeshを変えなければ全density/Memoryを再実行せず、変更経路に絞る。

### M5: 小型Temporary 4種の画像更新

- 変更内容: Parking / SandPile / BonePile / OutdoorLampの専用world画像とpreviewを制作。現在のForeground2d分類を維持。
- 変更ファイル: 外部原図、4種assetset、`asset_catalog.rs`、2D factory/preview/catalog、powered visual consumer。
- 完了条件:
  - [ ] 4種を通常／最遠表示で識別でき、線・色面が導入済み3D建物と調和する。
  - [ ] 砂・骨item iconと猫車本体の画像が意図せず変更されない。Parking画像に猫車を描き込まない。
  - [ ] Lamp専用画像で点灯／消灯が読め、配電変更・供給喪失・loadと同じ表示更新で同期。既存暗色tintとの二重適用なし。
  - [ ] world/ghost/Blueprint/pulse/catalogの画像・anchorが一致し、4種の当該受入・releaseを記録。
- 検証: 画像役割の分離・power状態のfocused test、密集時・壁/Soul付近・明暗場所のactual-window確認。2Dの常時前景を3D depth対応済みと扱わない。

### M6: 全10種の共存確認・文書同期・close

- 変更内容: 通常authorityの同一現場に全10種と現行Wall/Floor/Soulを置き、画風・識別・preview・lifecycleを確認。
- 変更ファイル: `docs/building.md`、`docs/art-style-criteria.md`、`docs/assets_workflow.md`、`docs/blender-setup.md`、`docs/rendering-performance.md`、必要なrest/energy/save仕様・crate README・Help、本計画と索引。
- 完了条件:
  - [ ] 全10種が「新asset導入」または「既存asset適合確認＋必要経路修正」のいずれかで閉じている。
  - [ ] 個別受入から変更がない項目は結果を再利用し、最終混在で新たに生じる問題だけを追加検証。
  - [ ] 保存・再開・失敗復旧、asset不正時fallback、有限pool、対象性能budgetの結果がそろう。
  - [ ] Help impact reviewと§7のfull gate、storage checkを通し、恒久仕様へ移管。本計画をarchiveまたは削除して索引更新。
- 検証: 全10種混在の通常authority storyboard、未検証範囲の明記、§7共通gateとclose確認。

## 6. リスクと対策

| リスク | 影響 | 対策 |
| --- | --- | --- |
| テクスチャで細部を盛りすぎる | 縮小時に黒く潰れる | 無地→色面→描線を段階確認し、最遠比較で情報量を減らす |
| 複数partと旧root前提が衝突 | 二重表示、絶対座標の上書き、不可視 | root/leaf markerを分離し、render layer・state consumer・診断も同時変更 |
| 接地原点と旧Cuboid中心が混ざる | 浮き・沈み・previewずれ | production接地原点とfallback中心原点をmode別に同一factoryで管理。bounce中も照合 |
| 後着assetやUIのhandle clone | 完成とカタログが別世代 | kind単位のactive revisionと全consumer同期、開いたままのUIも検査 |
| 資源画像の共有を上書き | 本体以外の表示も変化 | 建物専用roleを新設し、非対象画像のhashを比較 |
| Spaのphaseとworker対応を誤る | 建設中発光・稼働数誤表示 | durable parent＋実workerからmirror構築、ConstructingとOperationalを別検査 |
| 部品・材質数がinstanceごとに増える | 描画負荷・メモリ増加 | 有限pool、固定part上限、N/4Nと世代反復で検証 |
| Doorの既存仕事を再開する | 重複制作・長い受入 | g7を監査し変更面だけ再受入。旧trackのcloseは元計画へ残す |

## 7. 検証計画

### 共通gateとHelp

Rust変更の各batchではrust-analyzer診断、focused test、次を実行する。

```bash
python3 scripts/dev.py check
python3 scripts/dev.py cargo -- clippy --workspace --all-targets -- -D warnings
```

広い実装batchの完了・通常導入前と計画完了時は`python3 scripts/dev.py verify`を必須とする。
全workspace testは同gateの実行結果で確認し、理由なく同じtestを重複実行しない。
`git diff --check`、docs index/link検査、asset生成・loader/helperのfocused testも変更範囲に合わせる。

各production batchは`hell-workers-review-help-impact` Skillで実入力から表示までをレビューする。
水位・稼働・点灯・建設中の見分け方を説明する必要がある場合はprovider・coverage・exact snapshotを更新する。
美術差替えだけで説明が不変の場合も、到達経路と理由をno-impact判断へ残す。計画文書だけの変更はruntimeへ反映されない。

### 固定条件と必須ケース

新規`building-art` profileをM1で用意する。既存P02の凍結fixtureやWall/Door用asset inventoryへ代入しない。
現行production factoryを通る対象に対して、source・asset・binary・harnessのidentity、owner、part role、
実状態、投影位置、画像hashを結び付ける。描画前の画像、fallbackを本制作と誤認した画像を成功にしない。

| ID | 必須ケース | 証拠の分担 |
| --- | --- | --- |
| A1 | geometry、UV、法線、部品原点・pivot、画像role、previewの投影・anchor | export後のGLB/PNG検査＋無地・着色比較 |
| A2 | 3D root exactly one、期待leaf数、2Dは3D rootゼロ、同種有限pool | ECS test＋actual-window sidecarと画像 |
| A3 | asset遅延／不正／欠落／世代不一致／復旧／反復切替 | loader・ECS test。復旧の代表画面で同一世代と非増殖を確認 |
| A4 | 通常配置／建設／初期配置／Instant Build、cancel、完成bounce | 存在する経路を種別ごとに列挙し、通常操作storyboardとfocused testで分担 |
| A5 | ghost／Blueprint／pulse／move／catalog、ready後とUIを開いたままの更新 | 投影比較・UI同期test＋actual-window代表画像 |
| A6 | Tank3状態、Mixer開始停止、Rest空/利用中、Spa建設/0/1/4稼働、Lamp点消灯、Door2軸×3状態 | 全状態ruleのtest。実状態と画像を結ぶstoryboard、動作は複数時点を採取 |
| A7 | Tank/Mixer移動成功・拒否・取消、companion、save/load・rollback・pause中load | 既存通常経路のtest＋代表actual-window。world置換後最初のpresentation frameから再構成 |
| A8 | 通常解体・Spa施工cancel、owner消失、load反復 | 親子・pool・正本resourceのtest。残像・孤児leafゼロを確認 |
| A9 | 壁/床/Soul/運搬item、橋の両岸・隣接橋・通過、Door seamと支持変更 | 実画面。geometryから通行・Room・照明ruleを変更しない |
| A10 | 描画quality/DPI・通常/最遠zoom・明暗場所・全10種混在 | 以下の対象限定matrixと最終通常authority storyboard |

反復中はHigh/DPI 1の対象群を同じdev cacheで確認する。正式アート候補は対象群を同じgalleryへまとめ、
High/Medium/Low × DPI 1.0/1.5/2.0の9 caseで通常・最遠zoomを採る。部品運動・pause・状態遷移は
High/DPI 1のstoryboardで別確認し、静止画をanimationの証拠にしない。
新規profileでは各行の画面位置・十分な描画面積を固定し、最遠で対象が消えたgalleryを合格にしない。
Doorを改変しない場合は、新規gallery内の混在確認と変更したカタログ経路で足りる。既存9 caseを無条件に再開しない。

### 性能と資源の予算

M0で新profileのN/4N分布、停止・稼働割合、画面内面積、warmup/measurement、比較対象とbudgetを固定する。
初期fixture案は全10種各4棟のN=40、各16棟の4N=160とし、Bridgeやcompanionを含め合法配置できる範囲へ確定する。
静止galleryに加え、Mixer稼働・Spa区画・Rest粒子を含む動作caseを設ける。停止中だけの測定を動作時へ一般化しない。

- 差替え前と候補のseed・論理状態・kind数・画面条件・adapter/backendをそろえる。同じbinaryの明示legacy-controlを基本とし、assetの読込・保持差も記録する。
- frame p95/p99、native peak live bytes、RSS、mesh/material/textureのresident数とbytes、root/part数を比較する。
- 各対照は隣接・順序反転を含む3反復、正式な時間比較は30秒warmup＋60秒measureを基本とする。中央値とMADを併記する。
- 時間・RSSの許容差と追加asset bytes予算はM0で決め、候補の結果を見て緩めない。壁用の+5%や+4 MiBを測定条件の違う設備へ無条件には移植しない。
- N→4Nで共有asset数が増えないこと、世代切替・load反復後に非active poolが残らないことを必須とする。entity/instance bufferの必要な増加は別計上する。
- 描画構造の説明が必要な場合だけRenderDocを追加する。部品数をdraw call実測値と呼ばない。

### 実機実行と検証データ管理

正本は[保存管理ワークフロー](../../development-infra/validation-storage-workflow.md)と
[native acceptance Skill](../../../.codex/skills/hell-workers-run-native-acceptance/SKILL.md)。
以下は実装後の手順であり、計画作成時にはjob・worktree・binary copyを作らない。

1. 各開始・再開時にprimaryの保存管理規則を読み、対象candidate/cacheの現在の利用者を確認する。
2. primaryの`python3 scripts/dev.py validation`でretain/plan/executeを管理し、新設備helperが返す直接kitty launcherを使う。
3. 修正中は既存feedback dev buildを再利用。正式subjectをfreezeした後、Capture→Memoryを逐次実行し、sourceやassetを途中編集しない。
4. 実adapter/backend/window、owned client画像、nonce/ACK、state sidecarを独立verifierで照合。headlessはcorrectnessだけに使う。
5. 成功・失敗・中止をsealし、使い終えたjobを整理、finalize/checkを通す。変更した新subjectには新jobを使う。
6. フィードバック中の同じcandidate workspace/Cargo cacheを保持し、最終acceptance/closureまで撤去しない。固定日数・容量・全job archiveを要求しない。

各実行時に次を本計画または後続の群別記録へ追記する。exact保持pathはprimary台帳にも登録する。

| 項目 | 計画作成時の状態 |
| --- | --- |
| batch / owner / consumers | 未作成。実行時に群名と判断対象を登録 |
| subject / asset view / job root / workspace | 未作成。通常primary以外の作業場なし |
| 開始bytes / 結果 / 最終成果物の正本 | 新規検証出力0。原本は既定外部asset root、採用先はcanonicalとruntime mirror |
| 削除path / 前後bytes / filesystem差 | 削除なし。実行batch終了時に実測記録 |
| 残存path / owner / consumer / next action / release_when | 本計画専用の保持なし。他sessionの既存hold/cacheは変更しない |
| review状態 / 最新提示日時 | 未開始。実装後の提示・修正ごとに更新 |

## 8. ロールバック方針

- kindごとのasset世代を戻せる単位にする。mesh・texture・preview・local transform記述は同じ世代へ戻す。
- 初回release前は既存Cuboid／PNGをfallbackとして保持。後続releaseは直前の承認済み世代とlocator前像を復旧先にする。
- 失敗した候補を消して復旧させず、authority/locatorを正しく戻してactive poolを再解決する。
- 部品・previewが復旧世代へそろい、論理ownerと保存データが維持されることを確認する。
- asset更新で救済できないコード回帰は該当実装batchの差分を確認して修正する。他sessionの変更を破棄しない。

## 9. AI引継ぎメモ（最重要）

### 現在地

- 実装進捗: `0%`。計画作成とread-only調査のみ。
- 完了済みマイルストーン: なし。前提の制作仕様は同sessionで文書化済み。
- 次の作業: M0。全10種の寸法・状態・画像consumerを確定し、Tank/Mixerの構造ラフと予算契約から始める。
- 生産asset、code、外部原本、Help本文は本計画作成では変更していない。

### 次のAIが最初にやること

1. 現在のdirty差分・並行sessionと本計画の作業範囲を分ける。前turnの制作仕様や別sessionのCI計画を破棄しない。
2. `building-art-direction.md`と下記参照を読み、M0のfixture・寸法・状態表を確定する。
3. asset原本と候補の制作は既定stagingで開始する。モデル／画像制作時は該当Skillを使い、アートを最終判断する前に具体的なゲーム内比較を用意する。

### ブロッカー/注意点

- 全設備用のnative recipe・asset schemaは未実装。既存Wall/Door recipeを名前だけ変えて設備の受入済みにしない。
- 新root形式はroot数だけの既存監査では不十分。visible part、材質、layer、asset readinessまで調べる。
- SpaのConstructing、Tank companion、カタログ後着、砂icon共有が取りこぼしやすい。
- Doorは既存releaseを使う。旧計画の「未導入」などの古い文言だけから再制作を始めない。
- code編集は主担当だけが行う。agentはread-only探索・レビューに限る。

### 参照必須ファイル

- `docs/building-art-direction.md`、`docs/building.md`、`docs/art-style-criteria.md`、`docs/assets_workflow.md`、`docs/blender-setup.md`
- `docs/crate-boundaries.md`、`docs/invariants.md`、`docs/save_load.md`、`docs/soul_energy.md`、`docs/rest_area_system.md`
- `crates/hw_jobs/src/placement_geometry.rs`、`crates/hw_jobs/src/model.rs`
- `crates/bevy_app/src/systems/jobs/building_completion/spawn.rs`、`crates/bevy_app/src/systems/visual/building3d_cleanup.rs`、`crates/bevy_app/src/systems/visual/door_preview.rs`
- `crates/bevy_app/src/plugins/startup/visual_handles.rs`、`crates/bevy_app/src/plugins/startup/asset_catalog.rs`、`crates/bevy_app/src/assets.rs`
- `crates/bevy_app/src/assets/door_asset_set.rs`、`crates/hw_visual/src/visual3d.rs`、`crates/hw_ui/src/setup/submenus.rs`
- `docs/development-infra/validation-storage-workflow.md`、repositoryのnative acceptance / Help impact review Skill

### 最終確認ログ

- `2026-09-19`: `python3 scripts/dev.py docs --write`で索引を生成し、両索引を確認。`docs --check`、`git diff --check`、`python3 scripts/dev.py validation check`は成功。今回の計画用job・build・native出力は作成していない。
- `python3 scripts/check_help_impact.py`は成功。ただし既存commitのHelp更新を検出した結果であり、今後の実装に必要な実際のプレイヤー経路に基づくHelp判断の代替にはしない。
- read-onlyレビューを反映し、既存fallbackの中心原点とproductionの接地原点を区別。建築bounce中の高さを維持するため、表示mode切替時にroot原点・part構成・previewを同時更新する計画へ修正。
- `dev.py check` / Clippy / workspace test / `dev.py verify`: 未実行（今回は計画文書のみ）。実装完了の証拠なし。
- native / performance / art acceptance: 未実行。既存Door releaseの成果と今回の受入を区別する。

### Definition of Done

- [ ] M0〜M6の完了条件と全10種の処置が確定。
- [ ] 恒久仕様・Help判断・asset正本・release/復旧記録を同期。
- [ ] rust-analyzer、`dev.py check`、Clippy警告0、workspace testを含む`dev.py verify`が成功。
- [ ] 必須actual-window・対象性能budgetを満たし、未検証範囲を正確に記録。
- [ ] 全batchの結果確定と不要job / binary copyの整理、storage checkを報告前に完了。
- [ ] 最終closeで本計画consumerは0。残る共有資源は別の具体的consumer・owner・bytes・終了条件へ引継ぎ。
- [ ] 採用成果と最終結果を恒久的な正本へ移し、本計画をarchiveまたは削除、両索引を再生成。

## 10. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| `2026-09-19` | `Codex` | 制作仕様を前提に全10種の移行、9種の新asset経路、Door監査、段階導入・全表示consumer・受入・保存管理を計画。実装未着手 |
