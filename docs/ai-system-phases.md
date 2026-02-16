# AI システムフェーズ設計

本ドキュメントは、AIシステムの4フェーズ設計について説明します。

## 概要

AIの思考・行動サイクルは以下の4フェーズで構成されています：

```
Perceive → Update → Decide → Execute
  (知覚)    (更新)   (決定)    (実行)
```

各フェーズ間には `ApplyDeferred` が配置され、コンポーネント変更が次のフェーズで確実に反映されます。

## レイヤー構造

Familiar AI と Soul AI は**別々のシステムセット**として定義され、Familiar AI が先に実行されます：

```
┌─────────────────────────────────────────────────────────┐
│  FamiliarAiSystemSet（指揮層）                          │
│  Perceive → Update → Decide → Execute                   │
└─────────────────────────────────────────────────────────┘
                         │
                   ApplyDeferred
                         ↓
┌─────────────────────────────────────────────────────────┐
│  SoulAiSystemSet（実行層）                              │
│  Perceive → Update → Decide → Execute                   │
└─────────────────────────────────────────────────────────┘
```

### レイヤー分離の理由

1. **指揮系統の表現**: Familiar（指揮官）が先に決定 → Soul（従属者）が反映
2. **データ依存関係の明示化**: Familiar のタスク割り当てが Soul に自動的に伝播
3. **重複割り当ての防止**: Familiar の決定が Soul の自動割り当てに先行
4. **拡張性**: 将来的に敵AI等を追加する場合の階層設計が自然

## フェーズ定義

`src/systems/soul_ai/scheduling.rs`:

```rust
pub enum FamiliarAiSystemSet {
    Perceive,  // 環境情報の読み取り、変化の検出
    Update,    // 時間経過による内部状態の変化
    Decide,    // 次の行動の選択、要求の生成
    Execute,   // 決定された行動の実行
}

pub enum SoulAiSystemSet {
    Perceive,  // 環境情報の読み取り、変化の検出
    Update,    // 時間経過による内部状態の変化
    Decide,    // 次の行動の選択、要求の生成
    Execute,   // 決定された行動の実行
}
```

## 各フェーズの責任

### Perceive（知覚）

**責任**: 環境情報の読み取り、変化の検出

**原則**:
- コンポーネントの**読み取りのみ**
- キャッシュの再構築（必要に応じてタイマーゲート）
- 変化フラグの設定

**システム例**:
```rust
// Soul AI
- (現状は専用システムなし。将来の拡張ポイント)

// Familiar AI
- detect_state_changes_system    // 状態変化の検出
- detect_command_changes_system  // コマンド変化の検出
- sync_reservations_system       // リソース予約キャッシュの再構築（0.2秒間隔, 初回即時）
```

**sync_reservations_system の詳細**:

リソース予約の再構築は以下の2つのソースから行われます:

1. **`AssignedTask`** - 既にSoulに割り当てられているタスク
2. **`Designation` (Without<TaskWorkers>)** - まだ割り当て待ちのタスク候補

`Designation` からの予約は、付随するコンポーネント（`TargetMixer`, `TargetBlueprint`, `BelongsTo`）と `WorkType` に基づいて適切な予約カテゴリにカウントされます。これにより、自動発行システムが複数フレームにわたって過剰にタスクを発行することを防ぎます。

- 再構築は **0.2秒間隔（初回即時）** で実行されます。
- 同期間隔中の差分は `ResourceReservationRequest` により随時反映されます。

### Update（更新）

**責任**: 時間経過による内部状態の変化

**原則**:
- 自己完結した状態更新
- 外部エンティティへの影響なし
- イベントトリガーは許可（OnExhausted等）

**システム例**:
```rust
// Soul AI
- fatigue_update_system          // 疲労の増減
- familiar_influence_unified_system  // ストレス/やる気/怠惰の統合更新
- gathering_grace_tick_system    // 集会スポット猶予タイマー更新

// Familiar AI
- (現状は専用システムなし。将来の拡張ポイント)
```

### Decide（決定）

