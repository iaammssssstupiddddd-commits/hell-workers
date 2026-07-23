# 物流と備蓄システム (Logistics & Stockpile)

Hell-Workers の物流は、`TransportRequest` を中心にした自動発行 + 遅延解決方式で動作します。  
このドキュメントは「現在の仕様」と「実装上の挙動」に絞って記載します。

## 1. 主要データモデル

### 1.1 Stockpile
- コンポーネント: `Stockpile { capacity, resource_type }`
- 通常の Yard-owned セルには、現在内容とは独立した永続設定
  `StockpilePolicy { acceptance, inbound_priority, target_amount, allow_export }` も付与する。
  `StockpilePolicy` の存在が player-managed な通常セルの明示境界であり、Tank、Mud Mixer、
  `BucketStorage` など `Stockpile` を容量表現として再利用する特殊設備には付与しない。
- 初期配置（ゾーン配置）時:
  - `capacity = 10`
  - `resource_type = None`
  - policy は `Any`、`Normal`、`target_amount = capacity`、`allow_export = true`
- `resource_type` は最初の格納で確定し、最後の1個が取り出されると `None` に戻ります。
- `target_amount` は常に `0..=capacity` に正規化する。0 は新規搬入を許可しない設定であり、
  既存在庫を削除しない。
- `evaluate_stockpile_policy` は `NewInbound` / `CommittedInbound` / `NewOutbound` と、stored、
  incoming reservation、同一 cycle の reservation shadow を受け取る副作用のない共通判定である。
  committed 搬入は自分が所有する予約を識別し、変更後の acceptance / target を grandfather する。
  自動 producer、manual destination、wheelbarrow の候補化と grant 直前、Familiar の割当直前 resolver は
  `NewInbound` を使う。通常 haul と wheelbarrow の荷下ろしは、実 item の `DeliveringTo` が同じ destination を
  指す場合だけその item の予約を所有する `CommittedInbound` として扱う。通常セルでは owner 互換性も再確認し、
  policy を持たない特殊 storage は既存専用規則を維持する。

#### Policy editing

- `StockpilePolicy` を持つ通常セルは、格納済み `ResourceItem` と同じ座標でもセル側を優先して選択・検査できる。
  Tank、Mud Mixer、`BucketStorage` など policy を持たない特殊設備には editor を表示しない。
- Info Panel は `Accepting` / `TargetReached` / `Draining`、現在量、搬入予約量、物理容量、現在資源、
  acceptance、target、搬入優先度、搬出許可を live snapshot から表示する。`Draining` 中は
  `allow_export = false` でも実効搬出が許可されることを明示する。
- 単一セルでは acceptance、target、priority、export を部分 patch として変更する。`Apply Policy to Area` は
  表示中セルの4設定を1つの patch に固定し、次の矩形ドラッグへ一括適用する。
- widget は component を直接変更しない。単一・範囲の両操作は
  `UiIntent::ApplyStockpilePolicy` → `StockpilePolicyChangeRequest` → domain handler →
  `StockpilePolicyChangeOutcome` を通る。範囲対象は空間順に安定化・重複除去し、handler が生存と
  `Stockpile + StockpilePolicy` 境界を再検証する。
- handler はセルごとの物理容量へ target を clamp し、同値なら書き戻さない。在庫と
  `StoredIn` / `DeliveringTo` / `IncomingDeliveries` は変更せず、stale・特殊設備・clamp を件数だけの
  player-safe toast で報告する。

### 1.2 Relationship（Bevy 自動維持）

Source 側のみ手動操作し、Target 側は Bevy が自動更新する（tasks.md §2.1 参照）。

| Source（手動操作）| Target（Bevy自動）| 書き込み元 | 削除元 |
|:---|:---|:---|:---|
| `StoredIn(stockpile)` ← item | `StoredItems` ← stockpile | Haul dropping フェーズ | haul picking フェーズ（ConsolidateStockpile 含む）|
| `DeliveringTo(dest)` ← item | `IncomingDeliveries` ← dest | `apply_task_assignment_requests`（Execute）| `unassign_task` / タスク完了（tasks.md §2.1）|

**容量判定**: 通常 Stockpile の新規搬入は `evaluate_stockpile_policy(NewInbound)` を使い、物理容量、
目標量、資源互換性、`IncomingDeliveries`、同一 cycle shadow を同時に評価する（詳細 §6.1）。

### 1.3 物流関連コンポーネント（手動管理）

| コンポーネント | 書き込み元 | 読み取り元 | 非自明な挙動 |
|:---|:---|:---|:---|
| `WheelbarrowLease` ← TransportRequest | 仲裁システム（Arbitrate）| `assign_haul*` | 毎フレーム: 期限切れ・車消失・item 不足で自動 remove。消費後 request state が Claimed になるため次フレームの仲裁から自動除外 |
| `BelongsTo(owner)` | entity spawn 時 | producer / `assign_haul*` | タンク・バケツ・バケツ返却先の所有判定。`issued_by` と照合してソース選定を制御 |
| `BucketStorage` | entity spawn 時 | `bucket_auto_haul_system` | バケツ返却先マーカー。`BelongsTo` 一致チェックと組み合わせて判定 |
| `ReceiverPolicyTier` ← policy-driven request | `task_area_auto_haul_system` / `stockpile_consolidation_producer_system` | Familiar task ranking / wheelbarrow arbitration | 保存しない runtime carrier。`TransportRequest.priority` の maintenance/manual 意味と receiver policy tier を分離し、producer が load 後に再導出する |

### 1.4 アイテムの寿命 (Item Lifetime)
- 特定のアイテム（**StasisMud**, **Sand**）は、地面にドロップされた状態で放置されると **5秒後** に消滅します。
- **消滅しない条件**:
  - `LoadedIn(Entity)`: 手押し車などに積載されている
  - `StoredIn(Entity)`: Stockpile に格納されている
  - `DeliveringTo(Entity)`: 搬送中（リレーションシップあり）
  - `StoredByMixer(Entity)`: MudMixer に格納されている
- これにより、運搬されずに放置された余剰な中間素材が自動的にクリーンアップされます。

