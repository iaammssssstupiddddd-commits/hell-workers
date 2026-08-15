# P04: 室内 Light Field 実行時統合計画

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `single-scene-light-field-04-indoor-light-runtime-integration-plan-2026-08-03` |
| ステータス | `Completed — production, tooling, formal native evidence verified` |
| 作成日 | `2026-08-03` |
| 最終更新日 | `2026-08-16` |
| 作成者 | `Codex` |
| 親計画 | [`../single-scene-rtt-indoor-light-field-migration-plan-2026-08-03.md`](../single-scene-rtt-indoor-light-field-migration-plan-2026-08-03.md) |
| 直接依存 | [P02](02-topdown-presentation-plan-2026-08-03.md) M1 Door domain mutation（完了）、[P03](03-indoor-light-domain-core-plan-2026-08-03.md) API実装およびformal evidence（B07）、HVAC M0相当のRoom interior correctness |
| 後続 | [P05](05-indoor-light-save-lifecycle-plan-2026-08-03.md)、[P06](06-indoor-light-rendering-plan-2026-08-03.md)、[P07](07-indoor-light-gameplay-room-plan-2026-08-03.md) |
| 関連Issue/PR | `N/A` |

## 1. 目的

- 解決したい課題: completed Wall、Door、給電、Room、actor移動は異なるsystem setで更新され、manual Doorは現在Interfaceから直接mutationされる。照明更新を曖昧な`Changed` queryへ足すと、同Update内の不一致、despawnの見落とし、Room再生成に起因する無駄な再構築が起きる。
- 到達したい状態: `bevy_app`のroot adapterが値だけのsemantic snapshotを収集し、全topology writer、energy settlement、Door最終状態の後に最大1回だけP03 CPU fieldを再構築する。Door presentation consumerとCPU field snapshotは同じUpdateのstateを観測する。
- 成功指標: Wall完成、Door Open / Closed / Locked、Lamp給電変化が定義したUpdateでCPU fieldへ反映され、入力不変Updateではfull scan、rebuild、output revision増分、scoped allocation event / byteが全て0となる。GPU texture uploadはP06の責務であり、本計画の保証対象ではない。

## 2. スコープ

### 対象（In Scope）

- `hw_world`の確立済みDoor mutation APIを使う、auto / manual両方の単一writer経路。
- `bevy_app::systems::lighting`によるcompleted Wall、DoorState、typed emitter、`PowerSupplyState`、Room maskのsemantic snapshot adapter。
- lighting専用dirty tracker、入力・出力revision、named system set、`ApplyDeferred` barrier、CPU field availability。
- manual Door requestのN→N+1 pause semanticsとmessage lifecycleのroot境界。
- P00のP04 selector、fixture、timeline、sidecar、bundle validationを含むruntime instrumentationとheadless schedule / integration tests。

### 非対象（Out of Scope）

- Door sprite / 3D visualそのものの実装（P02）。P04はDoor state producerとpresentation観測順だけを統合する。
- save、world replacement、world epoch、`LoadResetRegistry`、reset実行（P05）。P04は通常runtime用のreset seamを定義するだけで、reset / request clear / dirty clearを実装しない。
- GPU upload、`Image`、renderer resource（P06）。
- Soul回復、Room照度summary（P07）。
- `hw_infra::lighting`へのBevy ECS、`World` query、energy、renderer依存の導入。

## 3. 現状と着手条件

- P02 / P02-Aは完了している。`apply_door_state`は`Door + WorldMap`だけを変更するdomain APIであり、auto Doorも同APIを通る。P04 M1の残作業はUI直接mutationをrequestへ置換し、観測順を固定することだけである。
- P03はsubject `834c7440`、formal attempt `cd700aed-68bb-4fcd-92b5-2f4a4effa1bc`で19 / 19 caseと独立verificationが完了し、親計画B07はvalidである。P04のentry blockは解除済みである。
- HVAC M0計画自体は未完了だが、既存の`RoomDetectionRole::InteriorFixture`が同等のRoom interior correctnessを提供する。P04はこの既存契約を前提にし、HVAC M0固有の別実装を待たない。
- Room entityは再検出時にdespawn / recreateされる。`Entity` ID、`Changed<Room>`、validation timerをindoor maskのidentityまたはdirty sourceにしてはならない。

