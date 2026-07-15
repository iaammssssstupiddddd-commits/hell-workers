# 実行時正しさ契約リファクタリング計画

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `runtime-correctness-contracts-plan-2026-07-12` |
| ステータス | `Completed` |
| 作成日 | `2026-07-12` |
| 最終更新日 | `2026-07-14` |
| 作成者 | `Codex` |
| 親ロードマップ | [system-wide-correctness-refactoring-plan-2026-07-12.md](../system-wide-correctness-refactoring-plan-2026-07-12.md) |
| 関連済み計画 | `archive/task-execution-refactor-plan-2026-07-07.md` / `archive/observer-message-optimization-plan-2026-03-23.md` |
| 関連Issue/PR | `N/A` |

## 1. 目的

### 解決したい課題

- `commands.trigger()` だけでMessageReaderにも配送されるという誤った通知契約。
- retryable abort後に`Completed`へfallthroughできるタスク終了API。
- chain後のtask identityがAssigned/Completed event間で定義されていない。
- 最後の`WorkingOn`削除で`TaskWorkers`自体が消えた際、TransportRequestが`Claimed`に残る。
- `RemovedComponents`の`.next()` / `.any()`短絡によるdirty event取りこぼし。
- `ObstaclePosition`の用途を区別せず、建物・移動予約・床養生の削除でもterrainをDirt化する経路。
- 障害物cleanupのsystem配線が実際のproducer/deferred適用順と一致していない。

### 到達したい状態

- 全通知型についてdomain Observerとpresentation Messageのtransportが表で一意に定義される。
- タスク終了状態はrelease buildでも一度だけ確定し、terminal APIの戻り値を無視できない。
- initial assignmentとchain中のcurrent targetを別identityとして保持する。
- RelationshipTarget不在を「worker 0件」として扱い、request stateを同じ`Update`の終了までにPendingへ戻す。再arbitrationは次のLogic frameで行う。
- predicateの有無にかかわらず、全RemovedComponents readerが毎フレーム最後まで消費される。
- obstacle removalはsource/provenanceに応じてpassabilityとterrain mutationを分離する。

### 成功指標

- 本計画の通知matrix全10行にApp-level testがある。
- `cleanup_task_assignment`の許容呼び出しが`unassign_task`内部と`TaskExecutionContext`内部だけになる。
- terminal APIを無視すると`unused_must_use`でcompileが失敗する。
- `rg -n '\.read\(\)\.(next|any)\(' crates --glob '*.rs'` にRemovedComponentsの短絡消費が残らない。
- 最後のworker解除後、同じ`Update`終了時に`TaskWorkers`不在・`TransportRequestState::Pending`となり、次のLogic frameでarbitration再候補になることを同じ回帰テストで確認できる。
- 自然物以外のObstaclePosition削除ではterrain typeが変化しない。

## 2. スコープ

### 対象（In Scope）

- 最小library/test harnessとall-target Clippy baseline。
- `hw_core`通知型、`MessagesPlugin`、全Producer/Consumer。
- `hw_soul_ai` task execution/chain/pathfinding fallback。
- `hw_jobs` task identity、WorkType、reservation lifecycle。
- `hw_logistics` TransportRequest state machine/arbitration。
- Familiar snapshot、Entity List、Task List、speech stackingのremoval reader。
- `hw_world` obstacle cleanupとWorldMap pathfinding semantics。
- 建築完了、建物移動、floor construction、自然物spawn/load rehydrateのObstaclePosition producer。

### 非対象（Out of Scope）

- Save format/schema/transactionの変更。load後のremoved-message resetはSave/Load子計画で扱う。
- production App plugin compositionの全面整理。
- SpatialGrid generic化。
- 全体rustfmt、CI導入、既存Clippy allow整理。
- pathfinding探索アルゴリズムの変更。

## 3. 設計判断

### 3.1 通知transport

