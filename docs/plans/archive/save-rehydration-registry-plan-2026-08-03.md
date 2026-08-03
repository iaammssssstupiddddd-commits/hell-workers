# Track C3 ロード後再構築レジストリ 実装計画

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `save-rehydration-registry-plan-2026-08-03` |
| ステータス | `Archived` |
| 作成日 | `2026-08-03` |
| 最終更新日 | `2026-08-04` |
| 作成者 | `Codex` |
| 関連提案 | `docs/proposals/gameplay-management-improvements-proposal-2026-07-17.md`（Track C3） |
| 関連Issue/PR | `N/A` |

## 1. 目的

- 解決したい課題: ロード後再構築の暗黙順序と通常ロード／rollbackの追従漏れを機械的に防げない
  - 現行のロード処理は `LoadResetRegistry` と複数の `rehydrate/*` サブモジュールまで分割済みだが、
    `rehydrate_after_load` がドメイン別の補完、shell 付与、flush、index 再構築を手書き順序で呼んでいる。
  - 新しい durable component や runtime-derived state を追加するたびに、schema、preflight、reset、rehydrate の
    どこへ登録するかをコードレビューだけで追う必要があり、順序依存や rollback 側の追従漏れを機械的に検出しにくい。
  - 現行の fallible validation と mutation が同じ rehydrate entrypoint に混在し、live world 置換前に否定できる
    不正条件まで apply 後の rollback に委ねる余地がある。
- 到達したい状態:
  - ロード候補についてlive置換前に判定可能な全domain validationを終え、その後の再構築を明示的なphaseと依存関係を持つ
    root-owned registry から一度だけ実行する。
  - 通常ロードと rollback 復旧が、同じ normalization / shell / derived-state pipeline を通る。
  - 各ドメインについて「保存する正本」「ロード後に補完する durable default」「再生成する runtime state」
    「presentation shell」を一箇所の台帳と coverage test で追跡できる。
  - `RecoveryFailed` 中だけ使用できる、rollback 候補を前提としない再ロード境界を用意し、通常ロードの
    transaction 保証を弱めずに Track C2 の fail-closed recovery を成立させる。
- 成功指標:
  - production registry の step 名重複、未知依存、循環、phase 逆行が自動テストで 0 件。
  - 通常ロードと rollback の両方で、各mutation stepが同じ確定順にexactly once実行される。
  - v0/v1 fixture、B1 Stockpile、B2 Familiar、B3 Soul Energy、construction、obstacle の既存 round-trip結果が変わらない。
  - preflight 可能な不正条件では persisted entity、Resource、WorldEpoch、UI/visual stateを変更しない。

## 2. スコープ

### 対象（In Scope）

- `rehydrate_after_load` の既存手書き列を、phase-aware な root registry と単一mutation runnerへ移行する。
- live prerequisite と candidate world invariant を読む非変更 validation hook。
- durable default/normalization、runtime normalization、shell 付与、derived index/cache再構築、domain wake の phase分離。
- phase間の `World::flush()` 所有権と、step間依存の検証。
- root adapterからのdomain step登録と、未登録・二重登録を検出するproduction coverage test。
- normal load / rollback の共通 finalizer、既存 v0/v1 fixture、fault injection の回帰。
- runtime task edge の schema writer 除外、legacy strip、期限付き item lifecycle の再構築契約。
- transaction coordinatorが所有する`SaveRecoveryMode::Healthy | RecoveryFailed`と、後者からだけ呼べる
  recovery-only replace modeの低位API・回帰。Track C2はこのstateをUIへ公開する。
- `docs/save_load.md`、`docs/architecture.md`、`docs/invariants.md`、HVAC M3の登録手順同期。

### 非対象（Out of Scope）

- セーブスロット、カタログ、オートセーブ（Track C2）。
- `RecoveryFailed` の画面、world input全体のallow-list、slot選択、再試行/終了操作（Track C2）。
- container format v2 や汎用 world schema migration engine。
- 非同期/バックグラウンドロード、複数フレームへ跨るworld replacement。
- 既存 `LoadResetRegistry` の廃止、またはresetとrehydrateを同じhook列へ統合すること。
- 各leaf crateをroot registry型へ依存させること。
- HVAC M0〜M2をC3完了まで待たせること。C3はHVAC M3のConduit保存・FluidGrid再構築前だけを必須境界とする。

## 3. 現状とギャップ

