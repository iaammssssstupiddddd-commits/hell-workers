# ワーカー AI (Soul AI)

ワーカー（Damned Soul）は、使い魔の指揮下で働く、または自律的に行動するための AI とバイタル（生体パラメータ）を備えています。

## 1. バイタルシステム (Vitals)

ワーカーには「疲労」「ストレス」「やる気」の 3 つの主要パラメータがあり、行動に影響を与えます。

### 1.1. 疲労 (Fatigue)
活動に応じて蓄積し、限界に達すると休息を必要とします。
- **増加**: タスク実行中 (+0.01/s)、タスク完了時 (採取:+0.10, 運搬:+0.05)。
- **減少**: 待機中 (-0.05/s)、使役下の待機 (-0.01/s)、集会所での休息。
- **閾値**: 使い魔ごとに設定された `fatigue_threshold`（デフォルト 0.8）を超えると使役解除。
- **限界**: 0.9 を超えると `OnExhausted` イベントが発生し、強制的に集会所へ移動（`ExhaustedGathering`）。

### 1.2. ストレス (Stress)
監視下での労働や、過酷な環境で蓄積します。
- **増加**: タスク実行中 (+0.015/s)、監視下での労働（使い魔の効率に応じて追加）、**リクルート時 (+0.10)**、**使い魔の激励 (+0.0125)**。
- **減少**: 通常待機 (-0.02/s)、集会所での休息 (-0.04/s)、Soul同士の会話完了時（即時減少）。
- **ブレイクダウン**: 1.0 に達すると `OnStressBreakdown` が発生。一定時間停止し、タスクと使役を放棄します。
- **回復**: 0.7 まで下がると完全回復し、再び使役可能になります。

### 1.3. やる気 (Motivation) & 怠惰 (Laziness)
使い魔の近接による影響を強く受け、作業効率や移動速度に影響します。
- **自然減少**: 作業・使役中 (-0.05/s)、通常待機 (-0.1/s)。
- **回復**: 監視（使い魔が近くにいる、+0.4x効率）、タスク完了ボーナス（Chop/Mine:+0.02, Haul:+0.01, Build系［Build/Refine/ReinforceFloorTile/PourFloorTile/CoatWall］:+0.05）、**リクルート時 (+0.3)**、**使い魔の激励 (+0.025)**。
- **ペナルティ**: 同僚との会話（サボり）終了時に減少 (-0.02)。
- **怠惰**: 待機時間が長いと蓄積し、高いと自律的な行動（ wandered 等）を取りやすくなります。監視によって減少します。

### 1.4. タスク放棄 (Task Abandonment)
やる気が **0.3 (30%)** を下回ると、ワーカーは現在のタスクを放棄 (`OnTaskAbandoned`) してアイドル状態に戻ります。
放棄時には「🙅‍♂️」の絵文字が表示されます。

## 2. 行動状態 (Idle Behavior)

タスクを持っていないワーカーは、バイタルに応じて以下の行動を取ります。

| 状態 | 条件 | 動作 |
| :--- | :--- | :--- |
| **`Wander`** | 通常（低怠惰） | 周辺を気ままに歩き回る。 |
| **`Idle`** | 待機 | その場に留まる。 |
| **`Gathering`** | 疲労 > 0.8 | 集会所に集まり、他のワーカーと談笑・休息する。 |
| **`ExhaustedGathering`** | 疲労 > 0.9 | 強制的に集会所へ向かう（移動以外の全行動不可）。 |
| **`GoingToRest`** | 休憩条件成立 | 休憩所へ移動中。集会の重なり検知をスキップし、目的地が上書きされない。 |
| **`Resting`** | 休憩所に到着 | 休憩所内で休息中（非表示、疲労・ストレス回復）。 |
| **`StressFrozen`** | ストレス 1.0 | ストレスによりその場で硬直する。 |
| **`Escaping`** | 使い魔接近 + ストレス > 0.3 | 使い魔から逃走し、安全な集会スポットを探す。 |
| **`Drifting`** | 未管理状態が長時間継続し、脱走判定に成功 | うろつきつつマップ端へ漂流し、端到達でデスポーン。 |