- gameplay stateを即時変更する通知は既存`On*`型を`EntityEvent`として維持する。
- presentationだけの通知は既存型を`Message`専用にする。
- gameplayとpresentationの両方が必要な通知は、domain `On*` EntityEventと`*VisualMessage`を別型にする。
- dual通知のProducerは`publish_*` helperを1回呼び、helper内部で`trigger`と`write_message`を明示実行する。
- presentation consumerは`GameSystemSet::Visual`のMessageReader systemに置く。

#### 通知matrix（正本）

| 現行通知 | Domain transport | Presentation transport | Producer方針 | Consumer |
| --- | --- | --- | --- | --- |
| `OnSoulRecruited` | `EntityEvent`維持 | `SoulRecruitedVisualMessage` | `publish_soul_recruited` | vitals/state Observer + speech MessageReader |
| `OnStressBreakdown` | `EntityEvent`維持 | `SoulStressBreakdownVisualMessage` | `publish_stress_breakdown` | cleanup Observer + speech MessageReader |
| `OnExhausted` | `EntityEvent`維持 | `SoulExhaustedVisualMessage` | `publish_soul_exhausted` | cleanup Observer + speech/expression MessageReader |
| `OnReleasedFromService` | なし | 既存型を`Message`専用化 | `write_message` | speech MessageReader |
| `OnGatheringJoined` | なし | 既存型を`Message`専用化 | `write_message` | speech MessageReader |
| `OnTaskAbandoned` | なし | 既存型を`Message`専用化 | `write_message` | speech MessageReader |
| `OnTaskAssigned` | なし | 既存型を`Message`専用化 | `write_message` | speech MessageReader |
| `OnTaskCompleted` | `EntityEvent`維持 | `TaskCompletedVisualMessage` | `publish_task_completed` | motivation Observer + speech MessageReader |
| `OnGatheringParticipated` | なし | 既存型を`Message`専用化 | `write_message` | expression MessageReader |
| `OnEncouraged` | `EntityEvent`維持 | `SoulEncouragedVisualMessage` | `publish_soul_encouraged` | vitals Observer + speech MessageReader |

低頻度であってもvisual-only Observerを例外として残さない。例外を追加する場合は本表と`docs/events.md`へ理由・timingを記載する。

### 3.2 task identity

`task_entity`へ異なる意味を混在させないため、Soulに非保存の`ActiveTaskIdentity`を持たせる。

```rust
pub struct ActiveTaskIdentity {
    pub assignment_entity: Entity, // initial TaskAssignmentRequest.task_entity
    pub current_target_entity: Entity, // current WorkingOn.0
    pub current_work_type: WorkType,
}
```

- initial assignment時はassignment/currentを同じentityで初期化する。
- chainは`assignment_entity`を維持し、current target/work typeだけを更新する。
- Assigned通知は`assignment_entity`とinitial current targetを持つ。
- Completed通知は`assignment_entity`とfinal current targetを両方持つ。
- chainは新しいAssigned通知を出さない。chainは同一assignment内のsegment遷移として扱う。
- `ActiveTaskIdentity`が欠落または`WorkingOn.0`と不一致なら、placeholderで完了通知を出さず、warnしてretryable abortする。

### 3.3 terminal API

- `TaskExecutionContext`はprivateな`try_begin_end(disposition) -> Result<(), AlreadyEnded>`で状態を一度だけ遷移させる。
- `complete_task` / `abort_retryable` / `abort_closed`は`#[must_use] TaskHandlerControl::Ended`を返す。
- handlerは`TaskHandlerControl`を返し、terminal branchは`return ctx.complete_task(...)`等で終了する。
- fatigue増加など完了に付随するmutationはterminal API呼び出し前に行う。
- raw `TaskEndDisposition`を引数に取る`clear_soul_assignment`は廃止し、`complete_after_custom_cleanup` / `abort_retryable_after_custom_cleanup`へ分ける。
- release buildでも二度目のterminal callは状態を上書きせず、errorを返す。
- retryable/closed内部abortでは`OnTaskAbandoned`を発行しない。外部ユーザー操作は area selection が`SoulTaskUnassignRequest { emit_abandoned: true }`を書き、Logic の`ApplyDeferred`後に Soul AI Perceive が`unassign_task(emit=true)`を適用する。これによりcleanupは同じ`Update`のExecuteより先に完了し、通知はその後に読める。

