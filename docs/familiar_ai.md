# 使い魔 AI (Familiar AI)

使い魔（Familiar）は、「地獄の監督官」として配下の魂（Damned Soul）を管理し、仕事を効率的に進めさせるための AI を備えています。

## 1. AI 状態 (FamiliarAiState)

使い魔は以下の 4 つの状態を持ち、状況に応じて遷移します。

| 状態 (State) | 説明 |
| :--- | :--- |
| **`Idle`** | プレイヤーからの命令が `Idle` の状態。その場に留まります。 |
| **`SearchingTask`** | 次の仕事（Designation）を探している状態。担当エリアを巡回します。 |
| **`Scouting`** | 遠方のフリーの魂をリクルートするために接近している状態。 |
| **`Supervising`** | 配下の魂を監視し、仕事の進捗を管理している状態。 |

## 2. 使役とリクルート (Recruitment)

使い魔は最大 `max_controlled_soul`（個体差あり）名までの部下を持つことができます。

### 使役数の上限変更

使い魔の使役数上限（`max_controlled_soul`）は、オペレーションダイアログから変更可能です（範囲: 1〜8）。

- **イベント駆動**: 使役数の変更は `FamiliarOperationMaxSoulChangedEvent` イベントで通知されます。
- **自動リリース**: 使役数を減少させた場合、超過分の魂が自動的にリリースされます。
  - タスクが割り当てられている場合は、タスクを解除してからリリースされます。
  - リリース時には使い魔がフレーズを表示します。
- **パフォーマンス**: 毎フレームチェックではなく、変更時のみ処理が実行されるため、パフォーマンスに優れています。

### リクルート条件

魂がリクルート対象となるための条件（詳細は [soul_ai.md](soul_ai.md) 参照）：

1. **未使役**: `CommandedBy` コンポーネントがないこと
2. **タスクなし**: `AssignedTask::None` であること
3. **バイタル良好**: 疲労・ストレスが閾値以下であること
4. **休息中でない**: `RestingIn` が付与されておらず、`IdleBehavior` が `Resting` / `GoingToRest` / `ExhaustedGathering` でないこと
5. **休憩クールダウン中でない**: `RestAreaCooldown.remaining_secs <= 0.0`

> **Note**: リクルート閾値はリリース閾値より低下させて設定されており、リクルート直後にリリースされることを防ぎます。

### リクルート挙動

- **即時リクルート**: `command_radius` 内に条件を満たす魂がいる場合、即座に配下に入れます。
- **複合スコア選定**: 候補は「距離 0.40 + 疲労余裕 0.30 + TaskArea 方向 0.15 + モチベーション 0.15」でスコア化し、最高スコアを選びます。TaskArea がない場合は方向スコアが 0.5 固定になりますが、リクルート自体は行われます。
- **スカウト挙動**: 段階的に検索範囲を拡大（20→40→80→160タイル）して候補を探索し、各段階の上位候補を比較します。`RECRUIT_GOOD_ENOUGH_SCORE(0.72)` 以上が見つかった段階で探索を打ち切ります。
- **リクルート遷移**: リクルート成功後、まだ空きがあれば再び `SearchingTask` に戻って仲間を探します。満員になった場合のみ `Supervising` に移行します。
- **Idle コマンド時のリクルート**: `FamiliarCommand::Idle`（TaskArea なし含む）でも `max_controlled_soul` を目標にリクルートを継続します。

## 3. 監視ロジック (Supervising)

監視モードでは、部下との「適切な距離」を保ちながら行動します。

### ターゲット選定
- **作業者優先**: 部下の中で「現在タスクを実行中（`AssignedTask` が `None` 以外）」の者を優先的にターゲットにします。
- **固定タイマー (Sticky Target)**: 頻繁なターゲット切り替えによるカクつきを防ぐため、一度ターゲットを決めたら **2.0 秒間** はその魂を注視し続けます。

### 誘導 (Guidance)
- **エリアへの回帰**: 全員が待機（Idle）状態で、かつ担当エリアの中心から **1.5 タイル** 以上離れている場合、使い魔はエリアの中心へ移動を開始します。部下はこれに合わせてエリア内へ誘導されます。

- **停止**: ターゲットとの距離が **3.0 タイル** 以内になったら停止。

