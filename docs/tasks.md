# タスクシステム (Task System)

このドキュメントでは、Hell-Workers におけるタスク（仕事）の指定、割り当て、管理、および実行の仕組みについて解説します。

## 1. 概要
タスクシステムは、プレイヤーまたはシステム（使い魔 AI）が世界中の実体（木、岩、アイテムなど）に対して「仕事（WorkType）」を指定し、それを適切な「魂（Damned Soul）」が実行するまでを管理します。

## 2. 実行アーキテクチャ (Global Cycle Framework)

タスクシステムは、競合（Race Condition）と遅延を防ぐため、厳密に定義された **Sense-Think-Act サイクル** に従って実行されます。

| フェーズ | システムセット (`SoulAiSystemSet`) | 役割 |
| :--- | :--- | :--- |
| **Perceive** | `Perceive` | 環境情報の収集と **リソース予約の再構築** (`sync_reservations_system`)。`AssignedTask`（実行中タスク）と `Designation`（割り当て待ちタスク）の両方から `SharedResourceCache` を **0.2秒間隔（初回は即時）** で再構築します。 |
| **Update** | `Update` | 時間経過による内部状態の変化（バイタル更新、タイマー等）。 |
| **Decide** | `Decide` | 意思決定と要求生成（`DesignationRequest`, `TaskAssignmentRequest`）。`SharedResourceCache` を参照して候補を選定します。 |
| **Execute** | `Execute` | 要求適用（`apply_designation_requests_system`, `apply_task_assignment_requests_system`）→ 実際の行動 (`task_execution`) → 予約更新の反映 (`apply_reservation_requests_system`)。 |

## 3. 主要なコンポーネント

Bevy 0.18 の **ECS Relationships** 機能を使用し、エンティティ間の参照を双方向かつ型安全に管理しています。

| コンポーネント | 役割 | 説明 |
| :--- | :--- | :--- |
| `Designation` | 基本 | エンティティが仕事の対象であることを示すフラグ。`WorkType` を持つ。 |
| `TaskSlots` | 制限 | 1つのタスクに同時に取り組める最大人数を管理する（参加人数は `TaskWorkers` で自動集計される）。 |
| **`ManagedBy(Entity)`** | **Relationship** | タスクから**使い魔**への参照。エイリアス: `IssuedBy`。 |
| **`ManagedTasks(Vec)`** | **Target** | 使い魔側の**管理タスク一覧**。自動的に維持される。 |
| **`WorkingOn(Entity)`** | **Relationship** | 魂から**タスク**への参照。 |
| **`TaskWorkers(Vec)`** | **Target** | タスク側の**作業者一覧**。自動的に維持される。 |
| **`Holding(Entity)`** | **Relationship** | 魂から**保持アイテム**への参照。 |
| **`HeldBy(Vec)`** | **Target** | アイテム側の**保持者一覧**（通常は1人）。自動的に維持される。 |
| **`StoredIn(Entity)`** | **Relationship** | アイテムから**備蓄場所**への参照。 |
| **`StoredItems(Vec)`** | **Target** | 備蓄場所側の**格納アイテム一覧**。自動的に維持される。 |
| `Inventory` | 必須 | 魂がアイテムを持つ能力を定義するコンポーネント。これがないとタスク実行システムは機能しない。 |

### Relationship のメリット
- **自動クリーンアップ**: 使い魔やタスクのエンティティが削除された際、関連する Relationship コンポーネントも Bevy によって自動的にクリーンアップされます。
- **効率的な逆引き**: 特定の使い魔が持つタスク一覧や、特定のタスクに取り組んでいる作業者一覧を、全エンティティをスキャンすることなく O(1) または O(人数) で取得できます。

## 3. タスクキュー (Task Queue)

タスクは以下のいずれかのキューで管理されます：

- **`TaskQueue`**: 使い魔（IssuedBy）ごとに管理されるキュー。使い魔の配下の魂が優先的に処理する。
- **`GlobalTaskQueue`**: `IssuedBy` がない「未指定」のタスクが入るキュー。

## 4. タスクのライフサイクル