P02で解消済みのDoor構造上の制約を戻さない。P04は`Sprite`有無をmutation成立条件にせず、production shell由来のDoor rootとactive presentation consumerを使うintegration testで境界を固定する。

## 4. 実装方針

### 4.1 crate境界とsemantic adapter

`hw_infra::lighting`はP03のpure、serializableなfield coreである。ECS adapterは`crates/bevy_app/src/systems/lighting/`に置く。root pluginだけが`DoorManualMutationSet`、`IndoorLightingDirtyCollectSet`、`IndoorLightingCollectSet`、`IndoorLightingRebuildSet`を宣言・configureし、`hw_infra`はsystem set、resource、query、Bevy依存を所有しない。したがって`crates/hw_infra/src/lighting/systems.rs`はP04の変更先に含めない。

| snapshot入力 | 正規化・収集条件 | dirty source |
| --- | --- | --- |
| Wall blocker | `BuildingType::Wall`かつ`Building.is_provisional == false`をblockerへ正規化する。`ProvisionalWall` markerは整合性診断にのみ使い、正本ではない。 | completion / removal / grid移動 |
| Door blocker | Door rootのOpenは非blocker、Closed / Lockedはblockerへ正規化する。 | state transition / spawn / removal |
| emitter | root adapterの`RadialLightEmitter`を持ち、`PowerSupplyState::Supplied`のfixtureだけを採用する。 | component / power / transform / mount変更 |
| indoor mask | `RoomTileLookup.tile_to_room.keys()`からcanonical row-major maskを作り、membership bytesが変わった時だけ進む`RoomTopologyRevision`で読む。 | Room publish成功時のmask差分 |

- 100×100 field用の`LightGridPos`変換はadapter境界でcheckedに行う。map外座標、重複stable key、同一cellの相反するWall / Door状態、invalid mountはreason付きdiagnosticを出し、当該入力を静かに採用したり前回のlit fieldを維持したりしない。rebuild結果はfail-dark / unavailableにする。
- stable keyはEntity IDやquery順から作らない。canonical row-major cell indexとmount tag（free = 0、wall N / E / S / W = 1 / 2 / 3 / 4）をcheckedで符号化した値を使い、入力をkey順に並べてP03へ渡す。P03のduplicate-key errorを握りつぶさない。
- ProvisionalWallとOpen Doorは遮光しない。completed Wall、Closed Door、Locked Doorは遮光する。semantic fixtureは全状態、entity順shuffle、duplicate / OOB / conflictを検証する。
- production v1の`RadialLightEmitter`はcompleted `OutdoorLamp` rootだけへ付与する。全`PowerConsumer`をLampと見なさない。未給電negative controlにもtyped componentは付与し、`PowerSupplyState::Supplied`でのみsnapshot採用する。状態遷移は`Supplied → Shed`、`Disconnected`、`InvalidDemand`、およびそれらから`Supplied`への復帰を検証する。`Unpowered` markerは採用判定の正本ではない。
- `FixtureMount`はP03が所有するpure value型を唯一のsemantic型とする。P04 root adapterはそれを読むだけとし、P05は同型のReflect / 保存登録・migrationを担い、同名・同等の型を再定義しない。
- `WallMounted`は`wall_grid`が同じsnapshot中のcompleted Wallである時だけ有効とする。ProvisionalWall、Door、missing、map edge、非cardinal `inward_normal`はdiagnostic付きfail-darkとし、別Wallへの推測的な付け替えはしない。
- `RoomTopologyRevision`は`hw_world::room_detection`がcanonical mask bytes / key membershipの比較後にpublishする。Room entity再生成とvalidation timerだけでは進めない。Room cooldownはmask更新だけを遅らせ、Wall / Door LOS dirtyを待たせない。

### 4.2 root schedule、pause、barrier