### 1.5 TransportRequest
- `TransportRequest { kind, anchor, resource_type, issued_by, priority, stockpile_group }`
- `TransportDemand { desired_slots, inflight }`
- `desired_slots` は request が同時に持てる worker の総 ceiling、`inflight` は既存 request に付いた
  `TaskWorkers` / lease 状態を producer が再反映した値として扱う。policy-driven Stockpile request では
  `desired_slots = current_workers + new_assignable` とし、`remaining()` が新規割当可能量になる。
- `TransportRequestState`:
  - `Pending` / `Claimed`
- request エンティティには通常 `Designation`, `ManagedBy`, `TaskSlots`, `Priority` も付与されます。

## 2. TransportRequest 基盤

`TransportRequestPlugin` は以下の順で実行されます。

1. `Perceive`（メトリクス集計 + フレームキャッシュ更新）
   - `update_floor_tile_waiting_cache_system`: `Changed<FloorTileBlueprint>` 検知時のみ `FloorTileWaitingCache`（site → bones/mud 不足量）を再構築
   - `update_wall_tile_waiting_cache_system`: `Changed<WallTileBlueprint>` 検知時のみ `WallTileWaitingCache`（site → wood/mud 不足量）を再構築
   - `update_cached_active_familiars_system`: Familiar / command / TaskArea の追加・変更・削除時だけ、Idle 以外のリストを再構築
   - `update_cached_active_yards_system`: Yard の追加・変更・削除時だけ全 Yard リストを再構築
   - `update_cached_stockpile_groups_system`（`.after(update_cached_active_yards_system)`）: grid / Yard / `StockpilePolicy` membership が変わった時だけ、`With<StockpilePolicy>` の通常セルから group と空間インデックスを再構築
   - `FloorTileWaitingCache` / `WallTileWaitingCache` は変化のないフレームでは再構築をスキップする（Change Detection ベース）
   - `CachedStockpileGroups` は membership と位置だけを保持する。policy 値、stored、incoming は live demand の正本にせず、変更のない tick では generation を進めない
   - `CachedActiveFamiliars` / `CachedActiveYards` / `CachedStockpileGroups` は全 producer が共有参照し、producer ごとの Vec / group 再構築を排除する
   - ⚠️ `task_area_auto_haul_system` と `stockpile_consolidation_producer_system` は `Decide` で `CachedStockpileGroups` を **読むだけ**にする。両 system が個別に `build_stockpile_groups` を呼ぶと同一フレームで重複構築になるため、Perceive の cache を経由すること
2. `Decide`（各 producer が request を upsert）
3. `Arbitrate`（手押し車仲裁 — 後述 §5.2）
4. `Execute`（`Changed<TaskWorkers>` に応じた state 同期）— worker がいる request は `Claimed`、target が残ったまま空になった request は `Pending` に遷移
5. `Reconcile`（Soul AI の `Execute` 後）— `WorkingOn` source の削除を適用してから、Relationship hook が内部 queue に積んだ空 `TaskWorkers` target の削除も適用する。`RemovedComponents<TaskWorkers>` を全件消費し、現存する worker なし request を同じ `Update` で `Pending` に戻す
6. `Maintain`（アンカー消失や不要 request の cleanup）

`Arbitrate` は `Reconcile` より前に実行済みであるため、最後の worker を失った request は次の Logic frame から再仲裁候補になる。

`task_finder` は `DesignationSpatialGrid` と `TransportRequestSpatialGrid` の両方を探索して候補を集約します。`Build` と Yard-owned Designation は、空間範囲外でも既存の補助全件走査から候補へ加えます。

### 2.1 ManualTransportRequest の close owner

手動搬送 request の UI cancel と anchor cleanup は、`transport_request::lifecycle` の
`close_manual_transport_request` を共用する。root UI は request component を個別 remove しない。

- live `ManualTransportRequest`、fixed source、worker、request が ResourceItem かを `ManualTransportCloseContext` で渡す。
- worker ごとに `SoulTaskUnassignRequest` を発行し、予約・所持品・`AssignedTask` の cleanup は Soul AI owner を通す。
- fixed source の `ManualHaulPinnedSource` を外す。
- request entity なら despawn、ResourceItem と同居する場合は transport/request/assignment component 一式だけを除去する。
- fixed source 欠落は `MalformedClosed` として安全に close し、非 manual request は `Unsupported` で変更しない。

anchor 消失、需要 0、issuer 消失、manual source 消失/搬送済みの Maintain 経路も同じ除去 primitive を通る。

## 3. Request 種別と実装

| kind | WorkType | producer | anchor | ソース解決 |
| :--- | :--- | :--- | :--- | :--- |
| `DepositToStockpile` | `Haul` | `task_area_auto_haul_system` | Stockpile | 割り当て時にアイテムを遅延解決 |
| `DeliverToBlueprint` | `Haul` | `blueprint_auto_haul_system` | Blueprint | 割り当て時に必要資材を遅延解決 |
| `DeliverToMixerSolid` | `HaulToMixer` | `mud_mixer_auto_haul_system` | Mixer | 割り当て時に Sand/Rock を遅延解決（Sand は原則猫車必須、近接ピックドロップ完結時は徒歩許可） |
| `DeliverToFloorConstruction` | `Haul` | `floor_construction_auto_haul_system` | FloorConstructionSite | 割り当て時に Bone / StasisMud ソースを遅延解決（搬入先は `site.material_center`） |
| `DeliverToWallConstruction` | `Haul` | `wall_construction_auto_haul_system` | WallConstructionSite | 割り当て時に Wood / StasisMud ソースを遅延解決（搬入先は `site.material_center`） |
| `DeliverToProvisionalWall` | `Haul` | `provisional_wall_auto_haul_system` | Wall (Building) | 割り当て時に StasisMud ソースを遅延解決（搬入先は壁足元） |
| `DeliverToSoulSpa` | `WheelbarrowHaul` | `soul_spa_auto_haul_system` | SoulSpaSite | 割り当て時に Bone ソース（地面 / BonePile）を遅延解決（猫車必須）。搬入先はサイト中央 |
| `DeliverWaterToMixer` | `BucketTransport` (source=Tank) | `mud_mixer_auto_haul_system` | Mixer | 割り当て時に tank + bucket を遅延解決 |
| `GatherWaterToTank` | `BucketTransport` (source=River) | `tank_water_request_system` | Tank | 割り当て時に bucket を遅延解決 |
| `ReturnBucket` | `Haul` | `bucket_auto_haul_system` | Tank | 割り当て時に dropped bucket と返却先 BucketStorage を同時遅延解決 |
| `ReturnWheelbarrow` | `WheelbarrowHaul` | `wheelbarrow_auto_haul_system` | WheelbarrowParking | ownerの駐車場外で未使用になった猫車を固定sourceとして返却する |
| `BatchWheelbarrow` | `WheelbarrowHaul` | `wheelbarrow_auto_haul_system` | Wheelbarrow | producer による生成停止済み。ファミリア AI も処理しないため実質無効。enum 値のみ残存 |
| `ConsolidateStockpile` | `Haul` | `stockpile_consolidation_producer_system` | Stockpile（レシーバーセル） | 割り当て時にドナーセルの `StoredIn` アイテムを遅延解決 |

