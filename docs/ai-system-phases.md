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

`src/systems/soul_ai/scheduling.rs` に `FamiliarAiSystemSet` / `SoulAiSystemSet` として定義（各フェーズ: `Perceive / Update / Decide / Execute`）。

## 各フェーズの責任

### Perceive（知覚）

**責任**: 環境情報の読み取り、変化の検出

**原則**:
- コンポーネントの**読み取りのみ**
- キャッシュの再構築（必要に応じてタイマーゲート）
- 変化フラグの設定

**システム例**:
- Soul AI: 現状は拡張ポイント（専用システムなし）
- Familiar AI: `detect_state_changes_system`, `detect_command_changes_system`, `sync_reservations_system`（0.2秒間隔, 初回即時）

`sync_reservations_system` は `AssignedTask` と `Designation`（Without\<TaskWorkers\>）の2ソースから予約を再構築。差分は `ResourceReservationRequest` で随時反映。

### Update（更新）

**責任**: 時間経過による内部状態の変化

**原則**:
- 自己完結した状態更新
- 外部エンティティへの影響なし
- イベントトリガーは許可（OnExhausted等）

**システム例**:
- Soul AI: `fatigue_update_system`, `familiar_influence_unified_system`, `gathering_grace_tick_system`
- Familiar AI: 現状は拡張ポイント

### Decide（決定）

**責任**: 次の行動の選択、要求の生成

**原則**:
- `TaskAssignmentRequest` / `DesignationRequest` 等の要求を生成
- 目的地の決定
- **`Commands` の発行は禁止**（自エンティティへの軽微な値更新のみ許容）

**システム例**:
- Soul AI: `idle_behavior_decision_system`, `blueprint_auto_haul_system`, `escaping_decision_system`（0.5秒間隔, 初回即時）
- Familiar AI: `familiar_ai_state_system`, `familiar_task_delegation_system`（0.5秒間隔, 初回即時）

### Execute（実行）

**責任**: 決定された行動の実行

**原則**:
- `Commands`による変更の実行
- エンティティの生成/削除
- イベントの発火
- 予約の確定

**システム例**:
- Soul AI: `apply_designation_requests_system`, `apply_task_assignment_requests_system`, `task_execution_system`, `idle_behavior_apply_system`, `escaping_apply_system`, `clear_item_reservations_system`, `gathering_spawn_system`
- Familiar AI: `handle_state_changed_system`, `apply_squad_management_requests_system`

## Message/Request パターン

フェーズ間通信は `Message`（`src/events.rs`）を使用。Decide で `MessageWriter<T>::write()` し、Execute で `MessageReader<T>::read()` で消費する。

| Request 型 | 用途 | Decide 側 | Execute 側 |
|:--|:--|:--|:--|
| `IdleBehaviorRequest` | 集会参加・休憩所操作 | `decide/idle_behavior.rs` | `execute/idle_behavior_apply.rs` |
| `DesignationRequest` | Designation 発行（`DesignationOp::Issue`） | `soul_ai/decide/work/auto_haul/` | `execute/designation_apply.rs` |
| `SquadManagementRequest` | 分隊員追加・解放（`AddMember` / `ReleaseMember`） | `decide/squad.rs` | `execute/squad_apply.rs` |

**利点**: 読み取り後自動消費、順序保証、エンティティ削除に安全。

## 新しいAI行動の追加パターン

- **検出** → Perceive にシステム追加
- **時間変化** → Update にシステム追加
- **行動選択** → Decide に Request 生成システム追加（`Commands` 禁止）
- **行動実行** → Execute に Request 消費システム追加

## 関連ファイル

| ファイル | 説明 |
|:--|:--|
| `src/systems/soul_ai/scheduling.rs` | `FamiliarAiSystemSet`, `SoulAiSystemSet`の定義 |
| `src/systems/soul_ai/mod.rs` | Soul AIのフェーズ登録、レイヤー間順序設定 |
| `src/systems/familiar_ai/mod.rs` | Familiar AIのフェーズ登録 |
| `src/systems/soul_ai/decide/idle_behavior/` | Decide側のRequest生成例 |
| `src/systems/soul_ai/execute/idle_behavior_apply.rs` | Execute側のIdleBehavior適用例 |
| `src/systems/soul_ai/execute/designation_apply.rs` | Execute側のDesignation適用例 |
| `src/events.rs` | Request型の定義 |
| `docs/architecture.md` | 全体アーキテクチャ |
| `docs/soul_ai.md` | Soul AI詳細 |
| `docs/familiar_ai.md` | Familiar AI詳細 |
