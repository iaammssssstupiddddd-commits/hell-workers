# P05: 室内 Light Field 保存・再構築 lifecycle計画

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `single-scene-light-field-05-indoor-light-save-lifecycle-plan-2026-08-03` |
| ステータス | `In Progress — implementation and native S1 complete; formal registration pending` |
| 作成日 | `2026-08-03` |
| 最終更新日 | `2026-08-16` |
| 作成者 | `Codex` |
| 親計画 | [`../single-scene-rtt-indoor-light-field-migration-plan-2026-08-03.md`](../single-scene-rtt-indoor-light-field-migration-plan-2026-08-03.md) |
| 直接依存 | [P03](03-indoor-light-domain-core-plan-2026-08-03.md)、[P04](04-indoor-light-runtime-integration-plan-2026-08-03.md)、[save rehydration registry計画（Archived / 完了）](../../archive/save-rehydration-registry-plan-2026-08-03.md) |
| 後続 | [P06](06-indoor-light-rendering-plan-2026-08-03.md)、[P07](07-indoor-light-gameplay-room-plan-2026-08-03.md) |
| 関連Issue/PR | `N/A` |

## 1. 目的

- 解決したい課題: CPU field、runtime emitter、GPU upload stateを保存すると古いworldの値やschemaを持ち越す。一方、durableなmountを持たずに再構築すると、将来のwall-mounted fixtureがload後に`FreeStanding`へ変わる。
- 到達したい状態: P03所有のpureな`FixtureMount`を意味論の唯一の正本に保ったまま、root adapterのdurable wrapperだけを保存する。world replacementでは旧fieldを読めないfail-darkへ遷移し、normal load、rollback、recovery-onlyは同じfrozen rehydrate planから一度だけwakeする。
- 成功指標: `stage=p05`の全7 behavior caseとformal 24 caseが成立し、preflight rejectではlive field / epochが不変、replacement開始後は旧epoch field readが0、successful replacementでは期待fixture checksumへ一度だけwakeする。

## 2. 確認済みの着手条件とスコープ

### 確認済みの着手条件

- save rehydration registryは完了済みで、`1b84f316`で導入された`SavePlugin::finish`のfreeze済みproduction planを使える。旧来の「owner合意またはmerge待ち」は解消済みである。
- `IndoorLightingPlugin`は`SavePlugin`より先にbuildされるため、lightingのroot facadeを登録してからregistryをfreezeできる。
- 着手時には`git status --short`、現行のproduction step snapshot、`LoadResetRegistry` inventoryを確認する。既存registryを迂回する照明専用load loop、stash / reset / checkoutによる他作業の破棄は行わない。

### 対象（In Scope）

- `FixtureMount`のroot-owned durable wrapper、legacy save migration、schema allow-list、candidate validation。
- completed `OutdoorLamp`の通常spawn / move / loadでdurable mountをauthoringし、runtime-only emitterをそのmountから再構築する経路。
- existing phase-aware rehydrate registryへのnamed step、`LoadResetRegistry`へのlighting reset hook、`WorldEpoch`を使うepoch-aware field read契約。
- P05の全behaviorケース、timeline telemetry、artifact validator、bundle、RenderDoc、native launcherを実行可能にするevidence owner。
- `docs/save_load.md`、`docs/indoor_lighting.md`、必要なarchitecture/state docs、親計画とindexの同期。

### 非対象（Out of Scope）

- 新しいwall-mounted fixtureのplacement UI、BuildingType、player workflow、art制作。P05ではcurrent player contentである`OutdoorLamp`の`FreeStanding` mountを正しく保存するだけであり、wall mountはdurable formatのforward-compatible test caseに留める。
- GPU `Image`、black texture upload、shader、render receiver（P06）。P05のGPU列は`not_applicable`であり、P06が自身のepoch comparisonとblack clearを所有する。
- Soul / Room consumer、Room summary reset（P07）。各consumer ownerはP05で確立する`LoadResetRegistry` extension pointとepoch-aware read APIを使う。
- generic save registryの再設計、phase順の変更、registry APIのleaf crateへの公開。

## 3. 正本・永続性・失敗分類