### 1. 指定 (Designation)
- **手動**: プレイヤーが UI やドラッグ操作で指定。
- **自動**（request エンティティ方式、M3〜M7 完了）:
    - `task_area_auto_haul_system`: ファミリアの **TaskArea 内 Stockpile グループ** 単位で `DepositToStockpile` request を生成（anchor=代表セル）。割り当て時にソースおよび**具体的な格納先セル**を遅延解決。
    - `bucket_auto_haul_system`: タンク位置（`anchor = tank`）に `ReturnBucket` request を生成。返却件数は `TransportDemand.desired_slots` で管理し、割り当て時にドロップバケツと返却先 `BucketStorage` を同時遅延解決。
    - `blueprint_auto_haul_system`: 設計図位置に `DeliverToBlueprint` request を生成。
    - `mud_mixer_auto_haul_system`: Mixer 位置に `DeliverToMixerSolid`（固体）および `DeliverWaterToMixer`（水）request を生成。
    - `tank_water_request_system`: タンクの空きに応じて `GatherWaterToTank` request を生成し、割り当て時にバケツを遅延解決。
    - request エンティティは Execute フェーズの `apply_designation_requests_system` で反映。ソース探索は割り当て時（`task_finder` → `assign_haul` 等）に遅延実行される。


### 2. 割り当て (Assignment)
- 使い魔 AI が自分のキュー、またはグローバルキューから最も近い有効なタスクを配下の魂に割り当てる。
- **排他制御 (SharedResourceCache)**:
    - 割り当て時は `SharedResourceCache` を参照し、過剰割り当てを防ぎます。
    - **予約の再構築**: `sync_reservations_system` は以下の2つのソースから予約を **0.2秒間隔（初回即時）** で再構築します:
        1. `AssignedTask` - 既にSoulに割り当てられているタスク
        2. `Designation` + `TransportRequest` (Without<TaskWorkers>) - まだ割り当て待ちの request 候補
    - これにより、自動発行システムが複数フレームにわたって過剰にタスクを発行することを防ぎます。
    - なお、フレーム内の即時更新は `ResourceReservationRequest` -> `apply_reservation_requests_system` で反映されます。
    - 決定したタスクは `TaskAssignmentRequest` と同時に予約更新要求がキューされ、Execute で反映されます。
- **優先度 (Priority)**:
    - **High (10)**: 建築作業 (`WorkType::Build`)、建築資材の運搬（設計図への `Haul`）。これらは距離に関わらず最優先で割り当てられる。
    - **Low (0)**: 通常の資源採取、備蓄への運搬。
    - 同じ優先度内では、使い魔からの距離が近いものが選ばれる。
- 魂の `AssignedTask` コンポーネントにターゲット情報が書き込まれる。
- **リクルート条件**: 使い魔は `command_radius`（影響範囲）内のワーカーのみリクルート可能。範囲外でも空きがあればスカウトに向かう（詳細は [familiar_ai.md](familiar_ai.md) 参照）。
- **疲労チェック**: 各使い魔が設定した `fatigue_threshold` を超えるワーカーにはタスクを割り当てない。
- **監視モードへの遷移**: 部下が上限（`max_controlled_soul`）に達した時点で自動的に `Supervising` 状態へ移行する。

### 3. 実行 (Execution)
- 魂が移動し、ターゲットに対して作業を行う。
- **採取 (Gather)**: 作業完了時に資源がドロップされます。
    - **木 (Tree)**: `Wood` x 5 をドロップ。
    - **岩 (Rock)**: `Rock` x 10 をドロップ。**作業時間は木の約2倍**かかる重労働です。
    - **スタック**: 報酬は同一タイル内にドロップされ、アイテム個数としてまとめてカウント（スタック）されます。
- **砂採取 (CollectSand)** / **骨採取 (CollectBone)**:
    - `SandPile`/`BonePile`、および砂/川（`TerrainType::Sand`/`River`）タイルは**無限ソース**として扱われます。
    - 採取は**即時完了**（待機プログレスなし）で、到達フレームでアイテムを生成して `Done` へ遷移します。
