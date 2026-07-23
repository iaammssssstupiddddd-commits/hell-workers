# 使い魔 AI (Familiar AI)

使い魔（Familiar）は、「地獄の監督官」として配下の魂（Damned Soul）を管理し、仕事を効率的に進めさせるための AI を備えています。

## プレイヤー入力コマンド

Familiar command の keyboard edge は `bevy_app::input_actions` が context 解決し、
`systems/command/input.rs` は `ResolvedInputFrame` だけを読む。対象 Entity も resolver が frame 開始時の
`SelectedEntity` から snapshot した Familiar を使い、同 frame の world / Entity List click は抑止する。

- 非 pause、`PlayMode::Normal`、互換 `TaskMode` のときだけ有効。
- `C/M/H/B` と `Digit1-4` は Chop/Mine/Haul/Build、`Digit0/Delete` は指定キャンセル。Escape は
  `TaskMode::None` の Normal 時だけ Idle/Patrol toggle とし、non-`None` TaskMode は active owner cancel を優先する。
- B と数字キーは Familiar context を World menu / 時間操作より優先する。Pause、in-progress gesture、
  pending non-Normal mode、modal では Familiar command を生成せず、停止中の入力を次 frame へ保持しない。
- 複数 command chord を同 frame に押した場合は旧 `else if` 順の優先度で 1 action に絞る。

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

> **Note**: `FamiliarOperation.fatigue_threshold` は既存memberのreleaseとtask assignmentに使う。
> 新規recruitはそこから`FAMILIAR_RECRUIT_FATIGUE_HYSTERESIS (0.2)`を引いた閾値を使う。
> releaseが`0.0`（または`f32::EPSILON`以下）の場合は新規recruitを無効化し、進行中のScoutingも中止する。
> 正のrelease値では常に`recruit < release`となる。設定UIの`0% (Recruit Off)`がこの特殊値を表す。

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

4フェーズ（[ai-system-phases.md](ai-system-phases.md) 参照）。主要システム: Perceive=`detect_state_changes_system`/`sync_reservations_system`（予約 dirty 時は即時、0.2秒ごとの安全監査あり）, Decide=`familiar_ai_state_system`/`blueprint_auto_gather_system`（1.0秒）/`familiar_task_delegation_system`（0.5秒）, Execute=`familiar_state_apply_system`/`apply_squad_management_requests_system`等。

### 5.2. 主要モジュール

**root 残留（意図的な残留）**

| ファイル | 区分 | root 残留理由 |
|:---|:---|:---|
| `perceive/resource_sync.rs` | root perceive system | `SharedResourceCache` snapshot と `ReservationSignatureCache` を実ワールドの `AssignedTask` / `Designation` / `TransportRequest` / relationship から同期するのは root の責務。`apply_reservation_op` / `apply_reservation_requests_system` は **`hw_logistics` に移設済み** |
| `mod.rs` | root wiring | `configure_sets`・`FamiliarAiCorePlugin` と root reservation resources（`SharedResourceCache` / `ReservationSyncTimer` / `ReservationSignatureCache`、profiling 時の metrics）の登録を担当（SpatialGrid の `init_resource` は `SpatialPlugin` に移設済み） |

**thin re-export（削除済み: 2026-03-22）**

以下の `decide/`・`execute/`・`helpers/`・`update/` 各ファサードモジュールはすべて削除済み。
callers は `hw_familiar_ai::*` の完全パスを直接参照する。

- `decide/mod.rs`・`helpers/mod.rs`・`decide/state_handlers/`・`decide/squad.rs`・`decide/recruitment.rs` 等 → `hw_familiar_ai::familiar_ai::decide::*`
- `perceive/state_detection` submodule → `hw_familiar_ai::familiar_ai::perceive::state_detection::*`
- `execute/mod.rs`（`encouragement_apply_system` / `squad_visual_system` / `max_soul_visual_system` 等）→ `FamiliarAiCorePlugin` が直接登録済み
- `helpers/mod.rs`（`FamiliarStateQuery` / `FamiliarSoulQuery` 等）→ `hw_familiar_ai::familiar_ai::decide::query_types`
- `update/mod.rs` → `hw_familiar_ai` 直接

**設計メモ**
- ECS 実状態の変更は `execute/` が担当する
- Decide フェーズの message 出力と world/grid/pathfinding を使うオーケストレーションは `hw_familiar_ai` が所有する
- root に残すのは `GameAssets` 依存 visual と `SharedResourceCache` 再構築のような app shell 固有処理だけに限定する