| 現行契約 | 残るギャップ | 本計画の対応 |
| --- | --- | --- |
| `LoadResetRegistry` がowner別resetを実行 | rehydrate側には同等の登録/coverage契約がない | resetは維持し、別の `RehydrateRegistry` を追加 |
| `rehydrate.rs` は prerequisites / presentation / construction / obstacles へ物理分割済み | facadeの関数呼出順が唯一の暗黙正本 | phase + dependency付きstep列を正本化 |
| success/rollbackは共通 `finalize_loaded_world` へ合流 | finalizer内部のstep同一性を直接検査できない | 同じrunner traceを両branchで検証 |
| Familiar policy補完は不正rosterを検査してから変更 | 検査がrehydrate mutation入口に残る | candidate validationとnormalizationを分離 |
| cache resetがenergy/room/spatialの再計算をwakeする | 即時再構築と次Logic wakeの区別が台帳化されていない | step ledgerに実行時点と正本を明記 |
| presentation shell後に明示 `world.flush()` | hookが任意flushすると順序が崩れる | barrierはrunnerだけが所有 |

## 4. 実装方針（高レベル）

### 4.1 registry境界

- root `bevy_app::systems::save` が次の4種類を所有する。
  - `CandidateValidator`: isolated stagingへ適用済みのcandidate persistent worldだけを読み、
    live worldを変更せず `Result` を返す。
  - `LivePrerequisite`: candidateとは独立したasset/time等のlive presentation前提だけを読み、`Result`を返す。
  - `RehydrateStep`: validation成功後のlive worldを変更する。期待される入力不備を返さないinfallible callbackとする。
  - `ResolvedRehydratePlan`: raw登録をphase/dependency順へ解決したimmutableなstep列。
- raw registryはplugin登録期間だけ変更可能とし、全pluginの登録後・schedule開始前にduplicate/unknown dependency/cycle/
  phase逆行を検査して`ResolvedRehydratePlan`へfreezeする。Bevy 0.19 の全 `Plugin::build` 完了後に呼ばれる
  `SavePlugin::finish` をfreeze境界とし、production構成不正は最初のload時まで遅延させない。
- 現行 `preflight_dynamic_world` が既に作るisolated staging worldをvalidation対象として再利用する。
  bodyの再deserializeやvalidatorごとのWorld cloneは行わず、全hook完了後にstagingを破棄してからlive replaceへ進む。
- transaction開始前にincoming candidateと、その時点で取得するrollback snapshotの両方を同じcandidate validatorへ通す。
  rollback候補が不正ならlive resetを始めず、recovery不能なtransactionを開始しない。
- `CandidateValidator`はdiagnosticだけを返し、stagingのEntity ID、参照、QueryStateをlive rehydrateへ持ち出さない。
  mutation stepはlive適用後のEntity/Relationshipを改めて解決する。
- leaf crateはroot型へ依存しない。自ドメインの `validate_after_deserialize` / `normalize_after_load` /
  `reset_for_world_replace` 相当を公開し、root plugin facadeが型消去して登録する。
- asset注入や複数crateを横断する処理はroot stepのまま保つ。C3をcrate移設計画にしない。

### 4.2 validation runner、mutation phase、barrier

preflightとlive rehydrateを一つのrunnerに見せない。全体境界を次に固定する。

```text
App/plugin finalize
  -> raw registryをResolvedRehydratePlanへfreeze（構成不正なら起動失敗）

normal load request
  -> decode + PreparedLoad normalization + schema validation
  -> live presentation prerequisite validation
  -> incoming staging write + candidate validation（live world非変更）
  -> rollback snapshot capture + PreparedLoad normalization
  -> rollback snapshot staging write + candidate validation（live world非変更）
  -> live reset / DynamicWorld write（write自体はfallible）
  -> ResolvedRehydratePlanのmutation runner:
     DurableNormalize
       -> RuntimeNormalize
       -> AttachShells
       -> runner-owned World::flush()
       -> RebuildDerived
       -> WakeDomains
       -> runner-owned final World::flush()
  -> write/fault失敗時はrollbackをwriteし、同じmutation runnerを再実行

RecoveryFailed専用ownerの`RecoveryLoadRequested`（coordinator guard経由のみ。production producer/slot UIはTrack C2）
  -> decode + PreparedLoad normalization + schema validation
  -> live presentation prerequisite validation
  -> incoming staging write + candidate validation（live world非変更）
  -> rollback snapshotは取得・検証しない
  -> idempotent live reset / DynamicWorld write / 同じmutation runnerを一度だけ実行
  -> 成功時だけRecoveryFailedを解除、失敗時はpaused fail-closedを維持
```

