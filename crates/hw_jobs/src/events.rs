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
/// `hw_soul_ai` の Observer が WorldMap 更新と ObstaclePosition の配置を担当する。
#[derive(Event, Debug, Clone)]
pub struct BuildingCompletedEvent {
    /// The exact construction owner replaced by this completed building.
    pub blueprint_entity: Entity,
    pub building_entity: Entity,
    pub kind: BuildingType,
    pub occupied_grids: Vec<(i32, i32)>,
}

/// Presentation receipt for an exact completed construction owner, which may
/// already be despawned when the next presentation system reads this message.
#[derive(Message, Debug, Clone)]
pub struct BuildingCompletedVisualMessage {
    pub blueprint_entity: Entity,
}

pub fn publish_building_completed(commands: &mut Commands, event: BuildingCompletedEvent) {
    commands.write_message(BuildingCompletedVisualMessage {
        blueprint_entity: event.blueprint_entity,
    });
    commands.trigger(event);
}
