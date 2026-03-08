pub use hw_core::events::{
    EncouragementRequest, EscapeOperation, EscapeRequest, FamiliarAiStateTransitionReason,
    FamiliarIdleVisualRequest, FamiliarOperationMaxSoulChangedEvent, GatheringManagementOp,
    GatheringManagementRequest, IdleBehaviorOperation, IdleBehaviorRequest, OnEncouraged,
    OnExhausted, OnGatheringJoined, OnGatheringLeft, OnGatheringParticipated,
    OnReleasedFromService, OnSoulRecruited, OnStressBreakdown, OnTaskAbandoned, ReleaseReason,
    SquadManagementOperation, SquadManagementRequest,
};

use crate::systems::jobs::WorkType;
use crate::systems::logistics::ResourceType;
use crate::systems::soul_ai::execute::task_execution::types::AssignedTask;
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

/// リソース予約の更新要求
#[derive(Message, Debug, Clone)]
pub struct ResourceReservationRequest {
    pub op: ResourceReservationOp,
}

/// リソース予約の操作
#[derive(Debug, Clone)]
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
        reserved_for_task: bool,
    },
}

// ============================================================
// Familiar AI Requests (Decide -> Execute)
// ============================================================

/// 使い魔のAI状態変更要求
#[derive(Message, Debug, Clone)]
pub struct FamiliarStateRequest {
    pub familiar_entity: Entity,
    pub new_state: crate::systems::familiar_ai::FamiliarAiState,
}
