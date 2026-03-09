use crate::assigned_task::AssignedTask;
use bevy::prelude::*;
use hw_core::events::ResourceReservationOp;
use hw_core::jobs::WorkType;

/// 魂がタスクに割り当てられた（実行要求）
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
