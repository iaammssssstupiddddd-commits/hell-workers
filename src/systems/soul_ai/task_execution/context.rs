//! タスク実行のコンテキスト構造体

use crate::entities::damned_soul::{DamnedSoul, Destination, Path};
use crate::systems::logistics::Inventory;
use crate::systems::soul_ai::task_execution::types::AssignedTask;
use bevy::prelude::*;

/// タスク実行の基本コンテキスト
/// 
/// 各ハンドラー関数に共通する引数をまとめます。
/// CommandsとQueryはライフタイムが複雑なため、引数として残します。
use crate::relationships::{ManagedBy, TaskWorkers};
use crate::systems::jobs::{Designation, TaskSlots, Priority, Blueprint};
use crate::systems::logistics::{Stockpile, InStockpile};
use bevy::ecs::system::SystemParam;

/// タスク実行に必要なクエリ群
#[derive(SystemParam)]
pub struct TaskQueries<'w, 's> {
    pub targets: Query<'w, 's, (
        &'static Transform,
        Option<&'static crate::systems::jobs::Tree>,
        Option<&'static crate::systems::jobs::Rock>,
        Option<&'static crate::systems::logistics::ResourceItem>,
        Option<&'static Designation>,
        Option<&'static crate::relationships::StoredIn>,
    )>,
    pub designations: Query<'w, 's, (
        Entity,
        &'static Transform,
        &'static Designation,
        Option<&'static ManagedBy>,
        Option<&'static TaskSlots>,
        Option<&'static TaskWorkers>,
        Option<&'static InStockpile>,
        Option<&'static Priority>,
    )>,
    pub stockpiles: Query<'w, 's, (
        Entity,
        &'static Transform,
        &'static mut Stockpile,
        Option<&'static crate::relationships::StoredItems>,
    )>,
    pub belongs: Query<'w, 's, &'static crate::systems::logistics::BelongsTo>,
    pub blueprints: Query<'w, 's, (&'static Transform, &'static mut Blueprint, Option<&'static Designation>)>,
    pub target_blueprints: Query<'w, 's, &'static crate::systems::jobs::TargetBlueprint>,
    pub items: Query<'w, 's, (&'static crate::systems::logistics::ResourceItem, Option<&'static Designation>)>,
}

/// タスク実行の基本コンテキスト
pub struct TaskExecutionContext<'a, 'w, 's> {
    pub soul_entity: Entity,
    pub soul_transform: &'a Transform,
    pub soul: &'a mut DamnedSoul,
    pub task: &'a mut AssignedTask,
    pub dest: &'a mut Destination,
    pub path: &'a mut Path,
    pub inventory: &'a mut Inventory,
    pub pf_context: &'a mut crate::world::pathfinding::PathfindingContext,
    pub queries: &'a mut TaskQueries<'w, 's>,
}

impl<'a, 'w, 's> TaskExecutionContext<'a, 'w, 's> {
    /// 魂の位置を取得
    pub fn soul_pos(&self) -> Vec2 {
        self.soul_transform.translation.truncate()
    }
}