**hw_familiar_ai 分担**: `FamiliarAiPlugin` は `hw_familiar_ai::FamiliarAiCorePlugin` を内部で `add_plugins` する。`WorldMapRead` / `WalkabilityConnectivityCache` / SpatialGrid / `MessageWriter` を使う Familiar Decide 系 system も `hw_familiar_ai` 側で所有する。

- `FamiliarAiCorePlugin` が直接登録するもの：
  - **Resources**: `FamiliarTaskDelegationTimer` / `FamiliarDelegationPerfMetrics` / `hw_world::WalkabilityConnectivityCache` / `BlueprintAutoGatherTimer`
  - **RegisterType**: `FamiliarAiState` / `EncouragementCooldown`
  - **Perceive**: `detect_state_changes_system` / `detect_command_changes_system`
  - **Decide**: `following_familiar_system`（独立）、`state_decision → ApplyDeferred → blueprint_auto_gather → ApplyDeferred → task_delegation → encouragement_decision`（chain）
  - **Execute**: `familiar_state_apply_system` / `handle_state_changed_system` / `max_soul_logic_system` / `squad_logic_system` / `encouragement_apply_system` / `cleanup_encouragement_cooldowns_system`
- `hw_familiar_ai` は `hw_soul_ai` に依存しない。分隊解放・使役数超過リリース時のタスク解除は `SoulTaskUnassignRequest`（`hw_core::events`）イベントを `MessageWriter` で送信し、`hw_soul_ai` 側の `handle_soul_task_unassign_system`（`SoulAiSystemSet::Perceive`）が処理する。
- root に残るのは `perceive/resource_sync`（ECS 実状態の再構築）と `configure_sets` の配線のみ
- `ConstructionSiteAccess` は **`hw_jobs::construction`** に移設済み（`hw_soul_ai` ではない）
- Blueprint auto gather の純計画層は `decide/auto_gather_for_blueprint/{planning,demand,supply,helpers}` に置き、orchestration 本体は `hw_familiar_ai::decide::blueprint_auto_gather` が担う

**プラグイン登録**: `FamiliarAiPlugin` は `crates/bevy_app/src/plugins/logic.rs` の `LogicPlugin` 内で登録される（`SoulAiPlugin` と同所）。

### 5.2. 関連コンポーネント

- `Familiar`: 使い魔の基本パラメータ（Radius, Speed 等）を保持。
    - `color_index`: 個体ごとに割り当てられた配色インデックス（0〜3）。タスクエリア等の描画に使用。
- `FamiliarOperation`: 指揮下に入れる最大人数や、既存memberを解放する疲労しきい値を保持。
  `recruit_fatigue_threshold()`が新規recruit用の`Option<f32>`を導出する。このruntime componentは現在save対象ではなく、load時にdefaultで再構築される。
- `ActiveCommand`: プレイヤーからの直接命令（Idle / Gather / Task）。
- `FamiliarAiState`: AI の現在の状態（Idle, SearchingTask, Scouting, Supervising）。
- `Commanding` (Relationship): 配下の魂への参照リスト。**オプショナル**（分隊が空のとき削除される）。
- `ManagedTasks` (Relationship Target): 管理下のタスクリスト。**オプショナル**（タスクがゼロのとき削除されるため、AI クエリでは `Option` として扱う）。
- `AssignedTask`: 魂が現在実行中のタスク（採取・運搬・建築）を管理。`hw_jobs::AssignedTask` として公開され、定義は `crates/hw_jobs/src/tasks/mod.rs`、各 payload は同ディレクトリの機能別ファイルに置く。
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
- **仕組み**: Perceiveフェーズで初回、reservation signature の差分、pending task 側の変更/削除、または **0.2秒の安全監査**時に snapshot を再構築し、各フレームの更新は `ResourceReservationRequest` を通じて反映されます。signature は active reservation operation だけを比較するため、進捗値の更新は再構築しません。
- **cache 境界**: `begin_frame()` は frame-local の pickup/store delta だけを clear し、snapshot 置換は delta を保ったまま予約 map だけを更新します。
- **load**: `ReservationSignatureCache` と同期 timer も cache と同時に reset され、次の Perceive が完全 snapshot を構築します。
- **境界**: `apply_reservation_op` / `apply_reservation_requests_system` の実装は `hw_logistics` にあるが、`SharedResourceCache` / `ReservationSignatureCache` の `init_resource` と `ResourceReservationRequest` の `add_message` は app shell が担当します。
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

