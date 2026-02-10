# 物流と備蓄システム (Logistics & Stockpile)

Hell-Workers における資源の備蓄、運搬、および管理の仕組みについて解説します。

## 1. 備蓄場所 (Stockpile)

備蓄場所は、採取された資源（木材、石材）を保管する拠点です。Bevy 0.18 の **ECS Relationships** を活用し、アイテムとの関係を厳格に管理しています。

### 主要な Relationship
| コンポーネント | 役割 | 説明 |
| :--- | :--- | :--- |
| **`StoredIn(Entity)`** | **Relationship** | アイテムから**備蓄場所**への参照。 |
| **`StoredItems(Vec)`** | **Target** | 備蓄場所側の**格納アイテム一覧**。自動的に維持される。 |

### 制限事項
- **資源タイプ制限**: 1つの備蓄場には1種類の資源のみ保管可能です。
    - 最初にアイテムが置かれた時点でタイプ（木材 or 石材）が決定されます。
    - アイテムがゼロになるとタイプはリセットされ、別の資源を置けるようになります。
- **容量制限 (10個)**: 1つの備蓄場所の最大容量は **10個** です。
    - **同一フレーム競合の回避**: `SharedResourceCache` の **Intra-frame tracking** により、複数のワーカーが同時に到着しても論理在庫を追跡し、上限を超えないよう制御されます。

## 2. 運搬プロセス (Hauling)

資源が備蓄場所へ運ばれるまでの流れは以下の通りです。

1.  **タスク発行**: `task_area_auto_haul_system` またはプレイヤーが資源に `WorkType::Haul` を指定。
2.  **割り当て**: 使い魔 AI が配下の魂に搬送タスクを割り当てる（`assign_task_to_worker`）。
3.  **エリア制限**: ワーカーは管理している**使い魔の `TaskArea`（担当エリア）内**にある備蓄場所のみを利用します。
4.  **搬送中（インフライト）の考慮**: 割り当て時に「現在搬送目的地となっている数」もカウントし、将来の満杯を予測して割り当てを制限します。
5.  **実行**:
    - **収集フェーズ**: 資源の場所へ移動し、`Holding` 状態にする。
    - **ドロップフェーズ**: 備蓄場所へ移動し、`StoredIn` 関係を構築して置く。
6.  **安全なドロップ**: 到着時に備蓄場所が満杯や消失していた場合、ワーカーはその場に資源を安全にドロップします。

## 3. オートホール (Auto-Haul)

オートホールシステムは、物流を自動化する重要なコンポーネントです。

### 3.1. 資源運搬（木材・石材）
- **トリガー**: 使い魔の担当エリア内に空きのある `Stockpile` が存在すること。
- **スキャン**: 備蓄場所の周辺にある「未指定のアイテム」を検索します。
- **自動指定**: 条件（エリア、型、容量）に合う資源を見つけると、自動的に `Haul` タスクを発行します。
- **優先度**: 通常の備蓄運搬タスクの優先度は **Low (0)** です。

### 3.2. バケツの自動返却
- **トリガー**: バケツ（空または水入り）が地面にドロップされ、紐付いたタンクの「バケツ置き場」に空きがあること。
- **自動指定**: `bucket_auto_haul_system` により、ドロップされたバケツは `BelongsTo` で紐付いたタンクかつ `BucketStorage` マーカー付きストレージへ自動搬送 (`Haul`) されます。
- **優先度**: バケツ返却は物流を止めないために優先度 **Medium-High (5)** で処理されます。

### 3.3. 水汲み指示の自動発行
- **トリガー**: 水タンク内の現在量（＋搬送予約分）が容量未満になり、かつ紐付いたバケツが「バケツ置き場」にあること。
- **自動指定**: `tank_water_request_system` が不足分を計算し、必要な数のバケツに対して `GatherWater` タスクを自動発行します。

### 3.4. MudMixerへの資材・水供給
- **トリガー**: MudMixerの在庫（Sand/Rock/Water）が容量（5個）未満であること。
- **Sand/Rock**:
    - 周辺のアイテムから検索対象を絞り込む。
    - `SharedResourceCache` を使用して、「搬送中 + 予約済み」の数が容量を超えないように厳密に管理する。
    - `MudMixerStorage::can_accept` メソッドにより、リソース種別ごとの受入可否を判定。
