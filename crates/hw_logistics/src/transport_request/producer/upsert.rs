//! Producer upsert/cleanup 共通化ヘルパー

use bevy::prelude::*;
use std::collections::HashSet;
use std::hash::Hash;

use hw_core::relationships::ManagedBy;
use hw_jobs::{Designation, Priority, TaskSlots, WorkType};

use crate::transport_request::{
    TransportDemand, TransportPolicy, TransportPriority, TransportRequest, TransportRequestKind,
    TransportRequestState,
};
use crate::types::ResourceType;

/// 既存 request のループで重複を検出した際の処理
#[inline]
pub fn process_duplicate_key<K: Hash + Eq>(
    commands: &mut Commands,
    entity: Entity,
    workers: usize,
    seen: &mut HashSet<K>,
    key: K,
) -> bool {
    if !seen.insert(key) {
        if workers == 0 {
            commands.entity(entity).try_despawn();
        }
        return false;
    }
    true
}

#[inline]
pub fn request_state_for_workers(workers: usize) -> TransportRequestState {
    if workers == 0 {
        TransportRequestState::Pending
    } else {
        TransportRequestState::Claimed
    }
}

/// Producer-owned components on an existing transport request.
///
/// Producers use this snapshot to avoid queueing equivalent deferred writes every frame. Runtime
/// worker relationships are intentionally not part of the snapshot; callers pass the derived
/// inflight count and state explicitly.
pub type ExistingRequestRuntime<'a> = (
    Option<&'a Transform>,
    Option<&'a Visibility>,
    Option<&'a Designation>,
    Option<&'a ManagedBy>,
    Option<&'a TaskSlots>,
    Option<&'a Priority>,
    Option<&'a TransportDemand>,
    Option<&'a TransportRequestState>,
    Option<&'a TransportPolicy>,
);

/// Desired producer-owned values for a request that already exists.
pub struct SemanticRequestSpec {
    pub key: (Entity, ResourceType),
    pub site_pos: Vec2,
    pub issued_by: Entity,
    pub desired_slots: u32,
    pub inflight: u32,
    pub priority: u32,
    pub transport_priority: TransportPriority,
    pub kind: TransportRequestKind,
    pub work_type: WorkType,
    pub state: TransportRequestState,
}

fn request_matches(current: &TransportRequest, desired: &SemanticRequestSpec) -> bool {
    current.kind == desired.kind
        && current.anchor == desired.key.0
        && current.resource_type == desired.key.1
        && current.issued_by == desired.issued_by
        && current.priority == desired.transport_priority
        && current.stockpile_group.is_empty()
}

fn policy_is_default(current: &TransportPolicy) -> bool {
    let desired = TransportPolicy::default();
    current.allow_cross_area_source == desired.allow_cross_area_source
        && current.allow_cross_familiar_claim == desired.allow_cross_familiar_claim
        && current.source_search_radius_tiles == desired.source_search_radius_tiles
}

