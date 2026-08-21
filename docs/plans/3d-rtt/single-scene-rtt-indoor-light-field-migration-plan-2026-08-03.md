# 単一 Scene RtT・室内 Light Field 移行プログラム計画

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `single-scene-rtt-indoor-light-field-migration-plan-2026-08-03` |
| ステータス | `In Progress — P07 complete; P08 bounded release closure pending` |
| 作成日 | `2026-08-03` |
| 最終更新日 | `2026-08-21` |
| 作成者 | `Codex` |
| 採用判断 | TopDown 2.5D、Scene RtT 1枚、map-space radial Light Field |
| 関連提案 | `N/A` |
| 置換した計画 | [`milestone-roadmap.md`](milestone-roadmap.md)の未完項目 / [`lighting-visual-plan-2026-04-04.md`](lighting-visual-plan-2026-04-04.md) / [Soul outline proposal](../../proposals/soul-outline-mask-ring-proposal-2026-04-16.md) |
| 並行計画 | [`../hvac-plumbing-plan-2026-07-13.md`](../hvac-plumbing-plan-2026-07-13.md) / [`../archive/save-rehydration-registry-plan-2026-08-03.md`](../archive/save-rehydration-registry-plan-2026-08-03.md) |
| 関連Issue/PR | `N/A` |

本書は移行全体の製品契約、依存順、統合gateだけを所有する。実装手順、変更ファイル、focused test、rollbackは下記の子計画を正本とし、本書へ重複させない。

## 1. 目的

### 解決したい課題

- 現行RtTは画面解像度の`scene`と`soul_mask`を常時保持し、2台のCamera3dと多サンプルcompositeを必要とする。
- 旧ロードマップのfull RtT、Soul mask、矢視、section、多層階は、現在のTopDown 2.5D方針と一致しない。
- 室内環境設備として必要なのは多数の放射状光源であり、Lampごとのshadow mapではなくWall / Door遮光を一定costで表現する必要がある。
- 現行Lamp gameplay効果は遮光を参照せず、表示とgameplayが別の正本になっている。
- Camera2d actorは3D Wallとdepthを共有できないため、Wall裏へ隠すactorだけ同一Sceneへ移す必要がある。

### 到達したい状態

1. 画面解像度依存のworld color targetはScene RtT 1枚だけである。
2. Terrain、Floor、Wall、Door、Bridge、大型構造物だけを構造3D sceneへ置く。
3. Soulは共有unlit billboardとして同じdepth sceneへ置くが、室内光のcaster / visual receiverにはしない。
4. Familiarと構造depth不要の小物・effectはcomposite後のforegroundへ置く。
5. `hw_infra`のCPU `IndoorLightField`を表示、Room summary、Lamp gameplay効果の唯一の照度正本にする。
6. 完成WallとClosed / Locked Doorは遮光し、Open DoorとProvisionalWallは通光する。
7. load / rollbackは旧worldの照度を表示せずfail-darkから再構築する。
8. V矢視とsection固有runtimeを段階的に撤去し、TopDown-onlyをHelpまで同期する。

## 2. 固定する製品契約

### 2.1 表示分類

P00でこの表をbaseline artifactと一緒に凍結する。分類変更は子計画内だけで行わず、本表とP02を同時更新する。

| 対象 | `RenderPresentationClass` | depth | 室内光 | shadow |
| --- | --- | --- | --- | --- |
| Terrain / Floor / Bridge | `Structural3d` | あり | receiver | directional sunのみ |
| Wall / Door | `Structural3d` | あり | receiver + occluder | directional sunのみ |
| Tank / MudMixer / RestArea / SoulSpa | `Structural3d` | あり | receiver | directional sunのみ |
| SandPile / BonePile / WheelbarrowParking / OutdoorLamp器具 | `Foreground2d` | なし | 非receiver | なし |
| Soul | `DepthBillboard3d` | Wallと共有 | 非receiver | casterにしない |
| Familiar | `Foreground2d` | 常時前景 | 非receiver | なし |
| 選択・範囲・speech等 | `Foreground2d` | 常時前景 | 非receiver | なし |

- BuildingCategory、`blocks_movement()`、footprintだけから表示分類を推論しない。
- OutdoorLampの器具spriteがforegroundでも、論理`RadialLightEmitter`はworld grid上に存在する。
- Soul billboardの開発中rollbackはvisible GLB 1系統だけとし、Soul mask RtTは戻さない。ただしこれは一時fallbackであり、billboard未成立のままP08 releaseを完了しない。

### 2.2 光と遮光

