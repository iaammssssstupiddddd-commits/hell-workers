# Track C1 一般建築物の解体・資源回収 実装計画

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `building-deconstruction-plan-2026-08-03` |
| ステータス | `Draft` |
| 作成日 | `2026-08-03` |
| 最終更新日 | `2026-08-04` |
| 作成者 | `Codex` |
| 関連提案 | `docs/proposals/gameplay-management-improvements-proposal-2026-07-17.md`（Track C1） |
| 前提計画 | `docs/plans/archive/save-rehydration-registry-plan-2026-08-03.md`（C3完了済み） |
| 関連Issue/PR | `N/A` |

## 1. 目的

- 解決したい課題: 完成建物を安全に撤去して配置ミスや区画変更から復旧するplayer workflowがない
  - 完成建物を撤去するplayer workflowがなく、配置ミスや区画変更を長期プレイ中に復旧できない。
  - 建物のowner構造は通常Building、Floor/Wall/Bridge、Tank companion、Wheelbarrow、Soul Spa、Power consumerで異なり、
    root entityの単純despawnでは要求、予約、WorldMap、Room/Power、visual、保管内容が残る。
  - 建設中cancelはowner別cleanupを持つ一方、完成後の共通lifecycleと回収量の正本がない。
- 到達したい状態:
  - Ordersから完成建物を1棟指定し、FamiliarがSoulへ `Deconstruct` を割り当て、作業完了後にroot ownerが
    関連状態を一つの終端としてcleanupする。
  - 建物内の資源・bucket・wheelbarrowを失わず安全な回収先へ戻し、建物種別別の固定回収表から資材をexactly once返す。
  - 解体指定はpriority、Familiar policy、A3 dashboard、save/load、A2通知の既存境界へ統合する。
- 成功指標:
  - 全既存 `BuildingType` とOperational Soul Spaについて、解体後の孤立task/request/reservation/relationship/entityが0件。
  - WorldMap building/door/bridge/stockpile、obstacle、Room、Powerがowner消失後の正規状態へ収束する。
  - 保存中の`DeconstructionOrder`はload後に再割当され、実行中runtime taskやcommit要求は保存されない。
  - 資材、storage内容、付属toolの消失・二重生成が0件。

## 2. スコープ

### 対象（In Scope）

- `WorkType::Deconstruct`、`AssignedTask::Deconstruct(DeconstructData)`、durable `DeconstructionOrder`、
  単体指定用 `TaskMode::DesignateDeconstruct(Option<Vec2>)`。
- 完成 `Building` 全種と `SoulSpaPhase::Operational` のSoul Spa root。
- Blueprint、Floor/Wall construction siteの既存cancel導線を維持し、Constructing Soul Spaだけ同等のcancel/refund導線を追加する。
- 1対象1worker、Familiar policy、task discovery、assignment、pathfinding、safe cancellation、A3 dashboard。
- owner-safeなWorldMap逆引き/解除、Door/Bridge cache、obstacle、Room、Power、visual cleanup。
- Tank/MudMixer/RestArea/WheelbarrowParking/Soul Spaの特殊cleanup matrix。
- 建築種別別の固定回収table、保管内容と付属toolの安全な回収先解決。
- typed preview/outcome、A2 notification、Help、恒久docs、save/load回帰、実機受入。

### 非対象（Out of Scope）

- 矩形/範囲一括解体、解体queue、解体priorityの専用一括editor。
- 解体速度や回収率への難易度、Dream Edict、Familiar rank、skillの影響。
- 建物の移設を解体+再建へ置き換えること。Tank/MudMixerの既存Moveは維持する。
- construction中Blueprint/Floor/WallをDeconstruct taskへ統合すること。これらは既存owner cancelを使う。
- HVAC `OssuaryConduit`。同計画のline eraseを唯一の撤去経路とし、一般Deconstructへ含めない。
- Yard、Site、Stockpile zone、Tree/Rock、ResourceItem等、完成建物ではないentity。
- 解体演出の最終アート。初版は作業progressと既存visual feedbackを再利用する。

## 3. 現状とギャップ

| 対象 | 現行構造/cleanup | C1で必要な対応 |
| --- | --- | --- |
| 通常Blueprint | owner cancelがworker/request/refund/WorldMapを処理 | 変更せず、完成BuildingだけDeconstruct対象 |
| Floor/Wall construction | site単位cancelとactual delivered material refund | 変更せず、完成tileは1棟ずつ解体 |
| 通常Building | `Building`はkindだけでfootprintを保持しない | `WorldMap`のowner逆引きを正本にcleanup plan作成 |
| Door | raw building占有とは別にdoor cache/stateを持つ | owner確認付きdoor解除とtopology dirty |
| Bridge | `bridged_tiles`がwalkabilityを変更する | 専用owner-safe remove APIを追加 |
| Tank | root Stockpile、companion Stockpile、bucket、water item | request停止、StoredIn解除、companion/toolを安全な回収先へ退避 |
| MudMixer | numeric sand/rock + `StoredByMixer` mud entity + water item + refine/haul request | task/request解除、sand/mudは別Mixerへ直接移送、rockはResourceItem化 |
| RestArea | occupants/reservations relationship | lifecycle helperから安全に退去/予約解除 |
| WheelbarrowParking | owned/parked wheelbarrowとloaded items | task解除後にwheelbarrowを安全なcellへunpark。loaded contentは積載状態を維持 |
| Soul Spa | root + 4 tile + GeneratePower worker + grid relation | site単位cleanup、worker解除、energy full dirty |
| 2D/3D visual | child layerと独立 `Building3dVisual { owner }` | childとowner proxyを同じ終端で除去 |