- `PreparedLoad normalization`: schema検査前にlegacy runtime-derived componentをstripする既存境界。
  `AssignedTask`を保存しない契約と矛盾するassignment/use/claim Relationshipは§4.4の台帳どおりsource/targetを
  対で除去し、load後はDesignation/TransportRequestから再割当させる。durable ownership/provenanceである
  `ManagedBy` / `ManagedTasks`はstripせずcandidate整合性を検査して保持する。
  live durable補完やpresentation生成を入れない。
- current writer は `AssignedTask` と同じく runtime task execution edge（`WorkingOn` / `TaskWorkers`、
  `DeliveringTo` / `IncomingDeliveries`、`PushedBy` / `PushingWheelbarrow`）とrequest claim/lease/pending timerを
  保存対象から除外する。旧v0/v1をdecodeできるloader-only型登録は残し、incoming/rollback双方の`PreparedLoad`で
  Relationshipはsource/targetを対で、singleton runtime stateはcomponent単位でstripする。
- `DurableNormalize`: 旧saveで欠落したFamiliar/Stockpile/Power policy補完、Soul Spa値、construction phase/counterの正規化。
- `RuntimeNormalize`: legacy payloadから残った`AssignedTask::None`と不整合な`WorkingOn` / `TaskWorkers`、
  item/carrier/request状態を§4.4のowner lifecycleで清掃し、presentation前のruntime初期状態へ揃える。
  `LoadedIn` / `LoadedItems`はcarrier位置をEntity remap後まで渡すstaging handoffとしてここまで保持し、
  保存しない搬送先へcargoを持ち越さないようcarrier近傍へ安全に荷下ろししてから両Relationshipを除去する。
- `AttachShells`: 正規化済み値からSoul/Familiar/Building/constructionのruntime shellと随伴visualを復元する。
- `RebuildDerived`: construction index、obstacle provenance/navigation cache等をdurable sourceから一度だけ再構築する。
- `WakeDomains`: energy、spatial、room、diagnostics等、次の名前付きschedule phaseで再計算するownerへfull-dirtyを通知する。
- step callbackは `flush` / `clear_trackers` を呼ばない。旧RemovedComponentsの破棄は従来どおりtransaction coordinator、
  phase barrierはregistry runnerだけが所有する。

### 4.3 順序と失敗契約

- 各stepはstable name、phase、`after`依存、callbackを持つ。名前の辞書順を独立stepのtie-breakに使い、
  plugin登録順を意味順にしない。
- 名前重複、未知依存、循環、後phaseから前phaseへの依存はApp/plugin finalize時にfail-closedで検証し、
  load transactionは解決済みimmutable planだけを受け取る。
- candidate data、rollback snapshot、registry、必須asset/resource、Familiar roster等の検査は全てpreflightへ置く。
  validation失敗では reset / despawn / WorldEpoch更新を開始しない。
- mutation phaseに新しいfallible処理を追加しない。外部I/Oはrehydrate stepへ入れない。
- reflect `DynamicWorld`のlive write自体は引き続きfallibleであり、失敗時のrollback契約を弱めない。
- normal apply後とrollback snapshot復旧後は、transaction開始前にfreeze済みの同じ`ResolvedRehydratePlan`を渡す。
  同一trace保証はmutation step列だけを対象とし、branch固有の手書き補完を禁止する。
- recovery-only replaceは通常transactionの例外として明示した別modeとし、coordinator-owned `SaveRecoveryMode`が
  `RecoveryFailed`でない呼出しをrejectする。rollback自体の失敗時にだけ同stateへ入り、成功時だけ`Healthy`へ戻す。
  Failed遷移時は`Time<Virtual>`を即時pauseし、recovery成功でも自動unpauseしない。Track C2のforeground UIから
  playerが明示resumeするまで停止を維持する。
  現worldは既に信頼できないためrollback候補を要求しない一方、incomingのdecode/schema/staging/domain validationと
  live prerequisite検査は省略しない。reset hookは同じworldへ再実行可能なidempotent契約とし、apply失敗後も
  coordinatorはsave/通常loadを許可しない。Track C2は同stateを使ってautosave/resume/world操作をgateし、
  別slot再ロードまたは終了だけを残す。
- 通常F9の`LoadRequested`をrecovery-onlyへ暗黙昇格しない。Track C3は専用`RecoveryLoadRequested` triggerと
  low-level transaction境界を提供するが、production producerはTrack C2まで接続しない。
- candidate自身の`WorldMap` shapeと内部Entity参照をstaging validatorで検査する。新worldの成立に旧live
  `WorldMap`を要求せず、信頼できないlive worldからのrecovery-only replaceも同じcandidate契約で成立させる。

### 4.4 task execution state ledger

`AssignedTask`を保存しない既存契約に合わせ、task周辺stateを型別に固定する。一括stripや次Logic任せにしない。