## 4. 自動運搬の仕様

### 4.1 Yard -> Stockpile (`DepositToStockpile`)
- **グループ単位の発行**:
  - **Yard 単位**で、Yard 境界内にある `Stockpile + StockpilePolicy` をひとつの構造 group として構成する。
    policy を持たない Tank / Mixer / `BucketStorage` は通常 group へ入れない。
  - 受入可能セルを `inbound_priority` ごとに分け、request identity を
    **`(issued_by Yard, resource_type, priority tier)`** とする。`stockpile_group` は同 tier かつ同資源を
    現在受け入れられるセルだけ、`anchor` はその subset 内の実セル、`priority` は tier と一致する。
  - policy-driven request には保存しない `ReceiverPolicyTier` を付け、manual request や maintenance 用の
    raw request priority と通常 Familiar の policy offset を混同しない。
  - **共有セルの扱い**: 複数の Yard が重複する場合、その領域内の Stockpile はそれぞれの Yard グループに含まれます。
    - producer は Yard / tier の安定順で cell 単位の資源別 cycle shadow を消費し、同じ物理枠を複数 request の
      `new_assignable` に数えない。
    - wheelbarrow grant も同じ cell 単位の資源別 shadow で直前再検証し、同じ空き枠を二重 lease しない。
- **需要計算**:
  - producer は cycle ごとに各セルの live `StockpilePolicy`、現在内容、`StoredItems`、資源別
    `IncomingDeliveries` を `NewInbound` evaluator へ渡す。
  - tier subset の `new_assignable` は `min(physical remaining, target remaining)` から incoming と cycle shadow を
    控除した合計である。既存 worker 数を加えた値を `TaskSlots.max` / `TransportDemand.desired_slots` とし、
    `inflight` には worker 数を保持する。
  - policy 不適合、目標到達、物理満杯、異種 contents / reservation のセルは request subset と需要へ入れない。
    既存 worker を持つ旧 request は新規枠を 0 に絞り、committed lifecycle が終わるまで保持する。
  - Pending request は `Designation` が外れた場合、または `Demand=0` の場合に arbitration が
    `DemandGone` で候補外にする。既存 lease / pending timer も同じ更新で解放し、無効 request が
    wheelbarrow を保持し続けない。Claimed request の lease は committed lifecycle のため維持する。
  - 既存 request の component は semantic diff がある場合だけ更新する。初回 pending timer が確定した後の
    policy / contents / incoming 不変 tick では arbitration generation を進めない。
- **収集対象範囲**:
  - **Yard 外周から 10 タイル以内**。
  - ただし、Yard 外側の「外周+10」領域では、**他 Yard 内**の位置を除外します。
  - 複数グループの範囲に入るアイテムは、最寄りグループ（Yard 外周距離）に排他的に割り当てられます。
- **搬入・ソース選定**:
  - request の `resource_type` は、収集範囲内の近傍フリーアイテムから tier ごとに推定する。
  - producer は `q_free_items` を **1回だけ走査**し、同距離では位置と Entity key を使う安定順で代表型を決める。
  - evaluator が新規搬入可能量を返すセルが1つ以上ある型だけを候補にする。
  - 搬入対象は `ResourceType::is_loadable() == true` の資材のみ。
  - owner 付き通常 Stockpile は同 owner の地面資材を優先し、利用可能な同 owner 資材がなければ
    owner 未設定資材だけへフォールバックする。他 owner の資材は混ぜない。
  - 重複 Yard で request group に複数 owner のセルが含まれても、通常 Familiar は実際に選んだ destination cell の
    owner を source 条件に使う。猫車 grant も実 lease item と再選択先セルの owner 互換性を再確認する。
  - Familiar resolver は割当直前にも request の tier subset、live policy、contents、incoming、cycle shadow を
    `NewInbound` で再評価し、stale request から新しい assignment を作らない。`WheelbarrowLease` がある場合は
    lease が保持する単一 destination だけを評価し、そのセルが無効になっても同じ group の別セルへ付け替えない。
  - 通常 Familiar の rank は従来の priority/distance base score を維持し、その後へ
    Low=-10 / Normal=0 / High=+10 / Critical=+20 unit の transport offset を一度だけ加える。
    1 unit は `0.65 / 40`、最終 score は clamp しないため、base priority 上限でも tier の単調順を保つ。
  - ソースは「地面アイテムのみ」（`StoredIn` 付きは除外）で、同一 Stockpile での pick-drop ループを防止します。

- **manual haul**:
  - 地面 item の明示搬送先も managed cell では `NewInbound` evaluator を通過した候補だけを選び、満杯・目標到達・
    policy 不一致セルへフォールバックしない。
  - 同じ area 操作内の複数 source は資源別 cycle shadow を共有する。policy を持たない `BucketStorage` は
    既存の bucket 専用選定を維持し、manual request の明示 priority は上書きしない。
  - owner 未設定の通常資材は owner 付き通常 Stockpile へ搬入でき、通常搬送・猫車搬送とも荷下ろし完了時に owner を確定する。
    別 owner の資材と、owner 未設定 bucket の owner 付き `BucketStorage` への搬入は許可しない。
  - area 内 source は位置、最後に Entity key の安定順で処理してから cycle shadow を消費する。

