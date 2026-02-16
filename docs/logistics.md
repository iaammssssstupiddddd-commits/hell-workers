# 物流と備蓄システム (Logistics & Stockpile)

Hell-Workers の物流は、`TransportRequest` を中心にした自動発行 + 遅延解決方式で動作します。  
このドキュメントは「現在の仕様」と「実装上の挙動」に絞って記載します。

## 1. 主要データモデル

### 1.1 Stockpile
- コンポーネント: `Stockpile { capacity, resource_type }`
- 初期配置（ゾーン配置）時:
  - `capacity = 10`
  - `resource_type = None`
- `resource_type` は最初の格納で確定し、最後の1個が取り出されると `None` に戻ります。

### 1.2 Relationship
- `StoredIn(Entity)`:
  - アイテム -> 格納先 Stockpile
- `StoredItems(Vec<Entity>)`:
  - Stockpile 側の逆参照
- `InStockpile(Entity)`:
  - 「備蓄中」判定用マーカー
- `DeliveringTo(Entity)`:
  - アイテム -> 搬入先（Stockpile / Blueprint / Mixer 等）への予約を示す Relationship
  - タスク割り当て時に自動挿入され、タスク完了・中断時に自動除去される
- `IncomingDeliveries(Vec<Entity>)`:
  - 搬入先側に Bevy が自動維持する RelationshipTarget
  - `IncomingDeliveries.len()` で「搬入予約済みアイテム数」を取得できる

### 1.3 物流関連コンポーネント
- `BelongsTo(Entity)`:
  - 所有関係（主にタンクとバケツ/バケツ置き場）
- `BucketStorage`:
  - バケツ返却先として扱う Stockpile マーカー
- `ReservedForTask`:
  - タスクで予約済みのアイテム

### 1.4 アイテムの寿命 (Item Lifetime)
- 特定のアイテム（**StasisMud**, **Sand**）は、地面にドロップされた状態で放置されると **5秒後** に消滅します。
- **消滅しない条件**:
  - `ReservedForTask`: タスク用に予約されている
  - `LoadedIn(Entity)`: 手押し車などに積載されている
  - `StoredIn(Entity)`: Stockpile に格納されている
  - `DeliveringTo(Entity)`: 搬送中（リレーションシップあり）
- これにより、運搬されずに放置された余剰な中間素材が自動的にクリーンアップされます。

### 1.5 TransportRequest
- `TransportRequest { kind, anchor, resource_type, issued_by, priority }`
- `TransportDemand { desired_slots, inflight }`
- `TransportRequestState`:
  - `Pending` / `Claimed` / `InFlight` / `CoolingDown` / `Completed`
- request エンティティには通常 `Designation`, `ManagedBy`, `TaskSlots`, `Priority` も付与されます。

## 2. TransportRequest 基盤

`TransportRequestPlugin` は以下の順で実行されます。

1. `Perceive`（メトリクス集計）
2. `Decide`（各 producer が request を upsert）
3. `Arbitrate`（手押し車仲裁 — 後述 §5.2）
4. `Execute`（`TaskWorkers` に応じた state 同期）
5. `Maintain`（アンカー消失や不要 request の cleanup）

`task_finder` は `DesignationSpatialGrid` と `TransportRequestSpatialGrid` の両方を探索して候補を集約します。

## 3. Request 種別と実装