## 4. 実装方針（高レベル）

### 4.1 指定とdurable order entity

- Ordersに `Deconstruct` を追加する。新しい既定keyboard shortcutは追加せず、ボタンから
  `PlayMode::TaskDesignation + TaskMode::DesignateDeconstruct(Option<Vec2>)` へ入り、`Option<Vec2>`は既存area系modeと
  同じくpointer capture中の開始点を表す。1 gestureのcommit後は`DesignateDeconstruct(None)`へ戻し、
  右click/Escapeでは`TaskMode::None`へ抜ける。
- 初版はhoverでroot targetとtyped reject reasonを表示し、左clickで1棟だけ指定する。multi-tileの任意tileや
  Soul Spa tileを指してもcanonical rootへ解決する。右click/EscapeはA1の共通mode cleanupを通る。
- completed building rootへ`Designation`を直接付けない。MudMixer等が既存Refine/Haul Designationを所有するため、
  accepted targetごとに独立したsave root entityを1件spawnする。

```text
DeconstructionOrder（persisted root marker）
  + Designation { work_type: Deconstruct }
  + PlayerIssuedDesignation
  + Priority / TaskSlots { max: 1 }
  + TargetDeconstructionRoot(order -> canonical target Relationship)
  + Transform（指定時anchor。pending中はtarget移動を禁止）
```

- target側のRelationshipTargetとruntime `DeconstructionPending`をproducer/move gateの正本とする。
  `DeconstructionPending`はorder Relationshipからrehydrateし、order cancel/commit/root消失でowner helperから外す。
- save中にSoulが作業していても `AssignedTask` / path / commit message / commit claimは保存しない。
  load後はorder entityのDesignationから再割当する。
- active Move task、`MovePlanned`、移動先reservationがあるTank/MudMixerは指定時とcommit前の両方でrejectする。
  order成立後は新しいMoveとfacility assignmentを止め、target anchorを不変にする。
- active Blueprint/Floor/Wall siteは既存cancel actionへ案内する。Constructing Soul Spaはtaskを作らず、
  `Cancel Soul Spa Construction` outcomeからrequest/tile/siteをcleanupし、搬入済みBoneを100%返す。

### 4.2 WorkTypeとtask lifecycle

- `WorkType::Deconstruct` は `WorkType::ALL` の末尾へ追加し、既存stable index 0〜15を変更しない。
- `DeconstructData` はorder entity、target root、phaseを持つstruct payloadとし、`AssignedTask`にtuple variantで追加する。

```text
GoingToTarget
  -> Dismantling { progress }
  -> AwaitingCommit
```

- target中心がnon-walkableの場合は既存の隣接到達点解決を再利用する。UIやtask finderから追加A*を実行しない。
- `Dismantling` 完了時にSoul executorがtargetを直接despawnせず、
  `DeconstructionCommitRequest { world_epoch, worker, identity: ActiveTaskIdentity, order, target }`を1件発行して
  `AwaitingCommit`へ遷移する。同identityでは再発行せず、root finalizerだけがterminal結果を返す。
- root exclusive finalizerは同frameの全requestをdrainし、`(world_epoch, target)`の安定順でgroup化する。
  各targetの最初のvalid requestだけについて、deferred `Commands`より前に同期的なruntime
  `DeconstructionCommitClaim { world_epoch, order }`をtargetへ挿入する。重複batch、別order、stale replayは
  cleanup snapshotを作らずtyped duplicate/stale outcomeへ終端する。
- claim取得後にrequestを再validateし、cleanup planを全てsnapshotした後、失敗しないapply phaseで副作用を確定する。
  stale target/owner不整合は非変更failure outcomeとしてworkerを安全に終端する。
- root finalizerから使えるgeneric `hw_soul_ai` owner adapterを追加し、requestの`worker + ActiveTaskIdentity`がlive状態と
  完全一致する場合だけterminal cleanupする。winner成功はassignmentをcompleteして`OnTaskCompleted`をexactly once発行し、
  validation blockはretryable abort、duplicate/loser/cancel/staleは型別abortとする。order/targetを先にdespawnして
  `AssignedTask::Deconstruct`をguard任せに残すことを禁止する。
- order成立後は対象facilityの新規producer/assignmentを止める。既存in-flight taskは強制的なcomponent剥離ではなく、
  finalizerの共通 `unassign_task` / owner lifecycleを通して解放する。
- validationまたは回収先解決が失敗した場合はclaimを同期解除し、orderを残して再試行可能にする。
  同じfinalizer内でexact identity付きowner adapterへ通し、`WorkingOn`/`TaskWorkers`/slot/identityを解放して
  `AssignedTask::None`へ戻す。orderにはlatest-only runtime
  `DeconstructionBlocker { reason, stamp: TaskDiagnosticInputStamp, domains: TaskDiagnosticDomainMask }`を置く。