rootの`GameSystemSet`を拡張して`PreActor`と`PostActor`を明示し、登録順に依存しない次の順序を一箇所でconfigureする。

```text
Input
  -> Spatial（unpaused）
  -> Logic（unpaused）
       WorldTopologyMutationSet
       -> ApplyDeferred
       -> EnergySettlementSet
       -> ApplyDeferred
       -> RoomTopologyRefreshSet
       -> ApplyDeferred
  -> PreActor（pause gate外）
       DoorManualMutationSet
  -> Actor（unpaused）
       DoorAutoOpenSet
       -> SoulMovementSet
       -> FamiliarMovementSet
       -> DoorAutoCloseSet
  -> PostActor（pause gate外）
       IndoorLightingDirtyCollectSet
       の前にcompleted OutdoorLampのruntime emitterを再構築
       -> IndoorLightingCollectSet
       -> IndoorLightingRebuildSet
  -> Visual
       DoorPresentationSyncSet（IndoorLightingRebuildSet後）
       -> behavior observer
  -> Interface
```

- `WorldTopologyMutationSet`にはwall / floor / building completion、construction state遷移、deconstruction / world editの全writerを列挙する。
- `DoorPresentationSyncSet`を`Interface`後に置く現行順序からVisual内へ移す。P02の「presentation consumer後に観測」の契約を維持し、behavior observerは`DoorPresentationSyncSet`の後に置く。
- `PreActor`と`PostActor`にはunpaused条件を付けない。manual Doorはpause中も次Updateに反映され、auto Door / movementはpause中に走らない。manual unlockはそのUpdateのActorが観測でき、manual lockはLockedをauto openが上書きしないdomain ruleに従う。
- P04の同Update保証はDoor presentation consumerとCPU field snapshotまでである。GPU field textureの同frame表示はP06で定義する。
- P07のSoul gameplay samplingは`IndoorLightingRebuildSet`後かつslow tick時に置く。gameplay pause policyはP07で固定する。

### 4.3 manual Door requestとlifecycle

`UiIntent::ToggleDoorLock`はDoor / WorldMapを直接変更せず、ownerだけを持つ`DoorLockToggleRequest`を送る。request型は`hw_world`、message registrationはrootが所有する。

```text
Update N Interface: UiIntent -> DoorLockToggleRequest をenqueue
Update N+1 PreActor: FIFOで一度だけconsume -> owner検証 -> apply_door_state -> lighting dirty
Update N+1 Actor: unpausedならauto Door / movementが確定stateを読む
Update N+1 PostActor: CPU fieldをcollect / 最大1回rebuild
Update N+1 Visual: Door presentation consumer、その後behavior observer
```

- owner不存在、construction中、wrong `WorldMap` owner、lock不可のrequestは一度だけ消費し、reason別telemetryを増やす。現行のsilent player pathを維持し、新規player notificationは導入しない。通知が必要になった場合は別UI / Help scopeとして扱う。
- auto Doorとmanual requestが同じUpdateに競合した場合、manual lockをPreActorで先に適用し、Lockedをauto openが上書きしないことをtestする。
- P04はgeneric message lifecycleとの接続点を持つが、save / world replacement時のrequest clear、dirty clear、epoch更新の実行ownerはP05である。P04でP05のregistryやreset producerを先取りしない。
- `load-normal-v1`はP05より前のため、load後にruntime-only generator workerを復元しない。P04 artifactは新worldから再構築したavailableかつdarkなfield（small fixtureではtyped emitter 2、eligible supplied 0）を要求し、P05でnamed rehydrate後の再点灯へ契約を進める。

### 4.4 dirty、snapshot、rebuild