**責任**: 次の行動の選択、要求の生成

**原則**:
- `TaskAssignmentRequest` / `DesignationRequest` 等の要求を生成
- 目的地の決定
- **`Commands` の発行は禁止**（自エンティティへの軽微な値更新のみ許容）

**システム例**:
```rust
// Soul AI
- idle_behavior_decision_system  // アイドル行動の決定
- blueprint_auto_haul_system     // DesignationRequest の生成
- escaping_decision_system       // 逃走行動の決定（0.5秒間隔, 初回即時）

// Familiar AI
- familiar_ai_state_system       // 状態遷移判定
- familiar_task_delegation_system // 0.5秒間隔, 初回即時
```

- `escaping_decision_system` は **0.5秒間隔（初回即時）** で再評価されます。
- `familiar_task_delegation_system` も **0.5秒間隔（初回即時）** で実行されます。

### Execute（実行）

**責任**: 決定された行動の実行

**原則**:
- `Commands`による変更の実行
- エンティティの生成/削除
- イベントの発火
- 予約の確定

**システム例**:
```rust
// Soul AI
- apply_designation_requests_system
- apply_task_assignment_requests_system
- task_execution_system
- idle_behavior_apply_system     // アイドル行動の適用
- escaping_apply_system
- clear_item_reservations_system
- gathering_spawn_system         // 集会スポット生成

// Familiar AI
- handle_state_changed_system
- apply_squad_management_requests_system  // 分隊管理要求の適用
```

## Message/Request パターン

フェーズ間の通信には**Message**を使用し、堅牢性を確保しています。

### IdleBehaviorRequest

`src/events.rs`:

```rust
#[derive(Message, Debug, Clone)]
pub struct IdleBehaviorRequest {
    pub entity: Entity,
    pub operation: IdleBehaviorOperation,
}

pub enum IdleBehaviorOperation {
    JoinGathering { spot_entity: Entity },
    LeaveGathering { spot_entity: Entity },
    ArriveAtGathering { spot_entity: Entity },
    ReserveRestArea { rest_area_entity: Entity },
    ReleaseRestArea,
    EnterRestArea { rest_area_entity: Entity },
    LeaveRestArea,
}
```

**使用例** (`decide/idle_behavior.rs` / `execute/idle_behavior_apply.rs`):

```rust
// Decide フェーズ
pub fn idle_behavior_decision_system(
    mut request_writer: MessageWriter<IdleBehaviorRequest>,
    ...
) {
    // 集会参加の決定
    request_writer.write(IdleBehaviorRequest {
        entity,
        operation: IdleBehaviorOperation::JoinGathering { spot_entity },
    });
}

// Execute フェーズ
pub fn idle_behavior_apply_system(
    mut commands: Commands,
    mut request_reader: MessageReader<IdleBehaviorRequest>,
) {
    for request in request_reader.read() {
        match &request.operation {
            IdleBehaviorOperation::JoinGathering { spot_entity } => {
                commands.entity(request.entity).insert(ParticipatingIn(*spot_entity));
                commands.trigger(OnGatheringParticipated { ... });
            }
// ...
        }
    }
}
```

### DesignationRequest

`src/events.rs`:

```rust
#[derive(Message, Debug, Clone)]
pub struct DesignationRequest {
    pub entity: Entity,
    pub operation: DesignationOp,
}

pub enum DesignationOp {
    Issue {
        work_type: WorkType,
        issued_by: Entity,
        task_slots: u32,
        priority: Option<u32>,
        target_blueprint: Option<Entity>,
        target_mixer: Option<Entity>,
        reserved_for_task: bool,
    },
}
```

**使用例** (`soul_ai/decide/work/auto_haul/*.rs` / `soul_ai/execute/designation_apply.rs`):

```rust
// Decide フェーズ: Designation 発行要求をキュー
designation_writer.write(DesignationRequest {
    entity: item_entity,
    operation: DesignationOp::Issue { ... },
});

// Execute フェーズ: 要求を実際のコンポーネントに反映
pub fn apply_designation_requests_system(
    mut commands: Commands,
    mut request_reader: MessageReader<DesignationRequest>,
) {
    for request in request_reader.read() {
        commands.entity(request.entity).insert(Designation { ... });
    }
}
```