- blocker中は候補から除外し、既存`TaskDiagnosticInputRevisions::is_current`でstampがstaleになった時だけlive validationを
  再実行する。availabilityにはStockpile policy/capacity、storage relationship、Mixer storage、tool状態、topologyには
  WorldMap obstacle revision、task-localにはorder/target/MovePlanned lifecycleを含め、各変更producerが対応revisionをbumpする。
  単一の架空global revision、固定tickの即時再割当、毎frame request再発行を禁止する。
  成功時はtargetとorderを同じ終端で消し、claim/blockerを残さない。
- dashboard cancelはclaim取得前だけ許可する。cancel時は該当orderを参照する全workerをexact identity付きowner adapterで
  先にterminal abortし、その後orderをdespawnする。claim後はexactly-once終端を優先し、stale cancel outcomeを返す。

### 4.3 Cleanup planとowner-safe apply

- `Building`にfootprintを後付けで推測せず、`WorldMap`の全layerをentity ownerで逆引きするpure snapshot APIを追加する。
- `DeconstructionCleanupPlan` はapply前に少なくとも次を確定する。
  - order / claim / canonical root / kind / anchor / owned grid集合
  - child、companion、independent visual proxy、owned item/tool
  - target/owned entityをpayload、anchor、issued_by、relationshipで参照するtask/request
  - WorldMap building/door/bridge/stockpileのowner一致箇所
  - 回収yield、storage内容、各資源の確定回収先
- applyは次の順序をroot-owned named setで行う。

```text
stop new work/request
  -> unassign related Soul/Familiar work and release reservations
  -> detach/transfer stored items, tools, occupants
  -> close TransportRequest and owner relationships
  -> owner-check WorldMap layers and runtime indexes
  -> despawn child/companion/proxy/root
  -> materialize recovery items at prevalidated destinations exactly once
  -> flush
  -> obstacle / wall / room / energy dirty propagation
```

- `WorldMap`のslotは現在ownerがtarget/owned entityである場合だけ解除する。別ownerへ置換済みのcellを消さない。
- Relationship Sourceをowner helperから外し、Target側collectionを直接編集しない。
- independent `Building3dVisual { owner }` は既存cleanup observer/systemを維持しつつ、same-transaction testで残存0を固定する。
- wall neighbor visual、Room detection、Power topologyは各ownerのdirty/reconcilerへ通知し、C1独自の再計算を複製しない。
- `LoadResetRegistry`でcommit request/claim/hover/dashboard runtime targetをclearする。durable order RelationshipはC3の
  validationと`RuntimeNormalize`でtarget存在・一意性を検査し、`DeconstructionPending`を再生成する。

### 4.4 Storage/occupant保全と回収先

- `StoredIn` / `LoadedIn` / `ParkedAt` / `BelongsTo`、関連Soulの`Inventory`でtargetまたはowned companion/taskに属する
  ResourceItem/toolを列挙し、reservation解除後に`RecoveryPlacementPlan`で事前確定した回収先へ移す。
- 通常資源とtoolは、撤去後もpassableでtarget footprint外、別owner非占有のgridをManhattan距離→Y→Xの
  安定順で選び、visibleなground itemへ戻す。Bridge/river上など安全なdrop cellが無ければcommit前に拒否する。
- `ResourceType::can_store_in_stockpile()`がSand/Bone/StasisMudを拒否する現行契約を維持し、通常Stockpileを
  Sand/StasisMudの退避先として扱わない。Boneは期限なしground itemとして通常dropする。
- I-L2によりgroundで5秒後に消滅するSand/StasisMudは地面へ落とさない。validなWheelbarrowの`LoadedIn`は
  積載維持し、それ以外はtarget/owned companion以外で、
  operational、Move/Deconstruct pendingでない別MudMixerを距離→Y→X→Entityの安定順で選び、snapshot時の
  reservation shadowで`MUD_MIXER_CAPACITY` / `MUD_MIXER_MUD_CAPACITY`を全量確保する。確保不能なら
  `NoSafeRecoveryMixer(resource, amount)`としてworld非変更で拒否する。
- `MudMixerStorage.sand` はitem化せず、確保済み別Mixerのnumeric `sand`へexact countを直接移す。`rock`だけは
  count分のRock itemへ変換して通常dropする。一方`mud`は
  `StoredByMixer(mixer)`を持つ既存StasisMud item数のmirrorなので新規spawnしない。snapshot時に
  `storage.mud == stored_mud_entities.len()`を検証し、不一致なら`InconsistentMixerInventory`でworld非変更拒否する。
  一致時は既存itemの`StoredByMixer`をowner helperで確保済み別Mixerへ付け替え、receiverのnumeric `mud`を同数増やす。
- 関連Soulが単体で所持するSand itemはreceiverのnumeric `sand`へ1として吸収してitemをexactly once despawnし、
  StasisMud itemは`StoredByMixer`をreceiverへ付けてnumeric `mud`を1増やす。表現変換前後の合計量をassertする。