| kind | WorkType | producer | anchor | ソース解決 |
| :--- | :--- | :--- | :--- | :--- |
| `DepositToStockpile` | `Haul` | `task_area_auto_haul_system` | Stockpile | 割り当て時にアイテムを遅延解決 |
| `DeliverToBlueprint` | `Haul` | `blueprint_auto_haul_system` | Blueprint | 割り当て時に必要資材を遅延解決 |
| `DeliverToMixerSolid` | `HaulToMixer` | `mud_mixer_auto_haul_system` | Mixer | 割り当て時に Sand/Rock を遅延解決（Sand は原則猫車必須、近接ピックドロップ完結時は徒歩許可） |
| `DeliverToFloorConstruction` | `Haul` | `floor_construction_auto_haul_system` | FloorConstructionSite | 割り当て時に Bone / StasisMud ソースを遅延解決（搬入先は `site.material_center`） |
| `DeliverToProvisionalWall` | `Haul` | `provisional_wall_auto_haul_system` | Wall (Building) | 割り当て時に StasisMud ソースを遅延解決（搬入先は壁足元） |
| `DeliverWaterToMixer` | `HaulWaterToMixer` | `mud_mixer_auto_haul_system` | Mixer | 割り当て時に tank + bucket を遅延解決 |
| `GatherWaterToTank` | `GatherWater` | `tank_water_request_system` | Tank | 割り当て時に bucket を遅延解決 |
| `ReturnBucket` | `Haul` | `bucket_auto_haul_system` | Tank | 割り当て時に dropped bucket と返却先 BucketStorage を同時遅延解決 |
| `BatchWheelbarrow` | `WheelbarrowHaul` | `wheelbarrow_auto_haul_system` | Wheelbarrow | 現状の主運搬経路では未使用（将来拡張用） |
| `ConsolidateStockpile` | `Haul` | `stockpile_consolidation_producer_system` | Stockpile（レシーバーセル） | 割り当て時にドナーセルの InStockpile アイテムを遅延解決 |

## 4. 自動運搬の仕様

### 4.1 TaskArea -> Stockpile (`DepositToStockpile`)
- **グループ単位の発行**:
  - **ファミリア (Active Familiars) 単位**で、それぞれの `TaskArea` 内にある Stockpile をひとつのグループとして構成します。
  - `TransportRequest` は**各ファミリアのグループごとに別個に発行**されます（`anchor` = 代表セル, `issued_by` = ファミリア）。
  - **共有セルの扱い**: 複数の `TaskArea` が重複する場合、その領域内の Stockpile はそれぞれのファミリアのグループに含まれます。
    - 結果として、同一セルに対する搬入リクエストが複数ファミリアから並行して存在する可能性があります。
    - **競合回避**: 実際の搬入（Assign時）には `IncomingDeliveries.len()` で搬入予約済み数を確認するため、同一セルへの容量超過は発生しません。
- **需要計算**:
  - グループ全体の `total_capacity - total_stored - total_in_flight` で算出。
- **収集対象範囲**:
  - **TaskArea 外周から 10 タイル以内**。
  - ただし、TaskArea 外側の「外周+10」領域では、**他 TaskArea 内**の位置を除外します。
  - 複数グループの範囲に入るアイテムは、最寄りグループ（TaskArea外周距離）に排他的に割り当てられます。
- **搬入・ソース選定**:
  - request の `resource_type` は、収集範囲内の近傍フリーアイテムから推定します。
  - producer 内部では、グループ受入可否（空き容量/固定型）を前計算し、`q_free_items` を **1回だけ走査**して代表型を決定します。
  - ただし、グループ内に「空き容量あり」かつ「`resource_type = None` または対象型と一致」のセルが1つ以上ある型のみ候補になります。
  - 搬入対象は `ResourceType::is_loadable() == true` の資材のみ。
  - 割り当て時にグループ内の**型互換かつ空き容量があるセル**を動的に決定して搬入します。
  - ソースは「地面アイテムのみ」（`InStockpile` 除外）で、同一 Stockpile での pick-drop ループを防止します。

### 4.2 Blueprint 搬入 (`DeliverToBlueprint`)
- `required_materials - delivered_materials - in_flight` を不足分として request 化。
- request は Blueprint 位置に生成し、ソースは割り当て時に探索。
- `Sand` 搬入は、`CollectSand` の別タスクを経由せず **同一 Soul の `HaulWithWheelbarrow` 1タスク内で完結**する。
  - ソース探索順: `SandPile` 優先、見つからない場合は `TerrainType::Sand` タイル。
  - 範囲: まず TaskArea 内を探索し、見つからなければ全体探索にフォールバック。
  - 積込: `Loading` フェーズで砂アイテムをその場生成し、1回で `min(不足量, WHEELBARROW_CAPACITY)` を猫車に積載。
  - ソース（砂置き場/砂タイル）は消費しない（無限ソース）。
  - 過剰割り当て防止のため、割り当て時に「必要量 - 予約済み」を再計算して積載量を決定する。