- root-ownedのunsaved `IndoorLightRuntime`（名称は実装で確定）はEntityを保持せず、current CPU field snapshot、availability、input / output revision、dirty reason、metricsだけを保持する。world epoch / reset処理は持たない。
- producerは`occlusion topology`、typed emitter definition / transform、typed emitter `PowerSupplyState`、Room maskのrevisionを発行する。change observer / queryはdirty reasonを記録するだけで、同一Updateの複数dirtyは最大1回のcollect / rebuildへcoalesceする。
- collect時はdirty producerのrevisionを比較し、必要なsnapshotだけを再構築する。steady stateではfull building / emitter snapshot scan、rebuild、output revision増分、scoped allocation event / byteを0にする。
- P03 coreがbytes同一と判定した場合、rebuild countは増えてもoutput field revisionは維持する。入力validationまたはP03 rebuildが失敗した場合はprevious lit fieldを返さず、fail-dark / unavailableをpublishする。
- Room mask更新はcanonical membership差分でのみdirtyになる。Room entityのrecreateやvalidationだけによるfalse dirtyは許可しない。

## 5. マイルストーン

## M1: manual Door request境界とpause契約を導入する

### 変更内容

1. P02で確立済みの`apply_door_state`を唯一のDoor mutation APIとして利用する。
2. `DoorLockToggleRequest`とroot message registrationを追加する。
3. UI intent handlerをrequest producerへ変更し、InterfaceからDoor / WorldMapの直接mutationを除く。
4. N→N+1、pause、競合、invalid owner、P05 lifecycle seamのテスト契約を作る。

### 主な変更ファイル

- `crates/hw_world/src/door_systems.rs`
- `crates/bevy_app/src/interface/ui/interaction/{intent_context.rs,intent_handler.rs}`
- `crates/bevy_app/src/plugins/{game.rs,messages.rs,visual.rs}`
- Door request / schedule integration tests

### 完了条件

- [x] production Door domain rootとactive presentation consumerでauto / manualが動く。P02 M3後のactive consumerは`Door3dVisual` exactly oneである
- [x] `RLV1-P02-DOOR-DOMAIN`を満たす実装・validatorを維持する
- [x] InterfaceからDoor / WorldMapの直接mutationがない
- [x] pause中のmanual lockがN+1 Updateで反映され、manual-vs-autoのLocked ruleが維持される
- [x] stale / invalid requestは一度だけ消費され、player notificationを増やさずreason別telemetryへ記録される
- [x] P05がworld replace clearを実装できるmessage lifecycle seamがある

## M2: semantic snapshotとdirty trackerを接続する

### 変更内容

1. `bevy_app`にWall / Door / emitter / power / Room mask adapterとroot-owned runtime resourceを実装する。
2. spawn、despawn、completion、state、power、mount別のrevision / dirty reasonを発行する。
3. checked grid conversion、canonical stable key、deterministic sort、conflict / duplicate / OOBのfail-darkをP03 snapshot境界へ接続する。
4. Room publish時にcanonical mask membershipだけで進む`RoomTopologyRevision`を実装する。
5. `WallMounted` anchorをcompleted Wall / provisional / Door / missing / map edgeのtableでsemantic validationする。

### 主な変更ファイル

- `crates/bevy_app/src/systems/lighting/{mod.rs,components.rs,adapter.rs,dirty.rs}`
- `crates/bevy_app/src/plugins/{game.rs,logic.rs,visual.rs}`
- `crates/hw_world/src/room_detection/{ecs.rs,room_systems.rs}`
- building completion / removalのdomain revision producer
- `crates/hw_infra/src/lighting/`（pure value / error境界の既存API確認のみ。ECS systemの追加なし）

### 完了条件

- [x] ProvisionalWall追加だけでは遮光せず、completed transition / removalは1回dirtyになる
- [x] typed emitter component `2 / 11 / 51`のうちeligible supplied `1 / 10 / 50`だけがsnapshotに入る
- [x] `Supplied`以外のemitter、non-Lamp `PowerConsumer`はsnapshotに入らない
- [x] checked grid conversion、stable-key順序、entity順shuffle、duplicate / OOB / Wall-Door conflictがdeterministicにfail-dark / unavailableとなる
- [x] completed Wallだけが`WallMounted` anchorとして有効で、provisional / Door / missing / OOB anchorはdiagnostic付きfail-darkとなる
- [x] Room entity再生成やvalidation timerだけではRoom mask revision / lighting dirtyが増えない
- [x] `FixtureMount`のsemantic型がP03の型に一意に保たれ、P05との二重定義がない