### 4.2 Blueprint 搬入 (`DeliverToBlueprint`)
- producer は `required_materials - delivered_materials` を demand として維持し、既存 request の `inflight` を別途保持する。
- request は Blueprint 位置に生成し、ソースは割り当て時に探索。
- `Site` 内の Blueprint は `PairedYard` の Yard を construction owner 候補に含める。Familiar が Idle で TaskArea owner が無い場合も、その Yard 名義で request を生成できる。
- Familiar 割り当て時には `delivered + IncomingDeliveries + ReservationShadow` を差し引いた残需要を再計算し、需要 0 の request は stale 扱いで割り当てない。
- Soul / wheelbarrow の搬入直前にも Blueprint の残需要を再確認し、充足済みなら Blueprint への消費を行わずタスクを中断する。
- `Sand` 搬入は `collect_source` パスを使用し、**同一 Soul の `HaulWithWheelbarrow` 1タスク内で完結**する（`CollectSand` 別タスクは経由しない）。MudMixer への Sand 搬入も同じ設計。
  - ソース探索順: `SandPile` 優先、見つからない場合は `TerrainType::Sand` タイル。
  - 範囲: まず TaskArea 内を探索し、見つからなければ全体探索にフォールバック。
  - 積込: `Loading` フェーズで砂アイテムをその場生成し、1回で `min(不足量, WHEELBARROW_CAPACITY)` を猫車に積載。
  - ソース（砂置き場/砂タイル）は消費しない（無限ソース）。
  - 過剰割り当て防止のため、割り当て時に「必要量 - 予約済み」を再計算して積載量を決定する。

#### 4.2.1 Blueprint / WallConstruction / MudMixer不足時の自動伐採/採掘（Wood / Rock）
- `familiar_ai` の `blueprint_auto_gather_system` が、`DeliverToBlueprint` request（Wood / Rock）、`DeliverToWallConstruction` request（Wood）、`DeliverToMixerSolid` request（Rock）の `issued_by` を需要 owner として不足量を検知する。
- 不足判定は owner/resource 単位で以下を差し引いて算出する:
  - 地面の未予約資材
  - 既存の手動 `Chop` / `Mine` 指定の期待ドロップ量
  - 進行中の自動 Gather（AutoGather）の期待ドロップ量
- 未指定 Tree/Rock と地面資材の owner 解決では、同じ resource に正の需要がある owner を優先する。資源が別 Familiar の TaskArea 内にあっても Yard 需要と別キーへ分断しない。該当需要がない場合だけ通常の位置ベース owner 解決へ戻る。
- 地面資材と既存 `Chop` / `Mine` の期待量は、owner の `path_start` から到達可能なものだけを供給として数える。地面資材は `DeliveringTo` のない未予約状態に限定し、搬送中数との二重控除を避ける。`ManagedBy` のない手動指定は Active Familiar / Yard のいずれかの `AreaBounds` 内にあり、task finder が実際に発見できる場合に限って需要を相殺する。
- Bridge の Wood/Rock 代替需要は、到達可能な既存供給と未指定候補の期待量へ配分する。Wood 候補が到達不能で Rock 候補だけが到達可能なら Rock 需要として `Mine` を選ぶ。
- 候補探索は owner の `AreaBounds`（Familiar の TaskArea または Yard 境界）を起点とする段階走査:
  - Stage 0: owner の `AreaBounds` 内
  - Stage 1: 外周 `<= 10` タイル
  - Stage 2: 外周 `<= 30` タイル
  - Stage 3: 外周 `<= 60` タイル
  - Stage 4: それ以遠の到達可能全域
- 各 Stage は近傍優先で処理し、必要量を満たした時点で終了。経路判定は Stage ごとの上限件数で制御する。
- 自動付与対象には `AutoGatherDesignation { owner, resource_type }` marker を付け、不要になった未着手指定は marker ベースで回収する。
- Yard-owned の `Chop` / `Mine` は Yard 境界外でも task finder の補助全件走査へ入り、60タイルの worker 距離制限と到達判定を通過した Soul へ割り当てられる。

### 4.3 MudMixer 固体搬入 (`DeliverToMixerSolid`)
- `Sand` / `Rock` の不足量を `SharedResourceCache` を含めて判定。
- request は Mixer 位置に生成し、ソースは割り当て時に探索。
- `Rock` 不足については 4.2.1 の自動Gather需要にも反映され、必要に応じて `Mine` 指定が追加発行される。
- `Sand` 搬入は **Blueprint Sand と同じ `collect_source` パスで完結**する（旧 `CollectSand` 指示経路は廃止）。
  - Familiar AI の `try_direct_collect_with_wheelbarrow_to_mixer` が空きミキサー容量・砂源・猫車を確認してタスク発行。
  - ソース探索順: `SandPile` 優先、見つからない場合は `TerrainType::Sand` タイル。
  - 積込: `Loading` フェーズで砂アイテムを砂源位置に直接生成し、`min(不足量, WHEELBARROW_CAPACITY)` 分を猫車に一括積載。
  - ソース（砂置き場/砂タイル）は消費しない（無限ソース）。
  - `Loading` フェーズで生成するため搬入先への `DeliveringTo` は挿入されない（Mixer 宛の予約は `ReserveMixerDestination` op で別途管理）。
  - 需要計算: `needed = MUD_MIXER_CAPACITY - current - inflight`。`inflight` には `DeliverToMixerSolid+Sand` に割り当て済みの worker 数を使用し、過剰タスク発行を防止する。

### 4.4 MudMixer 水搬入 (`DeliverWaterToMixer`)
- 水不足時に request を発行。
- 割り当て時に、エリア内の有効タンクと利用可能バケツを遅延解決して搬送。
- 実行フェーズは内部的に `BucketTransport` の共通表現へ収束し、Tank→Mixer / River→Tank の流れを共通ハンドラで解釈する。

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
- **対象**: 同じ owner の `StockpilePolicy` 付き通常セルだけを扱う。同種資材が2セル以上に分散し、
  1回の移動で少なくとも1つの donor を完全に空にできる場合に `Haul` request を作る。