| State | 分類 | PreparedLoad / RuntimeNormalizeの契約 |
| --- | --- | --- |
| `WorkingOn` / `TaskWorkers` | runtime assignment | source/targetを対でstripし、paused load直後からworker 0 |
| `DeliveringTo` / `IncomingDeliveries` | accepted haul claim | source/targetを対でstripし、不足需要を再評価可能にする |
| `PushedBy` / `PushingWheelbarrow` | runtime tool use | 対でstrip。validな`BelongsTo -> WheelbarrowParking`から`ParkedAt`を復元し、owner不正はcandidate reject |
| `Inventory(Some(item))` | runtime carried item | itemの存在・一意ownerとSoul近傍のdrop可能cellをcandidate validationする。owner helperでdropし、slotを`None`へする。Sand/StasisMudにはfresh 5秒timerを再付与する |
| `LoadedIn` / `LoadedItems` | serialized staging handoff | carrier不在/不正、非対称、容量超過はcandidate reject。remap済みcarrier位置（運搬中ならcarrying Soul位置）の最寄りwalkable cellへ内容を表示状態で荷下ろしし、両Relationshipを除去する |
| `ParkedAt` / `ParkedWheelbarrows` | reusable tool location | valid parking owner/capacityを検査して保持し、relationship targetはsourceから整合させる |
| `TransportRequestState` | request runtime claim | worker/claim解除後はsame rehydrateで`Pending`へ戻し、runtime lease/timerをclearする |
| `ManagedBy` / `ManagedTasks` | durable owner/provenance | strip禁止。source/target整合とowner生存をcandidate validationする |
| `ItemDespawnTimer` | runtime lifetime | saveしない。全Sand/StasisMud itemへload後fresh 5秒timerをexactly once再付与する。積載handoffはRuntimeNormalizeでground化するため直後から進み、`StoredIn` / `DeliveringTo` / `StoredByMixer`等の保護relationが残るitemだけ既存lifetime systemで停止する |
| Rest/occupant/reservation関係 | Idle/Rest lifecycle | task cleanupへ混ぜず、Rest ownerのvalidation/normalizationで保持または解除する |

各source removalはRelationshipTargetを直接編集せずowner helper/Relationship hookを使う。runner-owned flush後に
target collection、request Pending、wheelbarrow再利用可能性を検証する。

### 4.5 正本/派生台帳

`docs/save_load.md` に少なくとも次の列を持つdomain ledgerを置く。

| Domain | durable source | replace reset | compatibility normalization | runtime-derived rebuild | presentation | wake timing |
| --- | --- | --- | --- | --- | --- | --- |
| Familiar | `FamiliarOperation` / `FamiliarPolicy` / `TaskArea` | AI/runtime shell | missing値補完 | AI/runtime state | proxy/range shell | rehydrate内 |
| Stockpile | `Stockpile` / `StockpilePolicy` / relationship | request/group/cache | old cell policy補完 | group/cache | item/zone visual | 次Perceive |
| Construction | site/tile/Blueprint/Building | runtime shell/index/obstacle | phase/counter正規化 | index/obstacle | blueprint/site mirror | rehydrate内 |
| Soul Energy | site/policy/durable relationship | runtime grid/allocation | slot/policy正規化 | grid/allocation summary | Lamp/Spa inspection source | 次Logicのfull transaction |
| Room | Wall/Door/Floor/WorldMap | old Room/root overlay/lookup | なし | Room/lookup | boundary/overlay | 次Logicのroom detection |
| Task/Logistics | Designation/TransportRequest、durable owner/storage relation、積載staging handoff | assignment/claim/lease/timer | legacy runtime edge strip、cargo safe unload | request Pending、parking、item lifetime | item/tool shell | rehydrate内〜次Perceive |

実装時は全保存root markerを機械的に列挙する型一覧ではなく、owner、順序、silent failure条件を記述する。

### 4.6 設計判断