### 3.4 obstacle source契約

- `ObstaclePositionIndex`はRemovedComponentsの旧位置・provenance解決用であり、WorldMap全blockerの正本にはしない。
- `ObstacleSourceKind`を非保存のruntime-derived componentとして追加し、少なくとも`NaturalTerrainClearing` / `BuildingFootprint` / `PlacementReservation` / `ConstructionProtection`を区別する。
- `NaturalTerrainClearing`だけが削除時にterrainをDirtへ変更する。
- ECS markerとWorldMap recordが同じowner/gridを表す場合は1つの論理blockerとして扱う。完成建物の`BuildingFootprint`子はWorldMap occupancyの意図的なmirrorであり、重複禁止の対象にしない。異なるowner同士の同一grid重複だけをplacement validationで禁止する。
- source componentと非保存のmirror entityは次表のowner情報から決定論的にrehydrateし、その後indexをseedする。
- Door state変更はpassability/costの専用APIを正本とし、footprint marker数と比較しない。
- `obstacle_version`は`is_walkable`のtopology世代とする。Door追加/削除を含む全mutationは、最終`is_walkable`が変わる場合だけ更新する。`Open`↔`Closed`のcost変更では更新せず、`Locked`境界の変更は更新する。既存pathをcostだけで再探索する要件が実測で必要になった場合は、性能計画で別の`path_cost_version`を導入する。

#### source/rehydrate matrix（正本）

| Source | Runtime owner | Save body | Load時の復元 |
| --- | --- | --- | --- |
| `NaturalTerrainClearing` | `Tree` / `Rock` root | ownerと`ObstaclePosition`を保存、sourceは除外 | `Tree` / `Rock` markerからsourceを再付与 |
| `BuildingFootprint` | `blocks_movement()`がtrueの完成`Building`配下mirror child | child/sourceとも除外、WorldMap occupancyとBuilding rootを保存 | WorldMap entryのentityが完成`Building` rootと一致し、kindが`blocks_movement()`を満たす場合だけchildを再生成。Blueprint・Bridge・passableな完成Buildingは除外 |
| `PlacementReservation` | `MovePlantTask`配下mirror child | `Designation` root以外のtask payload/child/sourceは保存しない | durable sourceからobstacle bitmapを再構築して予約bitを除外し、不完全なMove `Designation` rootをdespawn。再生成しない |
| `ConstructionProtection` | Curing中の`FloorTileBlueprint` | site/tile/WorldMapを保存、sourceは除外 | site phaseとtile ownerを照合し、Curing中だけsourceを再付与 |

## 4. 期待する影響

- 主効果は通知配送、タスク終了、Relationship removal、障害物同期の正しさ回復であり、ゲームバランスは変更しない。
- obstacle source別の差分同期により、無関係な障害物変更時の全件再構築と不要な`Dirt` mutationを減らす。
- terminal APIとtransportの明示化で分岐は増えるが、release buildでも一度だけ終了する契約を優先する。
- 定量性能の採否は[性能計画](../system-wide-runtime-performance-plan-2026-07-12.md)のbaselineと対象カウンタで別途判定する。

## 5. マイルストーン

## M0: 最小library/test harnessと品質baseline

### 変更内容