| 入力 | 契約 |
| --- | --- |
| emitter | `PowerSupplyState::Supplied`の`RadialLightEmitter`だけを収集する |
| Wall | `BuildingType::Wall`かつ`Without<ProvisionalWall>`だけが遮光する |
| Door | Openは通光、Closed / Lockedは遮光する |
| Soul / Familiar / 家具 | caster / occluder / visual receiverにしない |
| 合成 | directional shadow適用後にlocal lightを加算する |
| gameplay | GPU textureではなく同じCPU fieldを読む |
| steady-state | input revision不変時はfield再計算0回、Image upload 0回 |

local lightの合成はlinear空間で`directional_styled_rgb + base_color_rgb * local_light_rgb`とし、local専用ambient floorは追加しない。CPU fieldを1.0でsaturateした後のtone mappingは既存Scene pipelineを使う。gain、clamp、色変換をP06だけで別調整せず、P00のgolden fixtureでこの式を固定する。

標準室内照明にshadow付きPointLight / SpotLightを使わない。例外的なhero lightは別計画、固定budget、実測を必須とする。

### 2.3 Viewと非対象

- productionの正本はTopDown 2.5Dとする。
- V矢視の入力とHelpをP02で削除し、到達不能になったsection固有material / shaderをP08で撤去する。
- section互換、多層階、3D volume light、天井高、光源高、soft shadow、specular、家具遮光は対象外とする。
- Direct-to-window、新しい室内照明overlay、Room照度UI、快適度・健康バランス、新規アート制作は対象外とする。

## 3. 分割計画と依存順

```text
P00 baseline / contract
 ├─> P01 one Scene RtT ─> P02 M1〜M4 feature ─> P02-A evidence ─> P02 M5〜M6 formal ─┐
 │                              └── P02-A M1 selector / history ──────┘                 │
 └─> P03 Light Field core ─> P04 runtime integration ─> P05 save ─┼─> P06 GPU rendering ─┐
 P02-A M1〜M4 ready extension point ──────────────────────> P03 M4 evidence                │
                                                    └──────────────┴─> P07 gameplay / Room ┤
 P01 + P02 + P03 + P04 + P05 + P06 + P07 ────────────────────────────────> P08 release
```

| ID | 実装計画 | 直接依存 | 独立した完了成果 |
| --- | --- | --- | --- |
| P00 | [`00-baseline-gates-plan-2026-08-03.md`](single-scene-light-field/00-baseline-gates-plan-2026-08-03.md) | C00-Aはなし。C00-B以降はHVAC M0または同等correctness commit | current capture、安定比較schema、数値gate、表示分類、system-order contract |
| P01 | [`01-single-scene-rtt-plan-2026-08-03.md`](single-scene-light-field/01-single-scene-rtt-plan-2026-08-03.md) | P00 | Soul mask target / camera / proxy / metricを除去したScene RtT 1枚 |
| P02-A | [`02a-p02-acceptance-infrastructure-plan-2026-08-12.md`](single-scene-light-field/02a-p02-acceptance-infrastructure-plan-2026-08-12.md) | P00, P01。M2〜M4はP02 M1〜M4のproduction sourceを入力にする | `stage=p02` selector / historical reader、P02 evidence sidecar、RenderDoc / bundle / native recipe、actual-window scenario |
| P02 | [`02-topdown-presentation-plan-2026-08-03.md`](single-scene-light-field/02-topdown-presentation-plan-2026-08-03.md) | P00, P01。M6はP02-A M1〜M4 | Door実経路修復、world 2D pass 1回、表示分類、Soul billboard / Familiar前景、Soul shadow動作停止、V入力停止、registered P02 formal artifact |
| P03 | [`03-indoor-light-domain-core-plan-2026-08-03.md`](single-scene-light-field/03-indoor-light-domain-core-plan-2026-08-03.md) | P00。P03 stage有効化はP02-A M1〜M4 ready extension pointを利用 | `hw_infra`のdeterministicな論理field / pure LOSと`p03 / field-core`正式evidence |
| P04 | [`04-indoor-light-runtime-integration-plan-2026-08-03.md`](single-scene-light-field/04-indoor-light-runtime-integration-plan-2026-08-03.md) | P02 M1, P03、HVAC M0または同等correctness commit | topology / energy / Room / Doorを結ぶ更新transactionとsteady-state dirty管理 |
| P05 | [`05-indoor-light-save-lifecycle-plan-2026-08-03.md`](single-scene-light-field/05-indoor-light-save-lifecycle-plan-2026-08-03.md) | P03, P04、完了済みC3 save registry（`1b84f316`） | durable mount、named rehydrate step、load / rollback fail-dark |
| P06 | [`06-indoor-light-rendering-plan-2026-08-03.md`](single-scene-light-field/06-indoor-light-rendering-plan-2026-08-03.md) | P01, P02, P04, P05 | user-approved RD0 timeout例外で受理した100×100 Light Field textureと全`Structural3d` receiver |
| P07 | [`07-indoor-light-gameplay-room-plan-2026-08-03.md`](single-scene-light-field/07-indoor-light-gameplay-room-plan-2026-08-03.md) | P03, P04, P05 | P05-lineage evidenceでSoul / Roomが同じfield revisionを読むgameplay統合 |
| P08 | [`08-legacy-cleanup-release-plan-2026-08-03.md`](single-scene-light-field/08-legacy-cleanup-release-plan-2026-08-03.md) | P01〜P07 | projector / section残骸撤去、bounded final-state renderer / cross proof、Help、最終gate |

