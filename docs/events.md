# イベントカタログ (Events)

全イベントの Producer / Consumer / Timing を一元管理します。
イベントを追加・削除・変更する際は必ずこのファイルを更新してください。

定義元: `crates/hw_core/src/events.rs` / `crates/hw_jobs/src/events.rs`
re-export: `crates/bevy_app/src/main.rs`（直接 `pub use`）

---

## 1. 通知イベント（Observer / MessageReader が受け取る）

Soul・Familiar の状態変化を通知する。即時整合が必要な副作用は `Observer`、
1 フレーム遅延可能な visual / log 系は `MessageReader` システムが担う。

| イベント | 定義 | Producer | Consumer | 主な副作用 |
|:---|:---|:---|:---|:---|
| `OnTaskAssigned` | `EntityEvent+Message` | `apply_task_assignment_requests_system` | speech システム（`MessageReader<OnTaskAssigned>`） | タスク開始時の Soul / Familiar 発話、command tone の visual 反応 |
| `OnTaskCompleted` | `EntityEvent+Message` | `task_execution_system` | やる気 Observer + speech システム（`MessageReader<OnTaskCompleted>`） | やる気ボーナス付与（Chop/Mine:+2%, Haul:+1%, Build系:+5%）/ 完了時の発話 |
| `OnTaskAbandoned` | `EntityEvent+Message` | `unassign_task(emit=true)` | 音声 Observer | 音声再生のみ。**cleanup は呼び出し元が完了済み（I-S4）** |
| `OnSoulRecruited` | `EntityEvent+Message` | `apply_task_assignment_requests_system` / `squad_logic_system` | バイタル Observer + Soul 状態正規化 Observer + speech Observer | やる気+30% / ストレス+10% / idle-path-drifting 正規化 / リクルート演出 |
| `OnExhausted` | `EntityEvent+Message` | バイタル更新システム（疲労 > 0.9） | cleanup Observer + speech Observer + 表情システム（`MessageReader<OnExhausted>`） | `unassign_task` + `CommandedBy` 削除 + `ExhaustedGathering` 設定 + 疲労演出 |
| `OnStressBreakdown` | `EntityEvent+Message` | バイタル更新システム（ストレス >= 1.0） | cleanup Observer + speech Observer | `unassign_task` + `StressBreakdown` 付与 + `CommandedBy` 削除 |
| `OnReleasedFromService` | `EntityEvent+Message` | `SquadManagementRequest::ReleaseMember` 処理 | 音声 Observer | 使役解除時の演出 |
| `OnEncouraged` | `EntityEvent+Message` | `EncouragementRequest` 処理 | バイタル Observer + speech Observer | やる気 / ストレス改善 + 激励演出 |
| `OnGatheringParticipated` | `EntityEvent+Message` | 集会参加処理 | 表情システム（`MessageReader<OnGatheringParticipated>`） | 集会オブジェクトに応じた表情ロック。`GatheringParticipants` は `ParticipatingIn` の Relationship が自動更新 |
| `OnGatheringJoined` | `EntityEvent+Message` | `IdleBehaviorOperation::ArriveAtGathering` 適用 | speech Observer | 集会到着時の演出 |
| `FamiliarAiStateChangedEvent` | `Message` | 状態遷移システム | ログ / ビジュアル | ログ記録 |
| `FamiliarOperationMaxSoulChangedEvent` | `Message` | UI 操作（使役数変更ダイアログ） | Squad 管理システム | 超過分の Soul を自動リリース |
| `DriftingEscapeStarted` | `Event` | `decide/drifting` | root adapter | `PopulationManager::start_escape_cooldown()` |
| `SoulEscaped` | `Event` | `execute/drifting`（マップ端到達） | root adapter | `PopulationManager::total_escaped` インクリメント |

---

## 2. リクエストイベント（Request / Command）

システム間の疎結合な通信に使う。受信側システムが次フレームまでに処理する。

| イベント | 定義 | 主な送信元 | 受信・適用システム | 内容 |
|:---|:---|:---|:---|:---|
| `TaskAssignmentRequest` | `Message` | `familiar_task_delegation_system` | `apply_task_assignment_requests` (Execute) | Soul へのタスク割り当て。`WorkingOn`・`CommandedBy`・`DeliveringTo` を設定 |
| `ResourceReservationRequest` | `Message` | `unassign_task` / `apply_task_assignment_requests` | `apply_reservation_requests_system` | `SharedResourceCache` の予約追加・解放 |
| `DesignationRequest` | `Message` | request producer / UI | `apply_designation_requests` (Execute) | `Designation` の発行 |
| `SquadManagementRequest` | `Message` | Familiar AI decide 層 | Squad 管理システム | 分隊メンバーの追加・解放 |
| `IdleBehaviorRequest` | `Message` | Soul AI decide 層 | アイドル行動システム | 集会参加・離脱・休憩所予約 等 |
| `EscapeRequest` | `Message` | Soul AI decide 層 | 逃走システム | 逃走開始・目的地更新・安全到達 |
| `GatheringManagementRequest` | `Message` | 集会管理システム | 集会管理適用システム | 集会の解散・統合・参加・離脱 |
| `FamiliarStateRequest` | `Message` | Familiar AI decide 層 | 状態遷移システム | Familiar の AI 状態変更 |
| `EncouragementRequest` | `Message` | Familiar AI execute 層 | 激励システム | Soul へのバイタル改善 |
| `FamiliarIdleVisualRequest` | `Message` | Familiar AI 状態遷移時 | visual アダプタ | Idle 遷移時の表示更新 |
| `GatheringSpawnRequest` | `Message` | `hw_soul_ai` 内の集会ロジック | root visual アダプタ | 集会スポットの生成 |
| `SoulTaskUnassignRequest` | `Message` | `hw_familiar_ai`（分隊解放・使役数超過）| `hw_soul_ai::handle_soul_task_unassign_system`（SoulAiSystemSet::Perceive）| 魂のタスク解除（`AssignedTask`リセット・インベントリ回収・予約解放） |

---

## 3. イベント使用上のルール

### R-E1: Request イベントは次フレームで適用される
Request 系イベントは送信フレームでは反映されない。
同フレーム内で反映が必要なら直接関数を呼ぶこと（ただし ECS 整合に注意）。

### R-E2: EntityEvent は対象エンティティが生存している前提
`EntityEvent` / `Message` の `entity` フィールドが指すエンティティは、
Observer 実行時点で生存していることを前提とする。
despawn 済みエンティティへの操作は `Commands::entity().try_insert()` 等で防御する。

### R-E3: Observer 内でタスク状態を変更しない（OnTaskAbandoned）
`OnTaskAbandoned` Observer はクリーンアップを行わない（I-S4 参照）。
誤って `unassign_task` を Observer 内で呼ぶと二重 cleanup が発生する。

### R-E5: 視覚・ログ系の副作用は Observer でなく MessageReader システムで消費する
`commands.trigger()` は即時 Observer push であり、ホットパス上で大量発生する場合にコストが読みにくい。
Speech bubble 生成などの 1 フレーム遅延が許容される副作用は、
イベント型に `#[derive(Message)]` を付与して `messages.rs` に登録し、
`MessageReader<T>` ベースのシステムで `GameSystemSet::Visual` 内に配置すること。
ゲーム状態の即時整合性が必要な副作用（バイタル変化・cleanup 等）は引き続き Observer を使用する。

### R-E4: イベント追加時はこのファイルを更新する
新しいイベントを `hw_core/src/events.rs` または `hw_jobs/src/events.rs` に追加した場合、
このカタログに Producer / Consumer / 副作用を記載すること。