### 3.4. 激励 (Encouragement) System
監視モード中、使い魔はランダムなタイミングで配下の魂を「激励」することがあります。
- **発動条件**: `Supervising` 状態かつ `Idle` 命令以外。
- **効果**: 対象一人に対し、**やる気 +2.5%** のボーナスを与える。
- **コスト**: 対象の **ストレス +1.25%** 増加。
- **演出**: 使い魔から「🔥」「⚡」等の激励絵文字、魂から「💪」または「😓」のリアクションが表示されます。
- **制限**: 同一の魂に対し **30秒間** のクールダウンがあります。

## 4. 移動と経路探索の最適化

使い魔の動きをスムーズにするため、以下の制御が行われています。

- **パス更新ガード**: 目的地が **0.5 〜 1.0 タイル** 以上変化しない限り、新しい経路（Path）を再計算しません。これにより、微小な移動に伴う「ガタつき」を排除しています。
- **明示的なパス解放**: 停止距離に到達した際、即座に経路情報をクリアすることで、ターゲット周囲での「足踏み」を防いでいます。

## 5. システム構造

`familiar_ai` システムは、保守性と可読性を向上させるため、以下のモジュールに分離されています：

### 5.1. 実行サイクル (Execution Cycle)

4フェーズ（[ai-system-phases.md](ai-system-phases.md) 参照）。主要システム: Perceive=`detect_state_changes_system`/`sync_reservations_system`（0.2秒）, Decide=`familiar_ai_state_system`/`blueprint_auto_gather_system`（1.0秒）/`familiar_task_delegation_system`（0.5秒）, Execute=`familiar_state_apply_system`/`apply_squad_management_requests_system`等。

### 5.2. 主要モジュール

**root orchestration（意図的な残留）**

| ファイル | 区分 | root 残留理由 |
|:---|:---|:---|
| `decide/auto_gather_for_blueprint.rs` | root orchestration | `Commands` / pathfinding / entity query / designation 付与・cleanup を担う。pure 計画層は hw_familiar_ai へ委譲 |
| `decide/auto_gather_for_blueprint/actions.rs` | root helper | designation marker 付与・回収など world 反映責務を持つ |
| `decide/auto_gather_for_blueprint/helpers.rs` | root helper | `is_reachable` は `WorldMap` + `PathfindingContext` 依存。pure helper（`OwnerInfo`, `resource_rank` 等）は hw_familiar_ai 側に抽出済みで re-export 済み |
| `perceive/resource_sync.rs` | root perceive system | `SharedResourceCache` 再構築・`AssignedTask`/`Designation`/`TransportRequest`/relationship の実ワールド再構築は root の責務。`apply_reservation_op` / `apply_reservation_requests_system` は **`hw_logistics` に移設済み** |

**thin re-export（実装は hw_familiar_ai に移設済み）**

| ファイル | 内容 |
|:---|:---|
| `decide/state_decision.rs` | `familiar_ai_state_system` 実体は `hw_familiar_ai::decide::state_decision` に移設済み。登録も `FamiliarAiCorePlugin` に移設済み |
| `decide/task_delegation.rs` | `familiar_task_delegation_system` / `ReachabilityFrameCache` / `ReachabilityCacheKey` の実体は `hw_familiar_ai::decide::task_delegation` / `resources` に移設済み。登録も `FamiliarAiCorePlugin` に移設済み |
| `decide/familiar_processor.rs` | `FamiliarDelegationContext` / `process_task_delegation_and_movement` 実体は `hw_familiar_ai::decide::delegation_context` に移設済み |
| `helpers/query_types.rs` | `FamiliarStateQuery` / `FamiliarSoulQuery` / `FamiliarTaskQuery` および narrow 5型すべて `hw_familiar_ai::decide::query_types` への re-export |
| `decide/state_handlers/` | `hw_familiar_ai::familiar_ai::decide::state_handlers` への thin re-export |
| `decide/squad.rs` | `SquadManager` 実体は `hw_familiar_ai` にある |
| `decide/recruitment.rs` | `RecruitmentManager` 実体は `hw_familiar_ai` にある |
| `decide/encouragement.rs` | 対象選定本体は hw_familiar_ai にある。root adapter は system 関数のみ追加 |
| `decide/scouting.rs` / `decide/supervising.rs` | スカウト・監視ロジック本体は hw_familiar_ai にある |
| `decide/auto_gather_for_blueprint/{demand,supply,planning}.rs` | 各 1 行 re-export。需要供給計画本体は hw_familiar_ai にある |
| `perceive/state_detection.rs` | `Changed<FamiliarAiState>` 検知実体は **hw_familiar_ai に移設済み** |
| `decide/task_management/` | thin bridge。実体は `hw_familiar_ai::familiar_ai::decide::task_management` にある |