### 3.1 着手とmergeの規則

- P00完了前にP01以降のproduction変更へ着手しない。
- P00 C00-BとP04は、室内設備が占有するfloor cellをRoom interiorとして維持するHVAC M0がmerge済み、または同じcorrectness変更を単一ownerで先行するまで開始しない。
- P03 M1〜M3とP01はP00後に並行可能だが、同じファイルを触る作業は同時に実行しない。P03 M4はP02-A M1〜M4 ready extension pointを拡張し、既存stageのschema / historical readerを再定義しない。
- P02-A M1のselector / historical readerはP02 M1と並行できる。P02-A M2〜M4は対応するP02 production sourceが完成してから接続し、P02 M6 formalはP02-A M1〜M4のready stateを必須とする。
- P04のentryはP02 M1 Door correctnessのままとし、P02-A full formalを新規blockerにしない。P04 / P06以降のstage固有metricは各planが所有する。
- P04はsave / rehydrateを編集せずruntime transactionだけを所有する。
- P05は完了済みC3 save registry（`1b84f316`）のfreeze済みproduction planを使用する。raw registryを迂回せず、既存step名・phaseを変えないedge付きnamed stepだけを追加する。
- P06とP07はP04 / P05のfield revision / epoch contractを変更せずconsumerとして実装する。凍結v1のP07 formalは`stage_without_gpu_owner`であるため、P07 evidence subjectはP05から分岐しP06を含めない。P08がP06 / P07統合とcross-consumer evidenceを所有する。
- 各子計画はfocused test、workspace check、Clippy、Help impact判断、必要なnative受入まで閉じてから次の依存計画を開始する。
- 子計画の途中状態を長期間productionへ残さない。1計画内のwork packageは連続commitとする。

### 3.2 推奨merge列

| batch | work package | merge可能条件 |
| --- | --- | --- |
| B00 | P00契約・fixture・4 evidence family / 5 current leg・stable gate ID・数値gate | Room interior-role owner確定後、clean commitのformal baselineが揃う |
| B01 | P01 M1〜M3 Scene-only runtime / mask camera・proxy・visual_test撤去 / stage-aware tooling | P00 C00-D / C00-Eのcurrent formal baselineが登録済み。public `SoulMask*`削除と全consumer追従を同一compile可能seriesで完了 |
| B02 | P01 M4 formal / native acceptance / P01 gate ledger | B01 green、current referenceをhistorical readerで再検証済み |
| B03 | P02 M1 production Door経路修復 + P02-A M1 selector / historical reader | P01完了。P04はP02 M1のproduction Door focused testがgreenになれば開始でき、P02 full formalを待たない |
| B04 | P02 M2 Camera2d一本化・V / Help削除 | B03 green |
| B05 | P02 M3 Building全分類・Bridge・移動 / 状態同期 | B04 green |
| B06a | P02 M4 Soul billboard・Familiar 3D撤去・Soul shadow動作停止 | B05 green |
| B06b | P02-A M2〜M4 P02 evidence / RenderDoc / native ready | B06a green。P02 M1〜M4のproduction sourceとcurrent / P01 historical readerが同一toolchainで再検証済み |
| B06c | P02 M5〜M6 P02 evidence接続・formal / native受入 | B06b green。P02 exact gate artifactとactual-window scenarioを同じ完了batchで閉じる |
| B07 | P03 pure Light Field core + P03 field-core evidence | P00後にB01〜B06cと並行可。P03が`hw_infra`をbootstrapし、P03 M4はP02-A M1〜M4 ready extension pointを拡張する。root Plugin / ECS runtimeは追加しない |
| B08 | P04 runtime snapshot / Door request / schedule / dirty（完了） | B03とB07完了 |
| B09 | P05 schema / registry / reset / epoch | B08完了。C3 save registry（`1b84f316`）のfreeze済みproduction planを利用 |
| B10 | P06 Image bridge→Terrain→structural material→native | B02、B06c、B08、B09完了。RD0 failureはartifact上invalidのまま、2026-08-17のユーザー判断で受理 |
| B11 | P07 Soul effect→Room summary→soak（完了） | B08、B09完了。P05-lineage subject `a749a580`のfrozen v1 formalを登録・offline再検証済み。P06 GPU ownerは含めない |
| B12 | P08 tooling→cross-consumer proof→projector / section / mirror cleanup→bounded release closure | B10、B11完了。workspace full gate後、fresh P08 subjectを3-process Capture / RenderDoc closureで確認する。historical reference bootstrapは今回のDoD外 |