- **Receiver**: 現在量の多い順、位置、Entity key の安定順で候補化し、`NewInbound` evaluator へ acceptance、
  target、物理容量、現在内容、資源別 `IncomingDeliveries`、同 cycle の receiver shadow を渡す。
  evaluator の `available_amount` が最小 donor の全量未満なら、その receiver は使わず次候補を調べる。
- **Donor**: `NewOutbound` evaluator を通過し、`SharedResourceCache` の source reservation を差し引いても
  全在庫を搬出できるセルだけを、在庫の少ない順に選ぶ。方針適合中の `allow_export = false` は donor 化を止めるが、
  acceptance と現在内容が不一致の `Draining` は override して搬出できる。
- **需要量**: donor 合計と receiver の `available_amount` の小さい方へ clamp し、同 cycle shadow へ加算する。
  `anchor` は receiver、`stockpile_group` は適格 donor 一覧。割当 resolver でも receiver の `NewInbound`、
  donor の `NewOutbound`、owner、実 source item を再検証する。
- **優先度**: 通常 Familiar 用 `Priority(0)` と猫車用 raw `TransportPriority::Low` は維持する。
  receiver の policy tier は別の `ReceiverPolicyTier` として保持し、Normal=0 の policy contribution として合成する。
- **ライフサイクル**: policy や在庫の変化で request が無効になった場合、worker なし request は無効化する。
  既存 worker がいる request は `desired_slots = inflight = worker数`、`Claimed` に絞り、新規 assignment だけを止めて
  committed 搬送を完了させる。既存 `Haul` が `StoredIn` を外し、receiver への格納時に再付与する。

### 4.7 Tank 自動補充 (`GatherWaterToTank`)
- 水タンクの不足量を監視し、`BUCKET_CAPACITY` 単位で必要タスク数を算出して request 化。
- 割り当て時に request anchor（tank）に紐づく利用可能バケツを選択して `BucketTransport`（source=River, destination=Tank）を実行。
- タンク容量（現在量 + 予約）を割り当て時にも再検証。

### 4.8 床建築搬入 (`DeliverToFloorConstruction`)
- `floor_construction_auto_haul_system` が site ごとに不足資材を算出し request を upsert。
- Reinforcing フェーズでは `Bone`、Pouring フェーズでは `StasisMud` を要求。
- 搬入先は常に `FloorConstructionSite.material_center`。
- `floor_material_delivery_sync_system` が `material_center` 周辺の資材を消費し、各タイルの `bones_delivered` / `mud_delivered` を進める。
- Familiar 割り当て時と wheelbarrow 荷下ろし時の両方で、待機中タイルの残数から `IncomingDeliveries` / 当フレーム予約分を差し引いた残需要を再確認する。
- `Bone` は以下の優先順で解決される:
  1. 地面アイテムを通常 `Haul` で搬送
  2. 地面アイテムがない場合は `BonePile` / River からの猫車直採取へフォールバック

### 4.9 仮設壁搬入 (`DeliverToProvisionalWall`)
- `provisional_wall_auto_haul_system` が `BuildingType::Wall && is_provisional` の壁を走査し、`ProvisionalWall.mud_delivered == false` の壁に request を upsert。
- request の anchor は壁エンティティで、割り当て時に `StasisMud` ソースを遅延解決する。
- `provisional_wall_material_delivery_sync_system` が壁近傍へ落ちた `StasisMud` を消費して `mud_delivered = true` に更新する。
- 割り当て時と荷下ろし時の両方で「まだ泥未搬入か」を確認し、充足済み壁への重複搬入を防ぐ。
- `provisional_wall_designation_system` が準備完了した壁へ `WorkType::CoatWall` を付与し、塗布タスクへ遷移させる。
- 互換レイヤーとして残存しており、`WallConstructionSite` 配下で管理される壁タイル実体（`spawned_wall`）は対象から除外される。

### 4.10 壁建築搬入 (`DeliverToWallConstruction`)
- `wall_construction_auto_haul_system` が site ごとに不足資材を算出し request を upsert。
- `Framing` フェーズでは `Wood`、`Coating` フェーズでは `StasisMud` を要求。
- `Site` 内の `WallConstructionSite` は `PairedYard` の Yard を construction owner 候補に含め、Familiar が Idle でも Wood request を維持する。
- 搬入先は常に `WallConstructionSite.material_center`。
- `wall_material_delivery_sync_system` が `material_center` 周辺の資材を消費し、各タイルの `wood_delivered` / `mud_delivered` を更新する。
- 割り当て時と wheelbarrow 荷下ろし時に、対象 phase の残需要を再確認して不要な資材を搬入先へ置かない。
- `wall_tile_designation_system` が `FramingReady -> WorkType::FrameWallTile`、`CoatingReady -> WorkType::CoatWall` を付与する。

### 4.11 Soul Spa 建設搬入 (`DeliverToSoulSpa`)
- `soul_spa_auto_haul_system` が `SoulSpaPhase::Constructing` のサイトを走査し、残 Bone 需要を算出して request を upsert。
- owner 解決には `collect_all_area_owners`（使い魔 TaskArea + Yard）を使用するため、サイトが使い魔のエリア外の Yard にある場合でも request が生成される。
- Bone は常に猫車（`WheelbarrowHaul`）で搬送。搬入先はサイト中央（`SoulSpaSite` の `Transform` 位置）。
- `soul_spa_delivery_sync_system` がサイト周辺（半径 1.5 タイル）の Bone を消費し `bones_delivered` を更新する。
- `bones_delivered >= bones_required`（= 12）で `SoulSpaPhase::Operational` に遷移し、建設 request は消滅する。
- 荷下ろし時（`unloading.rs`）は `WheelbarrowDestination::Stockpile` として到着し、サイト位置へアイテムをドロップ。delivery_sync 側が収集するためストックパイル容量チェックは行わない。

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
  - `DeliverToWallConstruction`（`StasisMud`）
  - `DeliverToSoulSpa`（`Bone`、猫車直採取）