### 7.5. 委譲候補の一回収集と到達判定 cache
委譲処理を「Familiar単位の候補収集」と「worker単位の再スコア・到達判定」に分割し、重複計算を削減しました。
- **候補一回化**: `collect_scored_candidates` を1回だけ実行し、全アイドルワーカーで候補を使い回します。
- **Worker基準再スコア**: 候補ごとに worker 距離を再評価し、worker ごとに最適候補順を作成します。
- **方針スコアの合成**: base score は従来どおり priority `0.65` + 距離 `0.35` で計算し、その後に
  policy contribution を一度だけ加えます。policy-driven `DepositToStockpile` は保存しない
  `ReceiverPolicyTier` から Low=-10 / Normal=0 / High=+10 / Critical=+20 unit を得て、1 unit は
  `0.65 / 40` です。最終 score は clamp せず、同じ合成済み score を Top-K と fallback の双方で使います。
  これにより Normal は従来値と bit 単位で一致し、base priority が上限でも tier 差を維持します。
- **優先度の分離**: manual haul の明示 priority や consolidation の maintenance 用 raw priority を
  receiver policy と推測して通常候補へ再加算しません。B1/B2 の contribution は
  `hw_jobs::Priority` へコピーせず、共有 scalar helper で合成します。
- **距離フィルタ**: `MAX_ASSIGNMENT_DIST_SQ`（60タイル）を超える候補は連結成分判定前に除外します。
- **version付き連結成分 cache**: `hw_world::WalkabilityConnectivityCache` が `WorldMap.obstacle_version` ごとに dense component ID 配列を一度だけ構築し、worker/target の Boolean 到達判定を O(1) にします。`WorldMap` の Bevy 変更検知や 60 frame TTL で全消去する旧 `ReachabilityFrameCache` は廃止しました。Open/Closed Door は cache を再構築せず、Locked 等の walkability topology 変更だけが次回問い合わせの flood-fill を発火します。save/load の world replacement では cache を明示 reset します。
- **Top-K 先行評価**: 優先候補（`TASK_DELEGATION_TOP_K`）を先に評価し、必要時のみ残り候補を評価します。
- **複数同時割り当て（仮想ワーカー追跡）**: 1回の委譲サイクル内で同一タスクへ複数ワーカーを同時割り当て可能です。`task_virtual_workers: HashMap<Entity, usize>` でサイクル内の仮想割り当て数を追跡し、スロット判定を `current_workers(ECS) + virtual_workers >= max_slots` で行います。これにより、壁建設で木材×10が必要な場合でも1サイクルで最大10体を同時発行でき、以前の「1体/0.5秒」による最大5秒の遅延を解消します。過剰割り当ては `ReservationShadow` によって引き続き防止されます。

### 7.5.1. 資材ソース探索の近傍優先化
- **地面資材**: `ResourceSpatialGrid` から近傍候補を取り出し、半径 `10 -> 20 -> 40 -> 80` タイルの順で探索範囲を段階拡張します。近傍に候補がない場合のみ全域相当の半径へフォールバックします。
- **ストックパイル内資材**: `(ResourceType, Stockpile)` 単位のフレームキャッシュを用い、必要な資材型とセルに限定して参照します。
- **所有互換性**: owner 付きストックパイルは同 owner の地面資材を優先し、見つからない場合のみ owner 未設定資材へフォールバックします。重複 Yard の mixed-owner group では、anchor ではなく実際に選んだ destination cell の owner をソース条件に使います。
- **安全条件**: `StoredIn` 付きアイテムと予約済みアイテムは地面ソース候補から除外します。

### 7.5.2. Stockpile 方針の割当直前再検証

- `DepositToStockpile` は request の tier subset 内だけを対象にし、live policy、contents、incoming、同 cycle shadow を
  `NewInbound` evaluator で再評価します。producer 後に target や acceptance が変わった stale request は割り当てません。
- `WheelbarrowLease` がある request は lease に記録された Stockpile だけを再検証します。無効になった lease 先から
  同じ group の別セルへ destination を付け替えません。
- `ConsolidateStockpile` は receiver の `NewInbound`、donor の `NewOutbound` と owner を再検証します。
  実 source item の owner が receiver と非互換、または donor が搬出禁止かつ draining でない場合は新規割当を止めます。

### 7.6. 状態遷移の自動検知（Bevy 標準機能の活用）
`Changed<FamiliarAiState>` フィルタを使用して状態遷移を自動検知します。
- **仕組み**: Bevy の `Changed<T>` フィルタにより、変更されたエンティティのみを処理
- **効果**: 毎フレーム全使い魔の状態をチェックするコストを排除し、変更時のみ処理を実行
- **イベント**: 状態遷移時に `FamiliarAiStateChangedEvent` を発火し、他のシステムが反応可能

