use bevy::ecs::system::SystemParam;
use bevy::prelude::*;

use hw_core::events::ResourceReservationRequest;
use hw_core::relationships::TaskWorkers;

use super::access::{DesignationAccess, MutStorageAccess, ReservationAccess};

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

type ChainUnavailableSourcesQuery<'w, 's> = Query<
    'w,
    's,
    (),
    Or<(
        With<TaskWorkers>,
        With<hw_core::relationships::DeliveringTo>,
        With<hw_logistics::transport_request::ManualHaulPinnedSource>,
        With<hw_jobs::Designation>,
    )>,
>;

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
    pub stockpile_policies: Query<'w, 's, &'static hw_logistics::zone::StockpilePolicy>,

    // 固有フィールド
    pub resource_items: ResourceItemsQuery<'w, 's>,
    pub chain_unavailable_sources: ChainUnavailableSourcesQuery<'w, 's>,
    pub mixer_stored_mud: Query<'w, 's, &'static hw_jobs::mud_mixer::StoredByMixer>,
    pub transport_request_status: TransportRequestStatusQuery<'w, 's>,
    pub deconstruction_order_targets: Query<
        'w,
        's,
        &'static hw_jobs::TargetDeconstructionRoot,
        With<hw_jobs::DeconstructionOrder>,
    >,
    pub deconstruction_pending: Query<'w, 's, &'static hw_jobs::DeconstructionPending>,
    pub deconstruction_claims: Query<'w, 's, (), With<hw_jobs::DeconstructionCommitClaim>>,
    pub deconstruction_blockers: Query<'w, 's, &'static hw_jobs::DeconstructionBlocker>,
    pub move_planned: Query<'w, 's, &'static hw_jobs::MovePlanned>,
    pub pending_moves: Query<'w, 's, &'static hw_jobs::PendingBuildingMove>,
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
impl_task_reservation_access!(TaskUnassignQueries<'w, 's>);