/// Updates only producer-owned values that differ from the desired semantic state.
#[inline]
pub fn update_request_runtime_if_needed(
    commands: &mut Commands,
    request_entity: Entity,
    current_request: &TransportRequest,
    current: ExistingRequestRuntime<'_>,
    desired: SemanticRequestSpec,
) {
    let (transform, visibility, designation, managed_by, slots, priority, demand, state, policy) =
        current;
    let desired_transform = Transform::from_xyz(desired.site_pos.x, desired.site_pos.y, 0.0);
    let mut entity_commands = commands.entity(request_entity);

    if transform.is_none_or(|current| {
        current.translation != desired_transform.translation
            || current.rotation != desired_transform.rotation
            || current.scale != desired_transform.scale
    }) {
        entity_commands.try_insert(desired_transform);
    }
    if visibility.is_none_or(|current| *current != Visibility::Hidden) {
        entity_commands.try_insert(Visibility::Hidden);
    }
    if designation.is_none_or(|current| current.work_type != desired.work_type) {
        entity_commands.try_insert(Designation {
            work_type: desired.work_type,
        });
    }
    if managed_by.is_none_or(|current| current.0 != desired.issued_by) {
        entity_commands.try_insert(ManagedBy(desired.issued_by));
    }
    if slots.is_none_or(|current| current.max != desired.desired_slots) {
        entity_commands.try_insert(TaskSlots::new(desired.desired_slots));
    }
    if priority.is_none_or(|current| current.0 != desired.priority) {
        entity_commands.try_insert(Priority(desired.priority));
    }
    if !request_matches(current_request, &desired) {
        entity_commands.try_insert(TransportRequest {
            kind: desired.kind,
            anchor: desired.key.0,
            resource_type: desired.key.1,
            issued_by: desired.issued_by,
            priority: desired.transport_priority,
            stockpile_group: Vec::new(),
        });
    }
    if demand.is_none_or(|current| {
        current.desired_slots != desired.desired_slots || current.inflight != desired.inflight
    }) {
        entity_commands.try_insert(TransportDemand {
            desired_slots: desired.desired_slots,
            inflight: desired.inflight,
        });
    }
    if state.is_none_or(|current| *current != desired.state) {
        entity_commands.try_insert(desired.state);
    }
    if policy.is_none_or(|current| !policy_is_default(current)) {
        entity_commands.try_insert(TransportPolicy::default());
    }
}

/// Removes activation components, and optionally reconciles demand, only when they differ.
#[inline]
pub fn disable_request_if_needed(
    commands: &mut Commands,
    request_entity: Entity,
    current: ExistingRequestRuntime<'_>,
    inflight: Option<u32>,
) {
    let (_, _, designation, _, slots, priority, demand, _, _) = current;
    let mut entity_commands = commands.entity(request_entity);
    if designation.is_some() {
        entity_commands.try_remove::<Designation>();
    }
    if slots.is_some() {
        entity_commands.try_remove::<TaskSlots>();
    }
    if priority.is_some() {
        entity_commands.try_remove::<Priority>();
    }
    if let Some(inflight) = inflight
        && demand.is_none_or(|current| current.desired_slots != 0 || current.inflight != inflight)
    {
        entity_commands.try_insert(TransportDemand {
            desired_slots: 0,
            inflight,
        });
    }
}

/// 新規 request entity を spawn するためのスペック。
pub struct SpawnRequestSpec<TTarget> {
    pub name: &'static str,
    pub key: (Entity, ResourceType),
    pub site_pos: Vec2,
    pub issued_by: Entity,
    pub desired_slots: u32,
    pub priority: u32,
    pub target: TTarget,
    pub kind: TransportRequestKind,
    pub work_type: WorkType,
}