- Tank内Water item、bucket storage、parking内wheelbarrowはentityを維持する。使用中のtaskをunassignしてから
  storage/parking relationshipだけを外す。Wheelbarrowはsafe cellへunparkするが、validな`LoadedIn`内容は
  Sand/StasisMudを含め積載状態のまま維持し、解体都合でgroundへ降ろさない。
- 上記の積載維持は **同一live world内のC1解体commit** の契約である。F5/F9を跨ぐ場合、Track C3は搬送先/claimを
  復元しないため`LoadedIn` / `LoadedItems`をcarrier位置のstaging handoffとして検証した後、carrier（運搬中なら
  carrying Soul）近傍へ安全に荷下ろしする。C1はload後も積載Relationshipが残ることを前提にせず、ground itemと
  再生成されたrequestから通常どおり再割当する。
- RestArea occupants/reservationsはrest-area owner helperで解除し、Soulを通常idle/rest判断へ戻す。
- player資源の退避とbuilding salvageを別集計にし、salvage率をstorage内容へ適用しない。

### 4.5 初版回収table

回収は建築別固定値とし、difficulty/Edictを参照しない。`required_materials()`や完成時に失われた
Bridgeの実投入mixを逆算しない。

| BuildingType | 回収資材 |
| --- | --- |
| `Wall` | Wood × 1 |
| `Door` | Wood × 1 |
| `Floor` | Bone × 1 |
| `Tank` | Wood × 1 |
| `MudMixer` | Wood × 2 |
| `RestArea` | Wood × 2 |
| `Bridge` | Rock × 3 |
| `SandPile` | なし（0。無限供給源のため） |
| `BonePile` | Bone × 5 |
| `WheelbarrowParking` | Wood × 1 |
| `SoulSpa` | Bone × 6 |
| `OutdoorLamp` | Bone × 1 |

- tableは`hw_jobs`所有のpure APIを唯一の正本とし、UIは返却予定の表示値だけを読む。
- BridgeのRock×3は投入履歴のrefundではなく、素材を問わず冥府の瓦礫へ正規化する意図的なsalvage ruleとする。
  Wood投入からRockへ変わり得ることをpreview/Helpへ明示する。
- SandPileは無限供給源であり、Sandをsalvageすると配置→解体だけで資源生成できるため0を明示する。
  preview/Helpでも「回収なし」と表示し、無言fallbackにはしない。
- construction cancelはこのtableを使わず、各siteが保持するactual delivered amountを100%返す。
- yieldが0の将来種別も明示entryを要求し、wildcard fallbackで無言0回収にしない。

### 4.6 UI、診断、Help

- hover previewは `Can deconstruct` またはtyped reason（construction中、移動中、既に指定済み、
  safe recovery先不足、Mixer在庫不整合、stale、unsupported、owner不整合）を表示する。
- 指定/cancel/commitはtyped outcomeをlogical requestごとに1件返す。dedupeで落とした同batch重複も
  duplicate outcomeだけを返し、root adapterがA2のbounded notificationへ変換する。
- A3 dashboardは `Deconstruct <building>`、priority、assigned count、blocker、focus、positive allow-list cancelを提供する。
- Familiar Operation dialogへDeconstruct行を追加し、default policyは既存 `default_rule`を継承する。
- HelpはOrders、task dashboard、建設/解体、回収、save中の再割当を説明する。実装後に
  `hell-workers-review-help-impact`でprovider/manifest/coverage/exact snapshotを更新する。

### 4.7 設計判断

| ID | 判断 |
| --- | --- |
| C1-D01 | 初版は単体指定。矩形解体は別計画 |
| C1-D02 | completed BuildingとOperational Soul SpaだけDeconstruct task対象 |
| C1-D03 | construction中はowner cancel。Constructing Soul Spaにも専用cancelを追加 |
| C1-D04 | durable orderは専用save root entity。完成building rootの既存Designationを上書きしない |
| C1-D05 | `WorkType::Deconstruct`は末尾appendし既存stable indexを維持 |
| C1-D06 | cleanup footprintはTransform推測でなくWorldMap owner逆引きを使う |
| C1-D07 | storage内容/toolは100%回収し、建物salvageと分離。Sand/StasisMudは地面へ落とさず別MudMixerへ移す |
| C1-D08 | 回収は建築別固定table。difficulty/Edict/投入履歴は初版対象外 |
| C1-D09 | Soul executorはdespawnせず、root finalizerだけがcross-domain cleanupをcommit |
| C1-D10 | Power/Room/wall/pathはowner dirty/reconcilerを再利用し独自再計算しない |
| C1-D11 | 安全なdrop cellまたはvolatile資源の別Mixer容量を全量確保できなければcommit前に拒否する |
| C1-D12 | Mixer mudは既存StoredByMixer entityを正本に移送し、numeric countとの差分を新規spawnで補わない |
| C1-D13 | target単位の同期commit claimをroot exclusive finalizerで取得し、同batch重複/cancel/replayを排他する |
| C1-D14 | DeconstructionOrder/Relationship/WorkType追加は既存v1 additive方針を使う。old executableのforward loadは保証しない |
| C1-D15 | Bridgeは投入素材に関係なくRock×3へ正規化する明示的なsalvage ruleとする |
| C1-D16 | active Move/MovePlanned/reservation中はrejectし、pending成立後の新規Moveをgateする |
| C1-D17 | commit失敗はSoulを完全unassignし、orderをrevision-gated Blockedへ戻して再割当loopを防ぐ |
| C1-D18 | commit requestはworkerとexact `ActiveTaskIdentity`を含み、root finalizerがwinner/loser/cancelをowner-safeに終端する |
| C1-D19 | blockerは既存`TaskDiagnosticInputStamp` + domain maskでinvalidateし、架空の単一global revisionを作らない |
| C1-D20 | SandPile salvageは0。MixerのSandはnumeric transfer、StasisMudは既存entity transfer、wheelbarrow積載物は積載維持 |
| C1-D21 | C1の積載維持はlive解体境界だけに適用し、C3 load後は積載handoffが安全に荷下ろしされるため`LoadedIn`存続を前提にしない |

