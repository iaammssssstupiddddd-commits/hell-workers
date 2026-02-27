# タスクシステム (Task System)

プレイヤーまたはシステムが世界の実体（木・岩・アイテム等）に `WorkType` を指定し、適切な Soul が実行するまでを管理します。

## 1. 実行アーキテクチャ

4フェーズ（Perceive → Update → Decide → Execute）で実行。詳細は [ai-system-phases.md](ai-system-phases.md) 参照。

## 2. コンポーネント接続マップ

ECS では「誰が書いて誰が読むか」が静的解析で追いにくい。以下に主要コンポーネントの接続を示す。

### 2.1 Relationship コンポーネント（Bevy 自動維持）

Bevy 0.18 の Relationship は **Source 側を操作すれば Target 側が自動更新** される。Target 側を手動で書かない。

| Source (手動操作) | Target (Bevy自動) | Source の書き込み元 | Source の削除元 |
|:---|:---|:---|:---|
| `WorkingOn(task)` ← Soul | `TaskWorkers` ← task | `apply_task_assignment_requests` (Execute) | `task_execution_system` 完了時 / `unassign_task` |
| `ManagedBy(familiar)` ← task | `ManagedTasks` ← familiar | request producer / `apply_designation_requests` | task despawn 時 |
| `Holding(item)` ← Soul | `HeldBy` ← item | `task_execution` の拾い上げフェーズ | Haul dropping フェーズ |
| `StoredIn(stockpile)` ← item | `StoredItems` ← stockpile | Haul dropping フェーズ | 持ち出し時 (`unassign_task` / haul picking) |
| `DeliveringTo(dest)` ← item | `IncomingDeliveries` ← dest | `apply_task_assignment_requests` (Execute) | タスク完了・`unassign_task` |
| `CommandedBy(familiar)` ← Soul | `Commanding` ← familiar | `prepare_worker_for_task_apply` | `unassign_task` / OnExhausted / OnStressBreakdown |

**エンティティ despawn 時**: そのエンティティの全 Relationship が Bevy によって自動除去され、Target 側も自動更新される（例: Soul が despawn すると `TaskWorkers` から自動削除）。

### 2.2 手動管理コンポーネント

| コンポーネント | 書き込み元 | 読み取り元 | 非自明な挙動 |
|:---|:---|:---|:---|
| `Designation` | request producer (Decide) / `apply_designation_requests` (Execute) | `DesignationSpatialGrid`（Change Detection、次フレームで反映）| **削除 = タスク消滅**。`unassign_task` は削除しない（再試行を許可）|
| `AssignedTask` | `apply_task_assignment_requests` (Execute) | `task_execution_system` (Execute) | `None` への遷移が `OnTaskCompleted` の発火条件 |
| `TaskSlots` | request producer | `task_finder/filter` | `TaskWorkers.len()` と照合される（Target は自動） |
| `ReservedForTask` | （未使用・legacy） | arbitration でフィルタに使用 | 現状は付与されない |

### 2.3 SharedResourceCache（予約の調整点）

`SharedResourceCache` は **2つのシステムが直接呼び合わずに調整する場所**。

- **再構築**: `sync_reservations_system` が `AssignedTask` + `Designation`（Without\<TaskWorkers\>）から 0.2秒間隔で再構築
- **フレーム内差分**: `ResourceReservationRequest` → `apply_reservation_requests_system` で即時反映
- **DeliveringTo との関係**: `resource_sync` は `DeliveringTo` リレーションシップを参照するため、ここで HashMap に積まない（二重カウント防止）

## 3. タスク発見性チェックリスト

Familiar の `task_finder` がタスクを発見できる条件（**全て満たす必要がある**）:

1. `Designation` コンポーネントがある
2. `Transform` コンポーネントがある
3. **`DesignationSpatialGrid` または `TransportRequestSpatialGrid` に登録されている**（Change Detection、スポーン後の次フレームで反映）、または `ManagedTasks` に入っている
4. ⚠️ **Haul系 WorkType** (`Haul` / `HaulToMixer` / `GatherWater` / `HaulWaterToMixer` / `WheelbarrowHaul`) は **`TransportRequest` コンポーネントが必須** — なければサイレントにフィルタされ、エラー・ログなし
5. ownership チェック通過: ManagedTasks 内 / unassigned / issued_by 一致 / エリア重複の引き継ぎ
6. `TaskWorkers.len() < TaskSlots.max`（デフォルト 1）
7. Mixer タスク以外は Familiar の `TaskArea` 内、または ManagedTasks 内
8. WorkType 別の状態チェック通過（Build: 資材完了済み / ReinforceFloorTile: `ReinforcingReady` / CoatWall: `is_provisional == true` 等）
9. スコア計算が `Some(priority)` を返す（None = スコア計算不能で除外）

## 4. タスクのライフサイクル

### 4.1 指定 (Designation)

**手動**: プレイヤーが UI/ドラッグ操作で指定。

**自動（request エンティティ方式）**: anchor 位置にエンティティを生成し、ソースは割り当て時に遅延解決:
- `task_area_auto_haul_system` → `DepositToStockpile`（Stockpile グループ単位）
- `blueprint_auto_haul_system` → `DeliverToBlueprint`
- `floor/wall_construction_auto_haul_system` → `DeliverToFloor/WallConstruction`
- `mud_mixer_auto_haul_system` → `DeliverToMixerSolid` / `DeliverWaterToMixer`
- `tank_water_request_system` → `GatherWaterToTank`
- `bucket_auto_haul_system` → `ReturnBucket`
- `provisional_wall_auto_haul_system` → `DeliverToProvisionalWall`（legacy）

**自動（gather 指定）**: `blueprint_auto_gather_system` が Wood/Rock 不足を検知し、`Tree`/`Rock` に `Chop`/`Mine` を直付与（`AutoGatherDesignation` marker）。

### 4.2 割り当て (Assignment)

- `familiar_task_delegation_system`（0.5秒間隔）が候補収集 → worker 別再スコア（priority 0.65 + 距離 0.35）→ `TaskAssignmentRequest` 発行（Execute で適用）
- 割り当て時に `DeliveringTo`・`WorkingOn`・`CommandedBy` を設定し、ソース（資材・バケツ等）を遅延解決
- **排他制御**: `SharedResourceCache` を参照（§2.3 参照）
- 60タイル超の候補は A* 前に除外。`ReachabilityFrameCache` で到達判定を5フレーム共有

### 4.3 実行 (Execution)

- **採取**: 木=Wood×5、岩=Rock×10ドロップ。Sand/BonePile/砂タイル/河川は無限ソース（即時完了）
- **運搬 (Haul)**: GoingToSource → Picking → GoingToDestination → Dropping
- **猫車運搬 (HaulWithWheelbarrow)**: GoingToParking → PickingUpWheelbarrow → GoingToSource → Loading → GoingToDestination → Unloading → ReturningWheelbarrow
- **Sand / StasisMud**: 原則猫車必須。例外: ソース隣接 3x3 の立ち位置からドロップ閾値内なら徒歩可
- **精製 (Refine)**: MudMixer で Sand+Water+Rock → StasisMud×5
- **壁**: FrameWallTile（material_center で木材受領 → フレーミング）/ CoatWall（塗布 → `is_provisional = false`）
- **⚠️ 消滅**: 地面に放置された Sand / StasisMud は **5秒で消滅**（ReservedForTask / LoadedIn / StoredIn / DeliveringTo いずれかあれば維持）

### 4.4 完了・放棄 (Completion / Abandonment)

`AssignedTask` が `None` に戻った瞬間に `OnTaskCompleted` が発火する。放棄は全経路で `unassign_task` を経由する。

**イベントチェーン**:

| イベント | 発火条件 | Observer の主な副作用 |
|:---|:---|:---|
| `OnTaskAssigned` | `apply_task_assignment_requests` が消費時 | 音声再生 / ログ |
| `OnSoulRecruited` | 未指揮 Soul へのタスク割り当て時（`OnTaskAssigned` 内から条件付き） | 移動クリア / やる気+30% / ストレス+10% |
| `OnTaskCompleted` | `AssignedTask` → `None` への変化 | **やる気ボーナス付与**（Chop/Mine+2%、Haul+1%、Build系+5%）/ 音声 |
| `OnTaskAbandoned` | `unassign_task(emit=true)` から | 音声再生のみ（**cleanup は呼び出し元が完了済み**） |
| `OnExhausted` | 疲労 > 0.9 の閾値超え | `unassign_task` + `CommandedBy` 削除 + `ExhaustedGathering` 設定 |
| `OnStressBreakdown` | ストレス >= 1.0 | `unassign_task` + `StressBreakdown { frozen }` 付与 + `CommandedBy` 削除 |

> `OnTaskAbandoned` は**通知専用**。cleanup は呼び出し元（`unassign_task`）が既に完了している。

## 5. unassign_task の契約

`src/systems/soul_ai/helpers/work.rs`

**実行すること**:
1. `emit_abandoned_event=true` なら `OnTaskAbandoned` を trigger（音声のみ）
2. `SharedResourceCache` の予約を解放（`ResourceReservationOp::Release*` を発行）
3. `HaulWithWheelbarrow` 中なら積載アイテムを可視化・座標復元、猫車を駐車に戻す
4. 通常 Haul 中なら保持アイテムを地面にドロップ（Designation は **残す** → 再試行可能）
5. `AssignedTask` を `None` にリセット

**実行しないこと（呼び出し元の責務）**:
- `WorkingOn` の削除 → `task_execution_system` が担当
- `CommandedBy` の削除 → `OnExhausted` / `OnStressBreakdown` observer が担当

**呼び出し元と責務**:

| 呼び出し元 | emit_abandoned | 追加でやること |
|:---|:---|:---|
| `task_execution_system`（ターゲット不整合） | false | `WorkingOn` 削除 |
| `on_exhausted` observer | true | `CommandedBy` 削除 |
| `on_stress_breakdown` observer | true | `CommandedBy` 削除 + `StressBreakdown` 付与 |
| player cancel（area_selection） | false | — |

## 6. 削除追跡（RemovedComponents）

Bevy 0.18 は削除イベントを持たないため、各システムが `RemovedComponents<T>` を polling する。

主な購読システム:
- `resource_sync`: `Designation` / `TransportRequest` / `TaskWorkers` / `AssignedTask` の削除を検知してキャッシュ無効化
- `arbitration`: `TransportRequest` / `WheelbarrowLease` 等の削除でリース cleanup
- `spatial/*`: `DamnedSoul` / `ResourceItem` の削除でグリッドから除去
- `obstacle`: `ObstaclePosition` の削除で WorldMap 更新

**注意**: Soul が突然 despawn した場合、`OnTaskAbandoned` は発火せず、予約も自動解放されない。`resource_sync` の次回再構築（0.2秒以内）で整合性が回復する。

## 7. オートホール・自動Gather

詳細は [logistics.md](logistics.md) 参照。

### 7.1 需要起点の自動Gather（Wood / Rock）
`blueprint_auto_gather_system` が `DeliverToBlueprint` / `DeliverToMixerSolid` request の不足を需要起点に、`Tree` / `Rock` へ `Chop` / `Mine` を直付与（`AutoGatherDesignation` marker 付き）。需要解消後に未着手指定は自動回収。

## 8. TaskArea 編集 UI

`Orders -> Area` で `TaskMode::AreaSelection` に入ると TaskArea 連続編集モード。

- 新規: 左ドラッグ矩形 / 直接編集: 既存エリアの内部ドラッグ（移動）・辺/角ドラッグ（リサイズ）
- `Shift+左リリース`: 適用して Normal 復帰 / `Esc`: Normal 復帰
- `Ctrl+Z/Y`: Undo/Redo / `Ctrl+C/V`: コピー/ペースト / `Ctrl+1..3`: プリセット保存 / `Alt+1..3`: プリセット適用

## 9. バイタル（疲労・ストレス・やる気）

詳細は [soul_ai.md](soul_ai.md) 参照。

## 10. UI

詳細は [task_list_ui.md](task_list_ui.md) 参照。