各batchは少なくともfocused testと`python3 scripts/dev.py check`がgreenな独立commitにする。Rust変更を含むbatchは`python3 scripts/dev.py cargo -- clippy --workspace --all-targets -- -D warnings`も通す。B04のHelp変更、B06a〜B06c / B10 / B12のvisual変更は各batch内でHelp impact review / native acceptanceまで閉じる。

## 4. 境界契約

### 4.1 crate ownership

| 所有先 | 所有するもの | 所有しないもの |
| --- | --- | --- |
| `hw_world` | world/grid変換、Wall / Door / Room topology、DoorState rule | 照度、GPU Image、Lamp効果 |
| `hw_energy` | demand、allocation、`PowerSupplyState`、OutdoorLampの整数tile radius default | visual `LightRadiusTiles`、色、LOS |
| `hw_infra` | emitter / mount、occlusion snapshot、CPU field、field revision（dirty schedulingを除く）、Room summaryのpure value / aggregation | Bevy `Component` / cache、render asset、UI、camera、root schedule |
| `hw_visual` | billboard / structural material、shader binding contract | gameplay照度、Door rule |
| `bevy_app` | cross-domain ordering、save adapter、`Assets<Image>` bridge、presentation mapping、Room summary component / cache / reset adapter | pure LOS / accumulation |

P03が`hw_infra`を最初にbootstrapする。P03が作る`lighting` pure coreはroot Plugin / ECS runtimeを作らない。P04/P05は同じnamespaceへadapterを追加できるが、core APIへBevy/world依存を逆流させない。HVACはP03後に既存crateを拡張できる。

### 4.2 update transaction

```text
Logic:
  WorldTopologyMutationSet
  -> ApplyDeferred
  -> Energy settlement
  -> ApplyDeferred
  -> RoomTopologyRefreshSet
  -> ApplyDeferred

Pause gate外 / pre-Visual:
  manual Door request（前frameのInterfaceから受信）

Actor（unpaused時）:
  auto-open -> movement -> auto-close

PostActor / pause-open:
  IndoorLightingRebuildSet
  -> SoulLightRecoverySet（P07独自unpaused gate）
  -> RoomIlluminationSummarySet（P07、pause-open）

Visual:
  IndoorLightUploadSet（revision / WorldEpoch変更時だけ）
  -> Door / structure presentation sync

Interface:
  UiIntent::ToggleDoorLock -> DoorLockToggleRequest（次updateで適用）
```

- auto Doorは同じvisual frame、manual Doorはpause中も操作の次visual frameまでにfieldと3D visualへ同時反映する。
- InterfaceはDoor / WorldMapを直接変更しない。
- Roomはindoor mask / summaryだけに使い、LOSの正本にしない。
- `WorldMap.obstacle_version`をlight dirtyへ流用しない。

### 4.3 save / rollback

- durable: Building、DoorState、Power policy、root-owned `LightingFixtureMount(FixtureMount)` adapter。
- reconstructible: `RadialLightEmitter`、presentation shell。
- derived: occlusion grid、field、Room summary、GPU Image、revision / upload cache。
- 旧saveに`LightingFixtureMount`がなければ`FreeStanding`へ移行する。pure `FixtureMount`のwall mountはanchor Wallとcardinal interior normalを保存し、推測復元しない。
- world replacement開始時にderived stateをdark / emptyへresetし、既存`WorldEpoch`更新後も旧Imageを有効扱いしない。
- paused load / failed rollbackでも旧worldのlight textureを次frameへ持ち越さない。

## 5. プログラム完了gate

| 観点 | 合格条件 |
| --- | --- |
| RtT | Scene以外のworld color target、mask Camera、mask proxy、mask bindingが0 |
| composite | 通常Scene sample 1回。mask拡張loopなし |
| presentation | 2D world pass 1回。表示分類が全`BuildingType`を網羅し、2D / 3D二重表示なし |
| Soul | Wall depthへ参加するunlit billboard。local light / shadowへ不参加 |
| lighting | Wall / Door / corner / mount sideのpure testが合格 |
| lifecycle | unchanged frameのrecompute / upload 0。load / rollbackはfail-dark。normal / rollback / recovery-onlyが同じnamed traceを通る |
| consistency | gameplay、Room summary、GPU uploadが同じfield revision由来 |
| performance | P00で固定した同一adapter matrixでp95 / p99、pass、RSS hard gateを満たす。p50は診断値として併記する |
| product | V / sectionのHelp、input、runtime、shader残骸がない |
| repository | `python3 scripts/dev.py verify`とnative acceptance artifact検証が成功 |

