use std::ops::{Deref, DerefMut};

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;

use crate::events::{ResourceReservationRequest, TaskAssignmentRequest};
use crate::systems::soul_ai::execute::task_execution::context::access::{
    DesignationAccess, MutStorageAccess, ReservationAccess, StorageAccess,
};
use crate::world::map::WorldMapRead;

/// タスク割り当てに必要なクエリ群（Familiar AI向け）
#[derive(SystemParam)]
pub struct TaskAssignmentReadAccess<'w, 's> {
    pub world_map: WorldMapRead<'w>,
    pub yards: Query<'w, 's, &'static crate::systems::world::zones::Yard>,
    pub items: Query<'w, 's, (&'static crate::systems::logistics::ResourceItem, Option<&'static crate::systems::jobs::Designation>)>,
    pub sand_piles: Query<
        'w,
        's,
        (
            Entity,
            &'static Transform,
            Option<&'static crate::systems::jobs::Designation>,
            Option<&'static crate::relationships::TaskWorkers>,
        ),
        With<crate::systems::jobs::SandPile>,
    >,
    pub bone_piles: Query<
        'w,
        's,
        (
            Entity,
            &'static Transform,
            Option<&'static crate::systems::jobs::Designation>,
            Option<&'static crate::relationships::TaskWorkers>,
        ),
        With<crate::systems::jobs::BonePile>,
    >,
    pub task_state: Query<'w, 's, (Option<&'static crate::systems::jobs::Designation>, Option<&'static crate::relationships::TaskWorkers>)>,
    pub move_plant_tasks: Query<'w, 's, &'static crate::systems::soul_ai::execute::task_execution::types::MovePlantTask>,
    pub transport_requests:
        Query<'w, 's, &'static crate::systems::logistics::transport_request::TransportRequest>,
    pub transport_demands:
        Query<'w, 's, &'static crate::systems::logistics::transport_request::TransportDemand>,
    pub transport_request_fixed_sources: Query<
        'w,
        's,
        &'static crate::systems::logistics::transport_request::TransportRequestFixedSource,
    >,
    pub familiar_task_areas: Query<
        'w,
        's,
        &'static crate::systems::command::TaskArea,
        With<crate::entities::familiar::Familiar>,
    >,
    pub free_resource_items: Query<
        'w,
        's,
        (
            Entity,
            &'static Transform,
            &'static Visibility,
            &'static crate::systems::logistics::ResourceItem,
        ),
        (
            Without<crate::systems::jobs::Designation>,
            Without<crate::relationships::TaskWorkers>,
            Without<crate::systems::logistics::ReservedForTask>,
            Without<crate::systems::logistics::transport_request::ManualHaulPinnedSource>,
        ),
    >,
    pub reserved_for_task: Query<'w, 's, &'static crate::systems::logistics::ReservedForTask>,
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
    pub wheelbarrow_leases:
        Query<'w, 's, &'static crate::systems::logistics::transport_request::WheelbarrowLease>,
    pub stored_items_query: Query<
        'w,
        's,
        (
            Entity,
            &'static crate::systems::logistics::ResourceItem,
            &'static crate::relationships::StoredIn,
        ),
        (
            Without<crate::systems::jobs::Designation>,
            Without<crate::relationships::TaskWorkers>,
            Without<crate::systems::logistics::ReservedForTask>,
        ),
    >,
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
    pub resource_items: Query<
        'w,
        's,
        (
            Entity,
            &'static Transform,
            &'static Visibility,
            &'static crate::systems::logistics::ResourceItem,
            Option<&'static crate::relationships::StoredIn>,
            Option<&'static crate::relationships::LoadedIn>,
        ),
    >,
    pub mixer_stored_mud: Query<'w, 's, &'static crate::systems::jobs::StoredByMixer>,
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

pub trait TaskReservationAccess<'w, 's> {
    fn reservation_writer(&mut self) -> &mut MessageWriter<'w, ResourceReservationRequest>;
    fn resources(&self) -> &Query<'w, 's, &'static crate::systems::logistics::ResourceItem>;
    fn belongs_to(&self, entity: Entity) -> Option<Entity>;
}

macro_rules! impl_task_reservation_access {
    ($ty:ty) => {
        impl<'w, 's> TaskReservationAccess<'w, 's> for $ty {
            fn reservation_writer(&mut self) -> &mut MessageWriter<'w, ResourceReservationRequest> {
                &mut self.reservation.reservation_writer
            }

            fn resources(&self) -> &Query<'w, 's, &'static crate::systems::logistics::ResourceItem> {
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
