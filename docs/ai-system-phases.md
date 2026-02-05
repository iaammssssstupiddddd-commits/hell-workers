# AI システムフェーズ設計

本ドキュメントは、AIシステムの4フェーズ設計について説明します。

## 概要

AIの思考・行動サイクルは以下の4フェーズで構成されています：

```
Perceive → Update → Decide → Execute
  (知覚)    (更新)   (決定)    (実行)
```

各フェーズ間には `ApplyDeferred` が配置され、コンポーネント変更が次のフェーズで確実に反映されます。

## フェーズ定義

`src/systems/soul_ai/scheduling.rs`:

```rust
pub enum AiSystemSet {
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
- キャッシュの再構築
- 変化フラグの設定

**システム例**:
```rust
// Soul AI
- escaping_detection_system      // 逃走条件の検出

// Familiar AI
- detect_state_changes_system    // 状態変化の検出
- detect_command_changes_system  // コマンド変化の検出
- sync_reservations_system       // リソース予約キャッシュの再構築
```

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
- stress_system                  // ストレス更新
- motivation_system              // やる気計算
- gathering_maintenance_system   // 集会スポット状態管理

// Familiar AI
- cleanup_encouragement_cooldowns_system  // クールダウン減少
```

### Decide（決定）

**責任**: 次の行動の選択、要求の生成

**原則**:
- `TaskAssignmentRequest`等の要求を生成
- 目的地の決定
- **コマンド発行は最小限**

**システム例**:
```rust
// Soul AI
- idle_behavior_decision_system  // アイドル行動の決定
- blueprint_auto_haul_system     // タスク割り当て要求
- escaping_behavior_system       // 逃走行動の決定

// Familiar AI
- familiar_ai_state_system       // 状態遷移判定
- familiar_task_delegation_system
```

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
- apply_task_assignment_requests_system
- task_execution_system
- idle_behavior_apply_system     // アイドル行動の適用
- gathering_spawn_system         // 集会スポット生成

// Familiar AI
- handle_state_changed_system
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
}
```

**使用例** (`idle/behavior.rs`):

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

### Message パターンの利点

1. **自動クリーンアップ**: 読み取り後に自動的に消費される
2. **順序保証**: 書き込み順に読み取られる
3. **エンティティ削除に強い**: エンティティに紐づかないため安全
4. **既存パターンとの整合性**: `TaskAssignmentRequest`で実績あり

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

## 将来の改善点

### familiar_ai_state_system の分割

現状、`familiar_ai_state_system`は複雑なヘルパー関数群を経由してCommands操作を行っています：

- `squad.rs`: `release_fatigued()` でCommandedBy削除
- `recruitment.rs`: `try_immediate_recruit()` でCommandedBy追加
- `scouting.rs`: `scouting_logic()` でリクルート実行

これらを`SquadManagementRequest`を使って分離することで、より明確なフェーズ分離が可能です：

```rust
// 既に定義済み (events.rs)
pub struct SquadManagementRequest {
    pub familiar_entity: Entity,
    pub operation: SquadManagementOperation,
}

pub enum SquadManagementOperation {
    AddMember { soul_entity: Entity },
    ReleaseMember { soul_entity: Entity },
}
```

**実装手順**:
1. `squad.rs`の`release_fatigued()`をRequest発行に変更
2. `recruitment.rs`の`try_immediate_recruit()`をRequest発行に変更
3. `scouting.rs`の`scouting_logic()`をRequest発行に変更
4. Executeフェーズに`squad_management_apply_system`を追加

## 関連ファイル

| ファイル | 説明 |
|:--|:--|
| `src/systems/soul_ai/scheduling.rs` | `AiSystemSet`の定義 |
| `src/systems/soul_ai/mod.rs` | Soul AIのフェーズ登録 |
| `src/systems/familiar_ai/mod.rs` | Familiar AIのフェーズ登録 |
| `src/systems/soul_ai/idle/behavior.rs` | Decide/Execute分割の実装例 |
| `src/events.rs` | Request型の定義 |
| `docs/architecture.md` | 全体アーキテクチャ |
| `docs/soul_ai.md` | Soul AI詳細 |
| `docs/familiar_ai.md` | Familiar AI詳細 |