| 分類 | data | owner / save方針 |
| --- | --- | --- |
| durable | Building、DoorState、power policy、Transform | 既存schemaを正本にする |
| durable（P05追加） | root-owned `LightingFixtureMount(FixtureMount)` | `bevy_app`のschema allow-listへ追加。legacy saveでcomponent欠落時だけ`FreeStanding`を補完する |
| reconstructible | `RadialLightEmitter`、presentation shell | 保存しない。durable mountとcompleted fixture rootからrebuildする |
| derived | CPU field bytes、input/output revision、dirty cache、Room summary | 保存しない。reset後、named wakeを経たP04 transactionで再計算する |
| render cache | GPU Image、uploaded revision / epoch | 保存しない。P06が自身のreset hookでblack clearする |

`FixtureMount`は`hw_infra::lighting`のpure serde valueであり、Bevy `Component` / `Reflect`を追加しない。P05は`bevy_app::systems::lighting`に、同値を一つだけ保持する`Component + Reflect` wrapper（実装名は`LightingFixtureMount`）を置く。wrapperはopaqueなreflect serialization adapterとして検証し、座標・向きの別DTOや同等enumを作らない。

current `OutdoorLamp`のroot `Transform` gridと`FixtureMount::origin()`は常に一致する。normal spawnとfree-standing moveではcanonical `WorldMap::world_to_grid`からmountを更新し、rehydrateとP04 emitter syncはwrapperを読む。missing wrapperを補うのはlegacy loadまたは新規completed lampのauthoring時だけであり、runtime emitter syncが毎frameに`FreeStanding`を上書きするfallbackを残さない。

### candidate rejectとfail-darkの境界

- wrapperが付与されたentityのowner不正、map外origin、Transform / mount origin不一致、`WallMounted`の非cardinal / map外inward cell、candidate内で`CompletedWall`ではないanchorはdurable candidate不正である。`lighting.fixture.validate`がstaging worldだけを読み、reset前にrejectする。live world、`WorldEpoch`、live fieldは不変である。
- validにload済みのworldで後からmount / topologyが壊れた場合はP04の既存runtime入力契約に従う。前回のlit snapshotを返さず`IndoorLightAvailability::Unavailable`としてfail-darkにする。P05は別anchorや向きを推測して修復しない。
- legacy component欠落はcorruptionではない。`lighting.mount.normalize`がcompleted `OutdoorLamp`のTransformから`FreeStanding`を一度だけ補完する。

## 4. registry・reset・epoch契約

### 4.1 rehydrate registry

P05は`RehydratePhase`やraw registryを公開しない。existing `register_logic_rehydrate_pipeline` / `register_visual_rehydrate_pipeline`と同じroot facadeとして`register_lighting_rehydrate_pipeline`を追加し、`IndoorLightingPlugin`から登録する。`SavePlugin::finish`が全plugin build後にfreezeする既存境界を変えない。

| phase | step ID | `after` | 役割 |
| --- | --- | --- | --- |
| `DurableNormalize` | `lighting.mount.normalize` | `construction.normalize` | legacy defaultを一度だけ補い、candidateで保証済みのdurable mountを正規化する |
| `RebuildDerived` | `lighting.emitters.rebuild` | `lighting.mount.normalize`、`power-consumer.policy`、`presentation.shells` | rootのdurable mountからruntime-only emitterを再付与する |
| `WakeDomains` | `lighting.wake` | `lighting.emitters.rebuild`、`domains.wake` | full dirtyを一度だけrequestする。field計算、power settlement、flushは行わない |

- callbackはinfallibleであり、`World::flush()` / `clear_trackers()` /独自schedule loopを呼ばない。AttachShellsからRebuildDerivedへのbarrierとfinal flushはresolved registry runnerだけが所有する。
- `lighting.wake`後の次Updateで、unpausedならexisting Logic power settlementの後、P04 PostActor transactionがfieldを作る。paused loadでは給電runtime stateがsettleするまでavailableなzero fieldまたはunavailable fail-darkを保ち、旧fieldへ戻らない。
- normal apply、rollback finalizer、recovery-only replaceは同じfrozen step列を使う。rollbackではreset hookは2回実行され得るが、`WorldEpoch`は1回だけadvanceし、final wakeは1回である。
- production step名・validator名・prerequisite名のexact snapshot testを更新し、重複名、unknown dependency、cycle、phase regressionがfreeze時にfail-closedのままであることを確認する。

### 4.2 world replacement / fail-dark

`IndoorLightingPlugin`がroot `LoadResetRegistry`へ`lighting-runtime` hookを登録する。hookは次だけを所有する。

