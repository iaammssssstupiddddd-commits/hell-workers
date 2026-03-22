# イベントカタログ (Events)

全イベントの Producer / Consumer / Timing を一元管理します。
イベントを追加・削除・変更する際は必ずこのファイルを更新してください。

定義元: `crates/hw_core/src/events.rs` / `crates/hw_jobs/src/events.rs`
re-export: `crates/bevy_app/src/main.rs`（直接 `pub use`）

---

## 1. 通知イベント（Observer が受け取る）

Soul・Familiar の状態変化を通知する。Observer が副作用を担う。

| イベント | 定義 | Producer | Consumer (Observer) | 主な副作用 |
|:---|:---|:---|:---|:---|
| `OnTaskAssigned` | `EntityEvent` | `apply_task_assignment_requests` | 音声・ログ系 Observer | 音声再生 / ログ。`OnSoulRecruited` を条件付き内包 |
| `OnTaskCompleted` | `EntityEvent` | `AssignedTask` → `None` 変化の Change Detection | やる気 Observer | やる気ボーナス付与（Chop/Mine:+2%, Haul:+1%, Build系:+5%）/ 音声 |
| `OnTaskAbandoned` | `EntityEvent` | `unassign_task(emit=true)` | 音声 Observer | 音声再生のみ。**cleanup は呼び出し元が完了済み（I-S4）** |
| `OnSoulRecruited` | `EntityEvent` | `OnTaskAssigned` Observer 内（条件付き） | バイタル Observer | やる気+30% / ストレス+10% / 移動クリア |
| `OnExhausted` | `EntityEvent` | バイタル更新システム（疲労 > 0.9） | cleanup Observer | `unassign_task` + `CommandedBy` 削除 + `ExhaustedGathering` 設定 |
| `OnStressBreakdown` | `EntityEvent` | バイタル更新システム（ストレス >= 1.0） | cleanup Observer | `unassign_task` + `StressBreakdown` 付与 + `CommandedBy` 削除 |
| `OnReleasedFromService` | `EntityEvent` | `SquadManagementRequest::ReleaseMember` 処理 | 音声 Observer | — |
| `OnEncouraged` | `EntityEvent` | `EncouragementRequest` 処理 | バイタル Observer | やる気 / ストレス改善 |
| `OnGatheringParticipated` | `EntityEvent` | 集会参加処理 | スポット管理 Observer | 集会スポットの参加者リスト更新 |
| `OnGatheringLeft` | `Event` | 集会離脱処理 | スポット管理 Observer | 集会スポットの参加者リスト更新 |
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

### R-E4: イベント追加時はこのファイルを更新する
新しいイベントを `hw_core/src/events.rs` または `hw_jobs/src/events.rs` に追加した場合、
このカタログに Producer / Consumer / 副作用を記載すること。