### 4.3 MudMixer 固体搬入 (`DeliverToMixerSolid`)
- `Sand` / `Rock` の不足量を `SharedResourceCache` を含めて判定。
- request は Mixer 位置に生成し、ソースは割り当て時に探索。

### 4.4 MudMixer 水搬入 (`DeliverWaterToMixer`)
- 水不足時に request を発行。
- 割り当て時に、エリア内の有効タンクと利用可能バケツを遅延解決して搬送。

### 4.5 バケツ返却 (`ReturnBucket`)
- 返却対象は「地面上のバケツ（`BucketEmpty` / `BucketWater`、`StoredIn` なし）」のみ。
- request は **タンクごとに最大1件**（`anchor = tank`）を維持する。
- 需要算出:
  - `dropped_buckets`: owner 一致の地面バケツ数
  - `free_slots_total`: owner 一致の `BucketStorage` 空き合計（予約込み）
  - `desired_slots = min(dropped_buckets, free_slots_total)`
- `desired_slots == 0` のときは request を休止（`Designation` / `TaskSlots` / `Priority` を remove）。
- 割り当て時に source と destination を同時解決:
  - source: owner 一致の dropped bucket（未予約）
  - destination: owner 一致かつ容量ありの `BucketStorage`（source から最短）
- 実行フェーズ（Dropping）で `BucketStorage` 専用ガードを適用:
  - バケツ型のみ
  - owner 一致
  - 予約込み容量チェック

### 4.6 ストックパイル統合 (`ConsolidateStockpile`)
- **概要**: 
  - **Soul** が行う `Haul` タスクの一種です。
  - グループ内で同種の資材が複数セルに分散している場合、それらを少数のセルに集約し、空きセル（`None` 状態の Stockpile）を確保することを目的とします。
- **発動条件**:
  - 同一グループ内に同じ資源タイプが2セル以上に分散していること。
  - 移動によって **少なくとも1つのセルが完全に空になる** 見込みがあること。
  - **並行動作**: 新規搬入タスク（`DepositToStockpile`）が存在する場合でも並行してリクエストが発行されます。優先度システムと予約システムにより、各 Soul は最適なタスクを選択します。
- **優先度**: 
  - `TransportPriority::Low`
  - 建築や製造への搬入などの高優先度タスクがない場合に実行されます。
- **統合ロジック (Greedy)**:
  - **Receiver（搬入先）**: グループ内でその資源を格納しているセルのうち、**満杯でないセル**（`stored < capacity`）の中から最も多くアイテムが入っているセルを選択します。ここを満杯にすることを目指します。
  - **Donor（搬出元）**: 受信先に選ばれたセル以外のすべてのセル。格納数が少ないセルから順に搬出元として選ばれます。
  - `anchor` = Receiver セル, `stockpile_group` = Donor セル一覧 としてリクエストが発行されます。
- **全セル満杯時の挙動**: グループ内のすべてのセルが満杯の場合は、統合の余地がないためリクエストは発行されません。
- **ソース選定と実行**:
  - 割り当て時に、Donor セルの `InStockpile` アイテムから未予約のものを選択します。
  - 既存の `Haul` タスクロジックを再利用して実行されます。アイテムを持ち出すと `StoredIn`/`InStockpile` が外れ、Receiver に格納されると再付与されます。

### 4.7 Tank 自動補充 (`GatherWaterToTank`)
- 水タンクの不足量を監視し、`BUCKET_CAPACITY` 単位で必要タスク数を算出して request 化。
- 割り当て時に request anchor（tank）に紐づく利用可能バケツを選択して `GatherWater` を実行。
- タンク容量（現在量 + 予約）を割り当て時にも再検証。