### SquadManagementRequest

`src/events.rs`:

```rust
#[derive(Message, Debug, Clone)]
pub struct SquadManagementRequest {
    pub familiar_entity: Entity,
    pub operation: SquadManagementOperation,
}

pub enum SquadManagementOperation {
    AddMember { soul_entity: Entity },
    ReleaseMember { soul_entity: Entity, reason: ReleaseReason },
}

pub enum ReleaseReason {
    Fatigued,
}
```

**使用例** (`familiar_ai/execute/squad_apply.rs`):

```rust
// Decide フェーズ: 疲労した魂のリリース要求
pub fn release_fatigued(...) {
    request_writer.write(SquadManagementRequest {
        familiar_entity: fam_entity,
        operation: SquadManagementOperation::ReleaseMember {
            soul_entity: member_entity,
            reason: ReleaseReason::Fatigued,
        },
    });
}

// Execute フェーズ: 要求の適用
pub fn apply_squad_management_requests_system(
    mut commands: Commands,
    mut request_reader: MessageReader<SquadManagementRequest>,
) {
    for request in request_reader.read() {
        match &request.operation {
            SquadManagementOperation::ReleaseMember { soul_entity, reason } => {
                // unassign_task の実行や Relationship の削除
                commands.entity(*soul_entity).remove::<CommandedBy>();
                // ...
            }
            // ...
        }
    }
}
```

### Message パターンの利点

1. **自動クリーンアップ**: 読み取り後に自動的に消費される
2. **順序保証**: 書き込み順に読み取られる
3. **エンティティ削除に強い**: エンティティに紐づかないため安全
4. **既存パターンとの整合性**: `TaskAssignmentRequest` / `DesignationRequest` で実績あり

## 新しいAI行動の追加パターン

### 1. 検出が必要な場合
Perceiveフェーズにシステム追加

### 2. 時間経過で変化する値
Updateフェーズにシステム追加

### 3. 行動選択ロジック
Decideフェーズにシステム追加

### 4. 実際の行動実行
Executeフェーズにシステム追加

### 例: 新しい「休憩」行動の追加

```rust
// 1. Request型を定義 (events.rs)
pub enum RestBehaviorOperation {
    StartResting { rest_spot: Entity },
    StopResting,
}

// 2. Perceive: 休憩場所の検出
fn detect_rest_spots_system(...) { ... }

// 3. Update: 休憩欲求の更新
fn rest_need_update_system(...) { ... }

// 4. Decide: 休憩行動の選択
fn rest_behavior_decision_system(
    mut request_writer: MessageWriter<RestBehaviorRequest>,
    ...
) { ... }

// 5. Execute: 休憩場所への移動開始
fn rest_behavior_apply_system(
    mut request_reader: MessageReader<RestBehaviorRequest>,
    ...
) { ... }
```

（この項目は実装済みです）

## 関連ファイル

| ファイル | 説明 |
|:--|:--|
| `src/systems/soul_ai/scheduling.rs` | `FamiliarAiSystemSet`, `SoulAiSystemSet`の定義 |
| `src/systems/soul_ai/mod.rs` | Soul AIのフェーズ登録、レイヤー間順序設定 |
| `src/systems/familiar_ai/mod.rs` | Familiar AIのフェーズ登録 |
| `src/systems/soul_ai/decide/idle_behavior.rs` | Decide側のRequest生成例 |
| `src/systems/soul_ai/execute/idle_behavior_apply.rs` | Execute側のIdleBehavior適用例 |
| `src/systems/soul_ai/execute/designation_apply.rs` | Execute側のDesignation適用例 |
| `src/events.rs` | Request型の定義 |
| `docs/architecture.md` | 全体アーキテクチャ |
| `docs/soul_ai.md` | Soul AI詳細 |
| `docs/familiar_ai.md` | Familiar AI詳細 |
