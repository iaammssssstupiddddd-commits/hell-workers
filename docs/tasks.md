# タスクシステム (Task System)

プレイヤーまたはシステムが世界の実体（木・岩・アイテム等）に `WorkType` を指定し、適切な Soul が実行するまでを管理します。

## 1. 実行アーキテクチャ

4フェーズ（Perceive → Update → Decide → Execute）で実行。詳細は [ai-system-phases.md](ai-system-phases.md) 参照。

## 2. コンポーネント接続マップ

ECS では「誰が書いて誰が読むか」が静的解析で追いにくい。以下に主要コンポーネントの接続を示す。

### 2.1 Relationship コンポーネント（Bevy 自動維持）

Bevy 0.19 の Relationship は **Source 側を操作すれば Target 側が自動更新** される。Target 側を手動で書かない。

| Source (手動操作) | Target (Bevy自動) | Source の書き込み元 | Source の削除元 |
|:---|:---|:---|:---|
| `WorkingOn(task)` ← Soul | `TaskWorkers` ← task | `apply_task_assignment_requests` (Execute) | `task_execution_system` 完了時 / `unassign_task` |
| `ManagedBy(familiar)` ← task | `ManagedTasks` ← familiar | request producer / `apply_designation_requests` | task despawn 時 |
| `StoredIn(stockpile)` ← item | `StoredItems` ← stockpile | Haul dropping フェーズ | 持ち出し時 (`unassign_task` / haul picking) |
| `DeliveringTo(dest)` ← item | `IncomingDeliveries` ← dest | `apply_task_assignment_requests` (Execute) | タスク完了・`unassign_task` |
| `CommandedBy(familiar)` ← Soul | `Commanding` ← familiar | squad加入 / `prepare_worker_for_task_apply` | squad release・使役数超過・`OnExhausted`・`OnStressBreakdown` 等のowner lifecycle |

**エンティティ despawn 時**: そのエンティティの全 Relationship が Bevy によって自動除去され、Target 側も自動更新される（例: Soul が despawn すると `TaskWorkers` から自動削除）。

`TaskWorkers` は `WorkingOn` source だけを正本とし、target を直接 remove してはならない。最後の source が外れると Bevy は空の `TaskWorkers` 自体を削除するため、`TransportRequestPlugin` は Soul AI `Execute` の後に source command と Relationship 内部 command を順に適用してから removal を読む。現存する worker なし `TransportRequest` は同じ `Update` で `Pending` に戻り、次の Logic frame で再割り当て候補になる。

### 2.2 手動管理コンポーネント

| コンポーネント | 書き込み元 | 読み取り元 | 非自明な挙動 |
|:---|:---|:---|:---|
| `Designation` | request producer (Decide) / `apply_designation_requests` (Execute) | `DesignationSpatialGrid`（Change Detection、次フレームで反映）| **削除 = タスク消滅**。`unassign_task` は削除しない（再試行を許可）|
| `AssignedTask` | `apply_task_assignment_requests` (Execute) | `task_execution_system` (Execute) | `complete_task` / `complete_after_custom_cleanup` が context 内で正常終了を確定した場合だけ `OnTaskCompleted` を発行 |
| `Inventory(Option<Entity>)` | Haul系のpickup/drop handler、`unassign_task` | task execution / visual mirror | Soulの携行品の正本。Relationshipではなく1slot componentで、pickup時に`Some(item)`、drop/cleanup時に`None`へ更新する |
| `TaskSlots` | request producer | `task_finder/filter` | `TaskWorkers.len()` と照合される（Target は自動） |

### 2.3 SharedResourceCache（予約の調整点）

`SharedResourceCache` は **2つのシステムが直接呼び合わずに調整する場所**。