### 2.1 逃走システム (Escaping System)

使役されていない魂（タスク未割当・UnderCommandなし）は、使い魔の影響圏に近づきすぎると逃走行動を開始します。

**逃走開始条件:**
- `UnderCommand` なし（使役されていない）
- 最も近い使い魔までの距離 < `command_radius * 1.5`（警戒圏内）
- `stress > 0.3`（ストレスが高い）
  - 警戒圏内にいる間はストレスが緩やかに増加する
- `ExhaustedGathering` 中は対象外（疲労行動を優先）
- `Gathering` 中も対象（ただし警戒圏内にいる場合のみストレスが増加する）
- 判定は `decide::escaping::escaping_decision_system` 内の検出タイマーにより **0.5秒間隔（初回即時）** で実行される

**逃走終了条件:**
- 全ての使い魔が安全圏外になった
  - `euclid < command_radius * 2.0 * 0.7` の近距離は、ユークリッド距離で即判定
  - それ以外は A* の経路距離で判定（`path_distance <= command_radius * 2.0` なら脅威継続）
- 安全な集会スポットに到着した
- タスクが割り当てられた、または使役された

**移動挙動:**
- 使い魔から離れる方向へ移動（基本ベクトル70%）
- 安全な集会スポットがある場合はそちらへ誘導（30%）
- 通常より速い速度で移動
- 逃走先の再評価は `decide::escaping::escaping_decision_system` 内の行動タイマーにより **0.5秒間隔（初回即時）** で実行される

**視覚フィードバック:**
- 青白い色（パニック感）
- 少し傾けた姿勢（走っている感じ）
- 軽い点滅アニメーション

### 2.2 人口システム連動（Drifting）

未管理状態が続く Soul は、`IdleBehavior::Drifting` へ遷移して自然脱走します。

- 開始条件（概略）:
  - `CommandedBy` なし
  - `AssignedTask::None`
  - `RestingIn` なし
  - `IdleState.total_idle_time >= SOUL_ESCAPE_UNMANAGED_TIME`
  - 判定タイマー/確率/グローバルクールダウンを満たす
- 挙動:
  - `DriftPhase::Wandering` と `DriftPhase::Moving` を繰り返し、最寄りのマップ端へ移動
- 終了:
  - 端近傍でデスポーン
  - リクルートまたはタスク再割り当てで Drifting は解除

詳細は **[population_system.md](population_system.md)** を参照してください。

### 2.3 休憩所システム (Rest Area)

休憩所が建設されていると、条件を満たしたワーカーは休憩所で休息を取り、疲労やストレスを回復します。
詳細な仕様については、以下の専用ドキュメントを参照してください。

- **[rest_area_system.md](rest_area_system.md)**

### 2.4 イベント駆動スプライト差し替え

Soul 本体画像は、Idle 状態だけでなくイベントでも一時差し替えされます。  
実装は `ConversationExpression`（画像種別・優先度・残り秒）でロック制御されています。

- `OnExhausted` -> `soul_exhausted`（4.0秒, 優先度30）
- `ConversationToneTriggered(Positive/Negative)` -> `soul_lough` / `soul_stress`（3.0秒 / 3.4秒, 優先度20）
- `OnGatheringParticipated` + `GatheringParticipants.len()` (または `GatheringSpot.object_type`):
  - `Barrel` -> `soul_wine`（2.2秒, 優先度15）
  - `CardTable` -> `soul_trump`（2.2秒, 優先度15）
- `ConversationCompleted(Positive/Negative)` -> `soul_lough` / `soul_stress`（1.4秒 / 1.8秒, 優先度10）

優先度の高いイベントだけが低いイベントを上書きし、低優先度イベントはロック中に破棄されます。  
詳細は `docs/speech_system.md` の「Soul 画像イベント」節を参照してください。

## 3. タスク実行ロジック (Task Execution)

割り当てられた `AssignedTask` に基づき、**Global Cycle Framework (4フェーズ)** に従ってタスクを実行します。

### 3.0. 実行サイクル (Execution Cycle)

