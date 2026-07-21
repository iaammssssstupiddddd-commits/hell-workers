use crate::familiar::FamiliarAiState;
use crate::gathering::GatheringObjectType;
use crate::jobs::WorkType;
use crate::logistics::ResourceType;
use crate::soul::DreamQuality;
use crate::world::GridPos;
use bevy::prelude::*;

/// Dream が共有 Pool へ移送された時点の presentation anchor。
///
/// Visual 側が Soul の現在状態や relationship を再計算せず、退出・despawn 後も
/// producer が観測した位置を fallback として使えるよう snapshot を保持する。
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DreamTransferVisualSource {
    Sleeping { origin: Vec2 },
    RestArea { rest_area: Entity, origin: Vec2 },
}

/// `DamnedSoul.dream` から `DreamPool` へ実際に移送された量の presentation 通知。
///
/// `amount` は producer が Pool に加算した同じ delta であり、Visual 側で
/// drain rate や現在の Soul state から再計算してはならない。`is_final` は
/// producer がこの transfer stream の終了を確定できた場合だけ立てる。
#[derive(Message, Debug, Clone, Copy, PartialEq)]
pub struct DreamTransferredVisualMessage {
    pub soul: Entity,
    pub amount: f32,
    pub quality: DreamQuality,
    pub source: DreamTransferVisualSource,
    pub is_final: bool,
}

/// 魂が使い魔に勧誘（使役開始）された際の domain 通知。
#[derive(EntityEvent, Debug, Clone, Copy, PartialEq, Eq)]
pub struct OnSoulRecruited {
    pub entity: Entity,
    pub familiar_entity: Entity,
}

/// 勧誘時の presentation 通知。
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct SoulRecruitedVisualMessage {
    pub entity: Entity,
    pub familiar_entity: Entity,
}

/// ストレスが限界に達した際の domain 通知。
#[derive(EntityEvent, Debug, Clone, Copy, PartialEq, Eq)]
pub struct OnStressBreakdown {
    pub entity: Entity,
}

/// ストレス崩壊時の presentation 通知。
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct SoulStressBreakdownVisualMessage {
    pub entity: Entity,
}

/// 疲労が限界に達した（強制集会へ）際の domain 通知。
#[derive(EntityEvent, Debug, Clone, Copy, PartialEq, Eq)]
pub struct OnExhausted {
    pub entity: Entity,
}

/// 疲労限界時の presentation 通知。
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct SoulExhaustedVisualMessage {
    pub entity: Entity,
}

/// 魂が使い魔の使役から解放された
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct OnReleasedFromService {
    pub entity: Entity,
}

/// 魂が自発的に集会に参加した
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct OnGatheringJoined {
    pub entity: Entity,
}

/// 魂のタスクが中断・放棄された
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct OnTaskAbandoned {
    pub entity: Entity,
}

/// 魂がタスクに割り当てられた
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct OnTaskAssigned {
    pub entity: Entity,
    pub assignment_entity: Entity,
    pub current_target_entity: Entity,
    pub current_work_type: WorkType,
}

/// 魂がタスクを完了した際の domain 通知。
#[derive(EntityEvent, Debug, Clone, Copy, PartialEq, Eq)]
pub struct OnTaskCompleted {
    pub entity: Entity,
    pub assignment_entity: Entity,
    pub current_target_entity: Entity,
    pub current_work_type: WorkType,
}

/// タスク完了時の presentation 通知。
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct TaskCompletedVisualMessage {
    pub entity: Entity,
    pub assignment_entity: Entity,
    pub current_target_entity: Entity,
    pub current_work_type: WorkType,
}

/// 使い魔の使役数上限が変更された
#[derive(Message)]
pub struct FamiliarOperationMaxSoulChangedEvent {
    pub familiar_entity: Entity,
    pub old_value: usize,
    pub new_value: usize,
}

/// 魂が集会に参加した（スポット管理用）
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct OnGatheringParticipated {
    pub entity: Entity,
    pub spot_entity: Entity,
}