- **予約 snapshot の再構築**: `sync_reservations_system` が `AssignedTask` + `Designation`（Without\<TaskWorkers\>）から構築する。初回、予約 operation を変える active task の signature 差分、pending task 側の変更/削除、または 0.2秒の安全監査で実行する。signature は `hw_jobs::lifecycle` の active reservation operation から導出するため、進捗値だけの更新では再構築しない。
- **差分適用**: タスク中断・実行 handler が送る `ResourceReservationRequest` は `hw_logistics::apply_reservation_requests_system` が Execute で適用する。`TaskAssignmentRequest` 内の `reservation_ops` は割り当て適用時に直接 cache へ反映する。
- **delta と snapshot の分離**: Perceive の先頭で `begin_frame()` が pickup/store の frame-local delta だけを clear する。予約 snapshot の置換は再構築時だけで、未反映の frame-local delta を消してはならない。
- **DeliveringTo との関係**: 搬入先予約は `DeliveringTo` / `IncomingDeliveries` の Relationship が所有する。`SharedResourceCache` には積まず、二重カウントしない。
- **load 後**: `SharedResourceCache`、reservation signature cache、同期 timer をまとめて reset し、次の Perceive で完全 snapshot を再構築する。

## 3. タスク発見性チェックリスト

Familiar の `task_finder` がタスクを発見できる条件（**全て満たす必要がある**）:

1. `Designation` コンポーネントがある
2. `Transform` コンポーネントがある
3. **`DesignationSpatialGrid` または `TransportRequestSpatialGrid` に登録されている**（Change Detection、スポーン後の次フレームで反映）、または `ManagedTasks` に入っている。例外として `Build` と Yard-owned Designation は補助全件走査からも収集される
4. ⚠️ **Haul系 WorkType** (`Haul` / `HaulToMixer` / `GatherWater` / `HaulWaterToMixer` / `WheelbarrowHaul`) は **`TransportRequest` コンポーネントが必須** — なければサイレントにフィルタされ、エラー・ログなし
5. ownership チェック通過: ManagedTasks 内 / unassigned / issued_by 一致 / issued_by が Yard / エリア重複の引き継ぎ
6. `TaskWorkers.len() < TaskSlots.max`（デフォルト 1）
7. 通常タスクは Familiar の `TaskArea` 内、Yard 内（全使い魔共通）、または ManagedTasks 内。Mixer / Build / Yard-owned タスクはこの位置制約を越えて候補になれる
8. WorkType 別の状態チェック通過（Build: 資材完了済み / ReinforceFloorTile: `ReinforcingReady` / CoatWall: `is_provisional == true` 等）
9. スコア計算が `Some(priority)` を返す（None = スコア計算不能で除外）

## 4. タスクのライフサイクル

### 4.1 指定 (Designation)

**手動**: プレイヤーが UI/ドラッグ操作で指定。

- edge-triggered keyboard shortcut は `input_actions` resolver が context と exact modifier を確定し、command consumer は semantic action だけを読む。
- Modal/Pause capture 中は assignment、area/zone/designation の pointer ingress を遮断する。capture 開始時に
  drag 中なら同じ `TaskMode` の待機状態へ戻し、AreaEdit の開始前 snapshot を復元するため、release edge が
  capture 中に消えても designation/history/task assignment を新規確定しない。

**自動（request エンティティ方式）**: anchor 位置にエンティティを生成し、ソースは割り当て時に遅延解決:
- `task_area_auto_haul_system` → `DepositToStockpile`（Stockpile グループ単位）
- `blueprint_auto_haul_system` → `DeliverToBlueprint`
- `floor/wall_construction_auto_haul_system` → `DeliverToFloor/WallConstruction`
- `mud_mixer_auto_haul_system` → `DeliverToMixerSolid` / `DeliverWaterToMixer`
- `tank_water_request_system` → `GatherWaterToTank`
- `bucket_auto_haul_system` → `ReturnBucket`
- `wheelbarrow_auto_haul_system` → `ReturnWheelbarrow`
- root `soul_spa_auto_haul_system` → `DeliverToSoulSpa`（Bone、猫車必須）
- `provisional_wall_auto_haul_system` → `DeliverToProvisionalWall`（legacy）

**自動（Designation 直発行）**: `DesignationRequest` で Designation を対象エンティティに直接付与する方式:
- `mud_mixer_auto_refine_system` → `Refine`（材料が揃った MudMixer に発行。`collect_all_area_owners` により Familiar の TaskArea と Yard を統合し、使い魔が Idle でも Yard 内ミキサーへ精製タスクを発行できる）

**自動（gather 指定）**: `blueprint_auto_gather_system` が Wood/Rock 不足を検知し、`Tree`/`Rock` に `Chop`/`Mine` を直付与（`AutoGatherDesignation` marker）。Decide 内では `ApplyDeferred` を挟んで `familiar_task_delegation_system` より先に確定する。