- 猫車不足時は request は `Pending` のまま待機する。

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
   - `kind` が仲裁対象（`DepositToStockpile` / 猫車必須資源の `DeliverToBlueprint` / `DeliverToMixerSolid`）
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
   - 半径 `TILE_SIZE * 10.0` 内で、source reservation 0 の近傍 `Top-K`
     （`WHEELBARROW_ARBITRATION_TOP_K`）を抽出する。予約済み item は Top-K 枠を消費せず lease にも入らない
   - 実際の検索範囲内で必要最小数を満たせる候補が予約で不足した場合は、全件予約だけでなく
     「未予約1件 + 予約済み1件、hard_min=2」のような混在不足も `SourceReserved`。予約を含めても必要数に
     届かなければ `NoSourceItems` とし、遠方 bucket 全体の予約状態を近傍判定へ混ぜない
   - Blueprint / mixer の無限距離 fallback 対象では、近傍が全件予約済みでも遠方の未予約候補を再探索する
   - `DepositToStockpile` では receiver tier と一致する各セルを `NewInbound` evaluator で評価し、
     group 合計ではなく**単一セルの最大 available**を batch 上限にする
   - 猫車必須資源は、`Top-K` 候補に対してピックドロップ完結可能判定を行い、成立時は仲裁候補から除外
   - 最小バッチ条件: 猫車必須資源は `1`、それ以外は `WHEELBARROW_MIN_BATCH_SIZE`
6. **スコア計算** — `score = batch_size * SCORE_BATCH_SIZE + priority * SCORE_PRIORITY - distance * SCORE_DISTANCE + pending_bonus`
   - `distance` = 最近の wheelbarrow からアイテム重心までの距離
   - `pending_bonus = min(pending_for, WHEELBARROW_SCORE_PENDING_TIME_MAX_SECS) * WHEELBARROW_SCORE_PENDING_TIME`
   - 小バッチ（1〜2個）には減点を適用
7. **候補間重複除去** — score と request Entity key の安定順で処理し、先行候補で消費した item を後続候補から除外。
   - 除外後の item 数が `hard_min` 未満になった候補はスキップ。
8. **grant 直前再検証** — live policy / contents / incoming と同一 arbitration cycle の資源別 cell shadow を再評価する。
   - batch 全件が収まる最小 available cellを選び、なければ最大 available cellへ itemを truncateする。
   - truncate 後に `hard_min` を満たさない、適格セルまたは destination Transform がない場合は lease を作らない。
9. **Greedy 割り当て** — スコア降順にソートし、各 request に最近の available wheelbarrow を割り当て。
   - 割り当て済みの wheelbarrow は available set から除去。
   - 同距離では wheelbarrow Entity key を最終 tie-break にする。
10. **小バッチ抑制** — 猫車必須資源で `batch_size < WHEELBARROW_PREFERRED_MIN_BATCH_SIZE` の場合:
   - `pending_since` から `SINGLE_BATCH_WAIT_SECS` 経過前は候補から除外
   - 経過後にのみ割り当て可能
11. **動的 lease 期間** — `wb -> source -> destination` の推定移動時間から期間を算出し、最小/最大値でクランプ。

#### 5.2.3 定数

定数（12種）は `crates/bevy_app/src/constants/` 参照。主要値: `WHEELBARROW_PREFERRED_MIN_BATCH_SIZE = 3`（小バッチ優先下限）/ `SINGLE_BATCH_WAIT_SECS = 5.0`（待機秒数）/ `WHEELBARROW_ARBITRATION_TOP_K = 24`（近傍候補上限）。

#### 5.2.4 割り当て時の lease 優先

`assign_haul` / `assign_haul_to_blueprint` / `assign_haul_to_mixer` は以下の順で手押し車を解決する:

1. **WheelbarrowLease あり** → lease の有効性を検証（wheelbarrow が parked か、items が未予約か）し、有効なら即採用。
2. **猫車必須資源かつ lease なし** → 原則待機（徒歩フォールバックなし）。
   - ただし、ピックドロップ完結可能な request は徒歩運搬を優先。
3. **Stockpile / Blueprint の猫車任意資源** → 仲裁外のアドホック猫車探索は行わず、単品運搬へフォールバック。
   - `Sand` / `Bone` の Blueprint 直接採取経路は互換のため維持。

#### 5.2.5 lease のライフサイクル

- **付与**: 仲裁システム（Arbitrate フェーズ）が `WheelbarrowLease` を insert。
- **消費**: `assign_haul` が lease を読み取り `HaulWithWheelbarrow` タスクを発行。割り当て後は request の state が Pending でなくなるため、次フレームの仲裁で自動的に対象外。
- **失効/無効化**: 仲裁システムが毎フレーム `lease_until < now`、手押し車消失、有効 item 数不足をチェックして remove。
- **request close**: `transport_request_anchor_cleanup_system` が request を閉じる際に `WheelbarrowLease` も除去。

#### 5.2.6 メトリクス

`TransportRequestMetrics` に `wheelbarrow_leases_active`, `wb_arb_*`（eligible/topk/dedup/pending/duration/elapsed）, `task_area_*`（groups/scanned/matched/elapsed）を追加。5秒間隔のデバッグログに出力。

#### 5.2.7 latest-only 診断

仲裁が dirty または fallback interval で rebuild したとき、同じ request / free-item / grant 走査から
`WheelbarrowArbitrationDiagnostics` を公開する。診断のための追加全件走査は行わず、rebuild しない frame は前 snapshot を保持する。

- header: generation、`SharedResourceCache::semantic_generation()`、物理車両の有無、available / leased 台数。
- request outcome: `LeaseGranted`、`NotApplicable`、車両/source/capacity 不足、source/capacity reservation、
  demand 消滅、preferred batch wait、arbitration contention、stale input。
- 全車が使用中/`PushedBy` の場合も「車両自体が存在しない」と区別する。
- Familiar producer は lease がない wheelbarrow task の upstream evidence として読み、cache generation が一致しない
  snapshot を blocker に使わない。

snapshot と `WheelbarrowArbitrationRuntime` は保存せず、world replacement で default に戻す。

## 6. 予約と競合回避

予約には **搬入先予約（Relationship）** と **ソース/ミキサー予約（SharedResourceCache）** の2種類があります。

### 6.1 搬入先予約（Relationship ベース）

Stockpile / Blueprint / Tank などへの搬入予約は、Bevy の Relationship で管理します。