| ID | 判断 |
| --- | --- |
| C3-D01 | C3は既存物理分割を捨てず、domain登録・順序・coverageを完成させるrefactorとする |
| C3-D02 | reset registryとrehydrate registryは寿命と責務が異なるため統合しない |
| C3-D03 | live replace前に判定可能なdomain validationをpreflightへ置き、rehydrate mutation stepはinfallibleに限定する |
| C3-D04 | phase barrierと`clear_trackers`はhookへ委譲せずroot runner/coordinatorが所有する |
| C3-D05 | normal loadとrollbackは同じregistry snapshotとrunnerを通す |
| C3-D06 | independent stepの安定順は名前で固定し、plugin登録順へ依存しない |
| C3-D07 | C3はsave container versionを変更せず、v0/v1 fixtureの意味を維持する |
| C3-D08 | candidate validationはreplace前のstaging runner、normal/rollback同一traceはmutation runnerだけの保証とする |
| C3-D09 | raw registryはschedule開始前にfreezeし、normal/rollbackへ同じimmutable planを渡す |
| C3-D10 | PreparedLoad normalization、DurableNormalize、RebuildDerivedを混在させない |
| C3-D11 | registry freezeは全plugin build後の`SavePlugin::finish`で行い、load schedule中にはgraphを解決しない |
| C3-D12 | recovery-only replaceは`RecoveryFailed`専用のfail-closed APIとし、通常ロードのrollback staging保証から分離する |
| C3-D13 | runtime task edgeと`ItemDespawnTimer`はwriterから除外し、legacy stripと型別runtime再構築で復元する |
| C3-D14 | `LoadedIn` / `LoadedItems`はcarrier位置をremapするstaging handoffとしてだけ保存し、destination/claimを復元しないloadではRuntimeNormalizeで安全に荷下ろししてから除去する |
| C3-D15 | `WorldMap`のshapeと全persisted Entity参照はcandidate自身をpreflightし、旧live `WorldMap`をrehydrate prerequisiteにしない |
| C3-D16 | `WorldMap.tile_entities`の全slotと、Blueprint / Wall / Soul Spa footprintの双方向一致をcandidateで要求し、欠落anchorやowner漂流をlive replace前に拒否する |
| C3-D17 | constructionのphase昇格は境界tileの`WaitingMud`化とtask trio除去を同じDurableNormalize stepで行う。仮壁未生成の正当なWall境界だけはcounterを復元して通常Logic chainへ渡す |
| C3-D18 | Soul Spaのsimulation ownershipは`SoulSpaTile.parent_site`を正本とし、絶対Transformへ二重offsetを生む`ChildOf`再付与は行わない |
| C3-D19 | 通行可能な完成建物も`WorldMap.buildings` ownerを保持し、Blueprint完成時はraw placement reservationだけを解除して完成entityへ同frame移譲する |

- Bevy 0.19 APIでの注意点:
  - `World::flush()`、Relationship hook、`clear_trackers()`の可視化順は既存テストとBevy 0.19一次情報を基準にする。
  - `App` plugin build中のResource登録とexclusive runnerのborrow解放は、現行 `LoadResetRegistry` の実装パターンを再利用する。
  - registryのfreezeは`Plugin::build`内で早期実行せず、Bevy 0.19 の`SavePlugin::finish`で行う。

## 5. マイルストーン

## M1: 現行step inventoryとpreflight境界の固定

- 変更内容:
  - `rehydrate_after_load` の全処理をphase、owner、入力、出力、fallibility、flush要否へ分類する。
  - PreparedLoad strip、durable normalize、runtime normalize、derived rebuildを別台帳に分類する。
  - `AssignedTask`非保存に伴うtask execution state ledgerを全persisted Relationship/Inventory/carrier/requestへ作る。
  - current schema writerからruntime task edge/claim/lease/timerを除外し、legacy payloadの対称stripをfixtureで固定する。
  - Sand/StasisMudの`ItemDespawnTimer`をruntime-derived stateとして棚卸しし、ground/carrier/mixer別の再付与契約を固定する。
  - 現行staging preflightをtyped candidate validation contextへ拡張し、Familiarのroster invariant等をapply前へ抽出する。
  - rollback snapshotもlive reset前に同じcandidate validatorへ通す。
  - `LoadResetRegistry`対象と非persisted runtime root entityを棚卸しし、旧Room/root overlayを含むreset coverageを固定する。
  - 現行手書き順のtrace testを先に追加し、移行前baselineを固定する。
- 主な変更ファイル:
  - `crates/bevy_app/src/systems/save/{load.rs,rehydrate.rs,transaction.rs}`
  - `crates/bevy_app/src/systems/save/rehydrate/tests/`
  - `docs/save_load.md`
- 完了条件:
  - [x] 全既存rehydrate処理がいずれか1phaseへ分類され、未分類がない。
  - [x] candidate invariant失敗でlive worldが完全に不変である。
  - [x] staging Entity IDをlive側へ保持せず、validatorはdiagnostic以外を返さない。
  - [x] incoming/rollback candidate検査でbody再deserializeやvalidatorごとのWorld cloneを行わない。
  - [x] §4.4の全stateが型別契約どおり処理され、`ManagedBy`/`ManagedTasks`は保持される。
  - [x] current save bodyにruntime task edge/claim/lease/timerが含まれず、legacy fixtureではsource/targetが対でstripされる。
  - [x] 全Sand/StasisMud itemがload直後にtimerをexactly one個持ち、保護relation中は停止しground化後に進行する。
  - [x] paused load直後もstale worker/delivery claim/pushed toolが0件で、requestはPending、wheelbarrowは再利用可能である。
  - [x] 既存非persistedRoom/root visual/cacheのreset ownerと削除方法が未分類0件である。
  - [x] 現行v0/v1とB1〜B3 fixtureのbaselineが通る。