### 4.2 割り当て (Assignment)

- `familiar_task_delegation_system`（0.5秒間隔）が root 側 orchestration を担当し、`hw_familiar_ai::familiar_ai::decide::task_management` の core に候補収集・worker 別再スコア（priority 0.65 + 距離 0.35）・assignment build を委譲して `TaskAssignmentRequest` を発行する（Execute で適用）
- 割り当て時に `DeliveringTo`・`WorkingOn`・`CommandedBy` を設定し、ソース（資材・バケツ等）を遅延解決
- `ConstructionSiteAccess` は root から注入され、floor / wall / provisional wall の construction site 座標解決だけを補助する
- **排他制御**: `SharedResourceCache` を参照（§2.3 参照）
- **Haul 系の需要再検証**: `DeliverToBlueprint` / `DepositToStockpile` / `DeliverToFloorConstruction` / `DeliverToWallConstruction` / `DeliverToProvisionalWall` は、`IncomingDeliveries` に加えて Think フェーズ内の `ReservationShadow` も差し引いた残需要が 0 の場合、新規 `AssignedTask` を発行しない。
- 60タイル超の候補は到達判定前に除外。Familiar assignment の Boolean 到達判定は `WalkabilityConnectivityCache` の version付き連結成分を使い、waypoint が必要な実経路生成 A* は起動しない

#### 4.2.1 タスク診断 snapshot

割り当て producer は通常の判定 cycle の副産物として latest-only 診断を公開する。UI 専用の候補探索は行わない。

- `hw_jobs` は表示非依存の 5 分類、producer mask、fixed-width counter、coverage、input stamp / revision を所有する。
- Familiar delegation は candidate universe に含まれた task だけを applicable とし、Familiar ごとの worker / source 分岐を
  1 terminal vote へ縮約する。submit、未評価、malformed / stale は blocker 票にしない。
- `ManagedBy` のない Blueprint `Build` は Familiar delegation と Blueprint auto-build の両 producer が applicable。
  `ManagedBy` 付き Blueprint と Blueprint ではない `Build` は auto-build が適用外で、Familiar delegation だけを使う。
  applicable producer の snapshot が欠ける、stale、または coverage 不足なら `PendingEvaluation` のままにする。
- `TaskWorkers` が現在 1 件以上なら診断より優先して `Working`。submit 済みでも worker がまだいなければ
  accepted の証拠ではないため `PendingEvaluation`。
- input revision は task、roster / TaskArea、resource / reservation availability、WorldMap topology を追跡する。
  availability は resource grid/cache に加え、`StoredItems` / `IncomingDeliveries` / `Inventory`、資源種別、
  loaded/stored/delivering/owner 関係、wheelbarrow の park/push/lease、transport demand、Stockpile / mixer / Blueprint 容量変更を含む。
  task-local revision は Blueprint / Floor / Wall phase、`ManagedBy`、request / demand も含む。record は代表理由が実際に
  依存する domain だけを持ち、producer header は Soul eligibility を表す roster stamp で evaluator coverage を検証する。
- producer map は cycle ごとに置換し、task × evaluator の行列や履歴を保持しない。

Familiar の auto-gather Commands は named flush で確定し、root revision sync を通ってから同じ Logic cycle の
delegation が診断する。これにより新規 task を古い revision で `Blocked` にしない。

### 4.3 実行 (Execution)