性能閾値はP00で実装前に本書とP00へ追記し、P08の結果を見て緩和しない。

## 6. 横断リスク

| リスク | 対策 |
| --- | --- |
| freeze済みsave registryを迂回・上書きする | P05はC3のresolved production planへedge付きnamed stepを追加し、raw registry APIを公開しない |
| `hw_infra`をHVACと二重作成する | P03をbootstrap ownerとし、HVACは既存crateを拡張する。P03はroot Pluginを登録しない |
| material移行前に`SectionMaterial`を削除する | P06でTopDown material parityとconsumer移行、P08で参照0確認後に削除する |
| UI Doorとauto Doorが別revisionになる | UIはrequest化し、Actorの単一writer経路へ集約する |
| load中に旧GPU bytesが見える | load resetでblack handle / epoch invalidationを同期適用する |
| billboardが透明sortでWallを貫通する | alpha mask + depth writeをnative fixtureで検証する |
| 数値調整で経路不成立を隠す | topology、給電、dirty、revision、bindingの成立を先にtestする |

## 7. ロールバック方針

- 子計画単位で独立commit列にし、親計画全体を一括revertしない。
- P01失敗時はmask関連変更だけを戻し、P03の論理fieldへ影響させない。
- P02 billboardが不合格なら開発中はvisible GLB 1系統へ戻せるが、mask RtTは戻さない。billboard成立までP08 releaseはblockedとする。
- P06 visualが不合格でもP03 / P04 / P05 logical lifecycleは保持し、local visualだけをfeature-offできる境界を残す。
- P07 gameplayが不合格なら旧Lamp effectを一時維持するが、全`PowerConsumer`をLamp扱いする誤りは別correctness fixとして残す。
- `LightingFixtureMount` adapterはdurable schemaなので、P05以後を戻す場合も互換reader / writerまたはmigration shimを残す。
- section / Vを再導入する場合はrollbackではなく別proposalとnative acceptanceを必要とする。

## 8. AI引継ぎメモ

### 現在地

- 進捗: `P06はuser-approved RD0 timeout例外で受理、P07はP05-lineage frozen v1 formalまで完了、P08はM1/M2 code cleanupとM3 hidden mirrorまで実装済み / 3-process bounded native closure未完`（P00〜P07とP02-A完了。P08実装進行中）
- 完了済み: 計画分割、設計契約、Room interior-role correctness、P00 current startup inventory、frozen
  `rtt-light-v1` contract、3規模static / behavior fixture、stable projection / gate row、window / RtT
  environment evidence、S1 / formal native recipe、RenderDoc capture / replay validator、runtime / offline ledger validator、
  P01 Scene-only runtime / compositeとSoul mask target / camera / proxy / material / toggle撤去、P02-A受入基盤計画、
  P03 `hw_infra::lighting` pure coreとfield-core計測/artifact/bundle/native extension、P05 durable mount / candidate validator / named rehydrate / reset-epoch lifecycleとP05 evidence tooling