1. `IndoorLightRuntime`からsnapshot、pending input、checksum、input/output revision、published epoch、read probeを消去し、readerにfieldを返さない`Unavailable` fail-dark状態へ遷移する。
2. `IndoorLightingDirty`をclearし、P05成功時の`lighting.wake`だけが次transactionをarmできるようにする。`RecoveryFailed`で空worldを再計算しない。
3. lighting固有のallocation / lifecycle probeをresetする。Door message、UI、Room、GPUのcacheは各existing / future ownerがresetする。

Door requestのclearは既存`MessagesPlugin`の`root-messages` reset hookが所有する。P05はそのownerを複製せず、normal / rollback / recovery-onlyの統合testでrequestが残らないことを確認する。

global epochの正本は既存`WorldEpoch`である。lighting runtimeはepochを生成せず、公開snapshotに`published_epoch`をtagする。P05以後のconsumer向けread APIはcallerの`WorldEpoch`を受け、tag不一致・reset中・unavailableなら`None`を返す。direct snapshot accessはlighting内部 / test-onlyに閉じ、P06 / P07にraw fieldを渡さない。behavior probeはold-epoch read attempt、reset後dark、wake回数を記録するが、steady-state production traceを保持しない。

P06は同じ`WorldEpoch`とepoch-aware field readを比較してGPU uploadをinvalid化しblack clearする。P07も同じread APIと自身のreset hookを使う。P05がP06 / P07専用messageやunused resourceを先行追加してはならない。

## 5. マイルストーン

## M1: durable mount adapterとcandidate境界を固定する

### 変更内容

1. `bevy_app`所有の`LightingFixtureMount` wrapperを追加し、P03 `FixtureMount`を唯一の意味論として保持する。opaque reflect serialization、`Component` registration、DynamicWorld allow-listをschema round-tripで確認する。
2. completed `OutdoorLamp`のpost-processとfree-standing Transform change経路でmountをauthoring / 同期する。runtime-only `RadialLightEmitter`はschema外のままにする。
3. `sync_outdoor_lamp_emitters_system`とload用emitter rebuildをmount-awareにし、legacy missing componentだけをcanonical Transform gridから補う。
4. isolated staging candidate validatorを登録する。live `WorldMap`、live Entity、旧fieldを参照せず、§3のdurable mount不正をreplace前にrejectする。
5. current / legacy save、valid wall-mounted schema fixture、corrupt owner / anchor / origin / transform mismatch fixtureを追加する。

### 主な変更ファイル

- `crates/bevy_app/src/systems/lighting/{components.rs,runtime.rs}`
- `crates/bevy_app/src/systems/jobs/building_completion/post_process.rs`
- `crates/bevy_app/src/systems/save/{schema.rs,load.rs,saving.rs}`
- `crates/bevy_app/src/systems/save/schema/tests.rs`
- `crates/bevy_app/src/systems/save/rehydrate/{candidate.rs,tests/}`

### 完了条件

- [x] P03 `FixtureMount`へBevy ECS / GPU dependencyを逆流させず、別のmount enum / DTOも作らない
- [x] persisted payloadにはwrapperだけが入り、`RadialLightEmitter`、field bytes、revision、GPU handleは入らない
- [x] old saveはfree-standing mountへ一度だけ移行し、current save round-tripはmountを保つ
- [x] corrupt durable mountはpreflight rejectし、live field / epoch / selectionを変えない
- [x] post-loadおよび通常spawn / moveでemitterがdurable mountを読み、暗黙のFreeStanding上書きがない

## M2: named rehydrate pathをregistryへ接続する

### 変更内容

1. root-owned lighting registration facadeと§4.1の3 stepを追加する。`IndoorLightingPlugin`から登録し、raw registry internalsをlighting / leaf crateへ露出しない。
2. mount normalize、emitter rebuild、wakeのdependency edgeとtrace expectationをproduction snapshotへ追加する。
3. normal、rollback、recovery-onlyが同じresolved planを使い、callback内でfield計算が0回であることをtestする。

### 主な変更ファイル

- `crates/bevy_app/src/systems/save/{mod.rs,rehydrate.rs}`
- `crates/bevy_app/src/systems/save/rehydrate/{registry.rs,tests/}`
- `crates/bevy_app/src/plugins/{lighting.rs,game.rs}`
- `crates/bevy_app/src/systems/lighting/{mod.rs,runtime.rs}`