- `task_execution_system`は`AssignedTask::None`をread-onlyで早期除外し、idle Soulのtask context用mutable accessを作らない。`WorkingOn`はfilter条件にしないため、target消滅後の`AssignedTask::Some + Without<WorkingOn>`も既存handler/cleanupへ到達する。
- Actor移動の再探索、task handler、bucket routing が `RuntimePathSearchBudget` 不足で `Deferred` になった場合は、到達不能・タスク中断ではない。`AssignedTask`、phase、予約、`WorkingOn`、`Destination`、`Path`を維持して次フレームに再試行する。task/bucket の direct 探索が失敗後に adjacent 探索で defer した場合は、direct を繰り返さず adjacent 段階から再開する。
- **採取**: 木=Wood×5、岩=Rock×10ドロップ。Sand/BonePile/砂タイル/河川は無限ソース（即時完了）
- **運搬 (Haul)**: GoingToSource → Picking → GoingToDestination → Dropping
- **猫車運搬 (HaulWithWheelbarrow)**: GoingToParking → PickingUpWheelbarrow → GoingToSource → Loading → GoingToDestination → Unloading → ReturningWheelbarrow
- **Sand / StasisMud**: 原則猫車必須。例外: ソース隣接 3x3 の立ち位置からドロップ閾値内なら徒歩可
- **水搬送 (BucketTransport)**: `AssignedTask::BucketTransport(BucketTransportData)` の単一バリアントで表現。`source`（`River` / `Tank`）と `destination`（`Tank` / `Mixer`）に応じて `bucket_transport/phases/` の共通フェーズハンドラで実行される。`WorkType` は River→Tank が `GatherWater`、Tank→Mixer が `HaulWaterToMixer` として返される。
- **運搬先ガード**: Blueprint / construction / provisional wall / stockpile は Dropping / Unloading 直前に受入可能量を再確認し、到着時点で需要が消えた cargo を搬入先へ反映しない。
- **精製 (Refine)**: MudMixer で Sand+Water+Rock → StasisMud×5。`mud_mixer_auto_refine_system` が `has_materials_for_refining` を確認し、`collect_all_area_owners`（Familiar TaskArea + Yard 統合）で `issued_by` を決定して `DesignationRequest` を発行する。使い魔が Idle でも Yard 経由でタスクが発行される。
- **壁**: FrameWallTile（material_center で木材受領 → フレーミング）/ CoatWall（塗布 → `is_provisional = false`）
- **⚠️ 消滅**: 地面に放置された Sand / StasisMud は **5秒で消滅**（LoadedIn / StoredIn / DeliveringTo / StoredByMixer のいずれかがあれば維持）

### 4.4 完了・放棄 (Completion / Abandonment)

タスク実行の終了は `TaskExecutionContext` の終了 API 経由で行う（`context/execution.rs`）:

| API | 用途 | `OnTaskCompleted` | タスク本体（Designation 等） |
|:---|:---|:---|:---|
| `complete_task` | 正常完了 | **発火** | 呼び出し元が済ませた後に呼ぶ |
| `abort_retryable` | 再アサイン可能な中断 | 発火しない | 残す |
| `abort_closed` | 対象消滅・designation 削除 | 発火しない | 呼び出し元が除去 |
| `complete_after_custom_cleanup` | 専用物理 cleanup 後の正常完了 | **発火** | 呼び出し元が済ませた後に呼ぶ |
| `abort_retryable_after_custom_cleanup` | 専用物理 cleanup 後の再アサイン可能な中断 | 発火しない | 残す |

terminal state は `TaskExecutionContext` の内部状態で一度だけ確定する。
`task_execution_system` は正常完了が確定したときだけ `publish_task_completed` を呼び、domain
`OnTaskCompleted` と presentation `TaskCompletedVisualMessage` を発行する。
inventory 不整合等の先頭ガードは `unassign_task` を経由する（`AbortedRetryable` 相当、完了イベントなし）。

**イベントチェーン**:

| 通知 | 発火条件 | domain / presentation の主な副作用 |
|:---|:---|:---|
| `OnTaskAssigned`（Message） | `apply_task_assignment_requests` が消費時 | speech MessageReader による音声 / command tone |
| `OnSoulRecruited` / `SoulRecruitedVisualMessage` | 未指揮 Soul へのタスク割り当て時 | domain Observer: 移動クリア / やる気+30% / ストレス+10%。presentation reader: 勧誘演出 |
| `OnTaskCompleted` / `TaskCompletedVisualMessage` | `complete_task` / `complete_after_custom_cleanup` による正常終了 | domain Observer: **やる気ボーナス付与**（Chop/Mine+2%、Haul+1%、Build系+5%）。presentation reader: 音声 |
| `OnTaskAbandoned`（Message） | `unassign_task(emit=true)` / designation cancel の `SoulTaskUnassignRequest` から | speech MessageReader のみ（**cleanup は発行前に完了済み**） |
| `OnExhausted` / `SoulExhaustedVisualMessage` | 疲労 > 0.9 の閾値超え | domain Observer: `unassign_task(emit=false)` + `CommandedBy` 削除 + `ExhaustedGathering`。presentation reader: 専用の疲労音声 / 表情 |
| `OnStressBreakdown` / `SoulStressBreakdownVisualMessage` | ストレス >= 1.0 | domain Observer: `unassign_task(emit=false)` + `StressBreakdown { frozen }` 付与 + `CommandedBy` 削除。presentation reader: 専用音声 |