/// 使い魔が魂を激励した際の domain 通知。
#[derive(EntityEvent, Debug, Clone, Copy, PartialEq, Eq)]
pub struct OnEncouraged {
    pub familiar_entity: Entity,
    #[event_target]
    pub soul_entity: Entity,
}

/// 激励時の presentation 通知。
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct SoulEncouragedVisualMessage {
    pub familiar_entity: Entity,
    pub soul_entity: Entity,
}

/// domain と presentation の両方が必要な勧誘通知を発行する。
pub fn publish_soul_recruited(commands: &mut Commands, entity: Entity, familiar_entity: Entity) {
    commands.trigger(OnSoulRecruited {
        entity,
        familiar_entity,
    });
    commands.write_message(SoulRecruitedVisualMessage {
        entity,
        familiar_entity,
    });
}

/// domain と presentation の両方が必要なストレス崩壊通知を発行する。
pub fn publish_stress_breakdown(commands: &mut Commands, entity: Entity) {
    commands.trigger(OnStressBreakdown { entity });
    commands.write_message(SoulStressBreakdownVisualMessage { entity });
}

/// domain と presentation の両方が必要な疲労限界通知を発行する。
pub fn publish_soul_exhausted(commands: &mut Commands, entity: Entity) {
    commands.trigger(OnExhausted { entity });
    commands.write_message(SoulExhaustedVisualMessage { entity });
}

/// domain と presentation の両方が必要なタスク完了通知を発行する。
pub fn publish_task_completed(
    commands: &mut Commands,
    entity: Entity,
    assignment_entity: Entity,
    current_target_entity: Entity,
    current_work_type: WorkType,
) {
    commands.trigger(OnTaskCompleted {
        entity,
        assignment_entity,
        current_target_entity,
        current_work_type,
    });
    commands.write_message(TaskCompletedVisualMessage {
        entity,
        assignment_entity,
        current_target_entity,
        current_work_type,
    });
}

/// domain と presentation の両方が必要な激励通知を発行する。
pub fn publish_soul_encouraged(
    commands: &mut Commands,
    familiar_entity: Entity,
    soul_entity: Entity,
) {
    commands.trigger(OnEncouraged {
        familiar_entity,
        soul_entity,
    });
    commands.write_message(SoulEncouragedVisualMessage {
        familiar_entity,
        soul_entity,
    });
}

/// 状態遷移の理由
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FamiliarAiStateTransitionReason {
    CommandChanged,
    SquadEmpty,
    SquadFull,
    RecruitSuccess,
    ScoutingCancelled,
    Unknown,
}

/// 使い魔のAI状態が変更された
#[derive(Message)]
pub struct FamiliarAiStateChangedEvent {
    pub familiar_entity: Entity,
    pub from: FamiliarAiState,
    pub to: FamiliarAiState,
    pub reason: FamiliarAiStateTransitionReason,
}

/// アイドル行動の変更要求
#[derive(Message, Debug, Clone)]
pub struct IdleBehaviorRequest {
    pub entity: Entity,
    pub operation: IdleBehaviorOperation,
}

/// アイドル行動の操作種別
#[derive(Debug, Clone)]
pub enum IdleBehaviorOperation {
    JoinGathering { spot_entity: Entity },
    LeaveGathering { spot_entity: Entity },
    ArriveAtGathering { spot_entity: Entity },
    ReserveRestArea { rest_area_entity: Entity },
    ReleaseRestArea,
    EnterRestArea { rest_area_entity: Entity },
    LeaveRestArea,
}

/// 逃走行動の変更要求
#[derive(Message, Debug, Clone)]
pub struct EscapeRequest {
    pub entity: Entity,
    pub operation: EscapeOperation,
}

/// 逃走行動の操作種別
#[derive(Debug, Clone)]
pub enum EscapeOperation {
    StartEscaping { leave_gathering: Option<Entity> },
    UpdateDestination { destination: Vec2 },
    ReachSafety,
    JoinSafeGathering,
}

/// 集会管理の変更要求
#[derive(Message, Debug, Clone)]
pub struct GatheringManagementRequest {
    pub operation: GatheringManagementOp,
}

