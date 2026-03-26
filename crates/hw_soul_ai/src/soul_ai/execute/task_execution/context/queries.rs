use std::ops::{Deref, DerefMut};

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;

use hw_core::events::ResourceReservationRequest;
use hw_core::relationships::{ParkedAt, PushedBy, TaskWorkers};
use hw_jobs::events::TaskAssignmentRequest;
use hw_logistics::types::{ReservedForTask, Wheelbarrow};
use hw_world::WorldMapRead;

use super::access::{DesignationAccess, MutStorageAccess, ReservationAccess, StorageAccess};

type SandPilesQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static Transform,
        Option<&'static hw_jobs::Designation>,
        Option<&'static TaskWorkers>,
    ),
    With<hw_jobs::SandPile>,
>;

type BonePilesQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static Transform,
        Option<&'static hw_jobs::Designation>,
        Option<&'static TaskWorkers>,
    ),
    With<hw_jobs::BonePile>,
>;

type FreeResourceItemsQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static Transform,
        &'static Visibility,
        &'static hw_logistics::types::ResourceItem,
    ),
    (
        Without<hw_jobs::Designation>,
        Without<TaskWorkers>,
        Without<ReservedForTask>,
        Without<hw_logistics::transport_request::ManualHaulPinnedSource>,
    ),
>;

type WheelbarrowsQuery<'w, 's> = Query<
    'w,
    's,
    (Entity, &'static Transform),
    (With<Wheelbarrow>, With<ParkedAt>, Without<PushedBy>),
>;

type StoredItemsQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static hw_logistics::types::ResourceItem,
        &'static hw_core::relationships::StoredIn,
    ),
    (
        Without<hw_jobs::Designation>,
        Without<TaskWorkers>,
        Without<ReservedForTask>,
    ),
>;

type ResourceItemsQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static Transform,
        &'static Visibility,
        &'static hw_logistics::types::ResourceItem,
        Option<&'static hw_core::relationships::StoredIn>,
        Option<&'static hw_core::relationships::LoadedIn>,
    ),
>;

type TransportRequestStatusQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static hw_logistics::transport_request::TransportRequest,
        &'static hw_logistics::transport_request::TransportDemand,
        &'static hw_logistics::transport_request::TransportRequestState,
        Option<&'static hw_logistics::transport_request::WheelbarrowLease>,
        Option<&'static TaskWorkers>,
    ),
>;

/// タスク割り当てに必要なクエリ群（Familiar AI向け）
#[derive(SystemParam)]
pub struct TaskAssignmentReadAccess<'w, 's> {
    pub world_map: WorldMapRead<'w>,
    pub yards: Query<'w, 's, &'static hw_world::zones::Yard>,
    pub items: Query<
        'w,
        's,
        (
            &'static hw_logistics::types::ResourceItem,
            Option<&'static hw_jobs::Designation>,
        ),
    >,
    pub sand_piles: SandPilesQuery<'w, 's>,
    pub bone_piles: BonePilesQuery<'w, 's>,
    pub task_state: Query<
        'w,
        's,
        (
            Option<&'static hw_jobs::Designation>,
            Option<&'static TaskWorkers>,
        ),
    >,
    pub move_plant_tasks: Query<'w, 's, &'static hw_jobs::MovePlantTask>,
    pub transport_requests:
        Query<'w, 's, &'static hw_logistics::transport_request::TransportRequest>,
    pub transport_demands: Query<'w, 's, &'static hw_logistics::transport_request::TransportDemand>,
    pub transport_request_fixed_sources:
        Query<'w, 's, &'static hw_logistics::transport_request::TransportRequestFixedSource>,
    pub familiar_task_areas:
        Query<'w, 's, &'static hw_core::area::TaskArea, With<hw_core::familiar::Familiar>>,
    pub free_resource_items: FreeResourceItemsQuery<'w, 's>,
    pub reserved_for_task: Query<'w, 's, &'static ReservedForTask>,
    pub task_slots: Query<'w, 's, &'static hw_jobs::TaskSlots>,
    pub wheelbarrows: WheelbarrowsQuery<'w, 's>,
    pub wheelbarrow_leases:
        Query<'w, 's, &'static hw_logistics::transport_request::WheelbarrowLease>,
    pub stored_items_query: StoredItemsQuery<'w, 's>,
}

/// タスク割り当てに必要なクエリ群（Familiar AI向け）
#[derive(SystemParam)]
pub struct TaskAssignmentQueries<'w, 's> {
    pub reservation: ReservationAccess<'w, 's>,
    pub designation: DesignationAccess<'w, 's>,
    pub storage: StorageAccess<'w, 's>,
    pub assignment_writer: MessageWriter<'w, TaskAssignmentRequest>,
    pub read: TaskAssignmentReadAccess<'w, 's>,
}

impl<'w, 's> Deref for TaskAssignmentQueries<'w, 's> {
    type Target = TaskAssignmentReadAccess<'w, 's>;

    fn deref(&self) -> &Self::Target {
        &self.read
    }
}

impl<'w, 's> DerefMut for TaskAssignmentQueries<'w, 's> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.read
    }
}

/// タスク解除に必要な最小クエリ群
#[derive(SystemParam)]
pub struct TaskUnassignQueries<'w, 's> {
    pub reservation: ReservationAccess<'w, 's>,
    pub designation: DesignationAccess<'w, 's>,
}

/// タスク実行に必要なクエリ群
#[derive(SystemParam)]
pub struct TaskQueries<'w, 's> {
    pub reservation: ReservationAccess<'w, 's>,
    pub designation: DesignationAccess<'w, 's>,
    pub storage: MutStorageAccess<'w, 's>,

    // 固有フィールド
    pub resource_items: ResourceItemsQuery<'w, 's>,
    pub mixer_stored_mud: Query<'w, 's, &'static hw_jobs::mud_mixer::StoredByMixer>,
    pub transport_request_status: TransportRequestStatusQuery<'w, 's>,
}

pub trait TaskReservationAccess<'w, 's> {
    fn reservation_writer(&mut self) -> &mut MessageWriter<'w, ResourceReservationRequest>;
    fn resources(&self) -> &Query<'w, 's, &'static hw_logistics::types::ResourceItem>;
    fn belongs_to(&self, entity: Entity) -> Option<Entity>;
}

macro_rules! impl_task_reservation_access {
    ($ty:ty) => {
        impl<'w, 's> TaskReservationAccess<'w, 's> for $ty {
            fn reservation_writer(&mut self) -> &mut MessageWriter<'w, ResourceReservationRequest> {
                &mut self.reservation.reservation_writer
            }

            fn resources(&self) -> &Query<'w, 's, &'static hw_logistics::types::ResourceItem> {
                &self.reservation.resources
            }

            fn belongs_to(&self, entity: Entity) -> Option<Entity> {
                self.designation
                    .belongs
                    .get(entity)
                    .ok()
                    .map(|belongs| belongs.0)
            }
        }
    };
}

impl_task_reservation_access!(TaskQueries<'w, 's>);
impl_task_reservation_access!(TaskAssignmentQueries<'w, 's>);
impl_task_reservation_access!(TaskUnassignQueries<'w, 's>);