1. module宣言、root共有Resource、public re-exportを`bevy_app/src/main.rs`から`lib.rs`へ移す。
2. production App composition、window/render/backend選択は`main.rs`に残す。
3. `lib.rs` unit testから、対象systemだけを`MinimalPlugins`へ登録できる`test_support` helperを追加する。full `LogicPlugin` / `VisualPlugin`はheadless testへ追加しない。
4. 現在のall-target Clippy 5警告を構造的に修正する。
5. 以降の共通gateを`cargo clippy --workspace --all-targets -- -D warnings`へ引き上げる。

### 主な変更ファイル

- `crates/bevy_app/src/{lib.rs,main.rs}`
- `crates/bevy_app/src/test_support.rs`（test-only、新規）
- `crates/hw_world/src/mapgen/validate/mod.rs`
- `crates/hw_world/src/pathfinding/mod.rs`
- `crates/hw_world/src/terrain_zones.rs`
- `docs/architecture.md`
- `docs/cargo_workspace.md`
- `docs/invariants.md`（現行I-A2矛盾も修正）

### 完了条件

- [x] binary起動構成に変更がない
- [x] library testからroot module/systemを参照できる
- [x] test Appがwindow/wgpuなしで対象systemを直接登録できる
- [x] `cargo clippy --workspace --all-targets -- -D warnings`成功
- [x] crate boundary docs間でLeafのCommands/Query方針が一致

### 検証

- `cargo test -p bevy_app@0.1.0 --lib`
- `cargo check -p bevy_app@0.1.0 --bin bevy_app`
- `cargo clippy --workspace --all-targets -- -D warnings`

## M1: RemovedComponentsの完全消費primitive

### 変更内容

1. `hw_core::ecs`に次の2 helperを追加する。
   - `drain_removed(reader) -> bool`: 全件消費し、1件以上あればtrue。
   - `drain_removed_where(reader, predicate) -> bool`: predicate結果をORしつつ全件評価する。`Iterator::any`は禁止。
2. Familiar snapshot、wheelbarrow arbitration、Entity List、Task List、speech stackingの短絡読み取りを置換する。
3. `hw_world::terrain_visual`もM4のsource-aware置換を待たず、同じhelperで全件消費へ直す。
4. query dirty判定とreader消費を別statementにし、queryがtrueでもreaderを必ず消費する。
5. I-U3をBevy 0.19のMessageCursor契約と上記helperへ更新する。

### 主な変更ファイル

- `crates/hw_core/src/{lib.rs,ecs.rs}`
- `crates/bevy_app/src/systems/familiar_ai/perceive/resource_sync.rs`
- `crates/hw_logistics/src/transport_request/arbitration/system.rs`
- `crates/bevy_app/src/interface/ui/list/change_detection.rs`
- `crates/bevy_app/src/interface/ui/panels/task_list/dirty.rs`
- `crates/hw_visual/src/speech/update.rs`
- `crates/hw_world/src/terrain_visual.rs`
- `docs/invariants.md`

### 完了条件

- [x] 同frameの複数reader・同reader複数removalを全件消費する
- [x] predicateの最初のmatch後も後続entityを評価する
- [x] 次frameに未消費event由来の不要なdirty rebuildがない
- [x] RemovedComponentsに対する`.read().next()` / `.read().any()`が0件

### 検証

- `cargo test -p hw_core`
- `cargo test -p hw_logistics --lib transport_request::arbitration::system::tests::resource_item_removal_predicate_consumes_nonmatching_entries`
- `cargo test -p bevy_app@0.1.0 --lib interface::ui::list::change_detection::tests::consumes_all_structure_removal_readers_in_one_update`
- Familiar reservation sync の既存 removal regression test
- `cargo test --workspace`

## M2: 通知transportの明示化

### 変更内容

1. §3.1 matrixどおりに全10型を変更する。
2. dual通知のpayload型と`publish_*` helperを`hw_core::events`へ追加する。
3. speech Observer 7本をMessageReader systemへ移し、Visual setへ登録する。
4. expression consumerを新visual messageへ切り替える。
5. Message-only Producerを`commands.write_message`またはMessageWriterへ変更する。
6. `MessagesPlugin`登録と`docs/events.md`表を同じ変更で同期する。

