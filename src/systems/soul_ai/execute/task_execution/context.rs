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
use crate::systems::jobs::{Blueprint, Designation, Priority, StoredByMixer, TaskSlots};
use crate::systems::logistics::Stockpile;
use bevy::ecs::system::SystemParam;

/// リソース予約・管理に必要な共通アクセス
#[derive(SystemParam)]
pub struct ReservationAccess<'w, 's> {
    pub resources: Query<'w, 's, &'static ResourceItem>,
    pub resource_cache: Res<'w, SharedResourceCache>,
    pub reservation_writer: MessageWriter<'w, ResourceReservationRequest>,
    pub incoming_deliveries_query: Query<'w, 's, &'static crate::relationships::IncomingDeliveries>,
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
            Option<&'static crate::systems::jobs::TreeVariant>,
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
            Option<&'static crate::relationships::StoredIn>,
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
    pub loaded_in: Query<'w, 's, &'static crate::relationships::LoadedIn>,
    pub loaded_items: Query<'w, 's, &'static crate::relationships::LoadedItems>,
    pub bucket_storages: Query<'w, 's, (), With<crate::systems::logistics::BucketStorage>>,
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
    pub floor_sites: Query<
        'w,
        's,
        (
            &'static Transform,
            &'static crate::systems::jobs::floor_construction::FloorConstructionSite,
            Option<&'static TaskWorkers>,
        ),
    >,
    pub floor_tiles: Query<
        'w,
        's,
        &'static crate::systems::jobs::floor_construction::FloorTileBlueprint,
    >,
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
    pub loaded_in: Query<'w, 's, &'static crate::relationships::LoadedIn>,
    pub loaded_items: Query<'w, 's, &'static crate::relationships::LoadedItems>,
    pub bucket_storages: Query<'w, 's, (), With<crate::systems::logistics::BucketStorage>>,
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
    pub target_mixers: Query<'w, 's, &'static crate::systems::jobs::TargetMixer>,
    pub floor_sites: Query<
        'w,
        's,
        (
            &'static Transform,
            &'static mut crate::systems::jobs::floor_construction::FloorConstructionSite,
            Option<&'static TaskWorkers>,
        ),
    >,
    pub floor_tiles: Query<
        'w,
        's,
        &'static mut crate::systems::jobs::floor_construction::FloorTileBlueprint,
    >,
}

/// タスク割り当てに必要なクエリ群（Familiar AI向け）
#[derive(SystemParam)]
pub struct TaskAssignmentQueries<'w, 's> {
    pub reservation: ReservationAccess<'w, 's>,
    pub designation: DesignationAccess<'w, 's>,
    pub storage: StorageAccess<'w, 's>,

    // 固有フィールド
    pub world_map: Res<'w, crate::world::map::WorldMap>,
    pub items: Query<'w, 's, (&'static ResourceItem, Option<&'static Designation>)>,
    pub sand_piles: Query<
        'w,
        's,
        (
            Entity,
            &'static Transform,
            Option<&'static Designation>,
            Option<&'static TaskWorkers>,
        ),
        With<crate::systems::jobs::SandPile>,
    >,
    pub bone_piles: Query<
        'w,
        's,
        (
            Entity,
            &'static Transform,
            Option<&'static Designation>,
            Option<&'static TaskWorkers>,
        ),
        With<crate::systems::jobs::BonePile>,
    >,
    pub task_state: Query<'w, 's, (Option<&'static Designation>, Option<&'static TaskWorkers>)>,
    pub transport_requests: Query<'w, 's, &'static crate::systems::logistics::transport_request::TransportRequest>,
    pub transport_demands:
        Query<'w, 's, &'static crate::systems::logistics::transport_request::TransportDemand>,
    pub transport_request_fixed_sources:
        Query<'w, 's, &'static crate::systems::logistics::transport_request::TransportRequestFixedSource>,
    pub free_resource_items: Query<
        'w,
        's,
        (
            Entity,
            &'static Transform,
            &'static Visibility,
            &'static ResourceItem,
        ),
        (
            Without<Designation>,
            Without<crate::relationships::TaskWorkers>,
            Without<crate::systems::logistics::ReservedForTask>,
            Without<crate::systems::logistics::transport_request::ManualHaulPinnedSource>,
        ),
    >,
    pub reserved_for_task:
        Query<'w, 's, &'static crate::systems::logistics::ReservedForTask>,
    pub assignment_writer: MessageWriter<'w, TaskAssignmentRequest>,
    pub task_slots: Query<'w, 's, &'static crate::systems::jobs::TaskSlots>,
    pub wheelbarrows: Query<
        'w,
        's,
        (Entity, &'static Transform),
        (
            With<crate::systems::logistics::Wheelbarrow>,
            With<crate::relationships::ParkedAt>,
            Without<crate::relationships::PushedBy>,
        ),
    >,
    pub wheelbarrow_leases: Query<'w, 's, &'static crate::systems::logistics::transport_request::WheelbarrowLease>,
    pub stored_items_query: Query<
        'w,
        's,
        (
            Entity,
            &'static ResourceItem,
            &'static crate::relationships::StoredIn,
        ),
        (
            Without<Designation>,
            Without<crate::relationships::TaskWorkers>,
            Without<crate::systems::logistics::ReservedForTask>,
        ),
    >,
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
    pub mixer_stored_mud: Query<'w, 's, &'static StoredByMixer>,
    pub transport_request_status: Query<
        'w,
        's,
        (
            &'static crate::systems::logistics::transport_request::TransportRequest,
            &'static crate::systems::logistics::transport_request::TransportDemand,
            &'static crate::systems::logistics::transport_request::TransportRequestState,
            Option<&'static crate::systems::logistics::transport_request::WheelbarrowLease>,
            Option<&'static crate::relationships::TaskWorkers>,
        ),
    >,
}

/// タスク解除に必要なアクセス
pub trait TaskReservationAccess<'w, 's> {
    fn reservation_writer(&mut self) -> &mut MessageWriter<'w, ResourceReservationRequest>;
    fn resources(&self) -> &Query<'w, 's, &'static ResourceItem>;
    fn belongs_to(&self, entity: Entity) -> Option<Entity>;
}

impl<'w, 's> TaskReservationAccess<'w, 's> for TaskQueries<'w, 's> {
    fn reservation_writer(&mut self) -> &mut MessageWriter<'w, ResourceReservationRequest> {
        &mut self.reservation.reservation_writer
    }

    fn resources(&self) -> &Query<'w, 's, &'static ResourceItem> {
        &self.reservation.resources
    }

    fn belongs_to(&self, entity: Entity) -> Option<Entity> {
        self.designation.belongs.get(entity).ok().map(|belongs| belongs.0)
    }
}

impl<'w, 's> TaskReservationAccess<'w, 's> for TaskAssignmentQueries<'w, 's> {
    fn reservation_writer(&mut self) -> &mut MessageWriter<'w, ResourceReservationRequest> {
        &mut self.reservation.reservation_writer
    }

    fn resources(&self) -> &Query<'w, 's, &'static ResourceItem> {
        &self.reservation.resources
    }

    fn belongs_to(&self, entity: Entity) -> Option<Entity> {
        self.designation.belongs.get(entity).ok().map(|belongs| belongs.0)
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