### 4.8 床建築搬入 (`DeliverToFloorConstruction`)
- `floor_construction_auto_haul_system` が site ごとに不足資材を算出し request を upsert。
- Reinforcing フェーズでは `Bone`、Pouring フェーズでは `StasisMud` を要求。
- 搬入先は常に `FloorConstructionSite.material_center`。
- `floor_material_delivery_sync_system` が `material_center` 周辺の資材を消費し、各タイルの `bones_delivered` / `mud_delivered` を進める。
- `Bone` は以下の優先順で解決される:
  1. 地面アイテムを通常 `Haul` で搬送
  2. 地面アイテムがない場合は `BonePile` / River からの猫車直採取へフォールバック

### 4.9 仮設壁搬入 (`DeliverToProvisionalWall`)
- `provisional_wall_auto_haul_system` が `BuildingType::Wall && is_provisional` の壁を走査し、`ProvisionalWall.mud_delivered == false` の壁に request を upsert。
- request の anchor は壁エンティティで、割り当て時に `StasisMud` ソースを遅延解決する。
- `provisional_wall_material_delivery_sync_system` が壁近傍へ落ちた `StasisMud` を消費して `mud_delivered = true` に更新する。
- `provisional_wall_designation_system` が準備完了した壁へ `WorkType::CoatWall` を付与し、塗布タスクへ遷移させる。

## 5. 手押し車運搬

### 5.1 基本動作

手押し車の実運用は request 割り当て時に判定されます。

- `Sand` / `StasisMud` は原則猫車必須資源（徒歩フォールバックなし）。
  - `Bone` は「地面アイテム搬送」では徒歩 `Haul` を許可。
  - ただし `BonePile` / River からの直接採取ルートでは猫車を使用。
  - 例外: 「その場ピック→ドロップ」で1件を完了できる距離関係なら徒歩運搬を許可し、猫車を使わない。
  - 判定は `source` 周囲 3x3 の立ち位置を評価して行う（実行時の距離しきい値に合わせる）。
  - Stockpile / Mixer へのドロップ成立条件: `distance(stand_pos, destination_pos) < TILE_SIZE * 1.8`
  - Blueprint へのドロップ成立条件: `stand_pos` が `occupied_grids` 外、かつ `distance(stand_pos, occupied_tile) < TILE_SIZE * 1.5`
  - Blueprint への `Sand` 搬入では「直採取モード」を使用し、同一 Soul が採取と運搬を連続実行する。
- 対象搬送先:
  - `DepositToStockpile`
  - `DeliverToBlueprint`（`Sand` / `StasisMud`）
  - `DeliverToMixerSolid`（`Sand`）
  - `DeliverToFloorConstruction`（`StasisMud` / 直採取 `Bone`）
- 猫車不足時は request は `Pending` のまま待機する。

- `resolve_wheelbarrow_batch_for_stockpile` が以下を満たすと一括運搬を選択:
  - 利用可能な手押し車あり
  - 同種アイテムが `WHEELBARROW_MIN_BATCH_SIZE` 以上
  - 目的 Stockpile の残容量あり
- 上限:
  - `WHEELBARROW_CAPACITY`
  - Stockpile 残容量
- 対象アイテムは地面上のみ（`InStockpile` は除外）。

### 5.2 手押し車仲裁システム (Wheelbarrow Arbitration)

複数の `DepositToStockpile` request が同時に手押し車を必要とする場合に、
全体最適に近い割り当てを一括決定する仲裁フェーズ。
`TransportRequestSet::Arbitrate`（`Decide` → `Execute` 間）で毎フレーム実行される。

#### 5.2.1 WheelbarrowLease コンポーネント

仲裁結果は request エンティティに `WheelbarrowLease` として付与される。