- 検証:
  - `cargo test -p bevy_app@0.1.0 --lib systems::save`

## M2: Phase-aware registryとrunner

- 変更内容:
  - validation hook / rehydrate step / phase / dependency validator / deterministic sorterを追加する。
  - 全pluginの`build`終了後、`SavePlugin::finish`でraw graphを検証/freezeし、immutable `ResolvedRehydratePlan`を作る。
  - runner-owned barrierを実装し、hook内flushを禁止する契約テストを置く。
  - production plugin compositionからroot/leaf adapter stepを一意登録する。
- 主な変更ファイル:
  - `crates/bevy_app/src/systems/save/rehydrate/registry.rs`
  - `crates/bevy_app/src/systems/save/{mod.rs,rehydrate.rs}`
  - `crates/bevy_app/src/plugins/`
- 完了条件:
  - [x] 重複名、未知依存、循環、phase逆行を全てrejectする。
  - [x] production App構築時にregistry不正を検出し、load前に解決済みplanが存在する。
  - [x] `SavePlugin::build`より後に登録されたdomain stepも`finish`時のproduction snapshotへ含まれる。
  - [x] 登録順を変えても同じstep順になる。
  - [x] production registryの必須step集合がexact snapshot testで固定される。
- 検証:
  - `cargo test -p bevy_app@0.1.0 --lib systems::save::rehydrate`

## M3: 全domain移行とnormal/rollback共通化

- 変更内容:
  - policy補完、runtime task整合、shell、construction、obstacle、domain wakeをregistry stepへ移す。
  - construction runtimeを`DurableNormalize`のphase/counter、`AttachShells`のmirror、`RebuildDerived`のindex/obstacleへ分割する。
  - success/rollback両branchへ同じimmutable planを渡し、branch固有の再構築を除去する。
  - Track C2向けに、`RecoveryFailed`からだけ使用可能なrecovery-only replace modeを追加し、incoming full preflight、
    idempotent reset、同一mutation runner、失敗時fail-closed維持をtransaction coordinatorへ実装する。
  - coordinator-owned `SaveRecoveryMode`を追加し、rollback失敗→Failed、recovery-only成功→Healthy以外の遷移を禁止する。
  - reset ledgerで不足したRoom/runtime root cleanupをowner hookへ追加し、paused loadでも旧world表示を残さない。
  - step追加APIとroot facade責務をREADME/architectureへ反映する。
- 主な変更ファイル:
  - `crates/bevy_app/src/systems/save/rehydrate/`
  - `crates/bevy_app/src/systems/save/{load.rs,reset.rs,transaction.rs}`
  - `docs/{save_load.md,architecture.md,invariants.md,cargo_workspace.md}`
- 完了条件:
  - [x] normal loadとrollbackのmutation traceが同じで、各step exactly onceである。
  - [x] recovery-only modeは通常状態からrejectされ、`RecoveryFailed`では不正なlive rollback snapshotに依存せずvalid slotを再適用できる。
  - [x] recovery-only applyが再度失敗してもpaused fail-closedのままで、別slotへの再試行時にreset二重実行由来の残骸がない。
  - [x] RecoveryFailed遷移でvirtual timeがpauseし、成功時にも自動unpauseせずUIの明示resumeを待つ。
  - [x] stale Entityを持つcache、relationship、visual proxyが残らない。
  - [x] construction shellはDurableNormalize後のphase/counterだけから生成され、paused loadでも一致する。
  - [x] normal/rollback/paused loadの全経路で旧Room/runtime root entityが0件になる。
  - [x] energy/room/spatialのwakeが必要な最初のframeを逃さない。
  - [x] `docs/plans/hvac-plumbing-plan-2026-07-13.md`のM3がreset、root marker、phase別step、
    `SavePlugin::finish` coverage、normal/rollback/recovery-only回帰まで実際に同期されている。
- 検証:
  - `cargo test -p bevy_app@0.1.0 --lib systems::save::load`
  - `cargo test -p bevy_app@0.1.0 --lib systems::save::rehydrate`

## M4: 回帰・文書・完了判定