### 完了条件

- [x] exact step snapshotに3 stepとdependency edgeがあり、freeze後のlate registrationはfail-closed
- [x] normal / rollback / recovery-onlyでfinal registry traceは同一順・各stepexactly once
- [x] `lighting.wake`はdirty requestだけを出し、rehydrate trace中のfield rebuildは0
- [x] rollbackではreset hook 2回、epoch delta 1、final wake 1をtable testで固定する

## M3: reset、epoch-aware read、failure matrixを閉じる

### 変更内容

1. lighting reset hook、explicit `Unavailable` fail-dark state、published epoch tag、epoch-aware read APIを実装する。
2. reset中 / recovery failure中はold snapshot、pending input、dirty flagを残さず、成功時だけnext UpdateのP04 transactionへ渡す。
3. Door requestはexisting root message resetに委譲し、lighting resourcesとの全owner resetをintegration testで確認する。
4. 下表をheadless transaction testで固定する。GPU upload列はP05では`not_applicable`である。

| P00 behavior case ID | expected live state | epoch delta | final wake |
| --- | --- | ---: | ---: |
| `load-preflight-reject-v1` | live field / epoch不変 | 0 | 0 |
| `load-normal-v1` | reset直後Unavailable、new fixtureからrebuild | 1 | 1 |
| `load-rollback-v1` | reset直後Unavailable、rollback fixtureからrebuild | 1 | 1 |
| `load-recovery-only-v1` | reset直後Unavailable、recovery fixtureからrebuild | 1 | 1 |
| `load-recovery-failed-v1` | Unavailableのままpaused、rebuildなし | 1 | 0 |
| `load-duplicate-reset-v1` | idempotent reset後もUnavailable、terminal rebuild 1回 | 1（coalesced） | 1 |

### 主な変更ファイル

- `crates/bevy_app/src/systems/lighting/{mod.rs,runtime.rs}`
- `crates/bevy_app/src/plugins/lighting.rs`
- `crates/bevy_app/src/systems/save/{reset.rs,transaction.rs}`
- `crates/bevy_app/src/systems/save/{native_acceptance.rs,tests}`

### 完了条件

- [x] replacement開始からwake / terminal failureまでold-epoch field readが0
- [x] stale Door requestとlighting runtime stateが残らず、Room / GPUをP05が誤って所有しない
- [x] preflight reject、normal、rollback、recovery-only、recovery-failed、duplicate resetを実transaction経路で通す
- [x] valid post-load P04 fieldのchecksumはfixture contractに一致し、invalid runtime inputではold lit fieldを返さない

## M4: P05 behavior / artifact / native evidenceを有効化する

### 変更内容

1. `p05` selectorをRust config、Python CLI、field-core、static / behavior / Capture / Memory / RenderDoc、bundle validator、native launcherへ一貫して追加する。P04 topologyを継承し、P06 GPU evidenceを偽装しない。
2. `PerfBehaviorCase`とbehavior driverへ残る5 lifecycle caseを追加する。profiling-only、non-persisted、job-private scenario injectionでpreflight reject、apply fault、rollback、recovery-only、RecoveryFailed、duplicate resetを本物のtransaction経路から駆動する。native acceptance専用`NativeLoadFaultInjection`をprofiling runnerへ暗黙流用しない。
3. timelineへregistry phase / step、epoch、read、dark、wake、terminal checksumを記録し、P05 gateが必要とする`live_field_unchanged`、`world_epoch_delta`、`old_epoch_field_reads`、`reset_is_dark`、`terminal_checksum_match`、`wake_count`を抽出する。
4. contractの既定7 case、P05 formal 24 case、artifact file集合、required / not-applicable列を更新する。frozen contractのschema / fingerprintを変える必要が生じた場合は、P00 ownerの明示的なversion / rebaseline判断を先に記録し、黙ってthresholdを変更しない。
5. malformed timeline、missing sidecar、stage / lane mismatch、改竄したepoch / trace / checksum / wake値をfail-closedにするfixture self-testを追加する。

### 主な変更ファイル