- **Water**:
    - **要件**: エリア内に水が入った `Tank`（貯水槽）が存在すること。
    - 川から直接ではなく、**Tankからバケツで水を汲んで** 運びます。
    - Mixer 位置に `DeliverWaterToMixer` の request エンティティを生成。割り当て時にタンク・バケツを遅延解決。
    - 水量はタスク数 × `BUCKET_CAPACITY` で計算されます。
- **Sand採取の発行**:
    - ミキサー近傍（3タイル以内）の `SandPile` に対して `CollectSand` を自動発行します。
    - `BelongsTo` 依存ではなく、近接判定ベースで動作します。
- **満杯時の制御**:
    - ミキサーが満杯になった場合、搬送中のタスクは即座にキャンセルされ、アイテムは適切に処分（Sandは消去、他はドロップ）されます。

## 7. 競合回避システム (Contention Avoidance)

自動発行システムが過剰にタスクを発行しないよう、`SharedResourceCache` と `sync_reservations_system` で予約を一元管理しています。

### 7.1. 予約の再構築
`sync_reservations_system` は **0.2秒間隔（初回即時）** で、以下の2つのソースから予約を再構築します:

1. **`AssignedTask`** - 既にSoulに割り当てられているタスク
2. **`Designation` (Without<TaskWorkers>)** - まだ割り当て待ちのタスク候補

補足:
- 同期タイマーの間隔中でも、`ResourceReservationRequest` による差分更新は `apply_reservation_requests_system` で即時反映されます。

### 7.2. Designation からの予約カウント
`Designation` を持つエンティティは、付随するコンポーネントによって適切な予約にカウントされます:

| WorkType | 条件 | 予約先 |
| :--- | :--- | :--- |
| `Haul` | `TargetBlueprint` あり | `destination_reservations` |
| `HaulToMixer` | `TargetMixer` + `ResourceItem` あり | `mixer_dest_reservations` |
| `HaulWaterToMixer` | `TargetMixer` あり | `mixer_dest_reservations` (Water) |
| `GatherWater` | `BelongsTo` あり | `destination_reservations` |

### 7.3. 自動発行システムのフィルタリング
各自動発行システムは、以下の方法で重複を防いでいます:

- **`Without<Designation>`**: 既にタスクが付与されているアイテムをクエリから除外
  - `task_area_auto_haul_system`, `blueprint_auto_haul_system`, `bucket_auto_haul_system`
- **`SharedResourceCache` 参照**: 予約数を確認して容量超過を防止
  - `mud_mixer_auto_haul_system`, `tank_water_request_system`

### 7.4. `AssignedTask::HaulWithWheelbarrow` の予約

手押し車タスクは複数の予約を同時に必要とするため、フェーズに応じた予約管理を行います:

| 予約対象 | 予約先 | 有効フェーズ |
| :--- | :--- | :--- |
| 手押し車エンティティ | `source_reservations` | 全フェーズ（二重使用防止） |
| 目的地ストックパイル | `destination_reservations` × N個 | 全フェーズ |
| アイテムソース | `source_reservations` | `GoingToParking`, `PickingUpWheelbarrow`, `GoingToSource` のみ |

`Loading` 以降のフェーズではアイテムは既に手押し車に `LoadedIn` されているため、アイテムソースの予約は不要です。

### 7.5. TransportRequest 基盤（計画: グローバル運搬 Request 化）

**観測基盤（M0）**:
- `TransportRequestMetrics`: 種別・状態ごとの request 数を5秒間隔でデバッグログ出力
- `transport_request_anchor_cleanup_system`: アンカー消失時に request を close（standalone は despawn、アイテム付きは TransportRequest/Designation を remove）

**タスク検索**:
- `task_finder` は `DesignationSpatialGrid` と `TransportRequestSpatialGrid` の両方から候補を収集し、重複を除外

**M3 Blueprint 搬入 request 化（完了）**:
- `blueprint_auto_haul_system` は Blueprint 単位で request エンティティをアンカー位置に生成
- アイテムへの直接 Designation 発行を廃止
- 割り当て時に `find_nearest_blueprint_source_item` で資材ソースを遅延解決

| auto_haul システム | 方式 | TransportRequestKind | anchor |
| :--- | :--- | :--- | :--- |
| `blueprint_auto_haul` | **request エンティティ** | DeliverToBlueprint | Blueprint |
| `mud_mixer_auto_haul`（固体） | **request エンティティ** | DeliverToMixerSolid | Mixer |
| `task_area_auto_haul` | **request エンティティ** | DepositToStockpile | Stockpile |
| `bucket_auto_haul` | アイテム直接 | ReturnBucket | Stockpile |
| `mud_mixer_auto_haul`（水） | **request エンティティ** | DeliverWaterToMixer | Mixer |

