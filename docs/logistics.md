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

### 1.3 物流関連コンポーネント
- `BelongsTo(Entity)`:
  - 所有関係（主にタンクとバケツ/バケツ置き場）
- `BucketStorage`:
  - バケツ返却先として扱う Stockpile マーカー
- `ReservedForTask`:
  - タスクで予約済みのアイテム

### 1.4 TransportRequest
- `TransportRequest { kind, anchor, resource_type, issued_by, priority }`
- `TransportDemand { desired_slots, inflight }`
- `TransportRequestState`:
  - `Pending` / `Claimed` / `InFlight` / `CoolingDown` / `Completed`
- request エンティティには通常 `Designation`, `ManagedBy`, `TaskSlots`, `Priority` も付与されます。

## 2. TransportRequest 基盤

`TransportRequestPlugin` は以下の順で実行されます。

1. `Perceive`
2. `Decide`（各 producer が request を upsert）
3. `Execute`（`TaskWorkers` に応じた state 同期）
4. `Maintain`（アンカー消失や不要 request の cleanup）

`task_finder` は `DesignationSpatialGrid` と `TransportRequestSpatialGrid` の両方を探索して候補を集約します。

## 3. Request 種別と実装

| kind | WorkType | producer | anchor | ソース解決 |
| :--- | :--- | :--- | :--- | :--- |
| `DepositToStockpile` | `Haul` | `task_area_auto_haul_system` | Stockpile | 割り当て時にアイテムを遅延解決 |
| `DeliverToBlueprint` | `Haul` | `blueprint_auto_haul_system` | Blueprint | 割り当て時に必要資材を遅延解決 |
| `DeliverToMixerSolid` | `HaulToMixer` | `mud_mixer_auto_haul_system` | Mixer | 割り当て時に Sand/Rock を遅延解決 |
| `DeliverWaterToMixer` | `HaulWaterToMixer` | `mud_mixer_auto_haul_system` | Mixer | 割り当て時に tank + bucket を遅延解決 |
| `GatherWaterToTank` | `GatherWater` | `tank_water_request_system` | Tank | 割り当て時に bucket を遅延解決 |
| `ReturnBucket` | `Haul` | `bucket_auto_haul_system` | バケツ置き場 Stockpile | 割り当て時に dropped bucket を遅延解決 |
| `BatchWheelbarrow` | `WheelbarrowHaul` | `wheelbarrow_auto_haul_system` | Wheelbarrow | 現状の主運搬経路では未使用（将来拡張用） |

## 4. 自動運搬の仕様

### 4.1 TaskArea -> Stockpile (`DepositToStockpile`)
- 対象は「非 Idle の Familiar」が持つ `TaskArea` 内の Stockpile。
- 需要は `capacity - current - in_flight` で算出。
- `resource_type = None` の空 Stockpile は、近傍の地面アイテムから搬入種別を推定して request を発行。
- 搬入対象は `ResourceType::is_loadable() == true` の資材のみ（液体/バケツ/手押し車は除外）。
- 割り当て時のソース選定は「地面アイテムのみ」。
  - 既に `InStockpile` のアイテムは対象外。
  - 同一 Stockpile での pick-drop ループを防止。

### 4.2 Blueprint 搬入 (`DeliverToBlueprint`)
- `required_materials - delivered_materials - in_flight` を不足分として request 化。
- request は Blueprint 位置に生成し、ソースは割り当て時に探索。

### 4.3 MudMixer 固体搬入 (`DeliverToMixerSolid`)
- `Sand` / `Rock` の不足量を `SharedResourceCache` を含めて判定。
- request は Mixer 位置に生成し、ソースは割り当て時に探索。

### 4.4 MudMixer 水搬入 (`DeliverWaterToMixer`)
- 水不足時に request を発行。
- 割り当て時に、エリア内の有効タンクと利用可能バケツを遅延解決して搬送。

### 4.5 バケツ返却 (`ReturnBucket`)
- dropped bucket（空/水入り）を検知。
- `BelongsTo` で紐づくタンクの `BucketStorage` 付き Stockpile へ返却 request を生成。

### 4.6 Tank 自動補充 (`GatherWaterToTank`)
- 水タンクの不足量を監視し、`BUCKET_CAPACITY` 単位で必要タスク数を算出して request 化。
- 割り当て時に request anchor（tank）に紐づく利用可能バケツを選択して `GatherWater` を実行。
- タンク容量（現在量 + 予約）を割り当て時にも再検証。

## 5. 手押し車運搬

手押し車の実運用は `DepositToStockpile` の割り当て時に判定されます。

- `resolve_wheelbarrow_batch_for_stockpile` が以下を満たすと一括運搬を選択:
  - 利用可能な手押し車あり
  - 同種アイテムが `WHEELBARROW_MIN_BATCH_SIZE` 以上
  - 目的 Stockpile の残容量あり
- 上限:
  - `WHEELBARROW_CAPACITY`
  - Stockpile 残容量
- 対象アイテムは地面上のみ（`InStockpile` は除外）。

## 6. 予約と競合回避

`SharedResourceCache` で以下を一元管理します。

- `destination_reservations`（Stockpile/Tank など）
- `mixer_dest_reservations`（Mixer + ResourceType）
- `source_reservations`（アイテムやタンク）

### 6.1 再構築
- `sync_reservations_system` が `AssignedTask` と未割り当て `Designation` から予約を再構築。
- 同期間隔は `RESERVATION_SYNC_INTERVAL`（初回は即時）。

### 6.2 差分適用
- `ResourceReservationRequest` を `apply_reservation_requests_system` でフレーム内反映。
- `RecordStoredDestination` / `RecordPickedSource` によりフレーム内の論理在庫差分も追跡。

### 6.3 水搬送の排他
- `HaulWaterToMixer` はタンクを source 予約して同時取水競合を抑制。

## 7. 備蓄資材の取り出し

- 建築/製造向けの搬送ソースには、条件を満たす `InStockpile` アイテムも利用されます。
- アイテムを持ち出すと `StoredIn`/`InStockpile` が外れ、`StoredItems` は自動更新されます。
- 取り出し後に Stockpile が空になると `resource_type` は `None` に戻ります。

## 8. 関連実装

- request producer:
  - `src/systems/logistics/transport_request/producer/`
- request plugin:
  - `src/systems/logistics/transport_request/plugin.rs`
- request lifecycle:
  - `src/systems/logistics/transport_request/lifecycle.rs`
- 割り当てロジック:
  - `src/systems/familiar_ai/decide/task_management/assignment/`
- 実行ロジック:
  - `src/systems/soul_ai/execute/task_execution/`

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
- 予約は「割り当て時」に `ResourceReservationOp` で付与し、成功・失敗・中断の全経路で解放/記録する。
- タスク実行で取得・格納が成功したら、`RecordPickedSource` / `RecordStoredDestination` を使う。
- 共有ソース（例: tank 取水）は `ReserveSource` で排他を取る。

### 9.4 ソース選定の安全条件
- `DepositToStockpile` のソースは地面アイテムのみを対象にする（`InStockpile` は除外）。
- 所有物資がある場合は `BelongsTo` を一致させ、他 owner の資材を混在させない。
- `Visibility::Hidden` / `ReservedForTask` / `TaskWorkers` 付きエンティティは候補から外す。

### 9.5 追加時に必ず更新する箇所
- 新しい producer を `TransportRequestPlugin`（`Decide`）へ登録する。
- 新しい `WorkType` / request 種別を導入した場合:
  - `task_finder/filter.rs`（有効タスク判定）
  - `assignment/policy/mod.rs`（割り当て分岐）
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