- `crates/bevy_app/src/plugins/startup/perf_scenario/{config.rs,behavior_driver.rs,field_core_driver.rs,output.rs,save_transaction.rs,renderdoc_capture.rs}`
- `scripts/perf_tool/{arguments.py,artifacts.py,fixtures.py,rtt_light_bundle.py,rtt_light_contract.py,renderdoc_capture.py,renderdoc_extract.py,renderdoc_foundation.py}`
- `scripts/perf_tool/contracts/rtt_light_migration_v1.json`
- `.codex/skills/hell-workers-run-native-acceptance/scripts/native_acceptance.py`

### 完了条件

- [x] `python3 scripts/perf.py behavior --stage p05 --dry-run`と`field-core --stage p05 --dry-run`がP05 contractを検証して受理する
- [x] 7 behavior case × 3 runとfield-core 3 runはvalid
- [ ] clean commitに紐づくstatic / Capture / Memory / RenderDocを含むformal 24 / 24 registrationは未実施
- [x] `RLV1-P05-LIFECYCLE`がCPU fail-dark、epoch、read、wake、checksumを判定し、P06-only GPU metricは`not_applicable`である
- [x] actual-window / RenderDoc / native launcherはP05をcontract-derivedなcase集合として扱い、P04固定のhardcodeを残さない

## M5: 文書、品質gate、native acceptanceを閉じる

### 変更内容

1. save/load domain ledgerへdurable wrapper、candidate reject、reset owner、rehydrate step、wake timing、silent fail-dark条件を追記する。
2. indoor-lighting documentationへmount保存、epoch-aware read、P05 lifecycleを追記する。crate boundary / architecture / stateにownerまたはreset contractの変更があれば同時更新する。
3. player-visible F5 / F9 load workflowへの影響を`hell-workers-review-help-impact` Skillで実際に判断する。No impactなら理由を変更batchに残し、更新が必要ならHelp catalog / provider / coverage / approval snapshotを同時更新する。
4. CPU lifecycleのnative acceptanceをP05自身で閉じる。GPU black upload / upload epochのnative assertionはP06へ残す。

### 完了条件

- [x] `docs/save_load.md`、`docs/indoor_lighting.md`、必要なarchitecture/state docs、親計画、plans indexが実装と一致する
- [x] Help impactは「既存F5/F9内部だけの変更で入力・文言・workflowに変更なし」として理由付き`No impact`へ確定
- [x] P05 native S1はAudit 3/3、actual-window Capture 18/18、Memory 18/18、field-core 3/3がvalid。CPU lifecycleの21/21 behavior artifactもvalid

## 6. 検証計画

- focused Rust tests:
  - schema wrapper / legacy migration / candidate validation
  - registry dependency / exact production snapshot / normal-rollback-recovery trace
  - reset idempotence / epoch-aware read / stale message integration
  - all P05 behavior fault cases and artifact negative fixtures
- local quality gates:
  - `python3 scripts/dev.py check`
  - `python3 scripts/dev.py cargo -- clippy --workspace --all-targets -- -D warnings`
  - `python3 scripts/dev.py cargo -- test --workspace`
  - `python3 scripts/dev.py verify`
- evidence:
  - P00 `stage=p05`のstatic、全7 behavior case、Capture、Memory、RenderDoc、field-coreのrequired runを実行し、formal 24 / 24 caseとexact gate ID集合を検証する。
  - native acceptanceは`hell-workers-run-native-acceptance` Skillのno-prompt launcherを使い、artifact存在とoffline bundle validationをfail-closedで確認する。
- docs / diff:
  - `python3 scripts/dev.py docs --write`
  - `python3 scripts/dev.py docs --check`
  - `git diff --check`

## 7. リスクと対策

| リスク | 対策 |
| --- | --- |
| pure coreへBevy persistenceを逆流させる | root-owned opaque wrapperだけをReflect登録し、P03 typeを複製しない |
| mountを保存してもruntime syncがFreeStandingで上書きする | spawn / move / rehydrate / emitter syncを同じwrapper正本へ統一する |
| corrupt saveのload開始後にfail-darkとなりlive worldを失う | durable candidate validationをreset前のstagingへ置く |
| reset後にdirtyが残りRecoveryFailed worldをrebuildする | resetはdirtyをclearし、successful `lighting.wake`だけがarmする |
| rollbackでepochやwakeを二重に進める | reset count 2、epoch delta 1、final wake 1をtransaction table testで固定する |
| P06 / P07用の未使用messageをP05で作る | global `WorldEpoch`、epoch-aware read、LoadResetRegistry extension pointだけを公開する |
| P05 contractだけが先行しrunnerがP04止まりのままになる | tooling / validator / native launcherをM4の必須実装成果にする |
| frozen thresholdをP05の測定結果に合わせて緩める | schema / rebaseline変更はP00 ownerの明示判断へ戻し、同じgenerationでthresholdを変更しない |

