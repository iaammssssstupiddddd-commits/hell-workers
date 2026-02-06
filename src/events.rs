use crate::systems::jobs::WorkType;
use crate::systems::logistics::ResourceType;
use crate::systems::soul_ai::task_execution::types::AssignedTask;
use bevy::prelude::*;

/// 魂がタスクに割り当てられた
#[derive(Message, EntityEvent)]
pub struct OnTaskAssigned {
    pub entity: Entity, // Observerのターゲット（魂）
    pub task_entity: Entity,
    pub work_type: WorkType,
}

/// 魂がタスクを完了した
#[derive(Message, EntityEvent)]
pub struct OnTaskCompleted {
    pub entity: Entity, // 魂
    pub task_entity: Entity,
    pub work_type: WorkType,
}

/// 魂が使い魔に勧誘（使役開始）された
#[derive(Message, EntityEvent)]
pub struct OnSoulRecruited {
    pub entity: Entity, // 魂
    pub familiar_entity: Entity,
}

/// ストレスによるブレイクダウン/// ストレスが限界に達した
#[derive(Message, EntityEvent)]
pub struct OnStressBreakdown {
    pub entity: Entity,
}

/// 疲労が限界に達した（強制集会へ）
#[derive(Message, EntityEvent)]
pub struct OnExhausted {
    pub entity: Entity, // 魂
}

/// 魂が使い魔の使役から解放された
#[derive(Message, EntityEvent)]
pub struct OnReleasedFromService {
    pub entity: Entity,
}

/// 魂が自発的に集会に参加した
#[derive(Message, EntityEvent)]
pub struct OnGatheringJoined {
    pub entity: Entity,
}

/// 魂のタスクが中断・放棄された
#[derive(Message, EntityEvent)]
pub struct OnTaskAbandoned {
    pub entity: Entity,
}

/// 使い魔の使役数上限が変更された
#[derive(Message)]
pub struct FamiliarOperationMaxSoulChangedEvent {
    pub familiar_entity: Entity,
    pub old_value: usize,
    pub new_value: usize,
}

/// 魂が集会に参加した（スポット管理用）
#[derive(Message, EntityEvent)]
pub struct OnGatheringParticipated {
    pub entity: Entity,
    pub spot_entity: Entity,
}

/// 魂が集会から離脱した（スポット管理用）
#[derive(Message, EntityEvent)]
pub struct OnGatheringLeft {
    pub entity: Entity,
    pub spot_entity: Entity,
}

/// 使い魔が魂を激励した
#[derive(Message, EntityEvent)]
pub struct OnEncouraged {
    pub familiar_entity: Entity,
    #[event_target]
    pub soul_entity: Entity,
}

/// 使い魔のAI状態が変更された
#[derive(Message)]
pub struct FamiliarAiStateChangedEvent {
    /// 使い魔のエンティティ
    pub familiar_entity: Entity,
    /// 遷移前の状態
    pub from: crate::systems::familiar_ai::FamiliarAiState,
    /// 遷移後の状態
    pub to: crate::systems::familiar_ai::FamiliarAiState,
    /// 遷移の理由
    pub reason: FamiliarAiStateTransitionReason,
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

/// リソース予約の更新要求
#[derive(Message, Debug, Clone)]
pub struct ResourceReservationRequest {
    pub op: ResourceReservationOp,
}

/// リソース予約の操作
#[derive(Debug, Clone)]
pub enum ResourceReservationOp {
    ReserveDestination {
        target: Entity,
    },
    ReleaseDestination {
        target: Entity,
    },
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
    RecordStoredDestination {
        target: Entity,
    },
    RecordPickedSource {
        source: Entity,
        amount: usize,
    },
}

/// タスク割り当て要求（Think -> Act）
#[derive(Message, Debug, Clone)]
pub struct TaskAssignmentRequest {
    pub familiar_entity: Entity,
    pub worker_entity: Entity,
    pub task_entity: Entity,
    pub work_type: WorkType,
    pub task_pos: Vec2,
    pub assigned_task: AssignedTask,
    pub reservation_ops: Vec<ResourceReservationOp>,
    pub already_commanded: bool,
}

// ============================================================
// Idle Behavior Requests (Decide -> Execute)
// ============================================================

/// アイドル行動の変更要求
#[derive(Message, Debug, Clone)]
pub struct IdleBehaviorRequest {
    pub entity: Entity,
    pub operation: IdleBehaviorOperation,
}

/// アイドル行動の操作種別
#[derive(Debug, Clone)]
pub enum IdleBehaviorOperation {
    /// 集会に参加
    JoinGathering { spot_entity: Entity },
    /// 集会から離脱
    LeaveGathering { spot_entity: Entity },
    /// 集会に到着（ExhaustedGathering -> Gathering）
    ArriveAtGathering { spot_entity: Entity },
}

// ============================================================
// Familiar AI Requests (Decide -> Execute)
// ============================================================

/// 使い魔の分隊管理要求
#[derive(Message, Debug, Clone)]
pub struct SquadManagementRequest {
    pub familiar_entity: Entity,
    pub operation: SquadManagementOperation,
}

/// 分隊管理の操作種別
#[derive(Debug, Clone)]
pub enum SquadManagementOperation {
    /// 魂を分隊に追加（Commanding関係を設定）
    AddMember { soul_entity: Entity },
    /// 魂を分隊から解放（Commanding関係を削除）
    ReleaseMember {
        soul_entity: Entity,
        reason: ReleaseReason,
    },
}

/// 分隊解放の理由
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReleaseReason {
    /// 疲労またはストレスによる自動解放
    Fatigued,
}