**execute（visual / relationship apply）**

`squad_apply.rs` / `max_soul_apply.rs` / `idle_visual_apply.rs` / `encouragement_apply.rs`

**設計メモ**

- ECS 実状態の変更は `execute/` が担当する
- Decide フェーズの request message 発行は root adapter が担当し、`hw_familiar_ai` 側は `ScoutingOutcome` / `SquadManagementOutcome` のような pure outcome を返す
- root の `state_decision.rs` / `task_delegation.rs` / `encouragement.rs` / `auto_gather_for_blueprint.rs` は concrete resource・pathfinding・message 出力を受け持ち、必要な view と context だけを `hw_familiar_ai` ロジックへ渡す

**hw_familiar_ai 分担**: `FamiliarAiPlugin` は `hw_familiar_ai::FamiliarAiCorePlugin` を内部で `add_plugins` する。`WorldMap`/SpatialGrid 依存のシステムは root 残留。

- `FamiliarAiCorePlugin` が直接登録するもの：
  - **Resources**: `FamiliarTaskDelegationTimer` / `FamiliarDelegationPerfMetrics` / `ReachabilityFrameCache`
  - **Perceive**: `detect_state_changes_system` / `detect_command_changes_system`
  - **Decide**: `following_familiar_system`（独立）、`state_decision → ApplyDeferred → task_delegation`（chain）
  - **Execute**: `familiar_state_apply_system` / `handle_state_changed_system`、`EncouragementCooldown` の type registration
- root に残るのは `auto_gather_for_blueprint`（Commands 依存）/ `encouragement`（system 関数のみ）/ `perceive/resource_sync`（ECS 実状態の再構築）と SpatialGrid の `init_resource`・`configure_sets` の配線のみ
- `ConstructionSiteAccess` は **`hw_jobs::construction`** に移設済み（`hw_soul_ai` ではない）
- Blueprint auto gather の純計画層は `decide/auto_gather_for_blueprint/{planning,demand,supply,helpers}` に置き、root は orchestration だけを担う

**プラグイン登録**: `FamiliarAiPlugin` は `crates/bevy_app/src/plugins/logic.rs` の `LogicPlugin` 内で登録される（`SoulAiPlugin` と同所）。

### 5.2. 関連コンポーネント

- `Familiar`: 使い魔の基本パラメータ（Radius, Speed 等）を保持。
    - `color_index`: 個体ごとに割り当てられた配色インデックス（0〜3）。タスクエリア等の描画に使用。
- `FamiliarOperation`: 指揮下に入れる最大人数や、魂を解雇する疲労しきい値を保持。
- `ActiveCommand`: プレイヤーからの直接命令（Idle / Gather / Task）。
- `FamiliarAiState`: AI の現在の状態（Idle, SearchingTask, Scouting, Supervising）。
- `Commanding` (Relationship): 配下の魂への参照リスト。**オプショナル**（分隊が空のとき削除される）。
- `ManagedTasks` (Relationship Target): 管理下のタスクリスト。**オプショナル**（タスクがゼロのとき削除されるため、AI クエリでは `Option` として扱う）。
- `AssignedTask`: 魂が現在実行中のタスク（採取・運搬・建築）を管理。`hw_jobs::assigned_task` で定義（`crates/bevy_app/src/systems/soul_ai/execute/task_execution/types.rs` 経由で re-export）。
- `IdleState`: 待機中の振る舞いを管理。`crates/bevy_app/src/entities/damned_soul/mod.rs` で定義。

## 6. 分隊が空になったときの挙動

分隊員が全員解放された場合（疲労・ストレス崩壊など）、使い魔は以下のように動作します：

- **スカウト中 (`Scouting`)**: ターゲットへの接近を継続し、リクルートを完了させます。
- **監視中 (`Supervising`)**: 自動的に `SearchingTask` に遷移し、新しい仲間を探します。

> **実装メモ**: Bevy の ECS Relationship システムでは、最後の `CommandedBy` が削除されると `Commanding` も自動削除されます。そのため、クエリでは `Option<&Commanding>` を使用し、`None` の場合は空の分隊として扱います。
## 7. パフォーマンス最適化 (Performance Optimization)