- タスク割り当て時に、搬入対象アイテムに `DeliveringTo(destination)` を自動挿入（`apply_task_assignment_requests_system`）。
- 搬入先エンティティには Bevy が `IncomingDeliveries` を自動維持。
- 通常 Stockpile の**新規搬入**は `evaluate_stockpile_policy(NewInbound)` へ現在量、物理容量、target、
  stored resource、transfer resource、資源別 `IncomingDeliveries`、同一 cycle shadow を渡す。
- `available_amount = min(physical remaining, target remaining) - incoming - cycle shadow` を saturating 計算し、
  空の `Any` セルでも別資源の予約があれば後続資源を拒否する。
- 通常 haul / wheelbarrow の実行時は、搬送 item 集合と destination の live `IncomingDeliveries` を Entity 単位で
  突き合わせる。自分の予約を持つ item だけを `CommittedInbound` とし、変更後の acceptance / target は
  grandfather するが、物理容量、現在内容の互換性、owner 互換性は再検証する。
- wheelbarrow の batch に committed item と予約を失った item が混在する場合、committed 分を先に評価し、
  残りだけを現在 policy の `NewInbound` で評価する。未許可 item は安全に地面へ戻し、予約 relationship を除去する。
- policy を持たない Tank / Mixer / `BucketStorage` は各専用容量判定を維持する。
- タスク完了・中断時にアイテムの `DeliveringTo` を除去すると、搬入先の `IncomingDeliveries` も自動更新される。
- HashMap による再構築が不要なため、**常に最新の予約状態**が ECS から直接取得可能。

### 6.2 ソース／ミキサー予約（SharedResourceCache）

`SharedResourceCache` で以下を管理します。

- `mixer_dest_reservations`（Mixer + ResourceType）
- `source_reservations`（アイテムやタンク）

#### 再構築
- `sync_reservations_system` が `AssignedTask` と未割り当て request（`Designation` + `TransportRequest`）から予約を再構築。
- 初回、予約 operation を変える active task の signature 差分、pending request 側の変更/削除、または `RESERVATION_SYNC_INTERVAL` の安全監査で snapshot を置換する。timer は遅延適用のためではなく、取りこぼし検出のための監査である。
- active task の signature は `hw_jobs::lifecycle::collect_active_reservation_ops` と同じ正規化経路から導出する。progress のみが変わった `AssignedTask` は snapshot を再構築しないが、予約対象・種別・phase が変わる遷移、assignment、completion、removal は再構築対象である。
- `AssignedTask` removal は安全側で snapshot を再構築する。`RemovedComponents` reader は全件消費し、同じ removal を次フレーム以降に繰り返し dirty と扱わない。

#### 差分適用
- `TaskAssignmentRequest` に含まれる `reservation_ops` は、その適用時に `apply_reservation_op` を通じて cache へ直接反映する。
- task の中断と実行 handler は `ResourceReservationRequest` を送信し、`hw_logistics::apply_reservation_requests_system` が Execute で `ResourceReservationOp` を適用する。
- `apply_reservation_requests_system` と `apply_reservation_op` の実装は `hw_logistics` にあり、system 登録は `hw_logistics::LogisticsPlugin`（`SoulAiSystemSet::Execute`）が担う。`ResourceReservationRequest` の `add_message` と `SharedResourceCache` の `init_resource` は app shell が担当する。
- `RecordPickedSource` によりソース論理在庫の差分も追跡する。

#### cache の寿命
- `SharedResourceCache::begin_frame()` は Perceive の先頭で pickup/store の frame-local delta だけを clear する。
- `replace_reservation_snapshot()` はソース/ミキサー予約 map だけを置換し、同一フレームにまだコンポーネントへ反映されていない delta を clear しない。
- load 時は cache、reservation signature cache、同期 timer を default に戻す。次の Perceive は初回同期として完全 snapshot を構築し、旧 world の Entity を cache に残さない。

### 6.3 水搬送の排他
- `BucketTransport`（`WorkType::GatherWater` / `WorkType::HaulWaterToMixer`）は、`AssignedTask::BucketTransport` の `source`/`destination`/`phase` の組み合わせに応じて
  - バケツ source の確保
  - 取水元 tank の確保
  - ミキサー destination の確保
  を制御し、フェーズ遷移に従って reservation を更新する。

## 7. 備蓄資材の取り出し

- 建築/製造向けの搬送ソースには、条件を満たす `StoredIn` アイテムも利用されます。
- アイテムを持ち出すと `StoredIn` が外れ、`StoredItems` は自動更新されます。
- 取り出し後に Stockpile が空になると `resource_type` は `None` に戻ります。

### 7.1 地面資材数ラベル

- 同一グリッド上に可視状態の `ResourceItem` が複数ある場合、タイル右上寄りに個数ラベルを表示します。
- 集計対象は `Visibility::Visible` または `Visibility::Inherited` のアイテムのみです。
- ラベル同期は初回即時、その後は 0.25 秒間隔で行います。これにより表示更新の遅延を 1/4 秒以内に抑えつつ、毎フレーム全 `ResourceItem` を再集計しません。

## 8. システム追加時の実装ルール

物流系システムを追加する際は、以下を満たしてください。

### 8.1 Request 発行方式の統一
- 新しい自動物流は、原則として「anchor に紐づく `TransportRequest` エンティティ」を発行する。
- アイテム実体への直接 `Designation` 連打は避け、ソースは割り当て時に遅延解決する。
- request の `kind` / `work_type` / `anchor` / `resource_type` の組み合わせは必ず一貫させる。

### 8.2 Producer の upsert/cleanup 規約
- 既存 request があれば再利用（upsert）し、不要時は以下で閉じる。
  - `TaskWorkers == 0` のときは `Designation` / `TaskSlots` / `Priority` を外す、または despawn。
- 同一 key の重複 request は許可しない。policy-driven `DepositToStockpile` の key は
  `(issued_by Yard, resource_type, receiver priority tier)` であり、anchor の変更で別 request にしない。
- demand 計算は `current + in_flight(+ reservation)` を使い、過剰発行を防ぐ。

### 8.3 予約の責務を統一
- **搬入先予約**: タスク割り当て時に `DeliveringTo` が自動挿入される。手動で `ResourceReservationOp` を発行する必要はない。
- **ソース予約**: 「割り当て時」に `ResourceReservationOp::ReserveSource` で付与し、成功・失敗・中断の全経路で解放する。
- タスク実行でソース取得が成功したら `RecordPickedSource` を使う。
- 共有ソース（例: tank 取水）は `ReserveSource` で排他を取る。

