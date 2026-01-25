//! タスク実行のコンテキスト構造体

use crate::entities::damned_soul::{DamnedSoul, Destination, Path};
use crate::systems::logistics::Inventory;
use crate::systems::soul_ai::task_execution::types::AssignedTask;
use bevy::prelude::*;

/// タスク実行の基本コンテキスト
/// 
/// 各ハンドラー関数に共通する引数をまとめます。
/// CommandsとQueryはライフタイムが複雑なため、引数として残します。
pub struct TaskExecutionContext<'a> {
    pub soul_entity: Entity,
    pub soul_transform: &'a Transform,
    pub soul: &'a mut DamnedSoul,
    pub task: &'a mut AssignedTask,
    pub dest: &'a mut Destination,
    pub path: &'a mut Path,
    pub inventory: &'a mut Inventory,
    pub pf_context: &'a mut crate::world::pathfinding::PathfindingContext,
}

impl<'a> TaskExecutionContext<'a> {
    /// 魂の位置を取得
    pub fn soul_pos(&self) -> Vec2 {
        self.soul_transform.translation.truncate()
    }
}
