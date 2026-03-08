pub use hw_core::events::{
    DesignationOp, DesignationRequest, EncouragementRequest, EscapeOperation, EscapeRequest,
    FamiliarAiStateChangedEvent, FamiliarAiStateTransitionReason, FamiliarIdleVisualRequest,
    FamiliarOperationMaxSoulChangedEvent, FamiliarStateRequest, GatheringManagementOp,
    GatheringManagementRequest, IdleBehaviorOperation, IdleBehaviorRequest, OnEncouraged,
    OnExhausted, OnGatheringJoined, OnGatheringLeft, OnGatheringParticipated,
    OnReleasedFromService, OnSoulRecruited, OnStressBreakdown, OnTaskAbandoned,
    OnTaskAssigned, OnTaskCompleted, ResourceReservationOp, ResourceReservationRequest,
    ReleaseReason, SquadManagementOperation, SquadManagementRequest,
};

use hw_core::jobs::WorkType;
use crate::systems::soul_ai::execute::task_execution::types::AssignedTask;
use bevy::prelude::*;

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
// Familiar AI Requests (Decide -> Execute)
// ============================================================