- Bevy 0.19 APIでの注意点:
  - Entity despawn時の`ChildOf`伝播、Relationship hook、`RemovedComponents`可視化はBevy 0.19一次情報と回帰で確認する。
  - `Commands`でのunassign/despawn/spawnとWorldMap更新の可視化点はnamed `ApplyDeferred`で固定する。

## 5. マイルストーン

## M1: Domain model、durable order、target resolver、回収table

- 着手条件: Track C3が完了し、production `RehydrateRegistry` / schema coverage / runtime task edge契約が利用可能である。
- 変更内容:
  - `WorkType`末尾variant、task payload/phase、`DeconstructionOrder` root/Relationship、pending/claim runtime型を追加する。
  - target classification、Move競合を含むeligibility/reject reason、recovery tableを追加する。
  - WorkType ALL/index、Reflect、Familiar policy、Help coverage、profiling audit encodingのexhaustive matchを更新する。
  - Building/SoulSpa tileからcanonical rootを返すpure resolverとWorldMap owner snapshot APIを追加する。
  - C3 registryへorder validation/pending rebuildを登録し、save schema root/componentとv0/v1 fixtureを追加する。
- 主な変更ファイル:
  - `crates/hw_core/src/{jobs.rs,game_state.rs}`
  - `crates/hw_jobs/src/{tasks/mod.rs,tasks/deconstruct.rs,deconstruction.rs}`
  - `crates/hw_world/src/map/`
  - `crates/bevy_app/src/systems/save/{schema.rs,rehydrate/}`
  - `crates/bevy_app/src/plugins/startup/perf_scenario/audit_encoding.rs`
- 完了条件:
  - [ ] 既存WorkType index 0〜15が不変でDeconstructだけ末尾になる。
  - [ ] building rootの既存Designationを変更せず、targetごとにorder rootが最大1件だけ存在する。
  - [ ] 全既存BuildingTypeのeligibilityとyieldにexhaustive testがある。
  - [ ] SandPileは明示0、BridgeはRock×3で、preview/Helpとpure recovery tableが一致する。
  - [ ] multi-tile/SoulSpa tileの任意hitが同じrootを返す。
  - [ ] owner逆引きが別ownerのWorldMap entryを含めない。
  - [ ] new executableが旧v0/v1を読み、Deconstruct orderを含むnew v1はold executableへのforward compatibilityを保証しない契約がfixture/docsに固定される。
- 検証:
  - `cargo test -p hw_core work_type`
  - `cargo test -p hw_jobs deconstruct`
  - `cargo test -p hw_world building`

## M2: Headless task vertical slice、commit claim、ordinary cleanup

- 変更内容:
  - order entityのDesignationをFamiliar filter/policy/validator/builderとSoul dispatcher/executorへ接続する。
  - targetがpendingになった時点でfacility producerの新規request/assignmentを止める。
  - worker/exact identity/world-epoch付きrequest、target単位の同期commit claim、validate/snapshot/apply finalizerを実装する。
  - `hw_soul_ai`へexact `ActiveTaskIdentity`一致時だけcomplete/retryable-abort/cancel-abortするroot向けowner adapterを追加する。
  - blockerを`TaskDiagnosticInputStamp` + domain maskへ統合し、StockpilePolicy/storage/Mixer/WorldMap/Move producerの
    revision bump coverageを追加する。
  - ordinary completed buildingのtask/request/reservation/WorldMap/child/3D proxy cleanupと固定salvageを接続する。
- 主な変更ファイル:
  - `crates/hw_familiar_ai/src/familiar_ai/decide/task_management/`
  - `crates/hw_familiar_ai/src/familiar_ai/decide/task_management/diagnostics.rs`
  - `crates/hw_soul_ai/src/soul_ai/{helpers/work.rs,execute/task_execution/}`
  - `crates/hw_jobs/src/diagnostics.rs`
  - `crates/bevy_app/src/systems/jobs/deconstruction/`
  - `crates/bevy_app/src/plugins/logic.rs`
  - `crates/hw_world/src/map/`
