//! タスク実行のコンテキスト構造体

use crate::entities::damned_soul::{DamnedSoul, Destination, Path};
use crate::systems::logistics::{Inventory, ResourceItem};
use crate::systems::soul_ai::execute::task_execution::types::AssignedTask;
use bevy::prelude::*;

use crate::events::{ResourceReservationOp, ResourceReservationRequest, TaskAssignmentRequest};
/// タスク実行の基本コンテキスト
///
/// 各ハンドラー関数に共通する引数をまとめます。
/// CommandsとQueryはライフタイムが複雑なため、引数として残します。
use crate::relationships::{ManagedBy, TaskWorkers};
use crate::systems::familiar_ai::perceive::resource_sync::SharedResourceCache;
use crate::systems::jobs::{Blueprint, Designation, Priority, TaskSlots};
use crate::systems::logistics::{InStockpile, Stockpile};
use bevy::ecs::system::SystemParam;

/// リソース予約・管理に必要な共通アクセス
#[derive(SystemParam)]
pub struct ReservationAccess<'w, 's> {
    pub resources: Query<'w, 's, &'static ResourceItem>,
    pub resource_cache: Res<'w, SharedResourceCache>,
    pub reservation_writer: MessageWriter<'w, ResourceReservationRequest>,
}

/// 指定・場所・属性確認に必要な共通アクセス
#[derive(SystemParam)]
pub struct DesignationAccess<'w, 's> {
    pub targets: Query<
        'w,
        's,
        (
            &'static Transform,
            Option<&'static crate::systems::jobs::Tree>,
            Option<&'static crate::systems::jobs::Rock>,
            Option<&'static ResourceItem>,
            Option<&'static Designation>,
            Option<&'static crate::relationships::StoredIn>,
        ),
    >,
    pub designations: Query<
        'w,
        's,
        (
            Entity,
            &'static Transform,
            &'static Designation,
            Option<&'static ManagedBy>,
            Option<&'static TaskSlots>,
            Option<&'static TaskWorkers>,
            Option<&'static InStockpile>,
            Option<&'static Priority>,
        ),
    >,
    pub belongs: Query<'w, 's, &'static crate::systems::logistics::BelongsTo>,
}

/// 倉庫・設備・ブループリントへの読み取り専用アクセス
#[derive(SystemParam)]
pub struct StorageAccess<'w, 's> {
    pub stockpiles: Query<
        'w,
        's,
        (
            Entity,
            &'static Transform,
            &'static Stockpile,
            Option<&'static crate::relationships::StoredItems>,
        ),
    >,
    pub blueprints: Query<
        'w,
        's,
        (
            &'static Transform,
            &'static Blueprint,
            Option<&'static Designation>,
        ),
    >,
    pub target_blueprints: Query<'w, 's, &'static crate::systems::jobs::TargetBlueprint>,
    pub mixers: Query<
        'w,
        's,
        (
            &'static Transform,
            &'static crate::systems::jobs::MudMixerStorage,
            Option<&'static TaskWorkers>,
        ),
    >,
    pub target_mixers: Query<'w, 's, &'static crate::systems::jobs::TargetMixer>,
}

/// 倉庫・設備・ブループリントへの変更可能アクセス
#[derive(SystemParam)]
pub struct MutStorageAccess<'w, 's> {
    pub stockpiles: Query<
        'w,
        's,
        (
            Entity,
            &'static Transform,
            &'static mut Stockpile,
            Option<&'static crate::relationships::StoredItems>,
        ),
    >,
    pub blueprints: Query<
        'w,
        's,
        (
            &'static Transform,
            &'static mut Blueprint,
            Option<&'static Designation>,
        ),
    >,
    pub mixers: Query<
        'w,
        's,
        (
            &'static Transform,
            &'static mut crate::systems::jobs::MudMixerStorage,
            Option<&'static TaskWorkers>,
        ),
    >,
}

/// タスク割り当てに必要なクエリ群（Familiar AI向け）
#[derive(SystemParam)]
pub struct TaskAssignmentQueries<'w, 's> {
    pub reservation: ReservationAccess<'w, 's>,
    pub designation: DesignationAccess<'w, 's>,
    pub storage: StorageAccess<'w, 's>,

    // 固有フィールド
    pub items: Query<'w, 's, (&'static ResourceItem, Option<&'static Designation>)>,
    pub assignment_writer: MessageWriter<'w, TaskAssignmentRequest>,
    pub task_slots: Query<'w, 's, &'static crate::systems::jobs::TaskSlots>,
}

/// タスク実行に必要なクエリ群
#[derive(SystemParam)]
pub struct TaskQueries<'w, 's> {
    pub reservation: ReservationAccess<'w, 's>,
    pub designation: DesignationAccess<'w, 's>,
    pub storage: MutStorageAccess<'w, 's>,

    // 固有フィールド
    pub resource_items: Query<
        'w,
        's,
        (
            Entity,
            &'static crate::systems::logistics::ResourceItem,
            Option<&'static crate::relationships::StoredIn>,
        ),
    >,
}

/// タスク解除に必要なアクセス
pub trait TaskReservationAccess<'w, 's> {
    fn reservation_writer(&mut self) -> &mut MessageWriter<'w, ResourceReservationRequest>;
    fn resources(&self) -> &Query<'w, 's, &'static ResourceItem>;
}

impl<'w, 's> TaskReservationAccess<'w, 's> for TaskQueries<'w, 's> {
    fn reservation_writer(&mut self) -> &mut MessageWriter<'w, ResourceReservationRequest> {
        &mut self.reservation.reservation_writer
    }

    fn resources(&self) -> &Query<'w, 's, &'static ResourceItem> {
        &self.reservation.resources
    }
}

impl<'w, 's> TaskReservationAccess<'w, 's> for TaskAssignmentQueries<'w, 's> {
    fn reservation_writer(&mut self) -> &mut MessageWriter<'w, ResourceReservationRequest> {
        &mut self.reservation.reservation_writer
    }

    fn resources(&self) -> &Query<'w, 's, &'static ResourceItem> {
        &self.reservation.resources
    }
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

    /// リソース予約更新の要求を追加
    pub fn queue_reservation(&mut self, op: ResourceReservationOp) {
        self.queries
            .reservation
            .reservation_writer
            .write(ResourceReservationRequest { op });
    }
}