- **運搬 (Haul)**: 「拾う」「備蓄場所へ運ぶ」「置く」のフェーズを経る。
- **猫車必須資源**: `Sand` / `StasisMud` は原則徒歩運搬不可。`HaulWithWheelbarrow` で搬送される。
  - 例外: ピック→ドロップでその運搬1件を完了できる場合は徒歩運搬を許可。
  - 判定は「ソース隣接 3x3 の立ち位置から、実行時ドロップしきい値を満たせるか」で評価する。
  - Stockpile / Mixer: `distance(stand_pos, destination_pos) < TILE_SIZE * 1.8`
  - Blueprint: `stand_pos` が `occupied_grids` 外、かつ `distance(stand_pos, occupied_tile) < TILE_SIZE * 1.5`
- **手押し車運搬 (HaulWithWheelbarrow)**: 手押し車を使って複数アイテムをまとめて運搬する。以下の7フェーズを経る:
    1. `GoingToParking` — 駐車エリアへ移動
    2. `PickingUpWheelbarrow` — 手押し車を取得（`PushedBy` 設定）
    3. `GoingToSource` — 積み込み元（地面/備蓄を含むアイテム群）へ移動
    4. `Loading` — アイテムを手押し車に積む（`LoadedIn` 設定、`Visibility::Hidden`）
       - Blueprint 向け `Sand` では直採取モードがあり、同一 Soul がこのフェーズで砂をその場生成して積載します（別の `CollectSand` タスクは使わない）。
    5. `GoingToDestination` — 目的地へ移動（速度ペナルティ `SOUL_SPEED_WHEELBARROW_MULTIPLIER`）
    6. `Unloading` — 搬送先（Stockpile / Blueprint / Mixer）に荷下ろし
    7. `ReturningWheelbarrow` — 手押し車を駐車エリアに返却
- **水運搬 (HaulWater)**: Tankから水を汲み、MudMixerへ運ぶ一連のプロセス。
    - バケツ確保 -> Tankへ移動 -> 汲む -> Mixerへ移動 -> 注ぐ -> バケツ返却


### 4. 完了・放棄 (Completion / Abandonment)
- **完了**: 資源が消滅、または目的地に到達。`AssignedTask::None` に戻り、コンポーネントがクリーンアップされる。この際、`OnTaskCompleted` イベントが発行される。
- **放棄（ストレス）**: ストレス崩壊（`OnStressBreakdown`）時に即座に中断。
- **放棄（モチベーション）**: **やる気が 0.3 (30%) を下回った際**、自発的にタスクを放棄 (`OnTaskAbandoned`)。
- **放棄（疲労）**: 疲労限界（`OnExhausted`）時に即座に中断し、集会所へ移動。
- **解雇**: 使い魔からの指揮解除によって中断。
- **割り当て通知**: タスクが割り当てられた際には `OnTaskAssigned` イベントが発行される。

これらのイベントは Bevy 0.18 の **Observer** によって処理され、ログ出力や関連エンティティの状態更新が即座に行われます。

## 5. 重要なメンテナンスロジック

### `unassign_task`
タスクが中断された際、以下の処理を確実に行います：
- **グリッド中心への吸着 (Grid Snapping)**: アイテムをドロップする際、タイルの中央（真の中心）に座標を補正する。
- **タスク解除と予約解放**: `Designation` を削除するだけでなく、`SharedResourceCache` 上の予約も整合性を保つように管理される（基本的には `Sense` フェーズで自動リセットされるため、メモリリークは発生しない設計）。

### 座標系 (Coordinate System)
- マップ全体は **(MAP_WIDTH - 1) / 2.0 (50x50 なら 24.5)** を数学的中心として定義されている。
- タイルの中心とワールド座標の整数値が一致するように設計されており、これにより 1px の狂いもない表示と判定を実現している。

