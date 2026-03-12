//! Familiar AI タスク管理クエリ境界型
//!
//! root の ConstructionSiteAccess に依存しないクエリ型と、
//! 建設サイト位置を抽象化するブリッジトレイトを定義する。

use std::ops::{Deref, DerefMut};

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;

use hw_core::events::ResourceReservationRequest;
use hw_core::relationships::{
    IncomingDeliveries, LoadedIn, LoadedItems, ManagedBy, ParkedAt, PushedBy, StoredIn,
    StoredItems, TaskWorkers,
};
use hw_jobs::{
    Blueprint, BonePile, Building, Designation, Priority, ProvisionalWall, SandPile, TargetBlueprint,
    TaskSlots, Tree,
};
use hw_jobs::construction::{FloorTileBlueprint, WallTileBlueprint};
use hw_jobs::mud_mixer::{MudMixerStorage, TargetMixer};
use hw_logistics::SharedResourceCache;
use hw_logistics::transport_request::{
    ManualHaulPinnedSource, TransportDemand, TransportRequest, TransportRequestFixedSource,
    WheelbarrowLease,
};
use hw_logistics::types::{BelongsTo, BucketStorage, ReservedForTask, ResourceItem, Wheelbarrow};
use hw_logistics::zone::Stockpile;
use hw_world::WorldMapRead;