### 8.4 ソース選定の安全条件
- `DepositToStockpile` のソースは地面アイテムのみを対象にする（`StoredIn` 付きは除外）。
- 所有物資がある場合は `BelongsTo` を一致させ、他 owner の資材を混在させない。
- `Visibility::Hidden` / `TaskWorkers` 付きエンティティは候補から外す。

### 8.4.1 AreaBounds によるオーナー解決

標準の transport request producer は `collect_all_area_owners` ヘルパーで **Familiar の TaskArea と Yard の境界を `AreaBounds`（共通矩形型）に統合** し、統一コードパスでオーナーを解決します。Blueprint / WallConstruction producer はさらに `collect_construction_area_owners` で `Site` 境界を対応する `PairedYard` の owner として追加します。

- `AreaBounds`（`hw_core::area`）は `{ min: Vec2, max: Vec2 }` の plain struct で、`contains` / `center` / `size` 等の共通メソッドを持つ。Component ではない。
- `TaskArea`、`Yard`、`Site` はそれぞれ `.bounds()` メソッドと `From` impl で `AreaBounds` に変換可能。
- `collect_all_area_owners` は `Vec<(Entity, AreaBounds)>` を返し、Familiar TaskArea と Yard 境界を同列に扱う。
- `collect_construction_area_owners` は上記に `(PairedYard, Site.bounds())` を加え、Site 内の建築需要を Yard に帰属可能にする。
- `find_owner` / `find_owner_for_position` は `AreaBounds` ベースで位置→オーナーを解決する。Yard 内では Yard 中心を含む owner 範囲のうち中心距離が最小のものを優先する。
- `issued_by` に Yard エンティティが入った request / Designation は、補助全件走査とタスクフィルターの `is_issued_by_yard = true` 判定により全 Familiar がアクセス可能になる。
- `task_finder/filter.rs` は複数 Yard すべてをチェックします（`yards.iter().any(|yard| yard.contains(pos))`）。

### 8.5 追加時に必ず更新する箇所
- 新しい producer を `TransportRequestPlugin`（`Decide`）へ登録する。
- 新しい `WorkType` / request 種別を導入した場合:
  - `task_finder/filter.rs`（有効タスク判定）
  - `policy/mod.rs`（割り当て分岐、`task_management` 配下。運搬は `policy/haul/`：mod.rs がディスパッチ、blueprint.rs / stockpile.rs が destination 別ロジック）
  - `sync_reservations_system`（予約再構築）
  - 必要なら `task_finder/score.rs`（優先度）
  - `transport_request_anchor_cleanup_system` で cleanup 要件を満たすこと

### 8.6 動作確認の最低ライン
- `cargo check --workspace` を通す。
- 少なくとも以下を確認する:
  - request が1フレームで増殖しない
  - 同一ソースへの二重割り当てがない
  - 需要 0 時に request が休止/消滅する
  - anchor 消失時に request が cleanup される

### 8.7 建設系搬入先の基礎需要は logistics 層へ統一

- `floor_construction.rs` / `wall_construction.rs` / `provisional_wall.rs` / `ground_resources.rs` に、建設系搬入先の
  **基礎需要計算**と**地面資材カウント**を集約する。
- 割り当て時（`policy/haul/demand.rs`）は、`IncomingDeliveries` / `ReservationShadow` の控除を行う。
- 実行時（`haul/dropping.rs` / `unloading.rs`）は、同一の基礎需要を参照したうえで「地面上で既に置かれた資材」を控除して受入可否を判定する。

---

## 9. 初期リソーススポーン（initial_spawn）

`crates/bevy_app/src/systems/logistics/initial_spawn/` は `StartupPlugin::PostStartup` チェーンの中で `initial_resource_spawner` 1 関数のみを公開する thin facade モジュールである。

### 9.1 モジュール構成と責務境界

| モジュール | 役割 | Bevy Commands 依存 |
|---|---|---|
| `mod.rs` | スポーン順序の orchestration のみ（~60行） | Res / Commands を受け取るが移譲する |
| `layout.rs` | pure 計算（Site/Yard グリッド境界・Parking 占有マス） | **なし**（WorldMap 参照のみ） |
| `terrain_resources.rs` | Tree / Rock / Wood の spawn と obstacle 登録 | あり |
| `facilities.rs` | Site / Yard / WheelbarrowParking の spawn と footprint 登録 | あり |
| `report.rs` | `InitialSpawnReport` によるログ集約 | なし |

### 9.2 スポーン順序（固定）

```
spawn_trees / spawn_rocks   ← add_grid_obstacle を伴う
spawn_initial_wood          ← obstacle 登録なし
spawn_site_and_yard         ← `GeneratedWorldLayout.anchors` 由来の `SiteYardLayout`
spawn_wheelbarrow_parking   ← layout 計算失敗時は warn & skip
InitialSpawnReport::log()   ← 結果集計ログ
```

### 9.3 layout.rs の境界

- `site_yard_layout_from_anchor(&AnchorLayout)`: `hw_world` のアンカー矩形を `SiteYardLayout` に写す。初期スポーンは `GeneratedWorldLayout.anchors` を渡す。縦位置は `generate_world_layout` 内で `AnchorLayout::aligned_to_worldgen_seed` により決まり、プレビュー川の南端（`preview_river_min_y`）より南に Site 北辺（`site.max_y`）が来る（`docs/world_layout.md` の固定アンカー・川節）。
- `compute_parking_layout(base, &WorldMap)`: `WorldMap::is_walkable` で 2x2 全マスの通行可能性を確認し、`Option<ParkingLayout>` を返す。

### 9.4 追加・変更時の手順

新しい初期エンティティを追加する場合:
1. レイアウト計算がある場合は `layout.rs` に pure helper を追加する
2. spawn 実装は種別に応じて `terrain_resources.rs` または `facilities.rs` に追加する
3. `mod.rs` の facade 関数でスポーン順序（障害物 → アイテム → 施設）を守って呼び出す
4. 結果を `InitialSpawnReport` に追加してログに反映する