- 完了条件:
  - [ ] order→assign→execute→claim→ordinary cleanupのheadless固定tick vertical sliceが成立する。
  - [ ] Familiar policyの禁止/priorityが候補評価へ一致する。
  - [ ] path到達不能は追加A*なしで既存diagnosticへ集約される。
  - [ ] executorはtargetを直接despawnせずworld-epoch付きcommit requestを発行する。
  - [ ] `AwaitingCommit`中は同identityのrequestを再発行せず、全terminal pathでworkerがexactly once終端する。
  - [ ] 同batch重複、2 order競合、cancel競合、stale replayでcleanup/salvage/success outcomeが各target1回だけになる。
  - [ ] stale/owner mismatch/Move競合ではtarget/orderを破壊せずtyped failureになる。
  - [ ] winnerだけが`OnTaskCompleted`を1回発行し、loser/cancel/staleでは発行せずassignment/identityが残らない。
  - [ ] commit failureでSoul/TaskWorkers/claimが解放され、blocker domain stampがcurrentな間は再assign/requestされない。
  - [ ] 関係するavailability/topology/task-local producerのrevision変化だけでblockerが再評価され、無関係domainでは起床しない。
- 検証:
  - `cargo test -p hw_familiar_ai deconstruct`
  - `cargo test -p hw_soul_ai deconstruct`
  - `cargo test -p bevy_app@0.1.0 deconstruct`

## M3: Orders UI、dashboard、特殊storage保全

- 変更内容:
  - Orders button、TaskMode、hover preview、single-click intent/outcome、capture/cleanupを実装する。
  - A3 dashboardとFamiliar Operation policyへDeconstructを追加する。
  - Tank/Mixer/Rest/Parkingのstored content、tool、occupant lifecycle helperと`RecoveryPlacementPlan`を追加する。
- 主な変更ファイル:
  - `crates/bevy_app/src/systems/jobs/deconstruction/`
  - `crates/hw_ui/src/{intents.rs,setup/submenus.rs,panels/task_list/}`
  - `crates/bevy_app/src/{input_actions/,interface/selection/,interface/ui/,systems/command/}`
  - `crates/hw_logistics/src/`
  - `crates/bevy_app/src/interface/ui/panels/task_list/`
- 完了条件:
  - [ ] click1回でcanonical targetへorder entityを1件だけ作り、building rootの既存Designationを維持する。
  - [ ] overlay/pause中にworld clickやcameraへ入力が漏れない。
  - [ ] Familiar policy、dashboard reason、priority/cancel capabilityがlive評価と一致する。
  - [ ] plant/temporary建物の全cleanup matrixが固定tick testで合格する。
  - [ ] storage内容、bucket、wheelbarrowが消失せず事前確定した安全な回収先へ戻る。
  - [ ] Sand/StasisMudは通常Stockpileへ入らず、別Mixerへ全量予約・移送され、ground lifetime対象にならない。
  - [ ] Mixer sandはnumeric→numeric、mudは既存entity relationship+numeric mirrorで移り、rockだけがground item化する。
  - [ ] safe cellへunparkしたwheelbarrowの`LoadedIn`内容はSand/StasisMudを含め維持される。
  - [ ] F5/F9を跨いだ場合は積載item数をload直後に失わずcarrier近傍へground化し、C1が古い`LoadedIn`を要求せず再割当できる。
  - [ ] Mixer mud countとStoredByMixer entityが不一致なら非変更rejectし、二重spawnしない。
  - [ ] Bridge/river上を含め安全な回収先が無い場合はtargetを残してtyped failureになる。
  - [ ] recovery itemは成功commitで1回だけ生成される。
  - [ ] stale/owner mismatchではworldを変更せずfailure outcomeを返す。
- 検証:
  - `cargo test -p bevy_app@0.1.0 deconstruction`
  - `cargo test -p hw_logistics deconstruction`

## M4: Structure、Soul Spa、Room/Power統合

- 変更内容:
  - Wall/Door/Floor/Bridgeのmap/cache/neighbor/Room dirtyを接続する。
  - Operational Soul Spaのtile、GeneratePower worker、DeliverToSoulSpa、grid relation、visualをsite単位でcleanupする。
  - Constructing Soul Spaのcancel/refundを追加する。
  - Outdoor Lamp/Soul Spa removalをenergy topology reconcilerへwakeする。
- 主な変更ファイル:
  - `crates/bevy_app/src/systems/jobs/deconstruction/`
  - `crates/bevy_app/src/systems/jobs/soul_spa_construction/`
  - `crates/bevy_app/src/systems/energy/`
  - `crates/hw_world/src/{map/,room_systems.rs}`
  - `crates/hw_visual/src/{wall_connection.rs,power.rs}`
- 完了条件:
  - [ ] Door/Bridge/Floor/Wall解体後にwalkabilityとRoomが正しく再計算される。
  - [ ] Soul Spa 4tile、worker、request、generator/grid relationが残らない。
  - [ ] Lamp/Soul Spa removal後のgrid summaryとvisualが最初の有効frameで一致する。
  - [ ] Constructing Soul Spaは実搬入Boneだけを100%返す。
- 検証:
  - `cargo test -p bevy_app@0.1.0 deconstruction`
  - `cargo test -p bevy_app@0.1.0 energy`
  - `cargo test -p hw_world room`

## M5: Save/Load、Help、実機受入、archive

- 変更内容:
  - mid-order/mid-task save、load再割当、cancel/priority/dashboard、world replacement resetを横断確認する。
  - Help impact reviewと恒久docsを更新する。
  - no-prompt native acceptanceで操作、描画、cleanupを確認し、計画をarchiveする。