### 主な変更ファイル

- `crates/hw_core/src/events.rs`
- `crates/bevy_app/src/plugins/messages.rs`
- `crates/hw_soul_ai/src/soul_ai/execute/`
- `crates/hw_soul_ai/src/soul_ai/update/`
- `crates/hw_familiar_ai/src/familiar_ai/`
- `crates/hw_visual/src/speech/{mod.rs,observers.rs}`
- `crates/bevy_app/src/entities/damned_soul/movement/expression_events.rs`
- `docs/events.md`
- `docs/invariants.md`

### 完了条件

- [x] matrix全10行についてProducer/Consumer/登録が一致
- [x] dual publisherはdomain Observerとvisual Messageを各1回だけ発行
- [x] visual-only処理がObserverに残っていない
- [x] `commands.trigger()`だけをProducerとする型をMessageReaderが購読していない

### 回帰テスト

- [x] matrix table-driven App test: domain受信数、visual受信数、payload一致
- [x] `OnExhausted`: domain Observer + speech / expression reader各1回
- [x] `OnSoulRecruited` / `OnStressBreakdown` / `OnEncouraged`: domain Observer + speech reader各1回
- [x] visual-only 5型: Messageだけ1回

## M3: タスク終了・identity・Relationship lifecycle

### 変更内容

1. §3.2の`ActiveTaskIdentity`を追加し、assignment、chain、completion、abortで更新/削除する。
2. §3.3の`TaskHandlerControl`とrelease-safe terminal遷移を導入する。
3. 全terminal API呼び出しを監査し、処理継続が必要な副作用を呼び出し前へ移す。
4. `cleanup_task_assignment`直呼びを`unassign_task`内部とcontext内部へ限定する。`pathfinding/fallback.rs`も検索対象に含める。
5. stockpile reject、haul-to-blueprint、bucket abort/helper、wheelbarrow cancel等を専用terminal APIへ移す。
6. `AssignedTask::get_target_entity`を`primary_payload_entity`へ変更し、event identityに使わない。
7. `HaulToMixer`のWorkTypeを`WorkType::HaulToMixer`へ修正する。
8. reservation lifecycle matchのwildcardを廃止する。
9. `TaskWorkers`手動insert/removeを全廃する。
10. early state syncとは別に`TaskWorkers` removal reconcile systemを追加し、M1 helperで全件消費して対象requestをPendingへ戻す。
11. `TransportRequestSet::Reconcile`を`SoulAiSystemSet::Execute`後に追加する。source commandの適用と、Relationship hookが内部queueへ積む空target削除の適用には二段の`ApplyDeferred`を置く。reconcileは同じ`Update`でPending復帰まで行い、既存arbitrationは次のLogic frameで再候補化する。

### 主な変更ファイル

- `crates/hw_jobs/src/tasks/mod.rs`
- `crates/hw_jobs/src/lifecycle.rs`
- `crates/hw_soul_ai/src/soul_ai/helpers/{query_types.rs,work.rs}`
- `crates/hw_soul_ai/src/soul_ai/execute/task_assignment_apply.rs`
- `crates/hw_soul_ai/src/soul_ai/execute/task_execution_system.rs`
- `crates/hw_soul_ai/src/soul_ai/execute/task_execution/context/execution.rs`
- `crates/hw_soul_ai/src/soul_ai/execute/task_execution/chain.rs`
- `crates/hw_soul_ai/src/soul_ai/execute/task_execution/`
- `crates/hw_soul_ai/src/soul_ai/pathfinding/fallback.rs`
- `crates/hw_logistics/src/transport_request/state_machine.rs`
- `crates/hw_logistics/src/transport_request/plugin.rs`
- `crates/hw_logistics/src/transport_request/arbitration/candidates.rs`
- `crates/bevy_app/src/systems/jobs/soul_spa_construction/delivery_sync.rs`
- `docs/tasks.md`
- `docs/invariants.md`
- `docs/soul_ai.md`

