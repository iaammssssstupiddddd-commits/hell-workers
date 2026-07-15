# イベントカタログ (Events)

全イベントの Producer / Consumer / Timing を一元管理します。
イベントを追加・削除・変更する際は必ずこのファイルを更新してください。

定義元: `crates/hw_core/src/events.rs` / `crates/hw_jobs/src/events.rs`（ほか、`hw_world` 固有の `Message` は各モジュールで定義）
re-export: `crates/bevy_app/src/lib.rs`（直接 `pub use`）

---

## 1. 通知イベント（EntityEvent Observer / MessageReader が受け取る）

Soul・Familiar の状態変化を通知する。即時整合が必要な gameplay 副作用は
`EntityEvent` の `Observer`、1 フレーム遅延可能な presentation 副作用は `MessageReader`
システムが担う。両方が必要な通知は domain の `On*` と presentation の
`*VisualMessage` を対にする。`commands.trigger()` は `Message` を自動で書き込まないため、
dual 通知の Producer は `publish_*` helper を使う。

| イベント | 定義 | Producer | Consumer | 主な副作用 |
|:---|:---|:---|:---|:---|
| `OnTaskAssigned` | `Message` | `apply_task_assignment_requests_system`（`write_message`） | speech システム（`MessageReader<OnTaskAssigned>`） | タスク開始時の Soul / Familiar 発話、command tone の visual 反応 |
| `OnTaskCompleted` / `TaskCompletedVisualMessage` | `EntityEvent` / `Message` | `task_execution_system` が `publish_task_completed`（context が正常終了を確定した場合のみ） | やる気 Observer (`OnTaskCompleted`) + speech システム（`MessageReader<TaskCompletedVisualMessage>`） | やる気ボーナス付与（Chop/Mine:+2%, Haul:+1%, Build系:+5%）/ 完了時の発話 |
| `OnTaskAbandoned` | `Message` | `unassign_task(emit=true)` / designation cancel の `SoulTaskUnassignRequest` | speech システム（`MessageReader<OnTaskAbandoned>`） | 音声再生のみ。**cleanup は発行前に完了済み（I-S4）** |
| `OnSoulRecruited` / `SoulRecruitedVisualMessage` | `EntityEvent` / `Message` | `apply_task_assignment_requests_system` / `squad_logic_system` が `publish_soul_recruited` | バイタル・Soul状態 Observer (`OnSoulRecruited`) + speech システム（`MessageReader<SoulRecruitedVisualMessage>`） | やる気+30% / ストレス+10% / idle-path-drifting 正規化 / リクルート演出 |
| `OnExhausted` / `SoulExhaustedVisualMessage` | `EntityEvent` / `Message` | バイタル更新システム（疲労 > 0.9）が `publish_soul_exhausted` | cleanup Observer (`OnExhausted`) + speech / 表情システム（`MessageReader<SoulExhaustedVisualMessage>`） | `unassign_task` + `CommandedBy` 削除 + `ExhaustedGathering` 設定 + 疲労演出 |
| `OnStressBreakdown` / `SoulStressBreakdownVisualMessage` | `EntityEvent` / `Message` | バイタル更新システム（ストレス >= 1.0）が `publish_stress_breakdown` | cleanup Observer (`OnStressBreakdown`) + speech システム（`MessageReader<SoulStressBreakdownVisualMessage>`） | `unassign_task` + `StressBreakdown` 付与 + `CommandedBy` 削除 |
| `OnReleasedFromService` | `Message` | `SquadManagementRequest::ReleaseMember` 処理 / commanded-soul cleanup（`write_message`） | speech システム（`MessageReader<OnReleasedFromService>`） | 使役解除時の演出 |
| `OnEncouraged` / `SoulEncouragedVisualMessage` | `EntityEvent` / `Message` | `EncouragementRequest` 処理が `publish_soul_encouraged` | バイタル Observer (`OnEncouraged`) + speech システム（`MessageReader<SoulEncouragedVisualMessage>`） | やる気 / ストレス改善 + 激励演出 |
| `OnGatheringParticipated` | `Message` | 集会参加処理（`write_message`） | 表情システム（`MessageReader<OnGatheringParticipated>`） | 集会オブジェクトに応じた表情ロック。`GatheringParticipants` は `ParticipatingIn` の Relationship が自動更新 |
| `OnGatheringJoined` | `Message` | `IdleBehaviorOperation::ArriveAtGathering` 適用（`write_message`） | speech システム（`MessageReader<OnGatheringJoined>`） | 集会到着時の演出 |
| `FamiliarAiStateChangedEvent` | `Message` | 状態遷移システム | ログ / ビジュアル | ログ記録 |
| `FamiliarOperationMaxSoulChangedEvent` | `Message` | UI 操作（使役数変更ダイアログ） | Squad 管理システム | 超過分の Soul を自動リリース |
| `DriftingEscapeStarted` | `Event` | `decide/drifting` | root adapter | `PopulationManager::start_escape_cooldown()` |
| `SoulEscaped` | `Event` | `execute/drifting`（マップ端到達） | root adapter | `PopulationManager::total_escaped` インクリメント |
| `TerrainChangedEvent` | `Message`（`hw_world::terrain_visual`） | `obstacle_sync_system`（`ObstacleSyncSet`、Actor phase） | `terrain_id_map_sync_system`（`MessageReader`、`GameSystemSet::Visual`） | 自然物由来 blocker の最後の削除で `WorldMap` 上の該当タイルが Dirt へ変わったとき `idx` を通知し、`TerrainIdMap` の対応ピクセルを書き換えて共有 `TerrainSurfaceMaterial` の見た目を更新する。**chunk entity（`TerrainChunk`）の再生成は不要**。shader が world-space で texture を参照するため、texture 1 ピクセル書き換えだけで全 chunk の見た目が更新される。登録は `VisualPlugin::add_message::<TerrainChangedEvent>()` |