```
WheelbarrowLease {
    wheelbarrow: Entity,     // 割り当てられた手押し車
    items: Vec<Entity>,      // 積載対象アイテム群
    source_pos: Vec2,        // アイテム重心（積み込み地点）
    destination: WheelbarrowDestination, // 搬送先（Stockpile/Blueprint/Mixer）
    lease_until: f64,        // 有効期限（ゲーム時刻）
}
```

#### 5.2.2 仲裁アルゴリズム

1. **期限切れ lease の除去** — `lease_until < now` の lease を remove。
2. **使用中 wheelbarrow の収集** — 有効な lease が付いている wheelbarrow を除外リストに追加。
3. **eligible request の抽出** — 以下すべてを満たす request:
   - `kind` が仲裁対象（`DepositToStockpile` / 猫車必須資源の `DeliverToBlueprint` / 猫車必須資源の `DeliverToMixerSolid`）
   - `state == Pending`（ワーカー未割当）
   - lease なし
   - `resource_type.is_loadable()`
4. **free item 前処理（1回走査）**:
   - `q_free_items` を1回だけ走査して `FreeItemSnapshot` を作成
   - 以下のバケットを構築:
     - `by_resource`
     - `by_resource_owner_ground`
5. **バッチ候補の評価** — 各 eligible request に対して:
   - request 種別に応じて対応バケットのみ参照（全 free item の再走査はしない）
   - 半径 `TILE_SIZE * 10.0` 内で近傍 `Top-K`（`WHEELBARROW_ARBITRATION_TOP_K`）を抽出
   - `DepositToStockpile` では「型互換セルのみ」で残容量を計算
   - 猫車必須資源は、`Top-K` 候補に対してピックドロップ完結可能判定を行い、成立時は仲裁候補から除外
   - 最小バッチ条件: 猫車必須資源は `1`、それ以外は `WHEELBARROW_MIN_BATCH_SIZE`
6. **スコア計算** — `score = batch_size * SCORE_BATCH_SIZE + priority * SCORE_PRIORITY - distance * SCORE_DISTANCE`
   - `distance` = 最近の wheelbarrow からアイテム重心までの距離
   - 小バッチ（1〜2個）には減点を適用
7. **Greedy 割り当て** — スコア降順にソートし、各 request に最近の available wheelbarrow を割り当て。
   - 割り当て済みの wheelbarrow は available set から除去。
8. **小バッチ抑制** — 猫車必須資源で `batch_size < WHEELBARROW_PREFERRED_MIN_BATCH_SIZE` の場合:
   - `pending_since` から `SINGLE_BATCH_WAIT_SECS` 経過前は候補から除外
   - 経過後にのみ割り当て可能

#### 5.2.3 定数

| 定数 | 値 | 用途 |
| :--- | :--- | :--- |
| `WHEELBARROW_PREFERRED_MIN_BATCH_SIZE` | 3 | 猫車必須資源で優先する最小バッチ |
| `SINGLE_BATCH_WAIT_SECS` | 5.0 | 1〜2個搬送を許可する待機時間 |
| `WHEELBARROW_LEASE_DURATION_SECS` | 30.0 | lease 有効期間（秒） |
| `WHEELBARROW_SCORE_BATCH_SIZE` | 10.0 | スコア: バッチサイズの重み |
| `WHEELBARROW_SCORE_PRIORITY` | 5.0 | スコア: 優先度の重み |
| `WHEELBARROW_SCORE_DISTANCE` | 0.1 | スコア: 距離のペナルティ重み |
| `WHEELBARROW_SCORE_SMALL_BATCH_PENALTY` | 20.0 | 小バッチ減点 |
| `WHEELBARROW_ARBITRATION_TOP_K` | 24 | request ごとに評価する近傍候補上限 |

#### 5.2.4 割り当て時の lease 優先

`assign_haul` / `assign_haul_to_blueprint` / `assign_haul_to_mixer` は以下の順で手押し車を解決する:

1. **WheelbarrowLease あり** → lease の有効性を検証（wheelbarrow が parked か、items が未予約か）し、有効なら即採用。
2. **猫車必須資源かつ lease なし** → 原則待機（徒歩フォールバックなし）。
   - ただし、ピックドロップ完結可能な request は徒歩運搬を優先。
3. **猫車任意資源** → 既存の `resolve_wheelbarrow_batch_for_stockpile` または単品運搬へフォールバック。

#### 5.2.5 lease のライフサイクル

- **付与**: 仲裁システム（Arbitrate フェーズ）が `WheelbarrowLease` を insert。
- **消費**: `assign_haul` が lease を読み取り `HaulWithWheelbarrow` タスクを発行。割り当て後は request の state が Pending でなくなるため、次フレームの仲裁で自動的に対象外。
- **期限切れ**: 仲裁システムが毎フレーム `lease_until < now` をチェックして remove。
- **request close**: `transport_request_anchor_cleanup_system` が request を閉じる際に `WheelbarrowLease` も除去。

#### 5.2.6 メトリクス

`TransportRequestMetrics` に以下が追加:

- `wheelbarrow_leases_active` — アクティブな lease 数
- `wheelbarrow_leases_granted_this_frame` — そのフレームで新規付与された lease 数
- `wheelbarrow_arb_eligible_requests` — 仲裁対象として評価した request 数
- `wheelbarrow_arb_bucket_items_total` — request が参照したバケット候補数（Top-K 前）
- `wheelbarrow_arb_candidates_after_topk` — Top-K 抽出後に残った候補数
- `wheelbarrow_arb_elapsed_ms` — 仲裁システム実行時間（ms）
- `task_area_groups` — TaskArea producer が評価したグループ数
- `task_area_free_items_scanned` — TaskArea producer が走査した free item 数
- `task_area_items_matched` — TaskArea producer で条件一致した item 数
- `task_area_elapsed_ms` — TaskArea producer 実行時間（ms）

5秒間隔のデバッグログに `wb_leases` / `wb_arb(...)` / `task_area(...)` として出力される。

## 6. 予約と競合回避

予約には **搬入先予約（Relationship）** と **ソース/ミキサー予約（SharedResourceCache）** の2種類があります。

### 6.1 搬入先予約（Relationship ベース）

Stockpile / Blueprint / Tank などへの搬入予約は、Bevy の Relationship で管理します。

- タスク割り当て時に、搬入対象アイテムに `DeliveringTo(destination)` を自動挿入（`apply_task_assignment_requests_system`）。
- 搬入先エンティティには Bevy が `IncomingDeliveries` を自動維持。
- **容量判定**: `現在量 (StoredItems.len()) + 搬入予約 (IncomingDeliveries.len()) < capacity` で空き容量を確認。
- タスク完了・中断時にアイテムの `DeliveringTo` を除去すると、搬入先の `IncomingDeliveries` も自動更新される。
- HashMap による再構築が不要なため、**常に最新の予約状態**が ECS から直接取得可能。

### 6.2 ソース／ミキサー予約（SharedResourceCache）

`SharedResourceCache` で以下を管理します。

- `mixer_dest_reservations`（Mixer + ResourceType）
- `source_reservations`（アイテムやタンク）

#### 再構築
- `sync_reservations_system` が `AssignedTask` と未割り当て request（`Designation` + `TransportRequest`）から予約を再構築。
- 同期間隔は `RESERVATION_SYNC_INTERVAL`（初回は即時）。

#### 差分適用
- `ResourceReservationRequest` を `apply_reservation_requests_system` でフレーム内反映。
- `RecordPickedSource` によりフレーム内のソース論理在庫差分も追跡。

### 6.3 水搬送の排他
- `HaulWaterToMixer` はタンクを source 予約して同時取水競合を抑制。

## 7. 備蓄資材の取り出し

- 建築/製造向けの搬送ソースには、条件を満たす `InStockpile` アイテムも利用されます。
- アイテムを持ち出すと `StoredIn`/`InStockpile` が外れ、`StoredItems` は自動更新されます。
- 取り出し後に Stockpile が空になると `resource_type` は `None` に戻ります。