- P00 formal: subject `10763a4d`、attempt `9e813f24-0f7b-47f5-8a8d-e3ff34775370`。5 leg、18 case、baseline index / current gate ledgerを登録・再検証済み
- P01 formal: subject `29a4a719`、attempt `8bc82f04-10ac-4903-89b6-89011dacdada`。5 leg、18 case、123 / 123 gate row、Scene-only RenderDoc topologyを登録・再検証済み
- P02 formal履歴: subject `6ea0bf99`、attempt `54d85a63-e237-4501-a0d0-33c1d0a29f3b`はfrozen v1の履歴として保持する。actual-window v1は改竄再検証不足のためreview remediation後のP02完了証跡には使わない。
- P02 current evidence: subject `c3515a40`、profile v9 / schema 7、actual-window 18 / 18、fresh formal attempt `9ff336ef-1312-4248-b0bf-bb454111decc`。job / manifest / formal / baseline historyの独立verifierがvalid、formal 5 legと128 / 128 gate rowがpassした。
- P03 formal: subject `834c7440`、attempt `cd700aed-68bb-4fcd-92b5-2f4a4effa1bc`。19 / 19 case、`RLV1-P03-FIELD` / `RLV1-BUNDLE-VALID`、baseline index登録、独立verifierがpassした。field-coreはp95 `0.347376 ms`、p99 `0.373993 ms`、allocation `6 events / 210400 bytes`
- P04 formal: subject `44d22adc`、attempt `51d2b81d-ee8d-4242-a502-3f957c302903`。S0 / S1 / formalがvalid、19 / 19 case、142 / 142 gate row、1,023 artifact封印、独立verifierがpassした。production adapterの600 unchanged Updateはfull scan / rebuild / revision increment / scoped allocation 0、field-coreはp95 `0.296658 ms` / p99 `0.335148 ms`
- P04完了: Door request / pause契約、root ECS snapshot adapter、Room mask revision、fail-dark dirty / rebuild transaction、P04 static / behavior / field-core toolingと正式実機証跡
- P05 current evidence: behavior 21 / 21、headless field-core 3 / 3、native S1 job `p05-s1-20260816`はAudit 3 / 3、Capture 18 / 18、Memory 18 / 18、field-core 3 / 3がvalid
- P05 formal: subject `56fa6bd3`、attempt `492ad69c-1275-48ea-90f2-ed1a8018b542`。24 / 24 case、190 / 190 gate row、RenderDoc replay、baseline index登録、独立attempt / 全baseline verifierがpassした。additive contract predecessorは旧stageの旧hashとSHA ledgerを保持し、P05以後には適用しない
- P06: commit `19ad5fec`で単一RGBA8 Light Field image、epoch-aware upload／load black reset、Terrain 3 LOD／全Structural3d receiver、Door root `MeshTag` anchor、P06 sidecar／timeline／RenderDoc／bundle toolingを実装。2026-08-17 headless behavior 21 / 21、local gates、clean-subject S0 / S1はvalid。formal RD0は`renderdoccmd capture` deadline / process-group failureでinvalidだったが、ユーザーが例外として受理した。
- P07 local: binary non-stackのepoch-aware Soul recovery、Room topology / summary / cache / reset、P07 selector / consumer-core / lifecycle evidence / native tooling、Help / docsを実装。full verify pass、final source binary `0a04b05a…`でbehavior 21 / 21とconsumer-core 3 / 3がvalid（p95 median `0.022560 ms`、p99 median `0.025054 ms`、allocation / stale effect / old-epoch consumer 0）。
- P07 formal: P05-lineage subject `a749a580370947f0f64c1685010e625a903324dd`、attempt `82bd460f-31c6-4fa0-8705-a4d702ff4c1f`。fresh S0 / S1、25 case、203 / 203 gate row、1,249 artifact、RenderDoc、consumer-coreを登録し、offline verifierがpassした。consumer-coreはp95 `0.023802 ms` / p99 `0.026977 ms`、allocation 0。
- 未完了: P08のみ。P07 frozen v1 formalはP06入りsubjectで置換しない。

### 次のAIが最初にやること

1. P08 bounded closure helper / Skill / docsをself-testし、workspace full gate済みのclean commitにする。
2. `--level closure --stage p08`でmedium GPU Capture 2起動とRenderDoc 1起動だけを実行し、same-checkpoint cross-consumer proofを閉じる。
3. historical reference bootstrapは32時間消費を受けたユーザー判断で停止済み。明示的な再承認なしにcurrent -> P01 -> P02 -> P06再測定へ戻らない。

P00の数値gateは実装前契約として確定済みである。candidate結果を見て同じbaseline generationの閾値を緩和しない。

### ブロッカー/注意点

- C3 save / rehydrate registryは`1b84f316`で完了済みである。freeze済みgraphとresolved planを迂回・上書きせず、P05 compatibility stepを既存facadeから追加する。
- `hw_infra`はP03がbootstrap済みである。HVAC M1は既存crateを拡張するが、`lighting` pure coreへECS/GPU依存を逆流させない。
- manual Door lockはP04でrequest化済みで、InterfaceはDoorState / WorldMapを直接変更しない。
- 現行Lamp gameplay queryは任意の`PowerConsumer`を発光扱いし、半径`5.0`をworld unitとして比較している。
- 現行Terrain 3 LODと全Structural3dは単一Light Field receiverである。P08で`TopDownStructuralMaterial`を独立ownerへ移し、`SectionMaterial`互換型／shader／projector fieldsは物理削除済み。native shader / pixel証跡はP08 final gateで取得する。
- P02のreview remediation subjectはv9 actual-windowとfresh S0 / S1 / formalを登録・再検証済みである。P03以降もfrozen contract / projection / P00〜P02 artifactをcandidate結果で変更しない。

### 最終確認ログ