**M4 TaskArea request 化（完了）**: resource_type 確定済みストックパイルについて、request エンティティを発行。バケツは `bucket_auto_haul` 専用のため除外。

**M5 水搬送 request 化（完了）**: Mixer への水供給を request エンティティ化。`mud_mixer_auto_haul_system` が Mixer 位置に `DeliverWaterToMixer` request を生成。割り当て時に `find_tank_bucket_for_water_mixer` でタンク・バケツを遅延解決。

**M6 手押し車 request 化（完了）**: `DepositToStockpile` request の割り当て時に、`resolve_wheelbarrow_batch_for_stockpile` で手押し車＋積載可能アイテムのバッチを遅延解決。batch 生成・容量制約（`WHEELBARROW_MIN_BATCH_SIZE`、`WHEELBARROW_CAPACITY`、ストックパイル残容量）を request resolver に集約。

**Blueprint / Mixer 固体・水**: request エンティティをアンカー位置に生成し、割り当て時にソースを遅延解決。`TransportRequestSet::Maintain` でアンカー消失時の cleanup を実施。

### 7.6. 拡張性
新しい自動発行システムを追加する場合:
1. `sync_reservations_system` の `match designation.work_type` に新しい `WorkType` を追加
2. 必要なターゲットコンポーネント（例: `TargetFurnace`）をクエリに追加
3. 適切な予約カテゴリにカウントを追加


## 4. 手押し車運搬 (Wheelbarrow Hauling)

手押し車は、複数のアイテム（最大 `WHEELBARROW_CAPACITY=10` 個、混載可）をまとめて運搬する手段です。

### 4.1. 手押し車の管理

- **駐車エリア (`WheelbarrowParking`)**: `BuildingType::WheelbarrowParking` として建設（2x2タイル、Wood x 2）
- **スポーン**: 建設完了時のポストプロセスで `capacity` 分の手押し車エンティティをスポーン
- **関連 Relationship**:
  - `PushedBy(Entity)` — 手押し車 → 使用中の魂
  - `LoadedIn(Entity)` / `LoadedItems(Vec)` — アイテム → 手押し車の積載関係
  - `ParkedAt(Entity)` / `ParkedWheelbarrows(Vec)` — 手押し車 → 駐車エリア

### 4.2. 発行条件（M6: request resolver で遅延解決）

- `DepositToStockpile` request の割り当て時に `resolve_wheelbarrow_batch_for_stockpile` で判定
- ストックパイルへ移動が必要な積載可能アイテムが `WHEELBARROW_MIN_BATCH_SIZE` 個以上
- 駐車エリアに利用可能（未予約）な手押し車がある
- アイテムの `ResourceType` が `is_loadable() == true`（液体・バケツ・手押し車自体は不可）

### 4.3. 速度ペナルティ

手押し車使用中の魂は `SOUL_SPEED_WHEELBARROW_MULTIPLIER` (0.7) の速度補正を受けます。

### 4.4. ビジュアル

手押し車は独立エンティティとして魂の進行方向前方に追従表示されます。
詳細は [gather_haul_visual.md](gather_haul_visual.md) を参照。

## 5. 備蓄からの利用 (Retrieval)

建築など、特定のタスクでは備蓄された資源を再利用します。
- **検索**: 建築現場から最も近い資源が検索されます。これにはストックパイル内のアイテムも含まれます。
- **エリア制限**: 使い魔は、**自分の担当エリア（TaskArea）内にあるストックパイル**からのみ資源を持ち出します。他者の備蓄を勝手に消費することはありません。
- **自動更新**: アイテムが持ち出されると `StoredIn` 関係が解除され、ストックパイル側の在庫リストも自動更新されます。最後の1個がなくなると、ストックパイルの管理リソース型は `None` にリセットされます。

## 6. 座標とレイヤー

- **スナッピング**: 備蓄場所にアイテムを置く際、座標はタイルの中心に正確に補完されます。
- **表示レイヤー (Z-Index)**:
    - 地面: `0.0`
    - ストックパイルタイル: `0.1`
    - **備蓄アイテム: `0.6`**
    - ソウル: `1.0`