/// 新規 request entity を spawn する
#[inline]
pub fn spawn_transport_request<TTarget: Component>(
    commands: &mut Commands,
    spec: SpawnRequestSpec<TTarget>,
) {
    commands.spawn((
        Name::new(spec.name),
        Transform::from_xyz(spec.site_pos.x, spec.site_pos.y, 0.0),
        Visibility::Hidden,
        Designation {
            work_type: spec.work_type,
        },
        ManagedBy(spec.issued_by),
        TaskSlots::new(spec.desired_slots),
        Priority(spec.priority),
        spec.target,
        TransportRequest {
            kind: spec.kind,
            anchor: spec.key.0,
            resource_type: spec.key.1,
            issued_by: spec.issued_by,
            priority: TransportPriority::Normal,
            stockpile_group: vec![],
        },
        TransportDemand {
            desired_slots: spec.desired_slots,
            inflight: 0,
        },
        TransportRequestState::Pending,
        TransportPolicy::default(),
    ));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Resource)]
    struct RequestFixture {
        request: Entity,
        owner: Entity,
        anchor: Entity,
    }

    fn reconcile_active_request(
        mut commands: Commands,
        fixture: Res<RequestFixture>,
        q_requests: Query<(&TransportRequest, ExistingRequestRuntime<'static>)>,
    ) {
        let (request, current) = q_requests.get(fixture.request).unwrap();
        update_request_runtime_if_needed(
            &mut commands,
            fixture.request,
            request,
            current,
            SemanticRequestSpec {
                key: (fixture.anchor, ResourceType::Wood),
                site_pos: Vec2::new(4.0, 8.0),
                issued_by: fixture.owner,
                desired_slots: 2,
                inflight: 0,
                priority: 3,
                transport_priority: TransportPriority::Normal,
                kind: TransportRequestKind::DeliverToBlueprint,
                work_type: WorkType::Haul,
                state: TransportRequestState::Pending,
            },
        );
    }

    fn disable_request(
        mut commands: Commands,
        fixture: Res<RequestFixture>,
        q_requests: Query<ExistingRequestRuntime<'static>, With<TransportRequest>>,
    ) {
        let current = q_requests.get(fixture.request).unwrap();
        disable_request_if_needed(&mut commands, fixture.request, current, Some(0));
    }

    fn active_bundle(anchor: Entity, owner: Entity) -> impl Bundle {
        (
            Transform::from_xyz(4.0, 8.0, 0.0),
            Visibility::Hidden,
            Designation {
                work_type: WorkType::Haul,
            },
            ManagedBy(owner),
            TaskSlots::new(2),
            Priority(3),
            TransportRequest {
                kind: TransportRequestKind::DeliverToBlueprint,
                anchor,
                resource_type: ResourceType::Wood,
                issued_by: owner,
                priority: TransportPriority::Normal,
                stockpile_group: Vec::new(),
            },
            TransportDemand {
                desired_slots: 2,
                inflight: 0,
            },
            TransportRequestState::Pending,
            TransportPolicy::default(),
        )
    }

    fn fixture() -> App {
        let mut app = App::new();
        let owner = app.world_mut().spawn_empty().id();
        let anchor = app.world_mut().spawn_empty().id();
        let request = app.world_mut().spawn(active_bundle(anchor, owner)).id();
        app.insert_resource(RequestFixture {
            request,
            owner,
            anchor,
        });
        app
    }

    #[test]
    fn second_active_reconciliation_does_not_dirty_owned_components() {
        let mut app = fixture();
        app.add_systems(Update, reconcile_active_request);
        app.update();
        app.world_mut().clear_trackers();
        app.update();

        let request_entity = app.world().resource::<RequestFixture>().request;
        let request = app.world().entity(request_entity);
        assert!(!request.get_ref::<Transform>().unwrap().is_changed());
        assert!(!request.get_ref::<Designation>().unwrap().is_changed());
        assert!(!request.get_ref::<ManagedBy>().unwrap().is_changed());
        assert!(!request.get_ref::<TaskSlots>().unwrap().is_changed());
        assert!(!request.get_ref::<Priority>().unwrap().is_changed());
        assert!(!request.get_ref::<TransportRequest>().unwrap().is_changed());
        assert!(!request.get_ref::<TransportDemand>().unwrap().is_changed());
        assert!(
            !request
                .get_ref::<TransportRequestState>()
                .unwrap()
                .is_changed()
        );
        assert!(!request.get_ref::<TransportPolicy>().unwrap().is_changed());
    }

    #[test]
    fn second_disable_reconciliation_queues_no_equivalent_demand_write() {
        let mut app = fixture();
        app.add_systems(Update, disable_request);
        app.update();
        app.world_mut().clear_trackers();
        app.update();

        let request_entity = app.world().resource::<RequestFixture>().request;
        let request = app.world().entity(request_entity);
        assert!(!request.contains::<Designation>());
        assert!(!request.contains::<TaskSlots>());
        assert!(!request.contains::<Priority>());
        let demand = request.get_ref::<TransportDemand>().unwrap();
        assert_eq!((demand.desired_slots, demand.inflight), (0, 0));
        assert!(!demand.is_changed());
    }
}