> `OnTaskAbandoned` は**通知専用**。designation cancel は request を書き、Logic の
> `ApplyDeferred` 後に Soul AI Perceive が `unassign_task(emit=true)` を適用する。これにより
> cleanup は同じ Update の Execute より先に終わり、取消と完了通知が競合しない。

## 5. unassign_task の契約

`crates/hw_soul_ai/src/soul_ai/helpers/work.rs`（`unassign_task` / `cleanup_task_assignment`）
`helpers::is_soul_available_for_work` 実体は `hw_soul_ai::soul_ai::helpers::work::is_soul_available_for_work`。

**実行すること**:
1. `emit_abandoned_event=true` なら `OnTaskAbandoned` Message を write（音声のみ）
2. `SharedResourceCache` の予約を解放（`ResourceReservationOp::Release*` を発行）
3. `HaulWithWheelbarrow` 中なら積載アイテムを可視化・座標復元、猫車を駐車に戻す
4. `Inventory` に通常 Haul の携行品があれば地面にドロップしてslotを`None`へ戻す（Designation は **残す** → 再試行可能）
5. `AssignedTask` を `None` にリセット
6. `WorkingOn` を削除

**実行しないこと（呼び出し元の責務）**:
- `CommandedBy` の削除 → squad release・使役数超過・`OnExhausted` / `OnStressBreakdown`等、使役関係を終了するowner lifecycleが担当

**呼び出し元と責務**:

| 呼び出し元 | emit_abandoned | 追加でやること |
|:---|:---|:---|
| `task_execution_system`（ターゲット不整合） | false | — |
| `on_exhausted` observer | false | 専用visual Messageを既に発行済み。`CommandedBy` 削除 |
| `on_stress_breakdown` observer | false | 専用visual Messageを既に発行済み。`CommandedBy` 削除 + `StressBreakdown` 付与 |
| player cancel（area_selection） | true | `SoulTaskUnassignRequest` を書き、Perceive で適用 |

## 6. 削除追跡（RemovedComponents）

Bevy 0.19 は削除イベントを持たないため、各システムが `RemovedComponents<T>` を polling する。

主な購読システム:
- `resource_sync`: `Designation` / `TransportRequest` / `TaskWorkers` / `AssignedTask` の削除を検知してキャッシュ無効化
- `transport_request_task_workers_reconcile`: 最後の `WorkingOn` 削除で消えた `TaskWorkers` を検知し、worker なし request を `Pending` へ復帰
- `arbitration`: `TransportRequest` / `WheelbarrowLease` 等の削除でリース cleanup
- `spatial/*`: `DamnedSoul` / `ResourceItem` の削除でグリッドから除去
- `obstacle_sync_system`: `ObstaclePosition` / `ObstacleSourceKind` / `ChildOf` の追加・変更・削除を
  index で追跡して WorldMap を同期。
  自然物の最後の blocker が外れた場合だけ terrain を Dirt 化する

**注意**: Soul が突然 despawn した場合、`OnTaskAbandoned` は発火せず、予約も自動解放されない。`resource_sync` の次回再構築（0.2秒以内）で整合性が回復する。

## 7. オートホール・自動Gather

詳細は [logistics.md](logistics.md) 参照。

### 7.1 需要起点の自動Gather（Wood / Rock）
`blueprint_auto_gather_system` が `DeliverToBlueprint` / `DeliverToWallConstruction` / `DeliverToMixerSolid` request の不足を需要起点に、`Tree` / `Rock` へ `Chop` / `Mine` を直付与（`AutoGatherDesignation` marker 付き）。資源位置が別 Familiar の TaskArea 内でも、同じ resource を必要とする owner を優先して供給候補を結び付ける。到達不能な地面資材や発見不能な手動指定は供給として数えず、Bridge の Wood/Rock 代替需要は到達可能な供給・候補へ配分する。Yard-owned 指定は Yard 外でも補助全件走査から委譲候補になり、需要解消後の未着手指定は自動回収される。

### 7.2 採集後チェーン (gather chain)