4フェーズ（Perceive → Update → Decide → Execute）で実行。詳細は [ai-system-phases.md](ai-system-phases.md) 参照。Soul AI の主要システム: Update=バイタル更新, Decide=`idle_behavior_decision_system`/`escaping_decision_system`/`drifting_decision_system`, Execute=`task_execution`等。

### 3.1. 採取 (Gather)
- **対象**: 木、岩、建築物など。
- **プロセス**: 対象へ移動 → 作業（プログレスバー表示） → 完了時にアイテムドロップ。

### 3.2. 運搬 (Haul)
- **対象**: 資源アイテム、建築材料。
- **プロセス**: アイテムへ移動 → 拾い上げる (`Holding`) → 備蓄場所へ移動 → 配置。

## 4. 関連 Relationship

タスク・アイテム系 Relationship（`CommandedBy` / `WorkingOn` / `Holding`）の書き込み元・削除元は **tasks.md §2.1** を参照。

- `ParticipatingIn(spot)` ← Soul / `GatheringParticipants` ← GatheringSpot: 集会参加状態。`idle_behavior_decision` が insert、集会終了・タスク割り当て時に remove。
- 休憩所関連 (`RestingIn`, `RestAreaOccupants` 等): 詳細は [rest_area_system.md](rest_area_system.md) を参照。

## 5. 移動と制御 (Movement & Control)

ワーカーの移動は、物理的な障害物（岩、木、川）との相互作用を考慮して制御されています。

### 5.1. パス検索と目的地設定
- **8方向パス検索**: A*アルゴリズムを用いた8方向（上下左右＋斜め）の移動に対応しています。
- **隣接点検索**: ターゲットが非歩行可能な場合は `find_path_to_adjacent` を使い、8方向隣接マスへの到達パスを探索します。
- **平滑化 (Smoothing)**: 現在は無効化されており、グリッド経路をそのまま使用します。
- **再計算抑制**:
  - パス探索の失敗時は `PathCooldown`（既定 10 フレーム）を付与し、即時リトライを抑制
  - 1フレームあたりの探索件数を `MAX_PATHFINDS_PER_FRAME`（既定 8）で制限
  - タスク実行中 Soul を優先する2パス走査で探索枠を配分
- **部分再利用**: 既存パスの後半だけが障害物で塞がれた場合、阻塞直前から目的地までの部分パスを再探索して前半を再利用します。

### 5.2. スライディング衝突解決 (Sliding Collision)
- **仕様**: 移動システム（`soul_movement`）において、進行方向が通行不可（`WorldMap::is_walkable` が `false`）な場合、X軸またはY軸のみの移動を試みます。
- **効果**: 壁や岩に斜めにぶつかった際、完全に停止するのではなく、壁に沿って滑るように移動できます。
- **救済措置**: 万が一、全方位が塞がれて一歩も動けなくなった場合は、そのウェイポイントを到達済みとみなして次の経路へスキップします。

### 5.3. 障害物埋まりエスケープ (Stuck Escape)
- **検出**: 毎フレーム、ソウルの現在位置が通行不可（`WorldMap::is_walkable_world` が `false`）かどうかを判定します。建築物の配置や障害物の追加で、ソウルが障害物と重なった場合に該当します。
- **処理**: 埋まったソウルは、現在位置から周辺5マス以内の最も近い歩行可能タイルへ即座に移動（テレポート）されます。パスはクリアされ、次フレームで目的地へ向けた経路が再計算されます。
- **実装**: `soul_stuck_escape_system` がパス検索の前に実行され、`WorldMap::get_nearest_walkable_grid` で脱出先を決定します。

### 5.4. インタラクション距離
- ターゲットへの到達判定しきい値は `TILE_SIZE * 1.5` に設定されています。これにより、隣接マスからタスク（採掘、伐採、建築）を開始することが可能です。

## 6. モジュール構成（SoulAiPlugin 配下）

`update/`: バイタル更新・影響計算 / `decide/`: 待機行動・逃走・集会管理の意思決定 / `execute/`: タスク実行・Request 適用・集会スポット生成 / `helpers/`: 集会型定義・クエリ型・作業共通ヘルパー / `visual/`: アイドル/集会/バイタルの視覚フィードバック