pub use hw_jobs::construction::ConstructionSitePositions;
/// リソース予約・管理に必要な共通アクセス
#[derive(SystemParam)]
pub struct ReservationAccess<'w, 's> {
    pub resources: Query<'w, 's, &'static ResourceItem>,
    pub resource_cache: Res<'w, SharedResourceCache>,
    pub reservation_writer: MessageWriter<'w, ResourceReservationRequest>,
    pub incoming_deliveries_query: Query<'w, 's, (Entity, &'static IncomingDeliveries)>,
}

/// 指定・場所・属性確認に必要な共通アクセス
#[derive(SystemParam)]
pub struct DesignationAccess<'w, 's> {
    pub targets: Query<
        'w,
        's,
        (
            &'static Transform,
            Option<&'static Tree>,
            Option<&'static hw_jobs::TreeVariant>,
            Option<&'static hw_jobs::Rock>,
            Option<&'static ResourceItem>,
            Option<&'static Designation>,
            Option<&'static StoredIn>,
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
            Option<&'static StoredIn>,
            Option<&'static Priority>,
        ),
    >,
    pub belongs: Query<'w, 's, &'static BelongsTo>,
}

/// 倉庫・設備・ブループリントへの読み取り専用アクセス（Familiar AI向け・建設サイト除く）
#[derive(SystemParam)]
pub struct FamiliarStorageAccess<'w, 's> {
    pub stockpiles: Query<
        'w,
        's,
        (
            Entity,
            &'static Transform,
            &'static Stockpile,
            Option<&'static StoredItems>,
        ),
    >,
    pub loaded_in: Query<'w, 's, &'static LoadedIn>,
    pub loaded_items: Query<'w, 's, &'static LoadedItems>,
    pub bucket_storages: Query<'w, 's, (), With<BucketStorage>>,
    pub blueprints: Query<
        'w,
        's,
        (
            &'static Transform,
            &'static Blueprint,
            Option<&'static Designation>,
        ),
    >,
    pub target_blueprints: Query<'w, 's, &'static TargetBlueprint>,
    pub mixers: Query<
        'w,
        's,
        (
            &'static Transform,
            &'static MudMixerStorage,
            Option<&'static TaskWorkers>,
        ),
    >,
    pub target_mixers: Query<'w, 's, &'static TargetMixer>,
    pub floor_tiles: Query<'w, 's, &'static FloorTileBlueprint>,
    pub wall_tiles: Query<'w, 's, &'static WallTileBlueprint>,
    pub buildings: Query<
        'w,
        's,
        (
            &'static Transform,
            &'static Building,
            Option<&'static ProvisionalWall>,
        ),
    >,
}

/// タスク割り当てに必要なクエリ群（読み取り専用、Familiar AI向け）
#[derive(SystemParam)]
pub struct TaskAssignmentReadAccess<'w, 's> {
    pub world_map: WorldMapRead<'w>,
    pub yards: Query<'w, 's, &'static hw_world::Yard>,
    pub items: Query<
        'w,
        's,
        (
            &'static ResourceItem,
            Option<&'static Designation>,
        ),
    >,
    pub sand_piles: Query<
        'w,
        's,
        (
            Entity,
            &'static Transform,
            Option<&'static Designation>,
            Option<&'static TaskWorkers>,
        ),
        With<SandPile>,
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
        With<BonePile>,
    >,
    pub task_state: Query<
        'w,
        's,
        (
            Option<&'static Designation>,
            Option<&'static TaskWorkers>,
        ),
    >,
    pub move_plant_tasks: Query<'w, 's, &'static hw_jobs::MovePlantTask>,
    pub transport_requests: Query<'w, 's, &'static TransportRequest>,
    pub transport_demands: Query<'w, 's, &'static TransportDemand>,
    pub transport_request_fixed_sources: Query<'w, 's, &'static TransportRequestFixedSource>,
    pub familiar_task_areas: Query<
        'w,
        's,
        &'static hw_core::area::TaskArea,
        With<hw_core::familiar::Familiar>,
    >,
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
            Without<TaskWorkers>,
            Without<ReservedForTask>,
            Without<ManualHaulPinnedSource>,
        ),
    >,
    pub reserved_for_task: Query<'w, 's, &'static ReservedForTask>,
    pub task_slots: Query<'w, 's, &'static TaskSlots>,
    pub wheelbarrows: Query<
        'w,
        's,
        (Entity, &'static Transform),
        (
            With<Wheelbarrow>,
            With<ParkedAt>,
            Without<PushedBy>,
        ),
    >,
    pub wheelbarrow_leases: Query<'w, 's, &'static WheelbarrowLease>,
    pub stored_items_query: Query<
        'w,
        's,
        (
            Entity,
            &'static ResourceItem,
            &'static StoredIn,
        ),
        (
            Without<Designation>,
            Without<TaskWorkers>,
            Without<ReservedForTask>,
        ),
    >,
}

/// タスク割り当てに必要なクエリ群（Familiar AI向け・独立型）
#[derive(SystemParam)]
pub struct FamiliarTaskAssignmentQueries<'w, 's> {
    pub reservation: ReservationAccess<'w, 's>,
    pub designation: DesignationAccess<'w, 's>,
    pub storage: FamiliarStorageAccess<'w, 's>,
    pub assignment_writer: MessageWriter<'w, hw_jobs::events::TaskAssignmentRequest>,
    pub read: TaskAssignmentReadAccess<'w, 's>,
}

impl<'w, 's> Deref for FamiliarTaskAssignmentQueries<'w, 's> {
    type Target = TaskAssignmentReadAccess<'w, 's>;

    fn deref(&self) -> &Self::Target {
        &self.read
    }
}

impl<'w, 's> DerefMut for FamiliarTaskAssignmentQueries<'w, 's> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.read
    }
}

pub trait TaskReservationAccess<'w, 's> {
    fn reservation_writer(&mut self) -> &mut MessageWriter<'w, ResourceReservationRequest>;
    fn resources(&self) -> &Query<'w, 's, &'static ResourceItem>;
    fn belongs_to(&self, entity: Entity) -> Option<Entity>;
}

impl<'w, 's> TaskReservationAccess<'w, 's> for FamiliarTaskAssignmentQueries<'w, 's> {
    fn reservation_writer(&mut self) -> &mut MessageWriter<'w, ResourceReservationRequest> {
        &mut self.reservation.reservation_writer
    }

    fn resources(&self) -> &Query<'w, 's, &'static ResourceItem> {
        &self.reservation.resources
    }

    fn belongs_to(&self, entity: Entity) -> Option<Entity> {
        self.designation.belongs.get(entity).ok().map(|b| b.0)
    }
}