- 主な変更ファイル:
  - `crates/bevy_app/src/systems/save/{schema.rs,rehydrate/,load.rs}`（必要な登録/回帰のみ）
  - `crates/bevy_app/src/interface/ui/help_content/`
  - `docs/{building.md,tasks.md,state.md,logistics.md,room_detection.md,soul_energy.md,save_load.md,events.md,invariants.md,architecture.md,help-screen.md}`
- 完了条件:
  - [ ] load後にorder/target Relationshipは残り、runtime task/commit request/claim/UI targetは残らず再割当される。
  - [ ] F9とrollback後に旧WorldEpochのrequestを再生しても新worldを変更しない。
  - [ ] UI/Help/notification/dashboardとdomain結果が一致する。
  - [ ] native V1〜V5が合格する。
  - [ ] full workspace gateが成功し計画をarchiveする。
- 検証:
  - `python3 scripts/dev.py verify`
  - `python3 scripts/check_help_impact.py`
  - `git diff --check`

## 6. リスクと対策

| リスク | 影響 | 対策 |
| --- | --- | --- |
| Transformからfootprintを推測 | multi-tile/cacheが残る | WorldMap owner逆引きとtarget-kind matrixを正本化 |
| rootだけdespawn | companion/request/visualが孤立 | cleanup planのexact entity/request setをsnapshot test |
| building rootへDesignationを付与 | Mixer等の既存taskがremove/overwrite | 専用DeconstructionOrder rootとtarget Relationshipを使う |
| UIが直接componentを剥がす | reservation/worker不変条件違反 | Intent→owner request→domain finalizerを固定 |
| task完了とcancel/重複requestが競合 | 二重回収/二重despawn | exclusive finalizerがtarget単位の同期claimをdeferred処理前に取得 |
| storage内容をsalvage扱い | player資源が減る/増える | stored content 100%返却と固定salvageを別集計 |
| Sand/StasisMudをground/通常Stockpileへ返す | 5秒後消滅またはpolicy拒否 | 別MudMixerのnumeric/mud容量へ全量を事前予約し、確保不能ならcommitを拒否 |
| Bridge/river footprintへdrop | 撤去直後に資源が到達不能 | footprint外のpost-teardown passable cellを事前確定し、無ければcommitを拒否 |
| Mixer mud countとentityを両方spawn | StasisMudが二重化 | StoredByMixer entity数一致を検査し既存entityだけを移送 |
| parking解体でwheelbarrowを一律unload | 積載Sand/StasisMudがgroundで消える | wheelbarrowだけsafe cellへunparkしvalid LoadedInは維持 |
| C3 load後も`LoadedIn`が残ると仮定 | load境界で荷下ろし済みcargoを見失う | C1 live commitの積載維持とC3 staging handoff/safe unloadを別境界として明記し、ground itemから再仲裁する |
| Move中のtargetをcleanup | 移動先予約/anchorが孤立 | 指定/commitでMove状態をrejectしpending後の新規Moveをgate |
| 持続blocker後に即再割当 | 解体progress/requestが無限loop | failureで完全unassignし既存diagnostic domain stampがstaleになるまで候補外にする |
| order/targetをworkerより先にdespawn | `AssignedTask::Deconstruct`がguard abortまで残る | exact identity付きowner adapterで全workerをterminal化してからorder/targetを消す |
| WorkType追加の追従漏れ | policy/UI/save/perfでpanic/非表示 | ALL/index/exhaustive Help/encoding coverageをM1で更新 |
| deconstruct指定後もproducerが動く | cleanup直前に新request | pending targetを全facility producerの共通gateにする |
| Room/PowerをC1内で直接再計算 | 正本が分岐 | owner dirty/reconcilerだけをwakeし既存pipelineを再利用 |

## 7. 検証計画

- 自動:
  - 全BuildingType recovery/eligibility exhaustive test。
  - task assign→execute→commitの固定tick vertical test。
  - cleanup matrix: task/request/reservation/WorldMap/door/bridge/stockpile/child/proxy/relationship残存0。
  - stored content/tool/occupant保全、別Mixerへのvolatile transfer、wheelbarrow積載維持、recovery exactly once。
  - Room/path/Powerのsame-cycleまたは明示next-cycle収束。
  - mid-order/mid-task save/load、world-epoch stale replay、rollback。
- native acceptance（実装時は `hell-workers-run-native-acceptance` のno-prompt launcherを使用）:
  - V1: Orders→Deconstruct、hover理由、click指定、progress、回収表示。
  - V2: Wall/Door/Floor/Bridgeを撤去し、通行性、壁接続、Room再検出を確認。
  - V3: Tank/Mixer/Rest/Parkingを内容ありで撤去し、資源/tool/occupantが失われないこと、
    Sand/StasisMudが別Mixerへ移りwheelbarrow積載が維持されること、別Mixer容量不足とBridgeの安全drop先不足では
    非変更拒否になることを確認。
  - V4: Operational/Constructing Soul SpaとLampを撤去し、worker/request/発電/給電表示を確認。
  - V5: 指定後F5/F9、overlay capture、load後再割当、dashboard cancel/priorityを確認。