### 完了条件

- [x] terminal dispositionはrelease buildでも上書き不能
- [x] raw disposition setterがpublic APIにない
- [x] stockpile rejectはretryable abort、完了報酬/Completed/Abandoned通知なし
- [x] normal completionはassignment/current targetが一致
- [x] chain completionはroot assignment維持、final current target/work type更新
- [x] identity欠落時にplaceholder完了通知を出さない
- [x] task abortはCompleted domain/visualを発行しない
- [x] 最後のWorkingOn削除後、同じUpdate終了時にTaskWorkers不在・request Pending、次のLogic frameでarbitration再候補化
- [x] reservation解放とWorkingOn削除が各終了経路で1回

### 回帰テスト

- [x] stockpile reject、blueprint消滅、bucket中断、pathfinding到達不能cleanup
- [x] normal assignment → completion identity
- [x] chain final segment → completion identity
- [x] terminal API二重呼び出し（debug/release共通ロジック）
- [x] Relationship source insert/removeとTransportRequest state遷移
- [x] lifecycle全AssignedTask variant table

## M4: source-aware obstacle同期

### 変更内容

1. 全`ObstaclePosition` Producerを棚卸しし、`ObstacleSourceKind`を必須付与する。
2. 自然物spawnには`NaturalTerrainClearing`、完成建物 footprintには`BuildingFootprint`、移動予約には`PlacementReservation`、床養生には`ConstructionProtection`を付与する。
3. indexはEntity→旧GridPos/source/論理ownerとGridPos owner refcountを保持し、Added/Changedを先に反映してからRemoved batchを処理する。同じownerのWorldMap recordとmirror markerを二重countしない。
4. removal後に別ownerのrefcountが0かつ、削除対象と異なるlive WorldMap building ownerがない場合だけpassability blockerを解除する。
5. terrain Dirt化は削除recordがNaturalTerrainClearingの場合だけ行う。
6. Door/building direct mutationとの関係を§3.4の非重複invariantで固定する。
7. deadな`bevy_app/.../building_completion/world_update.rs`を削除し、実所有者`hw_soul_ai::building_completed`を更新する。
8. building completionをVisual setからLogic内の`BuildingCompletionSet`へ移す。
9. `ApplyDeferred.after(SoulAiSystemSet::Execute/building completion).before(ObstacleSyncSet)`を追加し、`ObstacleSyncSet`をActor/pathfindingより前に置く。
10. §3.4のmatrixどおりにsource/mirrorを復元するidempotent helperを追加する。
11. load時は保存済み`WorldMap.obstacles`を正本にせず、一度clearして、`blocks_movement()`がtrueの完成Building、non-Bridge Blueprint、WallConstructionSite、Tree/Rock、Curing中FloorTileというdurable semantic sourceからbitmapを再構築する。完成Bridgeから`bridged_tiles`も再構築する。保存済み`door_states`を最終overrideとして適用し、Openはraw bitをclear、Closed/Lockedはraw bitを立てる。raw blocker / Door / Bridge cache は一括更新し、更新前後の最終walkabilityに差分があった場合だけversionを1回更新する。PlacementReservation bitは再構築対象外とし、不完全なMove `Designation` rootもdespawnする。
12. runtime M4内で現行`rehydrate_after_load`へhelperを暫定配線し、M4単独commit時点でも通常load/v0 loadを壊さない。Save/Load計画M4で固定phase coordinatorへ移す際は挙動とtestをそのまま維持する。

### 主な変更ファイル