## M3: schedule transactionを固定する

### 変更内容

1. rootで`PreActor` / `PostActor`を含むtopology / energy / Room / Door / lightingのnamed setをconfigureする。
2. deferred command境界へ明示的な`ApplyDeferred`を置く。
3. shuffled plugin registrationでも同じorder edgeを持つschedule testを追加する。
4. auto Door開閉、Wall完成、manual requestのframe-by-frame integration testを追加する。

### 主な変更ファイル

- `crates/hw_core/src/system_sets.rs`
- `crates/bevy_app/src/plugins/{game.rs,logic.rs,visual.rs}`
- `crates/bevy_app/src/systems/lighting/mod.rs`
- `crates/bevy_app/src/entities/damned_soul/mod.rs`
- `crates/bevy_app/src/systems/familiar_ai/mod.rs`
- relevant schedule tests

### 完了条件

- [x] Wall完成は最初のcompleted UpdateでCPU field snapshotへ遮光として入る
- [x] auto Doorのstate、CPU field snapshot、Door presentation consumer、behavior observerの順序と観測値が同じUpdateで一致する
- [x] pause中manual requestのN→N+1 handoffとunpaused Actorの観測をtestで固定する
- [x] Room cooldownに関係なくDoor LOSが更新される
- [x] P04がGPU textureを保証しないことがtest名・assertionから明確である

## M4: instrumentation、P00 P04証跡、steady-stateを閉じる

### 変更内容

1. reason別dirty / collect / rebuild / changed-cell / input-output revision / availability / scoped allocation metricを追加する。
2. `PerfRttLightSelection`、CLI parser、RenderDoc selection、native helperへ`p04/static`、`p04/behavior`、`p04/field-core`を追加する。
3. production ECS adapterを使うP04 fixture、behavior timeline、runtime steady sidecar、bundle / validator登録を実装する。P03のfrozen pure `indoor_light_cpu.csv`をP04 runtime証跡へ読み替えない。
4. P00 small / medium / large fixtureでburstとsteady-stateを実測する。field-coreの600 Updateは定数出力ではなくproduction ECS adapterを実際に走らせる。
5. `rtt-light-v1`を変更せず、事前定義済みP04 schema / fieldだけを有効化する。

### 主な変更ファイル

- `crates/bevy_app/src/plugins/startup/perf_scenario/{config.rs,indoor_light_fixture.rs,behavior_driver.rs,field_core_driver.rs,output.rs,renderdoc_capture.rs}`
- `scripts/perf_tool/{arguments.py,fixtures.py,artifacts.py,rtt_light_bundle.py}`
- native acceptance helper / P00 validator registration
- P04 runtime / schedule integration tests

### 完了条件

- [x] P04 selector、sidecar、artifact validatorが`p04/static`、`p04/behavior`、`p04/field-core`を受理する
- [x] supplied / unsupplied typed-emitter fixtureが`2 / 11 / 51`、`1 / 10 / 50`、unsupplied adoption 0をP04 sidecarへ出す
- [x] behavior timelineがliveなavailability、input revision、output revision、checksumを出し、`stage_before_field_owner`を残さない
- [x] production adapterのunchanged 600 Updateでfull scan / rebuild / output revision increment / scoped allocation event / byteが全て0である
- [x] 同一Updateの複数dirtyは最大1 rebuildへcoalesceされる
- [x] `RLV1-BUNDLE-VALID`、`RLV1-P01-RTT`、`RLV1-P02-DOOR-DOMAIN`、`RLV1-P03-FIELD`、`RLV1-P04-EMITTER`、`RLV1-P04-STEADY`を満たす

## 6. 検証計画