- 変更内容:
  - v0/v1、current、corrupt、preflight failure、live apply fault、rollback、recovery-only retry、paused loadを横断確認する。
  - candidate不正、registry構成不正、旧runtime root、task Relationship、construction shell順序をfault fixtureで確認する。
  - `hell-workers-review-help-impact`を実際のplayer-visible経路から実施する。挙動不変なら理由付きNo impact、
    表示/操作が変わる場合はHelp catalog/provider/coverage/snapshotを更新する。
  - 恒久docsと計画状態を同期し、完了後にarchiveする。
- 完了条件:
  - [x] 既存durable player valueは不変で、player-visible差分は§4.4とC3-D16〜D19に記録したtask/logistics、construction、WorldMap/Soul Spa occupancyの正常化に限定される。
  - [x] candidate/registry不正はlive reset前に拒否され、WorldEpoch/UI/selectionが不変である。
  - [x] paused loadとrollback直後に旧runtime entity/visual/task Relationshipが残らない。
  - [x] recovery-only成功後だけ`SaveRecoveryMode::Healthy`へ戻り、失敗中はcoordinatorがsave/通常loadをrejectする。
  - [x] workspace full gateが成功する。
  - [x] 計画がarchiveされ、索引が最新である。
- 検証:
  - `python3 scripts/dev.py verify`
  - `python3 scripts/check_help_impact.py`
  - `git diff --check`

## 6. リスクと対策

| リスク | 影響 | 対策 |
| --- | --- | --- |
| registry化で暗黙順が変わる | load直後だけstate/shellが欠ける | 移行前trace、明示dependency、phase barrier testを先行 |
| pluginがstep登録を忘れる | 新型がsaveから戻らない | production exact coverageとdomain ledgerを同時更新 |
| validation hookがworldを変更する | preflight不変保証が壊れる | immutable contextだけを渡し、mutation APIを型で与えない |
| staging Entityをrehydrateへ渡す | live Entity remap後に別対象を参照 | validatorはdiagnosticだけを返しEntity/QueryState保持を禁止 |
| fallible処理がmutation phaseへ残る | rollback依存が増える | M1 inventoryで全Result経路を分類し、validationへ移す |
| registry graphをload中に解決 | reset後にcycle/duplicateを検出 | plugin finalize時にfreezeしimmutable planだけをtransactionへ渡す |
| `build`中にregistryをfreezeして後続pluginを落とす | production stepがsilent未登録になる | 全plugin build後の`SavePlugin::finish`を唯一のfreeze境界にする |
| construction normalizeがshellより後 | paused loadで古いphase表示 | counter/phaseをDurableNormalize、mirrorをAttachShellsへ分離 |
| runtime rootをResource resetだけで済ませる | old Room/grid/overlay entityが残る | reset ledgerとowner hookでentity/relationship/cacheをreplaceごとに破棄 |
| registryが新たな巨大中央層になる | leaf ownershipが失われる | rootは順序と型消去だけを持ち、domain logicはowner側に保持 |
| deferred commandの可視化点がずれる | derived rebuildがshellを見落とす | flushをrunnerの固定barrierに限定しBevy 0.19回帰を置く |
| 期限付きitemのtimerがsave対象外のまま戻らない | Sand/StasisMudがload後に永久残存する | 型別runtime normalizeでfresh timerをexactly once再付与しrelation別停止を検証 |
| RecoveryFailedでも通常transactionを要求する | 壊れたlive worldをrollback候補化できず再ロード不能 | 専用modeだけrollback候補を省略し、incoming full preflightとfail-closed維持を必須化 |

## 7. 検証計画

- 必須:
  - registry pure tests（重複、未知依存、循環、安定順、phase逆行）。
  - production `SavePlugin::finish`時のregistry reject/late registration coverageとnormal load / rollback mutation同一trace。
  - v0/v1、B1〜B3、construction、obstacleのround-trip。
  - preflight failure前後のlive world structural equality。
  - paused loadを含むstale runtime root/task Relationship、二重shell、余分なdomain wakeが0件。
  - Sand/StasisMud timerのground/carrier/mixer round-tripと、RecoveryFailedからのvalid/invalid slot再試行。
- 計画完了時:
  - `cargo fmt --all -- --check`
  - `cargo check --workspace`
  - `cargo clippy --workspace --all-targets -- -D warnings`
  - `cargo test --workspace`
  - `python3 scripts/dev.py verify`
- 実機確認:
  - F5/F9、Pause中load、失敗load後の現world維持を、実装時のno-prompt native acceptance手順で短いsmokeとして確認する。
  - C3単独ではrenderer/GPU/native allocatorの新しい性能baselineを要求しない。