- パフォーマンス:
  - Deconstruct未使用steady stateで追加全Entity走査0。
  - cleanup scanは操作時だけ。M3で`deconstruction` fixtureをperf runnerへ追加し、`medium = 100 completed buildings`
    を固定fixture契約として、
    `PYTHONDONTWRITEBYTECODE=1 python3 scripts/perf.py audit --workload deconstruction --sizes medium --repeat 20 --output /tmp/hw-c1-deconstruction-perf-20260803`
    により1 commit CPU時間、scan/spawn数、steady-state 0 scanを記録する。
- 完了時:
  - `cargo fmt --all -- --check`
  - `cargo check --workspace`
  - `cargo clippy --workspace --all-targets -- -D warnings`
  - `cargo test --workspace`
  - `python3 scripts/dev.py verify`

## 8. ロールバック方針

- M1 model/schema、M2 headless vertical slice、M3 UI/storage、M4 structure/Spa、M5 docs/acceptanceを独立commitにする。
- M2はrequest consumerとordinary finalizerまで同時にlandし、consumerのないproduction commit requestを残さない。
- save bodyへDeconstructionOrderを出荷後に戻す場合、旧実行ファイルのforward loadは保証されない。
  rollback前にorder除去migrationまたはformat拒否方針を決める。
- recovery値の調整はtable単独で戻せるが、storage保全/cleanup契約は同時に弱めない。

## 9. AI引継ぎメモ（最重要）

### 現在地

- 進捗: `0%`
- 完了済みマイルストーン: なし
- 未着手/進行中: M1〜M5
- 前提状態: Track C3完了・archive済み。C1 M1を開始可能。

### 次のAIが最初にやること

1. current worktreeを確認し、C3の完成したregistry/schema coverageを前提にM1を開始する。
2. 専用order root/Relationship、WorkType末尾append、全BuildingType recovery table、WorldMap owner逆引きtestを先に固定する。
3. UI前にtask assign→commit→ordinary building cleanupのheadless vertical sliceを成立させる。

### ブロッカー/注意点

- `Building`はfootprintを保持しない。Transform/geometryだけでcleanup対象を推測しない。
- building rootへDeconstruct Designationを直接付けない。既存taskと競合しない専用order entityを使う。
- active Move中は指定/commitをrejectし、pending後の新規Moveを止める。
- 同期commit claimを取得する前にcleanup/refundのdeferred commandを発行しない。
- Mixer mudはStoredByMixer entityが実体であり、numeric countから二重spawnしない。
- Sand/StasisMudは通常Stockpileへ入らない。別MudMixer容量を確保し、Sand numeric/mud entityを直接移送する。
- commit requestからworker/exact identityを省略せず、order/target despawn前に全workerをterminal化する。
- Blueprint/Floor/Wall cancelのactual refundとcompleted salvageを混同しない。
- Target Relationship collectionを直接編集せずSource側owner helperを使う。
- Soul Spaはgeneric Building rootではなくsite/tile構造を正本にする。
- HVAC ConduitはC1対象外で、erase modeを維持する。
- production変更後は必ず `hell-workers-review-help-impact` Skillの判断を完了する。

### 参照必須ファイル

- `docs/building.md`
- `docs/tasks.md`
- `docs/invariants.md`
- `docs/save_load.md`
- `docs/soul_energy.md`
- `crates/hw_core/src/{jobs.rs,game_state.rs}`
- `crates/hw_jobs/src/{model.rs,tasks/mod.rs}`
- `crates/hw_familiar_ai/src/familiar_ai/decide/task_management/`
- `crates/hw_soul_ai/src/soul_ai/execute/task_execution/`
- `crates/bevy_app/src/systems/jobs/`
- `crates/hw_world/src/map/`

### 最終確認ログ

- 最終 `cargo check --workspace`: `未実施（計画作成のみ）`
- 最終 `cargo clippy --workspace --all-targets -- -D warnings`: `未実施（計画作成のみ）`
- 最終 `cargo test --workspace`: `未実施（計画作成のみ）`
- 未解決エラー: `N/A`

### Definition of Done

- [ ] M1〜M5が完了
- [ ] 全既存BuildingTypeとSoul Spaのcleanup matrixが合格
- [ ] 専用order、target単位commit claim、world-epoch resetが重複/cancel/load競合を排他する
- [ ] winner/loser/cancel/staleの全経路でexact identity付きworker terminalがexactly once成立する
- [ ] storage内容/tool/資源が消失・二重生成しない
- [ ] save/load、dashboard、policy、Helpが同期
- [ ] native V1〜V5が合格
- [ ] `python3 scripts/dev.py verify`が成功

## 10. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| `2026-08-03` | `Codex` | 専用order root、target単位commit claim、revision-gated failure復帰、owner-safe cleanup、揮発資材の事前storage確保、Mixer実体整合、Move競合、固定salvageをC1実装契約として確定 |
| `2026-08-03` | `Codex` | 自己レビューでC3を必須前提化し、exact worker identity終端、既存diagnostic stamp、別Mixerへのvolatile移送、wheelbarrow積載維持、SandPile回収0へ修正 |
| `2026-08-04` | `Codex` | 前提Track C3の実装・実機受入・archive完了を反映。C1はDraft/0%を維持し、次の着手点をM1へ更新 |