## 8. ロールバック方針

- M1のdurable writerを追加した後は、reader / writer compatibility shimを残す。新writerだけを削除して旧readerを失うrollbackは禁止する。
- M2 / M3のlogical lifecycleはP06を戻す場合にも保持する。GPU cacheはP06だけをfeature-offできる。
- P05 tool changesはcontract / runner / validator / native launcherを同じcompile可能batchに保つ。P05 selectorだけ、またはvalidatorだけを先行mergeしない。
- candidate validatorの誤rejectはlive reset前に止まるため、diagnostic fixtureを増やしてからpolicyを変更する。runtimeの別anchor推測で回避しない。

## 9. AI引継ぎメモ（最重要）

### 現在地

- 進捗: `97%（M1〜M5実装、docs / local gates / headless evidence / native S1完了。formal registration待ち）`
- 完了済み: durable mount adapter、candidate validator、named rehydrate/reset/epoch contract、P05 tooling、21/21 behavior、3/3 field-core、Help No impact、`dev.py verify`、native S1 42/42 actual-window/static run。
- 残作業: clean committed subjectとS0/S1 prerequisiteを要するformal 24 / 24 registrationをpublish境界で実施する。

### 次のAIが最初にやること

1. publish時はclean committed subjectでS0/S1 prerequisiteを揃え、formal 24 / 24を登録する。
2. P05 production / contractを変更せずP06のGPU upload / black clearへ進む。

### ブロッカー/注意点

- save registryはブロッカーではなく、P05 stepは既存phaseを変えずedge付きで追加済み。
- formal registrationはclean committed subject、S0/S1 prerequisite、actual-window Capture / Memory / RenderDocを要求する。未コミットworktreeを証跡登録のためだけにcommitしない。
- P05はplayer-facing wall fixture UIを追加していない。Help impactは既存save/load内部だけの変更としてNo impact確認済み。

### 最終確認ログ

- Rust gates: `2026-08-16` / `pass`（`dev.py check`、workspace Clippy 0 warning、workspace test、`dev.py verify`）
- headless evidence: `2026-08-16` / `pass`（behavior 21/21、field-core 3/3。p95 `0.625808 ms`、p99 `0.905667 ms`）
- native acceptance: `2026-08-16` / `pass`（job `p05-s1-20260816`、Audit 3/3、Capture 18/18、Memory 18/18、field-core 3/3、Intel / Vulkan / X11、artifact verification pass）
- native field-core: p95 `0.397500 ms`、p99 `0.436571 ms`
- docs / Help gate: `2026-08-16` / Help impact `No impact`。`docs --write / --check`、`diff --check` pass

### Definition of Done

- [ ] M1〜M5が完了し、durable / reconstructible / derived / render cacheの境界が実装と文書で一致する
- [x] all successful lifecycle branchesが同じresolved traceを通り、preflight rejectはlive stateを変えない
- [ ] `stage=p05`の全7 behavior case × 3 run、formal 24 / 24 case、exact gate ID集合を満たす
- [x] CPU fail-dark、old epoch read 0、wake、terminal checksumのheadless証跡とP05 native S1が確認済み
- [x] Help impact、恒久docs、docs index、full quality gateが完了している

## 10. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| `2026-08-16` | `Codex` | P05実装、21/21 behavior、field-core、full quality gate、Help No impact、native S1（Audit / Capture / Memory / field-core全valid）を完了。clean subjectを要するformal 24 / 24 registrationだけをpublish境界へ残した。 |
| `2026-08-16` | `Codex` | C3完了を反映してブロッカーを解除。pure mount / root persistence adapter、candidate reject、registry edge、reset / epoch、P05 evidence runner、P06との責務境界を実装可能な計画へ再構成。 |
| `2026-08-04` | `Codex` | P00 behavior case / lifecycle gateへ同期し、Room summary reset登録をP07 ownerへ修正。 |
| `2026-08-03` | `Codex` | runtime統合から保存・rollback lifecycleを分離し、現行registryへの接続点を具体化。 |