大規模な地獄（数百の魂、数千のタスク指示）でも FPS を維持するため、以下の最適化が行われています。

### 7.1. 共有リソースキャッシュ (SharedResourceCache)
タスク間のリソース競合を O(1) で管理します。従来の `HaulReservationCache` を統合・拡張したものです。
- **仕組み**: Perceiveフェーズで **0.2秒間隔（初回即時）** に再構築され、各フレームの更新は `ResourceReservationRequest` を通じて反映されます。
- **境界**: `apply_reservation_op` / `apply_reservation_requests_system` の実装は `hw_logistics` にあるが、`SharedResourceCache` の `init_resource` と `ResourceReservationRequest` の `add_message` は app shell が担当します。
- **機能**: 
  - **Destination Reservation**: 搬送先（ストックパイル、タンク、ミキサー）への予約。
  - **Source Reservation**: アイテム（拾う対象）の重複予約防止。
  - **Intra-frame Tracking**: 同一フレーム内での在庫変動（格納・取り出し）を追跡し、コマンド適用前の論理在庫を正確に把握します。

### 7.1.1. TaskQueries の分割
タスク割り当てとタスク実行で必要なクエリを分離し、システム並列性の阻害を抑えています。
- **`FamiliarTaskAssignmentQueries`**: Familiar AI の割り当てに必要なクエリを集約。定義本体は `hw_familiar_ai::familiar_ai::decide::task_management` にあり、root 側は re-export と construction site bridge を提供する
- **`TaskAssignmentQueries`**: Soul AI 側の割り当て適用・解除で使う full query を集約
- **`TaskQueries`**: Soul AI のタスク実行に必要なクエリを集約

### 7.2. タスク用空間グリッド (DesignationSpatialGrid)
未割り当てのタスク（伐採、採掘、運搬等）を座標ベースで高速検索します。
- **仕組み**: 指定エリア（`TaskArea`）に重なるグリッドセルのみを走査。
- **効果**: 全タスクをイテレートしてエリア判定を行うコスト (O(T)) を排除しました。数千の指示があっても、使い魔は即座に近くの仕事を発見できます。

### 7.3. 段階的リクルート検索
スカウト時の検索範囲を 20 → 40 → 80 → 160 タイルと段階的に拡大します。
- **効果**: 毎フレーム広範囲を検索するのではなく、近い場所から順に探して「十分スコア（0.72）」で早期終了することで、1フレームあたりのクエリ負荷を分散させています。

### 7.4. 使役数上限変更のイベント駆動処理
使役数の上限変更をイベント駆動で処理します。
- **仕組み**: UIで使役数が変更されたときのみ `FamiliarOperationMaxSoulChangedEvent` を発火し、超過分の魂をリリースします。
- **効果**: 毎フレーム全使い魔の使役数をチェックするコストを排除し、変更時のみ処理を実行することでパフォーマンスを向上させています。

### 7.5. 委譲候補の一回収集と到達判定キャッシュ
委譲処理を「Familiar単位の候補収集」と「worker単位の再スコア・到達判定」に分割し、重複計算を削減しました。
- **候補一回化**: `collect_scored_candidates` を1回だけ実行し、全アイドルワーカーで候補を使い回します。
- **Worker基準再スコア**: 候補ごとに worker 距離を再評価し、worker ごとに最適候補順を作成します。
- **距離フィルタ**: `MAX_ASSIGNMENT_DIST_SQ`（60タイル）を超える候補は A* 前に除外します。
- **フレーム共有キャッシュ**: `ReachabilityFrameCache`（`(worker_grid, target_grid)` キー）で A* 結果をフレーム間共有し、`WorldMap` 変更時または 60 委譲サイクルごとに安全クリアします。
- **Top-K 先行評価**: 優先候補（`TASK_DELEGATION_TOP_K`）を先に評価し、必要時のみ残り候補を評価します。
- **複数同時割り当て**: 1回の委譲処理で複数ワーカーを割り当て可能になり、アイドル解消の遅延を抑制します。