## 6. オートホール (Auto-Haul)
使い魔の `TaskArea`（担当エリア）内に **`Stockpile`（備蓄場所）** がある場合、その周辺の未指定資源を自動的に `Haul` タスクとして登録するシステム。
詳細は [logistics.md](logistics.md) を参照してください。
- 効率化のため空間グリッド（`ResourceSpatialGrid` および **`DesignationSpatialGrid`**）を利用して検索を行う。
- 型の一致（木材/石材）と備蓄場所の容量（最大10個）を厳格にチェックします。
- 同一フレーム内での過剰なタスク発行を抑えるため、予約済みの資源はスキップする。
- 数千の指示が存在する状況でも、使い魔はエリア内の未アサインタスクを O(1) に近い速度で発見できます。

## 7. 疲労とストレス (Vitals)

ワーカーの疲労、ストレス、やる気、およびそれらに基づく待機行動（休息、ブレイクダウン等）の詳細については、[soul_ai.md](soul_ai.md) を参照してください。

## 8. TaskArea編集UI（高頻度運用向け）

`Orders -> Area` から `TaskMode::AreaSelection` に入ると、使い魔の担当エリア（`TaskArea`）を連続編集できます。

### 入口と対象選択
- `Orders -> Area` 選択時、現在選択が Familiar でない場合は以下の優先順で対象 Familiar を自動選択
1. `TaskArea` をまだ持っていない Familiar
2. 全員が `TaskArea` を持っている場合は任意の Familiar（実装上は Entity index 最小）

### 編集フロー
- 新規指定: 左ドラッグで矩形を指定
- 直接編集: 既存 `TaskArea` の内部ドラッグで移動、辺/角ドラッグでリサイズ
- グリッド整列: 既存 `WorldMap::snap_to_grid_edge` の仕様に準拠

### モード遷移
- デフォルト: 適用後も `TaskMode::AreaSelection(None)` を維持（連続編集）
- `Shift + 左ボタンリリース`: 適用して通常モードへ復帰
- `Esc`: `PlayMode::Normal` へ復帰

### ショートカット（Areaモード中）
- `Tab` / `Shift + Tab`: Familiar のみ循環
- `Ctrl + Z`: Undo
- `Ctrl + Y` または `Ctrl + Shift + Z`: Redo
- `Ctrl + C` / `Ctrl + V`: `TaskArea` のコピー / ペースト
- `Ctrl + 1..3`: 現在エリアサイズをプリセット保存
- `Alt + 1..3`: プリセットサイズを現在 Familiar に適用（中心維持）

### 補助表示
- モードテキストとカーソル近傍プレビューに以下を表示
1. エリアサイズ（タイル数）
2. 現在のドラッグ状態（Move/Resize）
3. 他 Familiar の `TaskArea` との重複数・最大重複率
4. エリア内未割当タスク数（`Designation` かつ `Without<ManagedBy>`）
5. クリップボード状態（`Clip:Ready/Empty`）
- 高重複（最大重複率 50%以上）の場合、`WARN:HighOverlap` を表示

## 9. タスクエリアの視覚表現 (Task Area Visuals)

タスクエリア（`TaskArea`）は、カスタム WGSL シェーダーを用いて描画され、状況に応じた動的な視覚フィードバックを提供します。

### 階層とレイヤー
タスクエリアの表示は以下の優先順位で描画されます：
1. **境界線 (Border)**: 極細（1.0px）の実線または点線。
2. **コーナーマーカー**: 四隅を強調する L 字型のインジケータ。
3. **グラデーション (Vignette)**: 四隅から中心に向かって滑らかに広がる発光効果。

### 状態別フィードバック
エリアの状態に応じて、配色や透明度がリアルタイムに変化します：
| 状態 | 表現 |
|:---|:---|
| **Idle** | 低透明度、固定の境界線。 |
| **Hover** | 境界線が点線になり、視認性が向上。 |
| **Selected** | 境界線が強調（実線）され、塗りの透明度が上昇。 |
| **Editing** | 境界線が低周波でパルス（明滅）し、編集モードであることを強調。 |

### 配色の安定化
複数の使い魔が存在する場合、各使い魔に固有の配色が割り当てられます（詳細は [familiar_ai.md](familiar_ai.md) 参照）。これにより、重なり合うエリアの所属を一目で判別可能です。

## 10. UI
タスクリストの表示仕様については、[task_list_ui.md](task_list_ui.md) を参照してください。