- `crates/hw_jobs/src/`（Obstacle source component）
- `crates/hw_world/src/terrain_visual.rs`
- `crates/hw_world/src/map/{obstacles.rs,doors.rs,buildings.rs,tiles.rs}`
- `crates/hw_soul_ai/src/soul_ai/building_completed.rs`
- `crates/bevy_app/src/plugins/{logic.rs,visual.rs}`
- `crates/bevy_app/src/systems/logistics/initial_spawn/terrain_resources.rs`
- `crates/bevy_app/src/systems/logistics/initial_spawn/facilities.rs`
- `crates/bevy_app/src/systems/dream_tree_planting.rs`
- `crates/bevy_app/src/interface/selection/building_move/finalization.rs`
- `crates/bevy_app/src/systems/jobs/floor_construction/completion.rs`
- `crates/bevy_app/src/systems/save/rehydrate.rs`（暫定配線。Save/Load M4でcoordinatorへ移動）
- `crates/bevy_app/src/systems/jobs/building_completion/world_update.rs`（削除）
- `docs/architecture.md`
- `docs/invariants.md`

### 完了条件

- [x] 最後の自然物削除でblocker解除 + terrain Dirt化
- [x] 建物/移動予約/床養生削除でterrain維持
- [x] 同gridのECS obstacleが残る場合はblocker維持
- [x] Door open steady-stateで全障害物scanなし
- [x] walkability topology変更時だけobstacle_version増加。Door Open↔Closed/no-opでは不変、Locked境界では増加
- [x] representative Worldへrehydrate helperを2回適用しても、source/mirror/indexとsemantic source由来bitmapがmatrixどおりで重複しない
- [x] live Move reservationを含むv0相当Worldをrehydrateすると予約bitと不完全Move Designationが消え、durable blockerは維持される
- [x] obstacle removalが次のpathfinding実行前に反映

### 回帰テスト

- [x] hw_world: natural/building/reservation/construction removal policy
- [x] hw_world: duplicate logical owner/refcount、last removal、Door open/close/locked cost/topology version
- [x] bevy_app App schedule: task removal → ApplyDeferred → ObstacleSync → pathfinding
- [x] rehydrate helper: Tree/Rock、completed Building、Blueprint、Bridge cache再構築/stale cache除去、Door Open/Closed/Lockedとv0 door stateのtopology version、Curing floor、transient reservation、incomplete Move Designation
- [x] building placement validation: cross-domain blocker重複拒否

## 6. リスクと対策

| リスク | 影響 | 対策 |
| --- | --- | --- |
| 10通知の型変更でProducer漏れ | visual/gameplay欠落 | matrix testと`rg` inventoryを同じcommitで更新 |
| terminal戻り値移行でpost-completion mutation消失 | fatigue/成果物の退行 | mutation順序表とhandler family test |
| identity component欠落 | completion correlation不能 | assignment時required insert、実行前整合check、placeholder禁止 |
| RelationshipTarget削除をChanged queryが見ない | request再割当不能 | RemovedComponents reader + Pending復帰test |
| obstacle source付与漏れ | terrain誤変更 | Producer全件gate、source未付与ObstaclePositionを検出するdebug validation |
| building completion set移動 | completion visual/WorldMap順序退行 | App schedule testと手動建築完了scenario |
| source/mirror rehydrateが誤分類・重複spawn | blocker解除またはchild/index重複 | source matrix + owner照合 + idempotence test |

## 7. 検証計画

### 各マイルストーン必須

