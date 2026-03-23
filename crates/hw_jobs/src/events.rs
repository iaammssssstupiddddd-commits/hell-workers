use crate::model::BuildingType;
use crate::tasks::AssignedTask;
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

/// Blueprint が全工程完了し、建物エンティティが spawn された直後に発行される。
/// `hw_jobs` の Observer が WorldMap 更新と ObstaclePosition の配置を担当する。
#[derive(Event, Debug, Clone)]
pub struct BuildingCompletedEvent {
    pub building_entity: Entity,
    pub kind: BuildingType,
    pub occupied_grids: Vec<(i32, i32)>,
}