- パフォーマンス確認:
  - steady stateにはregistryを走らせない。load 1回あたりのstep数と各step実行回数をtest traceで固定する。
  - productionでstep traceやstaging snapshotを保持し続けず、preflight終了後に一時Worldを破棄する。

## 8. ロールバック方針

- M1のvalidation/schema抽出、M2のregistry導入、M3のdomain移行/recovery-only seamを別commitにする。
- M2で問題が出た場合は手書きrunnerを維持したままregistry型/testsだけを戻せるよう、切替を一度に行わない。
- save format/schemaはC3で変更しないため、rollbackでsave file migrationは不要。
- 診断traceやfault injectionはtest/profiling境界に限定し、production hot pathへ残さない。

## 9. AI引継ぎメモ（最重要）

### 現在地

- 進捗: `100%`
- 完了済みマイルストーン: M1〜M4
- 未着手/進行中: なし（archive済み）

### 次のAIが最初にやること

1. Track C1 `docs/plans/building-deconstruction-plan-2026-08-03.md` のM1へ進む。
2. C1のdurable orderはC3のcandidate validator / production registry coverageへ同時登録する。
3. Track C2はC1完了後に、C3の`SaveRecoveryMode`とrecovery-only replaceを利用して着手する。

### ブロッカー/注意点

- C3の未解決ブロッカーはない。既存`LoadResetRegistry`は維持し、追加domainはphase-aware registryへ登録する。
- `World::clear_trackers()`、recovery-only mode、runtime-derived energy stateのowner境界を後続Trackでも弱めない。
- HVAC M3はC3の完成したregistryへConduit/FluidGrid契約を追加し、手書きrehydrateを並設しない。
- native acceptanceは `/tmp/hell-workers-c3-native-acceptance-20260803-j` で合格。失敗run `h` / `i` は診断用に保持する。

### 参照必須ファイル

- `docs/save_load.md`
- `docs/architecture.md`
- `docs/invariants.md`
- `docs/plans/archive/save-load-hardening-plan-2026-07-12.md`
- `docs/plans/hvac-plumbing-plan-2026-07-13.md`
- `crates/bevy_app/src/systems/save/{load.rs,rehydrate.rs,reset.rs,transaction.rs}`
- `crates/bevy_app/src/systems/save/rehydrate/`

### 最終確認ログ

- 最終 `cargo check --workspace`: `2026-08-04 / pass`
- 最終 `cargo clippy --workspace --all-targets -- -D warnings`: `2026-08-04 / pass（0 warnings）`
- 最終 `cargo test --workspace --no-fail-fast`: `2026-08-04 / pass`
- 最終 `python3 scripts/dev.py verify`: `2026-08-04 / pass`
- 最終 native acceptance: `2026-08-04 / pass（/tmp/hell-workers-c3-native-acceptance-20260803-j、Intel Arc / Vulkan / X11）`
- 未解決エラー: `N/A`

### Definition of Done

- [x] M1〜M4が完了し、normal/rollbackが同じregistry runnerを通る
- [x] runtime task edgeのwriter除外/legacy stripとSand/StasisMud timer再構築が型別fixtureで固定されている
- [x] RecoveryFailed専用replace modeがvalid retry成功/再失敗の両方でfail-closed契約を満たす
- [x] live replace前に判定可能な全domain validationがpreflightで完了する
- [x] domain ledgerとproduction step coverageが同期している
- [x] v0/v1とB1〜B3 fixtureが成功する
- [x] 影響ドキュメントとHelp impact判断が更新済み
- [x] `python3 scripts/dev.py verify`が成功

## 10. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| `2026-08-03` | `Codex` | 現行のLoadResetRegistry/物理分割を前提に、typed staging validation、App構築時freeze、phase別immutable mutation plan、task state ledger、normal/rollback共通化をC3実装契約として確定 |
| `2026-08-03` | `Codex` | 自己レビューでfreezeを`SavePlugin::finish`へ固定し、runtime task edgeのwriter除外、Sand/StasisMud lifetime再構築、`RecoveryFailed`専用replace modeを追加 |
| `2026-08-04` | `Codex` | M1〜M3を実装。candidate/live validation、phase-aware registry、normal/rollback/recovery-only共通runner、runtime task/cargo正規化、construction/obstacle/Soul Spa再構築を追加 |
| `2026-08-04` | `Codex` | C3-D16〜D19の全map anchor・双方向footprint、construction atomic normalize、Soul Spa parent正本、通行可能完成建物owner移譲を回帰へ固定 |
| `2026-08-04` | `Codex` | Help、恒久docs、workspace全ゲートを同期し、実X11/Vulkan受入で正常load・paused維持・破損load事前拒否・world意味不変を確認してarchive |