## 8. 関連実装

- request producer:
  - `src/systems/logistics/transport_request/producer/`
- 手押し車仲裁:
  - `src/systems/logistics/transport_request/arbitration/`（mod, candidates, grants, types）
- request plugin:
  - `src/systems/logistics/transport_request/plugin.rs`
- request lifecycle:
  - `src/systems/logistics/transport_request/lifecycle.rs`
- 割り当てロジック:
  - `src/systems/familiar_ai/decide/task_management/`（builders, policy, validator）
  - `task_management/policy/haul/`（blueprint, consolidation, stockpile, source_selector, lease_validation, wheelbarrow）: 運搬割り当ての責務分割
- 実行ロジック:
  - `src/systems/soul_ai/execute/task_execution/`（haul, haul_to_mixer, haul_to_blueprint, haul_with_wheelbarrow, haul_water_to_mixer 等）
  - `task_execution/handler/`（task_handler, impls, dispatch）: TaskHandler トレイトとディスパッチ
  - `task_execution/transport_common/`（reservation, cancel, lifecycle, wheelbarrow）: 予約解放・中断・予約寿命定義・手押し車駐車の共通API

## 9. システム追加時の実装ルール

物流系システムを追加する際は、以下を満たしてください。

### 9.1 Request 発行方式の統一
- 新しい自動物流は、原則として「anchor に紐づく `TransportRequest` エンティティ」を発行する。
- アイテム実体への直接 `Designation` 連打は避け、ソースは割り当て時に遅延解決する。
- request の `kind` / `work_type` / `anchor` / `resource_type` の組み合わせは必ず一貫させる。

### 9.2 Producer の upsert/cleanup 規約
- 既存 request があれば再利用（upsert）し、不要時は以下で閉じる。
  - `TaskWorkers == 0` のときは `Designation` / `TaskSlots` / `Priority` を外す、または despawn。
- 同一 key（anchor + resource_type など）の重複 request は許可しない。
- demand 計算は `current + in_flight(+ reservation)` を使い、過剰発行を防ぐ。

### 9.3 予約の責務を統一
- **搬入先予約**: タスク割り当て時に `DeliveringTo` が自動挿入される。手動で `ResourceReservationOp` を発行する必要はない。
- **ソース予約**: 「割り当て時」に `ResourceReservationOp::ReserveSource` で付与し、成功・失敗・中断の全経路で解放する。
- タスク実行でソース取得が成功したら `RecordPickedSource` を使う。
- 共有ソース（例: tank 取水）は `ReserveSource` で排他を取る。

### 9.4 ソース選定の安全条件
- `DepositToStockpile` のソースは地面アイテムのみを対象にする（`InStockpile` は除外）。
- 所有物資がある場合は `BelongsTo` を一致させ、他 owner の資材を混在させない。
- `Visibility::Hidden` / `ReservedForTask` / `TaskWorkers` 付きエンティティは候補から外す。

### 9.5 追加時に必ず更新する箇所
- 新しい producer を `TransportRequestPlugin`（`Decide`）へ登録する。
- 新しい `WorkType` / request 種別を導入した場合:
  - `task_finder/filter.rs`（有効タスク判定）
  - `policy/mod.rs`（割り当て分岐、`task_management` 配下。運搬は `policy/haul/`：mod.rs がディスパッチ、blueprint.rs / stockpile.rs が destination 別ロジック）
  - `sync_reservations_system`（予約再構築）
  - 必要なら `task_finder/score.rs`（優先度）
  - `transport_request_anchor_cleanup_system` で cleanup 要件を満たすこと

### 9.6 動作確認の最低ライン
- `cargo check` を通す。
- 少なくとも以下を確認する:
  - request が1フレームで増殖しない
  - 同一ソースへの二重割り当てがない
  - 需要 0 時に request が休止/消滅する
  - anchor 消失時に request が cleanup される