- 最終 `python3 scripts/dev.py check`: `2026-08-14` / `pass`
- 最終 `python3 scripts/dev.py cargo -- clippy --workspace --all-targets -- -D warnings`: `2026-08-14` / `pass (0 warning)`
- 最終 `python3 scripts/dev.py verify`: `2026-08-15` / `pass`
- 最終 `python3 scripts/dev.py verify`: `2026-08-13` / `pass`
- P01 native acceptance: `2026-08-12` / `pass`（Intel Arc / Vulkan / X11、attempt `8bc82f04-10ac-4903-89b6-89011dacdada`、全5 leg valid、123 / 123 gate row pass）
- P02 native acceptance: `2026-08-14` / v9 actual-window 18 / 18、fresh formal attempt `9ff336ef-1312-4248-b0bf-bb454111decc`、5 leg valid、128 / 128 gate row pass、formal / baseline historyの独立verify pass。v1 actual-windowとattempt `54d85a63-e237-4501-a0d0-33c1d0a29f3b`は履歴として保持する。
- P03 native acceptance: `2026-08-15` / subject `834c7440`、formal attempt `cd700aed-68bb-4fcd-92b5-2f4a4effa1bc`、19 / 19 case valid、field-core p95 `0.347376 ms` / p99 `0.373993 ms`、allocation `6 events / 210400 bytes`、Intel Arc / Vulkan / X11、独立verify pass。
- P04 native acceptance: `2026-08-16` / subject `44d22adc`、S0 `task-dashboard-20260815T174144Z-a3a9eca7`、S1 `rtt-light-s1-20260815T174443Z-7f4b40f4`、formal attempt `51d2b81d-ee8d-4242-a502-3f957c302903`、19 / 19 case valid、142 / 142 gate row、Intel Arc / Vulkan / Mesa `26.1.5` / X11、独立verify pass。
- P05 native acceptance: `2026-08-16` / subject `56fa6bd3`、attempt `492ad69c-1275-48ea-90f2-ed1a8018b542`、24 / 24 case、190 / 190 gate row、RenderDoc replay valid、Intel Arc / Vulkan / Mesa `26.1.6` / X11。attempt verifierとcurrent / P01 / P02 / P04 / P05 baseline全4,947 fileの独立verify pass。
- 最終 `python3 scripts/dev.py check` / workspace Clippy / workspace test / `dev.py verify`: `2026-08-16` / `pass`
- 最終 docs / Help gate: `2026-08-16` / Help impact No impact、docs write/checkとdiff check pass
- P06 acceptance: `2026-08-17` / commit `19ad5fec`、headless behavior 21 / 21、perf／RenderDoc／native helper self-test、`dev.py check`、clean-subject S0 / S1 valid。formal RD0は`renderdoccmd capture` deadline / process-group failureでinvalidだが、ユーザーが例外として受理
- P07 native acceptance: `2026-08-18` / P05-lineage subject `a749a580370947f0f64c1685010e625a903324dd`、S0 `task-dashboard-20260817T172253Z-32b4de03`、S1 `rtt-light-s1-20260817T172521Z-34c5062d`、formal attempt `82bd460f-31c6-4fa0-8705-a4d702ff4c1f`。25 case、203 / 203 gate row、1,249 artifact、全7 behavior / Capture / Memory / RenderDoc / field-core / consumer-core valid、Intel Arc / Vulkan / Mesa `26.1.6` / X11、registered attempt / offline verifier pass

### Definition of Done

- [ ] P00〜P08とP02-Aが全て完了
- [ ] 全子計画のDefinition of Doneが合格
- [ ] P08 bounded renderer / cross-consumer gateが合格し、historical性能合格を過大主張していない
- [ ] Help impact reviewとnative acceptanceが完了
- [ ] 恒久docsへ契約を移し、本計画と子計画をarchiveまたは削除