---

## 2. リクエストイベント（Request / Command）

システム間の疎結合な通信に使う。適用時点は送信側・受信側の system set と実行順序に従う。

| イベント | 定義 | 主な送信元 | 受信・適用システム | 内容 |
|:---|:---|:---|:---|:---|
| `TaskAssignmentRequest` | `Message` | `familiar_task_delegation_system` | `apply_task_assignment_requests` (Execute) | Soul へのタスク割り当て。`WorkingOn`・`CommandedBy`・`DeliveringTo` を設定 |
| `ResourceReservationRequest` | `Message` | `unassign_task` / task handler の `TaskExecutionContext::queue_reservation` | `hw_logistics::apply_reservation_requests_system`（Execute） | `ResourceReservationOp` の適用。予約解放と `RecordPickedSource` によるソース取得差分記録など |
| `DesignationRequest` | `Message` | request producer / UI | `apply_designation_requests` (Execute) | `Designation` の発行 |
| `SquadManagementRequest` | `Message` | Familiar AI decide 層 | Squad 管理システム | 分隊メンバーの追加・解放 |
| `IdleBehaviorRequest` | `Message` | Soul AI decide 層 | アイドル行動システム | 集会参加・離脱・休憩所予約 等 |
| `EscapeRequest` | `Message` | Soul AI decide 層 | 逃走システム | 逃走開始・目的地更新・安全到達 |
| `GatheringManagementRequest` | `Message` | 集会管理システム | 集会管理適用システム | 集会の解散・統合・参加・離脱 |
| `FamiliarStateRequest` | `Message` | Familiar AI decide 層 | 状態遷移システム | Familiar の AI 状態変更 |
| `EncouragementRequest` | `Message` | Familiar AI execute 層 | 激励システム | Soul へのバイタル改善 |
| `FamiliarIdleVisualRequest` | `Message` | Familiar AI 状態遷移時 | visual アダプタ | Idle 遷移時の表示更新 |
| `GatheringSpawnRequest` | `Message` | `hw_soul_ai` 内の集会ロジック | root visual アダプタ | 集会スポットの生成 |
| `SoulTaskUnassignRequest` | `Message` | `hw_familiar_ai`（分隊解放・使役数超過）/ area_selection のユーザー取消 | `hw_soul_ai::handle_soul_task_unassign_system`（SoulAiSystemSet::Perceive）| 魂のタスク解除（`AssignedTask`リセット・インベントリ回収・予約解放）。area_selection は Perceive 前の `ApplyDeferred` を通し、同じ Update の Execute より先に適用する |

---

## 3. イベント使用上のルール

### R-E1: Request の可視化時点は ApplyDeferred と system set で決まる
`Commands::write_message()` と component 操作は次の `ApplyDeferred` まで他 system から見えない。
reader がその barrier より後に schedule されていれば同じ Update で適用でき、後なら次 frame になる。
同フレーム適用が必要な request は producer と consumer の間に明示した `ApplyDeferred` と順序制約を置く。

### R-E2: EntityEvent と Message の Entity lifetime を分ける
`EntityEvent` の対象エンティティは trigger 時点で生存していなければならない。
一方、presentation `Message` は次の Visual system で読むため、payload の `entity` が
reader 実行時には despawn 済みであり得る。MessageReader consumer は Query の失敗を no-op とし、
deferred command 内でも entity 生存を確認する。

### R-E3: OnTaskAbandoned の presentation reader はタスク状態を変更しない
`OnTaskAbandoned` の MessageReader はクリーンアップを行わない（I-S4 参照）。
誤って `unassign_task` を presentation system 内で呼ぶと二重 cleanup が発生する。

### R-E5: domain と presentation の transport を自動連結しない
`commands.trigger()` は EntityEvent を Observer へ配送するだけで、`Message` を書き込まない。
ゲーム状態の即時整合性が必要な副作用（バイタル変化・cleanup 等）は domain `EntityEvent` の
Observer に置く。speech bubble など遅延可能な副作用は `#[derive(Message)]` と
`MessagesPlugin` 登録を持つ presentation type を使い、`GameSystemSet::Visual` の
`MessageReader<T>` system で消費する。

両方が必要な通知は `publish_*` helper が `trigger` と `write_message` を各1回実行する。
presentation 専用通知は `write_message` のみを使い、visual-only Observer を追加しない。

### R-E4: イベント追加時はこのファイルを更新する
新しいイベントを `hw_core/src/events.rs` / `hw_jobs/src/events.rs` / その他クレートに追加した場合、
このカタログに Producer / Consumer / 副作用・`add_message` / `add_event` 登録箇所を記載すること。

### R-E6: `hw_world` 由来の `Message`（例: `TerrainChangedEvent`）
ドメインに近い通知で `hw_core` に置きにくい型は `hw_world` に置き、bevy_app の該当 `Plugin` で `add_message` / `add_event` を登録する。カタログの「定義」列に実パスを書く。

### R-E7: persistent world replacement前のMessage破棄
ロードは別のEntity世代へ切り替わるため、前worldで発行されたrequest/presentation `Message`を
new worldへ適用してはならない。`MessagesPlugin`に登録するroot message型は単一typed macroから
`add_message`と`Messages<T>::clear()`を生成し、`LoadResetRegistry`のreplace phaseで全bufferをclearする。
leaf UI messageとroot facade登録の`TerrainChangedEvent`も同じphaseでclearする。`EntityEvent`のobserver配送は
即時でありmessage bufferを持たないため、この対象には含めない。