- production shell由来Doorのauto / manual / pause / invalid request integration tests。
- Wall provisional→completed→removed、Door Open→Closed→Locked→Open、`Supplied → Shed / Disconnected / InvalidDemand → Supplied`のCPU field revision tests。
- canonical Room mask、entity順shuffle、Room再生成、stable-key duplicate / OOB / conflictのdeterminism tests。
- root schedule、`ApplyDeferred`、presentation consumer、behavior observerの順序を確認するschedule tests。
- production ECS adapterを実走する600-Update steady-state counter / scoped-allocation tests。
- P00 audit / behavior / Capture / Memory / field-core各3 run、RenderDoc固定1 frame、offline bundle gate検証。
- 必須Rust gate:
  - `python3 scripts/dev.py check`
  - `python3 scripts/dev.py cargo -- clippy --workspace --all-targets -- -D warnings`
  - `python3 scripts/dev.py cargo -- test --workspace`
  - `python3 scripts/dev.py verify`
- native Door / Wall same-Update acceptanceは`hell-workers-run-native-acceptance` Skillのno-prompt launcherとartifact validationで実行する。
- 実装完了時に、実際のplayer-visible pathから`hell-workers-review-help-impact` SkillでHelp impactのUpdate required / No impactを判断する。
- ドキュメント変更後は`python3 scripts/dev.py docs --write`、`python3 scripts/dev.py docs --check`、`git diff --check`を実行する。

## 7. リスクと対策

| リスク | 対策 |
| --- | --- |
| synthetic Doorだけ通りproductionが失敗する | completion helper由来domain root + active presentation fixtureを必須にする |
| manual requestがpause gate内で停滞する | `PreActor`をpause外に置き、N→N+1 timeline testを持つ |
| `DoorPresentationSyncSet`がInterface後のままになる | Visual内への移動とconsumer→observer order testを同じM3で必須にする |
| ECS adapterがP03 pure coreへ混入する | root `bevy_app`を唯一のECS ownerとし、`hw_infra`依存とfilesをCI review対象にする |
| `Changed`だけではdespawn / Room再生成を正しく扱えない | domain revisionとcanonical Room mask revisionを使い、Entity IDを保持しない |
| invalid input後に古い明るいfieldが残る | diagnostic後にfail-dark / unavailableをpublishする |
| P04証跡をP03 pure driverで代用する | selector、runtime sidecar、production adapterの600 UpdateをP04 M4の必須artifactにする |

## 8. ロールバック方針

- M1 Door correctnessは照明と独立した不具合修正として残し、Light Field rollbackで旧`Door + Sprite` queryへ戻さない。
- root snapshot adapterはplugin registrationを外して無効化でき、P03 pure coreへ影響させない。
- request schedulingを戻す場合もpending messageをclearし、二重writerを残さない。
- fail-dark / unavailableの状態は復旧不能な入力を隠すために前回のlit fieldへ戻さない。P05のepoch / reset rollbackはP05の契約に従う。

## 9. AI引継ぎメモ

### 現在地

- 進捗: `M1〜M4完了。P04 formal native evidenceと独立verificationがvalid`
- 完了済み: P03 formal entry、Door request / pause境界、runtime snapshot / dirty / schedule、P04 selector / sidecar / validators、production ECS adapterの600-Update steady test、actual-window / RenderDoc / Memory / field-core formal matrix
- formal subject: `44d22adcf1238de46ba8385615ae653f242c4aa0`
- S0 / S1: `target/native-acceptance/task-dashboard-20260815T174144Z-a3a9eca7`、`target/native-acceptance/rtt-light-s1-20260815T174443Z-7f4b40f4`（ともに`valid`）
- formal job / attempt: `target/native-acceptance/rtt-light-formal-20260815T175436Z-7320b1b6` / `target/perf-runs/rtt-light/rtt-light-v1/p04-44d22adcf1238de4/attempts/51d2b81d-ee8d-4242-a502-3f957c302903`
- formal result: 19 / 19 case valid、142 / 142 gate row pass、1,023 raw artifactをmanifestで封印し、独立`verify-rtt-light`もpass
- P04固有結果: typed emitter `2 / 11 / 51`、eligible supplied `1 / 10 / 50`、unsupplied adoption 0、600 unchanged Updateでfull scan / rebuild / revision increment / scoped allocationが全て0、field rebuild p95 `0.296658 ms` / p99 `0.335148 ms`
- 実機環境: Intel Arc Graphics (MTL)、Vulkan、Mesa `26.1.5`、X11、1920×1080、`auto_no_vsync`（effective `immediate`）
- 未完了: P04内になし。次の依存計画はP05