/// 集会管理の操作種別
#[derive(Debug, Clone)]
pub enum GatheringManagementOp {
    Dissolve {
        spot_entity: Entity,
        aura_entity: Entity,
        object_entity: Option<Entity>,
    },
    Merge {
        absorber: Entity,
        absorbed: Entity,
        participants_to_move: Vec<Entity>,
        absorbed_aura: Entity,
        absorbed_object: Option<Entity>,
    },
    Recruit {
        soul: Entity,
        spot: Entity,
    },
    Leave {
        soul: Entity,
        spot: Entity,
    },
}

/// リソース予約の更新要求
#[derive(Message, Debug, Clone)]
pub struct ResourceReservationRequest {
    pub op: ResourceReservationOp,
}

/// リソース予約の操作
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResourceReservationOp {
    ReserveMixerDestination {
        target: Entity,
        resource_type: ResourceType,
    },
    ReleaseMixerDestination {
        target: Entity,
        resource_type: ResourceType,
    },
    ReserveSource {
        source: Entity,
        amount: usize,
    },
    ReleaseSource {
        source: Entity,
        amount: usize,
    },
    RecordPickedSource {
        source: Entity,
        amount: usize,
    },
}

/// Designation の発行要求
#[derive(Message, Debug, Clone)]
pub struct DesignationRequest {
    pub entity: Entity,
    pub operation: DesignationOp,
}

/// Designation 発行の操作種別
#[derive(Debug, Clone)]
pub enum DesignationOp {
    Issue {
        work_type: WorkType,
        issued_by: Entity,
        task_slots: u32,
        priority: Option<u32>,
        target_blueprint: Option<Entity>,
        target_mixer: Option<Entity>,
    },
}

/// 使い魔のAI状態変更要求
#[derive(Message, Debug, Clone)]
pub struct FamiliarStateRequest {
    pub familiar_entity: Entity,
    pub new_state: FamiliarAiState,
}

/// 使い魔による激励要求
#[derive(Message, Debug, Clone)]
pub struct EncouragementRequest {
    pub familiar_entity: Entity,
    pub soul_entity: Entity,
}

/// 使い魔のIdle遷移時ビジュアル要求
#[derive(Message, Debug, Clone)]
pub struct FamiliarIdleVisualRequest {
    pub familiar_entity: Entity,
}

/// 使い魔の分隊管理要求
#[derive(Message, Debug, Clone)]
pub struct SquadManagementRequest {
    pub familiar_entity: Entity,
    pub operation: SquadManagementOperation,
}

/// 分隊管理の操作種別
#[derive(Debug, Clone)]
pub enum SquadManagementOperation {
    AddMember {
        soul_entity: Entity,
    },
    ReleaseMember {
        soul_entity: Entity,
        reason: ReleaseReason,
    },
}

/// 分隊解放の理由
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReleaseReason {
    Fatigued,
}

/// 集会スポット生成リクエスト (hw_ai → root visual adapter へのブリッジ)
#[derive(Message, Debug, Clone)]
pub struct GatheringSpawnRequest {
    pub pos: Vec2,
    pub object_type: GatheringObjectType,
    pub initiator_entity: Entity,
    pub created_at: f32,
}

/// 漂流脱走開始シグナル (decide/drifting → root adapter へのブリッジ)
/// root adapter が `PopulationManager::start_escape_cooldown()` を呼び出す。
#[derive(Event, Debug)]
pub struct DriftingEscapeStarted;

/// Soul がマップ端に到達して脱出したシグナル (execute/drifting → root adapter へのブリッジ)
/// root adapter が `PopulationManager::total_escaped` をインクリメントする。
#[derive(Event, Debug)]
pub struct SoulEscaped {
    pub entity: Entity,
    pub grid: GridPos,
}

/// 魂のタスク解除要求（Familiar AI → Soul AI Pub/Sub ブリッジ）
///
/// `hw_familiar_ai` が送信し、`hw_soul_ai` の Perceive フェーズで処理される。
#[derive(Message, Debug, Clone)]
pub struct SoulTaskUnassignRequest {
    pub soul_entity: Entity,
    /// `true` の場合、`OnTaskAbandoned` イベントを発行する
    pub emit_abandoned: bool,
}