採集完了 (`GatherPhase::Done`) 直後、同フレーム内で `chain::find_haul_chain_after_gather` が起動し、採集地点から **4タイル以内の空きアイテム** と **pending な TransportRequest** を照合して同一 Soul が即座に運搬タスクへ移行する。

**チェーン先の優先順位**:

| 優先度 | TransportRequest kind | Soul に割り当てるタスク | 対象リソース |
|:---|:---|:---|:---|
| 1 | `DeliverToWallConstruction` | `AssignedTask::Haul { stockpile: wall_site }` | Wood |
| 1 | `DeliverToFloorConstruction` | `AssignedTask::Haul { stockpile: floor_site }` | 各種 |
| 2 | `DeliverToBlueprint` | `AssignedTask::HaulToBlueprint` | Wood / Rock 等 |
| 3 | `DeliverToMixerSolid` | `AssignedTask::HaulToMixer` | Rock |
| 4 | ―（フォールバック） | `AssignedTask::Haul { stockpile }` | 最近傍ストックパイル |

**フィルタ条件**: `state == Pending` / `WheelbarrowLease.is_none()` / `demand.remaining() > 0` / `resource_type` 一致。  
**実装**: `crates/hw_soul_ai/src/soul_ai/execute/task_execution/chain.rs` — `GatherHaulChain` enum + `find_haul_chain_after_gather`

### 7.3 搬入後チェーン (haul chain)

Blueprint / FloorSite / WallSite への搬入完了直後、`chain::find_chain_opportunity` が呼ばれ、スロット空きがあれば同一 Soul が作業タスクへ即移行する。

**チェーン対応表**:

| 運搬タスク | 搬入先 | チェーン先タスク | チェーン開始フェーズ |
|:---|:---|:---|:---|
| HaulToBlueprint（any素材、`materials_complete == true`） | Blueprint | Build | `BuildPhase::GoingToBlueprint` |
| Haul（Bone） | FloorSite | ReinforceFloorTile | `ReinforceFloorPhase::PickingUpBones` |
| Haul（StasisMud） | FloorSite | PourFloorTile | `PourFloorPhase::PickingUpMud` |
| Haul（Wood） | WallSite | FrameWallTile | `FrameWallPhase::PickingUpWood` |
| Haul（StasisMud） | WallSite | CoatWall | `CoatWallPhase::PickingUpMud` |

**実装**: `crates/hw_soul_ai/src/soul_ai/execute/task_execution/chain.rs` — `ChainOpportunity` enum + `find_chain_opportunity` + `execute_chain`

## 8. TaskArea 編集 UI

`Orders -> Area` で `TaskMode::AreaSelection` に入ると TaskArea 連続編集モード。

- 新規: 左ドラッグ矩形 / 直接編集: 既存エリアの内部ドラッグ（移動）・辺/角ドラッグ（リサイズ）
- `Shift+左リリース`: 適用して Normal 復帰 / `Esc`: Normal 復帰
- `Ctrl+Z/Y` または `Ctrl+Shift+Z`: Undo/Redo / `Ctrl+C/V`: コピー/ペースト / `Ctrl+1..3`: プリセット保存 / `Alt+1..3`: プリセット適用

## 9. バイタル（疲労・ストレス・やる気）

詳細は [soul_ai.md](soul_ai.md) 参照。

## 10. UI

詳細は [task_list_ui.md](task_list_ui.md) 参照。

手動エリア指定で発行した Chop / Mine には保存対象 `PlayerIssuedDesignation` を付ける。既存の
`AutoGatherDesignation` を手動指定で覆った場合は auto marker を除去し、選択 Familiar の有無に合わせて
`ManagedBy` も置換または除去して、auto と manual の provenance を共存させない。
タスクダッシュボードはこの positive provenance と live component を適用時に再検証し、そのタスクと
`ManualTransportRequest` だけを 0 / 5 / 10 の priority tier で変更できる。Blueprint、Move、自動 gather、
自動 TransportRequest、GeneratePower、旧 save 由来で provenance 不明な task は priority read-only とする。

キャンセルは owner 別 lifecycle へルーティングする。generic manual designation、manual transport、Blueprint、
Floor / Wall site の cleanup を汎用 despawn へ統合しない。Pause / Modal capture 中の intent は読み捨てず typed 拒否として
drain し、capture 解除後に遅延適用しない。