### 次のAIが最初にやること

1. P05のsave registry ownerと現worktreeを確認し、P04のruntime-only境界を変更せずP05計画をレビューする。
2. P05ではP03所有の`FixtureMount`を再定義せず、Reflect / 保存登録・migrationだけを追加する。
3. P04のformal attemptをP05のhistorical referenceとして独立再検証する。

### ブロッカー/注意点

- P05のsave registry / epoch / reset worktreeをP04で編集しない。
- root ordering owner以外のcrateからcross-domain `.before()` chainを増殖させない。
- Room entityは再生成され得るため、dirty state、stable key、mask identityへRoom Entityを保持しない。
- `FixtureMount`をP05で再定義せず、P03 pure value型を使う。
- P04のCPU field保証をP06のGPU / renderer証跡に読み替えない。

### 参照必須ファイル

- `docs/plans/3d-rtt/single-scene-light-field/03-indoor-light-domain-core-plan-2026-08-03.md`
- `docs/plans/3d-rtt/single-scene-rtt-indoor-light-field-migration-plan-2026-08-03.md`
- `crates/hw_infra/README.md`
- `crates/hw_core/src/system_sets.rs`
- `crates/bevy_app/src/plugins/{game.rs,visual.rs}`
- `crates/bevy_app/src/plugins/startup/perf_scenario/config.rs`

### 最終確認ログ

- Rust gates: `2026-08-15` / focused tests、profiling feature checks、`python3 scripts/dev.py check`、workspace Clippy、`python3 scripts/dev.py verify` pass
- native acceptance: `2026-08-16` / S0、S1、formal 19 / 19 case・142 / 142 gate row、独立`verify-rtt-light` pass（subject `44d22adc`、Intel Arc / Vulkan / X11）
- docs / Help gate: `2026-08-16` / Help impactはNo impact。P04は既存のpre-P05 fail-dark lifecycle契約を一貫適用し、player controls、labels、workflows、Door / indoor-light semanticsを変更しない

### Definition of Done

- [x] P03 formal evidence blockが解除されている
- [x] M1〜M4が完了
- [x] production Door direct-mutation pathが解消されている
- [x] Wall / Door / power / Room maskの更新順、errorのfail-dark、steady-stateがtestで固定されている
- [x] `stage=p04`のexact gate ID集合を満たす
- [x] Help impactの実際の判定、必要なnative acceptance、影響ドキュメント更新が完了している

## 10. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| `2026-08-16` | `Codex` | subject `44d22adc`のS0 / S1 / formalを実行し、19 / 19 case、142 / 142 gate row、独立verificationをvalidとして登録。M1〜M4とP04を完了し、P05へhandoff |
| `2026-08-16` | `Codex` | subject `8b155963`の初回formalはload-normal projection rowのfixture contract不一致をfail-closedでinvalidとし、契約の単一helper化後に新subjectで全証跡を再採取 |
| `2026-08-15` | `Codex` | P03 formal block解除後にM1〜M3とM4 toolingを実装。root-owned ECS adapter、Door request、Room mask revision、fail-dark、600-Update steady検証、P04 selector / sidecar / validatorを追加し、formal native evidenceだけをclean subject後へ残した |
| `2026-08-15` | `Codex` | P02 / P03の実状態、root ECS境界、Door / presentation順、Room revision、P04 evidence / steady-state契約を監査結果に合わせて改定 |
| `2026-08-04` | `Codex` | Room interior依存、stage別Door consumer、typed / eligible emitter exact gateをP00へ同期 |
| `2026-08-03` | `Codex` | 統合計画からDoor・energy・topologyのruntime transactionを独立化 |