### 7.7. タスク委譲のタイマーゲート
- **仕組み**: `familiar_task_delegation_system` は **0.5秒間隔（初回即時）** で実行されます。
- **効果**: タスク候補ごとの Boolean 到達判定は連結成分 cache を使うため、実経路生成 A* を起動せずに判定できます。timer は候補収集・スコアリングの頻度を抑制します。

### 7.8. Blueprint / WallConstruction / Mixer不足資材の自動Gather
- **仕組み**: `blueprint_auto_gather_system` が **1.0秒間隔（初回即時）** で実行され、`DeliverToBlueprint` request（Wood / Rock）、`DeliverToWallConstruction` request（Wood）、`DeliverToMixerSolid` request（Rock）から不足を検知します。
- **オーナー**: Active な Familiar と **Yard エンティティの両方**を `owner_infos` に登録して需要・供給を集計します。Tree/Rock と地面資材は同じ resource に正の需要がある owner を優先して単一 owner へ結び付け、該当需要がない場合だけ位置ベース解決へ戻ります。Yard エンティティの `path_start` はヤード中心の最寄り歩行可能グリッドで算出されます。
- **探索順**: `TaskArea`（または Yard 境界）内 -> 外周 10 タイル -> 30 -> 60 -> 到達可能な全域の順で候補を走査し、近傍優先で決定します。
- **負荷制御**: 各段階で連結成分による到達判定件数に上限を設け、必要量が満たされた時点で探索を打ち切ります。判定そのものは waypoint A* を起動しません。
- **整合性**: 既存の地面資材・手動指定・既発行AutoGatherを加味して過剰発行を抑制し、不要になった未着手AutoGatherは marker ベースで回収します。
- **到達性と代替資材**: 地面資材と既存指定は owner から到達可能で、地面資材は `DeliveringTo` のない未予約状態、手動未所有指定は task finder の探索範囲にある場合だけ供給として数えます。Bridge の flexible Wood/Rock 需要は到達可能な既存供給・候補へ配分し、到達不能な Wood で reachable Rock の `Mine` を妨げません。
- **発見性と順序**: Yard-owned `Chop` / `Mine` は Yard 外でも補助全件走査から候補になります。AutoGather の Commands は `ApplyDeferred` で確定してから同じ Decide chain の task delegation が実行されます。

### 7.9. latest-only タスク候補診断

`familiar_task_delegation_system` は通常の 0.5 秒 cycle で `FamiliarTaskCandidateDiagnostics` を作り、
前 cycle の map を置換する。dashboard 表示の有無は探索回数や割り当て判断を変えない。

- candidate universe は空間 index、`ManagedTasks`、Yard-owned / global Build 補助 scan を統合して重複排除した集合。
  static filter より前に membership を記録するため、材料待ちなどで落ちた task も applicable evaluator として扱える。
- `CandidateRejectReason` は AI 内部の typed 分岐で、UI へは 5 つの `TaskDiagnosticClass` だけを公開する。
  `MalformedTask` / `StaleInput` / `Unevaluated` は coverage を partial にし、推測した blocker を作らない。
- 1 Familiar 内では worker / source の分岐回数ではなく reason presence を固定順で 1 票へ縮約する。
  `submitted → partial → idle worker 0 → representative reason` の順で判定する。
- assignment builder が `TaskAssignmentRequest` を実際に writer へ渡した場合だけ submitted とする。
  submitted は accepted ではないため、root UI は current `TaskWorkers` が付くまで Pending と表示する。
- worker 距離外、walkable start 不在、connectivity false、slot、依存フェーズ、source / capacity / reservation、
  wheelbarrow arbitration の typed outcome を既存判定経路で記録する。Top-K や先行成功で未評価の候補は partial。

`FamiliarTaskDecisionSet` は `BlueprintAutoGather → AutoGatherFlush → TaskRevisionSync → Delegation` を named seam として
公開する。root revision bridge は `ResourceSpatialGrid::generation()`、`SharedResourceCache::semantic_generation()`、
  roster/task change、保管・搬入・所持品・車両・設備容量の availability change、`WorldMap.obstacle_version` を同期してから
delegation を実行する。Soul の roster revision は `AssignedTask`、所有者、休息・breakdown、疲労・Dream の
作業可否境界だけで進め、idle timer や閾値内の疲労変動では進めない。代表理由は
`NoEligibleFamiliar=task+roster`、資源/競合=`task+availability`、依存待ち=`task`、
`Unreachable=task+topology` の domain mask を持つ。producer header の roster stamp は evaluator coverage 全体を
失効させるため、構成人数が変わった旧 cycle を再利用しない。
load 時は revisions と snapshot を default に戻し、新 world の最初の通常 cycle から再評価する。

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