## 9. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| `2026-08-21` | `Codex` | historical reference bootstrapの32時間消費を受け、P08 completionをfresh subjectの3-process / 2-build bounded closureへ変更。frozen formal契約とartifactは保持するが、current / P01 / P02 / P06再採取とbaseline登録を今回のDoDから除外 |
| `2026-08-18` | `Codex` | P08を現行mainline / frozen v1 / evidence lineageへ再レビューし、M0 tooling・valid reference bootstrap、same-checkpoint cross-consumer proof、TopDown material re-home、stable projection維持を実装順へ固定。B12をM0着手可能へ更新した。 |
| `2026-08-18` | `Codex` | P07 P05-lineage subject `a749a580`のfresh S0 / S1 / frozen v1 formalを登録・offline再検証。25 case・203 / 203 gate row・1,249 artifactをpassし、B11 / P07を完了、次対象をP08へ更新した。 |
| `2026-08-17` | `Codex` | P07 local implementation、full verify、consumer-core 3 / 3、Help / docs完了を反映。凍結v1のP05-lineage native formalだけを未完了として維持。 |
| `2026-08-17` | `Codex` | P06 formal RD0の`renderdoccmd capture` deadline / process-group failureをinvalidのまま記録し、ユーザー承認の例外としてP06を受理。P07をP05-lineage frozen-v1 evidenceとP08統合へ分離する改訂を反映。 |
| `2026-08-16` | `Codex` | P05 subject `56fa6bd3`のformal 24 / 24 case・190 / 190 gate rowを登録し、additive contract lineage、attempt、全baseline SHA ledgerを独立再検証。P05を完了して次対象をP06へ更新した。 |
| `2026-08-16` | `Codex` | P05 production / evidence tooling、21 / 21 behavior、full quality gate、Help No impact、native S1全レッグvalidを反映。clean subjectでのformal 24 / 24 registrationだけを残した。 |
| `2026-08-16` | `Codex` | C3完了を依存・着手条件へ反映し、P05をReadyへ更新。P05のdurable mount / registry edge / reset-epoch / evidence runnerのレビュー済み責務を親計画へ同期。 |
| `2026-08-16` | `Codex` | P04 subject `44d22adc`のS0 / S1 / formal（19 / 19 case、142 / 142 gate row）と独立verificationを完了し、B08 / P04を完了、次対象をP05へ更新 |
| `2026-08-15` | `Codex` | P04 runtime / schedule / perf tooling実装を反映し、状態をformal native evidence待ちへ更新 |
| `2026-08-15` | `Codex` | P03 subject `834c7440` のformal native attempt `cd700aed-68bb-4fcd-92b5-2f4a4effa1bc`を登録・独立再検証し、P03を完了、次対象をP04へ更新 |
| `2026-08-14` | `Codex` | P03 pure coreとfield-core evidence extensionの実装、unit/tool/full verification、Help No impact reviewを反映。formal native bundleだけをclean subject待ちとして残した。 |
| `2026-08-14` | `Codex` | P02 actual-window v9（subject `c3515a40`、18 / 18）とfresh formal attempt `9ff336ef-1312-4248-b0bf-bb454111decc`（5 leg、128 / 128 gate row、独立verify）を反映。P02 / P02-Aを完了に更新した。 |
| `2026-08-13` | `Codex` | P02 review remediationにより、旧actual-window v1証跡を完了判定から外した。P02 / P02-A v2 phase/ROI artifactとfresh S0 / S1 / formalを再採取するまで親ロードマップも再オープン。 |
| `2026-08-13` | `Codex` | P02 / P02-Aのactual-window、S0 / S1、formal 5 leg・128 / 128 gate row登録と独立再検証を完了し、次対象をP03へ更新 |
| `2026-08-12` | `Codex` | P02 M1〜M5の実装完了と、M6 native / formal未完了を親ロードマップへ反映 |
| `2026-08-12` | `Codex` | P02-A受入基盤計画を追加し、P02 feature、evidence、formal / native受入の依存とmerge境界を明確化 |
| `2026-08-12` | `Codex` | P01のScene-only RtT移行とcanonical formal attempt登録・再検証の完了を反映し、P02を着手可能へ更新 |
| `2026-08-11` | `Codex` | P00 canonical current baseline（attempt `9e813f24-0f7b-47f5-8a8d-e3ff34775370`）の登録・再検証完了を反映し、P01を着手可能へ更新 |
| `2026-08-05` | `Codex` | P01 reviewにより、P00 formal baseline登録を着手条件化し、public mask型を削除するM1〜M3をvisual_test / formal toolingと同じcompile可能seriesへ統合 |
| `2026-08-05` | `Codex` | P00 contract / behavior / projection / native / RenderDoc実装完了と、formal baseline未採取の環境条件を現在地へ同期 |
| `2026-08-04` | `Codex` | P00の3規模static production fixture、全Building runtime audit、Door初期状態、Tank companion、realtime終端再検証を現在地へ反映 |
| `2026-08-04` | `Codex` | P00 medium/large全Building showcaseのexact contract、size別artifact validator、Bridge completion-only境界を現在地へ反映 |
| `2026-08-04` | `Codex` | P00のRoom / startup inventory、contract / runner / window軸の実装開始と残るnative / RenderDoc blockerを現在地へ反映 |
| `2026-08-04` | `Codex` | P00のstable gate ID、core lane、p50診断 / p95・p99 hard gate区分を統合gateへ同期 |
| `2026-08-04` | `Codex` | P00のC00-A〜C00-D実行順と、実装前に固定済みの数値gateをAI引継ぎへ同期 |
| `2026-08-04` | `Codex` | P00のstable baseline契約とRoom interior-roleのHVAC M0依存を依存表・merge条件へ反映 |
| `2026-08-03` | `Codex` | 統合版を作成 |
| `2026-08-03` | `Codex` | 統合版を9つの独立実装計画へ分割し、本書を依存関係と統合gateの正本へ縮約 |
