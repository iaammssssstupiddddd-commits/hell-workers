use bevy::prelude::*;
use hw_core::events::{ResourceReservationOp, ResourceReservationRequest};
use hw_core::soul::{DamnedSoul, Destination, Path};
use hw_logistics::types::Inventory;
use hw_world::PathfindingContext;

use hw_jobs::AssignedTask;

use super::queries::TaskQueries;

/// タスク実行の基本コンテキスト
pub struct TaskExecutionContext<'a, 'w, 's> {
    pub soul_entity: Entity,
    pub soul_transform: &'a Transform,
    pub soul: &'a mut DamnedSoul,
    pub task: &'a mut AssignedTask,
    pub dest: &'a mut Destination,
    pub path: &'a mut Path,
    pub inventory: &'a mut Inventory,
    pub pf_context: &'a mut PathfindingContext,
    pub queries: &'a mut TaskQueries<'w, 's>,
}

impl<'a, 'w, 's> TaskExecutionContext<'a, 'w, 's> {
    /// 魂の位置を取得
    pub fn soul_pos(&self) -> Vec2 {
        self.soul_transform.translation.truncate()
    }

    /// リソース予約更新の要求を追加
    pub fn queue_reservation(&mut self, op: ResourceReservationOp) {
        self.queries
            .reservation
            .reservation_writer
            .write(ResourceReservationRequest { op });
    }
}