- 変更したRustファイルを`rustfmt --edition 2024 <file...>`で整形
- `cargo check --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- 対象crate test
- rust-analyzer workspace diagnostics 0件
- `git diff --check`

### 計画完了時

- `cargo test --workspace`
- 通知matrix test全行成功
- task lifecycle/chain/Relationship test成功
- obstacle policy/schedule test成功
- `python scripts/update_docs_index.py`

### 手動確認

1. 通常task開始/完了speechが各1回。
2. 満杯stockpileでabortし、完了報酬/完了speech/放棄speechなし、再割当可能。
3. Gather→Haul chain完了でroot/current identityがログ上整合。
4. Exhausted/StressBreakdown/Encouraged/Recruitedでgameplayとspeech各1回。
5. 木/岩削除はDirt化、建物移動予約解除は元terrain維持。
6. Door Open↔Closedでcostだけが更新されtopology versionは不変、Locked切替ではversionが更新され、steady-state scanなし。

## 8. ロールバック方針

- M0〜M4を独立コミットにする。
- M2は型、Producer、Consumer、docs、matrix testを同一コミットに含める。
- M3はtask family単位に小分け可能だが、旧terminal APIと新APIを同一handlerに混在させない。
- M4はsource component導入、schedule移動、cleanup置換を小コミットに分け、各時点でtestをgreenに保つ。

## 9. AI引継ぎメモ

### 現在地

- 進捗: `100%`
- 完了済み: M0、M1、M2、M3、M4
- 残作業: 計画書のarchive判断のみ
- `docs/proposals/hvac-plumbing-proposal.md`の既存変更は対象外。

### 次のAIが最初にやること

1. archive 実施時に `docs/plans/archive/` へ移動し、indexを再生成する。
2. 実ゲームで通知・cancel・door topology の手動確認を行う。

### ブロッカー/注意点

- `commands.trigger`とMessageは自動連結されない。
- Bevyは最後のRelationship source削除時にtarget component自体を削除する。
- `Iterator::any`はRemovedComponentsを完全消費しない。
- `TaskEndDisposition` enum自体は導入済み。重複enumを追加しない。
- chainは新assignmentではなく同一assignment内segmentとして扱う。
- obstacle indexはWorldMap全blockerの正本ではない。
- building completionの実所有者は`hw_soul_ai::building_completed`。bevy_appの`world_update.rs`は死蔵。

### Definition of Done

- [x] M0〜M4完了
- [x] 全確認済みruntime不具合に回帰テストあり
- [x] `cargo check --workspace`成功
- [x] `cargo clippy --workspace --all-targets -- -D warnings`成功
- [x] `cargo test --workspace`成功
- [x] docs更新・index再生成済み
- [ ] 計画書archive済み

## 10. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| `2026-07-12` | `Codex` | 全体計画の自己レビュー指摘を反映して新規作成 |
| `2026-07-12` | `Codex` | 再レビューを反映し、removal schedule、通知/terminal依存、source rehydrate、Door topology version、v0 obstacle再構築を確定 |
| `2026-07-12` | `Codex` | M0完了: bevy_app library境界、focused test harness、all-target Clippy baseline、関連ドキュメントを実装 |
| `2026-07-14` | `Codex` | M1完了: `RemovedComponents` 全量 drain helper、Familiar/arbitration/UI/speech/terrain の短絡 reader 置換、回帰テストと I-U3 を更新 |
| `2026-07-14` | `Codex` | M2完了: domain / presentation notification transport を分離し、全10行の matrix test・Speech MessageReader 移行・仕様書を同期 |
| `2026-07-14` | `Codex` | M3 Relationship lifecycle slice: `TaskWorkers` の手動削除を廃止し、二段 deferred 後の removal reconcile と同Update回帰テストを追加 |
| `2026-07-14` | `Codex` | M3完了: ActiveTaskIdentity、release-safe terminal API、cancel順序、bucket abort / pathfinding failure cleanup の回帰テストを固定 |
| `2026-07-14` | `Codex` | M4完了: source-aware obstacle同期、topology-aware WorldMap、load rehydrate、placement bridge validation、Actor前scheduleと恒久仕様を同期 |
| `2026-07-14` | `Codex` | workspace check / all-target Clippy / workspace test とrelease terminal testを完了し、計画をCompletedへ更新 |
| `2026-07-15` | `Codex` | 最終監査を反映。v0 Door state のtopology version、Bridge cache再構築/stale cache除去、source-less marker debug validation、rehydrate経路の恒久仕様を追加 |