### 7.5.1. 資材ソース探索の近傍優先化
- **地面資材**: `ResourceSpatialGrid` から近傍候補を取り出し、半径 `10 -> 20 -> 40 -> 80` タイルの順で探索範囲を段階拡張します。近傍に候補がない場合のみ全域相当の半径へフォールバックします。
- **ストックパイル内資材**: `(ResourceType, Stockpile)` 単位のフレームキャッシュを用い、必要な資材型とセルに限定して参照します。
- **所有互換性**: owner 付きストックパイルは同 owner の地面資材を優先し、見つからない場合のみ owner 未設定資材へフォールバックします。
- **安全条件**: `StoredIn` 付きアイテムと予約済みアイテムは地面ソース候補から除外します。

### 7.6. 状態遷移の自動検知（Bevy 標準機能の活用）
`Changed<FamiliarAiState>` フィルタを使用して状態遷移を自動検知します。
- **仕組み**: Bevy の `Changed<T>` フィルタにより、変更されたエンティティのみを処理
- **効果**: 毎フレーム全使い魔の状態をチェックするコストを排除し、変更時のみ処理を実行
- **イベント**: 状態遷移時に `FamiliarAiStateChangedEvent` を発火し、他のシステムが反応可能

### 7.7. タスク委譲のタイマーゲート
- **仕組み**: `familiar_task_delegation_system` は **0.5秒間隔（初回即時）** で実行されます。
- **効果**: タスク候補ごとの到達可能性チェック（A*）の呼び出し頻度を抑制し、ピーク時のCPU負荷を削減します。

### 7.8. Blueprint / Mixer不足資材の自動Gather
- **仕組み**: `blueprint_auto_gather_system` が **1.0秒間隔（初回即時）** で実行され、`DeliverToBlueprint` request（Wood / Rock）と `DeliverToMixerSolid` request（Rock）から不足を検知します。
- **オーナー**: Active な Familiar と **Yard エンティティの両方**を `owner_infos` に登録して需要・供給を集計します。Yard エンティティの `path_start` はヤード中心の最寄り歩行可能グリッドで算出されます。
- **探索順**: `TaskArea`（または Yard 境界）内 -> 外周 10 タイル -> 30 -> 60 -> 到達可能な全域の順で候補を走査し、近傍優先で決定します。
- **負荷制御**: 各段階で経路判定件数に上限を設け、必要量が満たされた時点で探索を打ち切ります。
- **整合性**: 既存の地面資材・手動指定・既発行AutoGatherを加味して過剰発行を抑制し、不要になった未着手AutoGatherは marker ベースで回収します。

## 8. ビジュアルとアニメーション (Visuals & Animation)

使い魔の移動状況に応じて、視覚的なフィードバックを提供します。

### 8.1. 移動アニメーション
- **スプライト**: `familiar_spritesheet.png` を使用（3フレーム）。
- **更新レート**: **5 FPS**（約0.2秒ごとにフレーム切り替え）。
- **挙動**:
    - **移動中**: フレーム 0→1→2 のループ再生。
    - **停止中**: フレーム 0（待機ポーズ）で固定。

### 8.2. 向きの制御 (Flipping)
- 使い魔の移動方向（ベクトルの X 成分）に基づいて、スプライトを左右反転させます。
- **左向き (デフォルト)**: `flip_x = false`
- **右向き**: `flip_x = true`

### 8.3. オーラ演出
- 使い魔の周囲には、指揮範囲を示す 3 つのレイヤーのオーラが表示されます：
    1. **Border**: 指揮範囲の境界を示す固定枠。
    2. **Pulse**: 内側でゆっくりと拡大縮小を繰り返すパルス演出。
    3. **Outline**: エンティティ選択時に表示される強調用のアウトライン。

### 8.4. 個体別の配色安定化 (Visual Personalization)
複数の使い魔を配置した際の視認性を向上させるため、スポーン時に各使い魔へ固有の色が割り当てられます。

- **割り当てロジック**: `FamiliarColorAllocator` リソースにより、新規スポーンごとにインデックスが `0 -> 1 -> 2 -> 3` と順番に循環して割り当てられます。
- **固定**: 一度割り当てられた `color_index` はコンポーネントに保持され、その個体が存在し続ける限り色が維持されます。
- **パレット**: 地獄のテーマに合わせた 4 色が定義されています。
  - **紫 (Purple)**: 深淵の魔力
  - **黄橙 (Yellow-Orange)**: 業火の熱
  - **毒緑 (Toxic Green)**: 腐敗の硫黄
  - **真紅 (Crimson Red)**: 鮮烈な流血
- **適用範囲**: タスクエリア（`TaskArea`）の境界線やグラデーションは、この `color_index` に基づいた色で描画されます。
